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
//! The armature handling of `gltf_skins` (Armature modifiers disabled while evaluating) is M6.

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId, ObjectId};
use frap_scene::id::Object;

use crate::export::gather::Gather;
use crate::export::mesh::gather::slot_materials;

/// `export_apply` is honoured (no "not supported yet" WARNING).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static APPLY_PROPS: &[&str] = &["export_apply"];

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
