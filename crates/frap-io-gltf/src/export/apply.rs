// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_apply=True` (M4, WP 4.7): the exported mesh of an object with modifiers is its
//! viewport-evaluated mesh (`ExportSettings::evaluated_meshes`, filled by the operator), and the
//! add-on's sharing rules decide which objects still share one glTF mesh. `mesh/gather.rs` asks
//! [`evaluated`] and [`evaluated_materials`].
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/nodes.py
//! (`__gather_mesh`, add-on 5.2.41) and obj_data.py (`get_data_cache_key`):
//!
//! - `modifiers = blender_object.modifiers; if len(modifiers) == 0: modifiers = None`. With
//!   `gltf_apply` and no modifiers the original data is exported, so linked duplicates without
//!   modifiers keep sharing one glTF mesh (and its accessors), whatever their materials.
//! - With at least one modifier — even when none of them is enabled in the viewport — the data
//!   is `blender_object.evaluated_get(depsgraph).to_mesh(preserve_all_data_layers=True)`, a
//!   temporary mesh named after the original data. The cache key `(id(data), modifiers,
//!   materials)` holds the object's `ObjectModifiers` collection, so every object with modifiers
//!   gets its own glTF mesh, with its own vertex accessors (index accessors are still shared by
//!   content, `@cached gather_accessor`); linked duplicates with modifiers never share.
//! - The materials of such a mesh are the temporary mesh's own (`blender_data.materials`, the
//!   evaluated data's slots, which a Boolean TRANSFER extends), so an object-linked slot is
//!   **not** applied (verified on 5.2.4: an object with a disabled Bevel and an OBJECT-linked
//!   slot exports the mesh's material); only when that list is exactly `(None,)` does the
//!   add-on fall back to `ms.material for ms in blender_object.material_slots`.
//! - The temporary mesh carries the original's custom properties (`gltf2_mesh_applied` is
//!   black-listed), so its `extras` are the original's.
//!
//! - An object outside the viewport depsgraph is not evaluated: `evaluated_get()` returns the
//!   original object and `to_mesh()` its data, modifiers ignored (verified on 5.2.4: a Cube with
//!   a Subdivision and `hide_viewport=True` exports 24 vertices). It still has modifiers, so it
//!   still gets a glTF mesh of its own. [`viewport_evaluated_objects`] says which objects the
//!   depsgraph evaluates; the operator evaluates only those.
//!
//! The armature handling of `gltf_skins` (Armature modifiers disabled while evaluating) is M6.

use std::collections::BTreeSet;
use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{CollectionId, IdCode, IdRef, MaterialId, ObjectId};
use frap_scene::id::{ModifierSettings, Object};
use frap_scene::{Context, LayerCollection, Main};

use crate::export::gather::Gather;
use crate::export::mesh::gather::slot_materials;

/// `export_apply` is honoured (no "not supported yet" WARNING), and so is
/// `export_current_frame`: without it the operator evaluates the meshes at frame 0
/// (`export.save`'s `frame_set(0)`); object transforms do not depend on the frame before
/// animation (M6).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static APPLY_PROPS: &[&str] = &["export_apply", "export_current_frame"];

/// The evaluated mesh to export for `ob` instead of its data (`None`: export the original):
/// `Some` only with `export_apply=True`, for an object with at least one modifier whose
/// evaluation the operator could provide.
pub fn evaluated(g: &Gather, ob: IdRef<ObjectId>) -> Option<Arc<Mesh>> {
    let meshes = g.settings.evaluated_meshes.as_ref()?;
    let object = g.main.objects.get(ob)?;
    if !has_modifiers(object) {
        return None;
    }
    meshes.get(&ob).cloned()
}

/// The objects whose geometry the viewport depsgraph of the context view layer evaluates; for
/// every other object `evaluated_get(depsgraph).to_mesh()` is the original data.
///
/// Ported from blender/source/blender/depsgraph/intern/builder/deg_builder_nodes_view_layer.cc
/// (`build_view_layer`: only bases with `BASE_ENABLED_AND_MAYBE_VISIBLE_IN_VIEWPORT`),
/// deg_builder_nodes.cc (`build_collection`: `COLLECTION_HIDE_VIEWPORT` hides the nested
/// collections too) and eval/deg_eval_visibility.cc (`deg_graph_flush_visibility_flags`: an
/// object pulled in by a modifier has its geometry evaluated only when an enabled modifier of an
/// evaluated object reads that geometry). Verified on 5.2.4:
///
/// - enabled: in a non-excluded layer collection whose collection (and every ancestor
///   collection) is not `hide_viewport`, and the object not `hide_viewport`; one such path is
///   enough. `LayerCollection.hide_viewport` and `hide_set(True)` do not matter.
/// - pulled in: the operand of a viewport-enabled Boolean (the object, or every object of the
///   collection and its children) and the caps of an Array, recursively. A Boolean disabled in
///   the viewport, an Array offset object (transform only) and a hidden parent (in a freshly
///   loaded file) do not evaluate the geometry.
pub fn viewport_evaluated_objects(main: &Main, ctx: &Context) -> BTreeSet<IdRef<ObjectId>> {
    fn walk(main: &Main, ctx: &Context, lc: &LayerCollection, out: &mut BTreeSet<IdRef<ObjectId>>) {
        if lc.exclude {
            return;
        }
        let coll = match lc.collection {
            None => &ctx.scene().collection,
            Some(c) => match main.collections.get(c) {
                Some(c) => c,
                None => return,
            },
        };
        // A hidden collection hides its children (`is_parent_collection_visible_`).
        if coll.hide_viewport {
            return;
        }
        for &o in &coll.objects {
            if main.objects.get(o).is_some_and(|ob| !ob.hide_viewport) {
                out.insert(o);
            }
        }
        for c in &lc.children {
            walk(main, ctx, c, out);
        }
    }
    let vl = ctx.view_layer();
    let mut out = BTreeSet::new();
    walk(main, ctx, &vl.layer_collection, &mut out);
    out.retain(|o| vl.bases.iter().any(|b| b.object == *o));
    // Geometry dependencies of the evaluated objects' enabled modifiers.
    let mut stack: Vec<IdRef<ObjectId>> = out.iter().copied().collect();
    while let Some(o) = stack.pop() {
        let Some(ob) = main.objects.get(o) else {
            continue;
        };
        for md in ob.modifiers.iter().filter(|m| m.show_viewport) {
            for dep in geometry_dependencies(main, &md.settings) {
                if out.insert(dep) {
                    stack.push(dep);
                }
            }
        }
    }
    out
}

/// The objects whose evaluated geometry a modifier reads (`DEG_add_object_relation(..,
/// DEG_OB_COMP_GEOMETRY, ..)` in the types' `update_depsgraph`): the Boolean operand object or
/// collection (`all_objects`), the Array start and end caps.
fn geometry_dependencies(main: &Main, s: &ModifierSettings) -> Vec<IdRef<ObjectId>> {
    let ids = match s {
        ModifierSettings::Boolean(b) if b.operand_type == "OBJECT" => vec![b.object],
        ModifierSettings::Boolean(b) => vec![b.collection],
        ModifierSettings::Array(a) => vec![a.start_cap, a.end_cap],
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for id in ids.into_iter().flatten() {
        match id.code {
            IdCode::OB => out.extend(IdRef::<ObjectId>::from_any(id)),
            IdCode::GR => {
                let mut stack: Vec<IdRef<CollectionId>> =
                    IdRef::<CollectionId>::from_any(id).into_iter().collect();
                let mut seen = BTreeSet::new();
                while let Some(c) = stack.pop() {
                    if !seen.insert(c) {
                        continue;
                    }
                    if let Some(coll) = main.collections.get(c) {
                        out.extend(coll.objects.iter().copied());
                        stack.extend(coll.children.iter().copied());
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// `len(blender_object.modifiers) != 0` (enabled or not).
pub fn has_modifiers(ob: &Object) -> bool {
    !ob.modifiers.is_empty()
}

/// The material per slot of an applied mesh: the evaluated mesh's own slots, or the object's
/// material slots (over the original data `orig`) when the evaluated mesh has exactly one empty
/// slot.
pub fn evaluated_materials(
    ob: &Object,
    orig: &Mesh,
    evaluated: &Mesh,
) -> Vec<Option<IdRef<MaterialId>>> {
    if evaluated.materials.len() == 1 && evaluated.materials[0].is_none() {
        slot_materials(ob, orig)
    } else {
        evaluated.materials.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::IdKey;
    use frap_scene::id::{SlotLink, SlotOverride};

    fn mat() -> Option<IdRef<MaterialId>> {
        Some(IdRef::from_key(IdKey::next()))
    }

    #[test]
    fn export_apply_is_supported() {
        assert!(crate::export::is_supported("export_apply"));
    }

    #[test]
    fn evaluated_materials_ignore_object_links_unless_single_empty_slot() {
        let (a, b, c) = (mat(), mat(), mat());
        let mut ob = Object::new("Ob", None);
        ob.material_slot_overrides = vec![SlotOverride {
            link: SlotLink::Object,
            material: c,
        }];
        let mut orig = Mesh::new("");
        orig.materials = vec![a];
        // The evaluated mesh's slots win over the object-linked slot.
        let mut ev = Mesh::new("");
        ev.materials = vec![a, b];
        assert_eq!(evaluated_materials(&ob, &orig, &ev), vec![a, b]);
        // No slots at all: no materials (the object's slots are not consulted either).
        ev.materials = vec![];
        assert_eq!(evaluated_materials(&ob, &orig, &ev), vec![]);
        // `(None,)`: the object's material slots.
        ev.materials = vec![None];
        assert_eq!(evaluated_materials(&ob, &orig, &ev), vec![c]);
    }
}
