// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_variants` export (WP 7.5).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/primitives.py` (`__gather_extensions`),
//! `blender/exp/material/extensions/variants.py` (`gather_variant`), `blender/exp/tree.py`
//! (`variants_reset_to_original`) and `blender/exp/exporter.py` (`ChildOfRootExtension`
//! traversal), Apache-2.0, The glTF-Blender-IO authors:
//!
//! - only while the add-on preference `KHR_materials_variants_ui` is on and
//!   `bpy.data.scenes[0]` has variants;
//! - per primitive (material slot `s` of its mesh): every `Mesh.gltf2_variant_mesh_data` entry
//!   with `material_slot_index == s`, in order, gives one mapping of its material (gathered
//!   like a primitive's; an empty slot gives a mapping without `material`) and its variants,
//!   skipping a variant listed twice in the entry, and one an earlier entry of the slot already
//!   maps (with the add-on's warning `Variant <i> has 2 different materials for a single slot.
//!   Skipping it.`); an entry left without variants gives no mapping;
//! - a variant is `bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[variant_idx]`
//!   (Python indexing: negative from the end, out of range is the add-on's `IndexError`), and
//!   the root list holds each distinct `{name}` once, in first-use order (the root list is
//!   built while the primitives are traversed, equal dicts shared);
//! - `variants_reset_to_original` (before gathering a scene): every mesh object of the scene's
//!   export tree gets back, per material slot, the default material recorded for it
//!   (`Mesh.gltf2_variant_default_materials`); the change stays on the Blender data, so the
//!   operator ([`reset_to_original`]) applies it to `bpy.data` before exporting.
//!
//! - with `export_apply=True`, an object with modifiers exports the temporary mesh
//!   `evaluated_get(depsgraph).to_mesh()` (`blender/exp/nodes.py`), which carries
//!   `gltf2_variant_mesh_data` only when the modifier stack passed the evaluated copy of the
//!   data through (deform and attribute-only modifiers, or none evaluated); a modifier that
//!   builds a new mesh (`BKE_mesh_new_nomain_from_template` copies no ID properties) leaves it
//!   without mappings, and with no other mapped primitive the file has no variants at all
//!   ([`applied`]);
//! - a mapping's material goes through `gather_material` itself, which ignores
//!   `export_materials='PLACEHOLDER'` (the primitive then has no material, the mapping keeps
//!   its own; `materials::gather_unconditional`).
//!
//! `mesh/gather.rs` calls [`primitive`] for every primitive it builds; `export()` calls
//! [`finish`] after the scenes. Deviation (deviations.md): a material only a mapping uses gets
//! its `materials[]` index after the primitive's own material (the add-on traverses a
//! primitive's `extensions` before its `material`).

use frap_gltf::ext::khr_materials_variants::{
    KhrMaterialsVariants, KhrMaterialsVariantsPrimitive, Variant, VariantMapping,
};
use frap_gltf::json::{Index, Primitive};
use frap_mesh::Mesh;
use frap_rna::{IdRef, ObjectId, SceneId};
use frap_scene::id::{ModifierSettings, Object};
use frap_scene::{ChangeLog, Context, Main, Preferences};
use smol_str::SmolStr;

use crate::export::gather::Gather;
use crate::export::settings::ExportSettings;
use crate::export::{ExportError, materials};

const NAME: &str = "KHR_materials_variants";

/// The root variant list built while gathering (a `Gather` state).
#[derive(Clone, Debug, Default)]
struct RootVariants(Vec<SmolStr>);

/// Set while the primitives of an `export_apply` temporary mesh without the variant data are
/// built (a `Gather` state).
#[derive(Clone, Copy, Debug, Default)]
struct WithoutVariantData(bool);

/// Runs `f` (building the glTF mesh of the `export_apply` object `ob`, whose data is `orig` and
/// whose viewport-evaluated mesh is `evaluated`) with the variant mappings left out when the
/// add-on's temporary mesh has no `gltf2_variant_mesh_data` ([`applied_keeps_data`]).
pub fn applied<R>(
    g: &mut Gather,
    ob: &Object,
    orig: &Mesh,
    evaluated: Option<&Mesh>,
    f: impl FnOnce(&mut Gather) -> R,
) -> R {
    let without = !applied_keeps_data(ob, orig, evaluated);
    let prev = std::mem::replace(&mut g.state::<WithoutVariantData>().0, without);
    let r = f(g);
    g.state::<WithoutVariantData>().0 = prev;
    r
}

/// Whether `blender_object.evaluated_get(depsgraph).to_mesh()` of an object with modifiers
/// still has the data's variant properties (they are ID properties of the mesh: kept by
/// `BKE_mesh_copy_for_eval`, the copy a deform modifier works on, lost by a mesh built from a
/// template). Verified on 5.2.4 type by type with default settings:
///
/// - not evaluated (outside the viewport depsgraph, `evaluated == None`): `to_mesh()` copies
///   the data, kept;
/// - modifiers off in the viewport, or disabled (`is_disabled`: Bevel `width == 0`, Boolean
///   without operand, except EXACT with a COLLECTION operand): skipped;
/// - kept by every deform-only and attribute-only type, NORMAL_EDIT and WEIGHTED_NORMAL, and by
///   the types' early returns of their input (Decimate COLLAPSE `ratio == 1`, UNSUBDIV
///   `iterations == 0`, DISSOLVE `angle_limit == 0`; Subdivision `levels == 0`);
/// - Weld and Geometry Nodes return their input when nothing changes the topology (nothing
///   merged; Set Position, a Merge by Distance merging nothing), and a new mesh otherwise
///   (Subdivide Mesh, Triangulate): decided by the evaluated element counts (deviations.md);
/// - lost after any other type (Array, even with `count == 1`; Bevel, Boolean, Edge Split,
///   Mirror, Remesh, Screw, Skin, Solidify, Subdivision, Triangulate, Wireframe, Decimate,
///   …).
pub fn applied_keeps_data(ob: &Object, orig: &Mesh, evaluated: Option<&Mesh>) -> bool {
    let Some(ev) = evaluated else {
        return true;
    };
    let same_counts = ev.verts_num() == orig.verts_num()
        && ev.edges_num() == orig.edges_num()
        && ev.faces_num() == orig.faces_num()
        && ev.corners_num() == orig.corners_num();
    ob.modifiers
        .iter()
        .filter(|m| m.show_viewport)
        .all(|m| passes_data_through(&m.settings, same_counts))
}

/// Whether one viewport-enabled modifier leaves the ID properties of its input in place.
fn passes_data_through(s: &ModifierSettings, same_counts: bool) -> bool {
    use ModifierSettings as S;
    match s {
        S::Bevel(b) => b.width == 0.0,
        S::Boolean(b) if b.operand_type == "OBJECT" => b.object.is_none(),
        S::Boolean(b) => b.collection.is_none() && b.solver != "EXACT",
        S::Decimate(d) => match d.decimate_type.as_str() {
            "COLLAPSE" => d.ratio == 1.0,
            "UNSUBDIV" => d.iterations == 0,
            "DISSOLVE" => d.angle_limit == 0.0,
            _ => false,
        },
        S::Subsurf(m) => m.levels == 0,
        S::Weld(_) | S::Nodes(_) => same_counts,
        _ => matches!(
            s.type_identifier(),
            "ARMATURE"
                | "CAST"
                | "CLOTH"
                | "COLLISION"
                | "CORRECTIVE_SMOOTH"
                | "CURVE"
                | "DISPLACE"
                | "HOOK"
                | "LAPLACIANDEFORM"
                | "LAPLACIANSMOOTH"
                | "LATTICE"
                | "MESH_CACHE"
                | "MESH_DEFORM"
                | "PARTICLE_SYSTEM"
                | "SHRINKWRAP"
                | "SIMPLE_DEFORM"
                | "SMOOTH"
                | "SOFT_BODY"
                | "SURFACE"
                | "SURFACE_DEFORM"
                | "WARP"
                | "WAVE"
                | "DATA_TRANSFER"
                | "UV_PROJECT"
                | "UV_WARP"
                | "VERTEX_WEIGHT_EDIT"
                | "VERTEX_WEIGHT_MIX"
                | "VERTEX_WEIGHT_PROXIMITY"
                | "NORMAL_EDIT"
                | "WEIGHTED_NORMAL"
        ),
    }
}

/// Whether the add-on preference `KHR_materials_variants_ui` is on.
pub fn ui_on(prefs: &Preferences) -> bool {
    prefs
        .addons
        .get("io_scene_gltf2")
        .and_then(|p| p.get("KHR_materials_variants_ui"))
        .and_then(frap_rna::Value::as_bool)
        .unwrap_or(false)
}

/// `bpy.data.scenes[0].gltf2_KHR_materials_variants_variants` names.
fn scene_variants(main: &Main) -> Vec<SmolStr> {
    main.scenes.iter().next().map_or_else(Vec::new, |(_, s)| {
        s.gltf2_variants
            .variants
            .iter()
            .map(|v| v.name.clone())
            .collect()
    })
}

/// `gather_variant(variant_idx)`: the name of the variant at that Python index.
fn variant_name(list: &[SmolStr], idx: i32) -> Result<SmolStr, ExportError> {
    let n = list.len() as i64;
    let i = i64::from(idx);
    let j = if i < 0 { i + n } else { i };
    if (0..n).contains(&j) {
        return Ok(list[j as usize].clone());
    }
    Err(ExportError::Json(if i < 0 {
        format!("bpy_prop_collection[{i}]: out of range.")
    } else {
        format!("bpy_prop_collection[index]: index {i} out of range, size {n}")
    }))
}

/// Adds the `KHR_materials_variants` mappings of `mesh`'s material slot `slot` to `prim`.
pub fn primitive(
    g: &mut Gather,
    mesh: &Mesh,
    slot: usize,
    prim: &mut Primitive,
) -> Result<(), ExportError> {
    if !ui_on(g.ctx.prefs) || g.state::<WithoutVariantData>().0 {
        return Ok(());
    }
    let list = scene_variants(g.main);
    if list.is_empty() {
        return Ok(());
    }
    let mut mappings: Vec<VariantMapping> = Vec::new();
    let mut in_use: Vec<i32> = Vec::new();
    let entries = mesh
        .gltf2_variants
        .mesh_data
        .iter()
        .filter(|e| usize::try_from(e.material_slot_index).ok() == Some(slot));
    for e in entries {
        let mut variants: Vec<Index> = Vec::new();
        for (idx, v) in e.variants.iter().enumerate() {
            if e.variants[..idx]
                .iter()
                .any(|o| o.variant_idx == v.variant_idx)
            {
                continue;
            }
            if in_use.contains(&v.variant_idx) {
                g.warn(format!(
                    "Variant {} has 2 different materials for a single slot. Skipping it.",
                    v.variant_idx
                ));
                continue;
            }
            let name = variant_name(&list, v.variant_idx)?;
            in_use.push(v.variant_idx);
            let root = &mut g.state::<RootVariants>().0;
            let i = match root.iter().position(|n| *n == name) {
                Some(i) => i,
                None => {
                    root.push(name);
                    root.len() - 1
                }
            };
            variants.push(i as Index);
        }
        if variants.is_empty() {
            continue;
        }
        let material = match e.material {
            Some(m) => materials::gather_unconditional(g, m, mesh)?,
            None => None,
        };
        mappings.push(VariantMapping {
            material,
            variants,
            ..VariantMapping::default()
        });
    }
    if mappings.is_empty() {
        return Ok(());
    }
    let ext = KhrMaterialsVariantsPrimitive {
        mappings,
        ..KhrMaterialsVariantsPrimitive::default()
    };
    prim.extensions.0.insert(
        NAME.to_owned(),
        serde_json::to_value(&ext).map_err(|e| ExportError::Invalid(e.to_string()))?,
    );
    g.use_extension(NAME, false);
    Ok(())
}

/// Writes the root variant list (after every primitive).
pub fn finish(g: &mut Gather) -> Result<(), ExportError> {
    let names = std::mem::take(&mut g.state::<RootVariants>().0);
    if names.is_empty() {
        return Ok(());
    }
    let root = KhrMaterialsVariants {
        variants: names
            .into_iter()
            .map(|name| Variant {
                name: name.to_string(),
                ..Variant::default()
            })
            .collect(),
        ..KhrMaterialsVariants::default()
    };
    g.doc.extensions.0.insert(
        NAME.to_owned(),
        serde_json::to_value(&root).map_err(|e| ExportError::Invalid(e.to_string()))?,
    );
    Ok(())
}

/// The mesh objects of the export tree of each exported scene (`variants_reset_to_original`
/// runs per scene on `get_all_node_of_type(OBJECT)` after the tree is filtered).
fn tree_mesh_objects(
    main: &Main,
    ctx: &Context,
    settings: &ExportSettings,
) -> Vec<(IdRef<SceneId>, IdRef<ObjectId>)> {
    let mut out = Vec::new();
    for scene in crate::export::scenes::exported_scenes(main, ctx.win.scene, settings) {
        let mut win = ctx.win.clone();
        if frap_scene::window::set_scene(main, &mut win, scene).is_err() {
            continue;
        }
        let cx = Context {
            main,
            win: &win,
            prefs: ctx.prefs,
        };
        let tree = crate::export::vtree::build(main, &cx, settings);
        for n in &tree.nodes {
            if n.keep
                && n.kind == crate::export::vtree::VType::Object
                && main
                    .objects
                    .get(n.object)
                    .is_some_and(|o| o.object_type() == "MESH")
            {
                out.push((scene, n.object));
            }
        }
    }
    out
}

/// `VExportTree.variants_reset_to_original`, applied to `bpy.data` before the export (the
/// add-on writes the default materials into the material slots of the exported mesh objects
/// and leaves them there). Does nothing while `KHR_materials_variants_ui` is off. Returns
/// whether anything was written.
pub fn reset_to_original(
    main: &mut Main,
    ctx_win: &frap_scene::WindowState,
    prefs: &Preferences,
    settings: &ExportSettings,
    log: &mut ChangeLog,
) -> bool {
    if !ui_on(prefs) {
        return false;
    }
    let objects = {
        let ctx = Context {
            main,
            win: ctx_win,
            prefs,
        };
        tree_mesh_objects(main, &ctx, settings)
    };
    let mut changed = false;
    for (_, ob) in objects {
        let Some(o) = main.objects.get(ob) else {
            continue;
        };
        let Some(me) = o
            .data
            .and_then(|d| d.typed::<frap_rna::MeshId>())
            .and_then(|m| main.meshes.get(m))
        else {
            continue;
        };
        let n = frap_scene::id::slots::slot_count(main, o);
        let resets: Vec<(usize, Option<IdRef<frap_rna::MaterialId>>)> = (0..n)
            .filter_map(|s| me.gltf2_variants.default_for_slot(s).map(|m| (s, m)))
            .collect();
        for (s, m) in resets {
            let current = main
                .objects
                .get(ob)
                .map(|o| frap_scene::id::slots::slot(main, o, s).1);
            if current != Some(m) {
                frap_scene::id::slots::set_slot_material(main, ob, s, m, log);
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_names_use_python_indexing() {
        let list: Vec<SmolStr> = vec!["A".into(), "B".into()];
        assert_eq!(variant_name(&list, 1).unwrap(), "B");
        assert_eq!(variant_name(&list, -1).unwrap(), "B");
        let e = variant_name(&list, 2).unwrap_err().to_string();
        assert_eq!(
            e,
            "bpy_prop_collection[index]: index 2 out of range, size 2"
        );
        assert!(variant_name(&list, -3).is_err());
    }

    #[test]
    fn modifiers_pass_the_variant_data_through_as_verified() {
        use frap_scene::generated::modifier_settings::ModifierType as T;
        let fresh = |t| ModifierSettings::new(t);
        // Verified on 5.2.4 with `modifiers.new` defaults.
        for t in [
            T::Armature,
            T::Displace,
            T::SimpleDeform,
            T::Smooth,
            T::Cast,
            T::Lattice,
        ] {
            assert!(passes_data_through(&fresh(t), false), "{t:?}");
        }
        for t in [
            T::WeightedNormal,
            T::NormalEdit,
            T::UvProject,
            T::DataTransfer,
        ] {
            assert!(passes_data_through(&fresh(t), false), "{t:?}");
        }
        for t in [
            T::Array,
            T::Bevel,
            T::Mirror,
            T::Subsurf,
            T::Triangulate,
            T::Solidify,
        ] {
            assert!(!passes_data_through(&fresh(t), true), "{t:?}");
        }
        // Disabled (no operand) and the early returns keep the data.
        assert!(passes_data_through(&fresh(T::Boolean), false));
        assert!(passes_data_through(&fresh(T::Decimate), false));
        let ModifierSettings::Decimate(mut d) = fresh(T::Decimate) else {
            unreachable!()
        };
        d.ratio = 0.5;
        assert!(!passes_data_through(&ModifierSettings::Decimate(d), true));
        let ModifierSettings::Subsurf(mut s) = fresh(T::Subsurf) else {
            unreachable!()
        };
        s.levels = 0;
        assert!(passes_data_through(&ModifierSettings::Subsurf(s), false));
        let ModifierSettings::Bevel(mut b) = fresh(T::Bevel) else {
            unreachable!()
        };
        b.width = 0.0;
        assert!(passes_data_through(&ModifierSettings::Bevel(b), false));
        // Weld and Geometry Nodes: by the evaluated counts.
        assert!(passes_data_through(&fresh(T::Weld), true));
        assert!(!passes_data_through(&fresh(T::Weld), false));
        assert!(passes_data_through(&fresh(T::Nodes), true));
    }
}
