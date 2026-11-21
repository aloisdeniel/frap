// SPDX-License-Identifier: GPL-3.0-or-later
//! The glTF add-on's per-mesh `KHR_materials_variants` data (M7 W0 contract, decision log
//! M7W0-8): the PropertyGroups `io_scene_gltf2` registers on `Mesh` while its preference
//! `KHR_materials_variants_ui` is on (`blender/com/gltf2_blender_ui.py`; dumped with that
//! preference on, §4.1). Plain data: WP 7.5 binds the RNA (`Mesh.gltf2_variant_mesh_data`,
//! `gltf2_variant_default_materials`, `gltf2_variant_pointer`) and implements the import and
//! export; WP 7.6 persists it in `.frap`.

use frap_rna::{IdRef, MaterialId};
use smol_str::SmolStr;

/// `gltf2_KHR_materials_variant_pointer`: one variant of a mapping, by index into
/// `bpy.data.scenes[0].gltf2_KHR_materials_variants_variants` (`variant.variant_idx`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GltfVariantPointer {
    /// `variant.variant_idx`
    pub variant_idx: i32,
}

/// `gltf2_KHR_materials_variants_primitive`: the material a set of variants selects on one
/// material slot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GltfVariantPrimitive {
    /// `material_slot_index`
    pub material_slot_index: i32,
    /// `material`
    pub material: Option<IdRef<MaterialId>>,
    /// `variants`
    pub variants: Vec<GltfVariantPointer>,
    /// `active_variant_idx`
    pub active_variant_idx: i32,
}

/// `gltf2_KHR_materials_variants_default_material`: the material a slot gets back when the
/// variants are reset (`variants_reset_to_original`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GltfVariantDefaultMaterial {
    /// `material_slot_index`
    pub material_slot_index: i32,
    /// `default_material`
    pub default_material: Option<IdRef<MaterialId>>,
}

/// Every variant property of one mesh.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MeshGltfVariants {
    /// `Mesh.gltf2_variant_mesh_data`
    pub mesh_data: Vec<GltfVariantPrimitive>,
    /// `Mesh.gltf2_variant_default_materials`
    pub default_materials: Vec<GltfVariantDefaultMaterial>,
    /// `Mesh.gltf2_variant_pointer` (a UI string of the add-on)
    pub pointer: SmolStr,
}

impl MeshGltfVariants {
    /// Every material this data references (user-counted, as ID pointers of ID properties
    /// are).
    pub fn materials(&self) -> impl Iterator<Item = IdRef<MaterialId>> + '_ {
        self.mesh_data.iter().filter_map(|p| p.material).chain(
            self.default_materials
                .iter()
                .filter_map(|d| d.default_material),
        )
    }

    /// Re-points (or with `None` clears) every reference to material `from`.
    pub fn remap_material(&mut self, from: IdRef<MaterialId>, to: Option<IdRef<MaterialId>>) {
        for p in &mut self.mesh_data {
            if p.material == Some(from) {
                p.material = to;
            }
        }
        for d in &mut self.default_materials {
            if d.default_material == Some(from) {
                d.default_material = to;
            }
        }
    }
}
