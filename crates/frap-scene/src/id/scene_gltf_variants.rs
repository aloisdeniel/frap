// SPDX-License-Identifier: GPL-3.0-or-later
//! The glTF add-on's scene-level `KHR_materials_variants` data
//! (`Scene.gltf2_KHR_materials_variants_variants`, `Scene.gltf2_active_variant`), registered by
//! `io_scene_gltf2` while its preference `KHR_materials_variants_ui` is on (§4.1). The add-on
//! reads them from `bpy.data.scenes[0]`.
//!
//! M7 W0 (decision log M7W0-8) froze the data; WP 7.5 owns this file afterwards (RNA binding,
//! the add-on's variant operators if needed). The per-mesh data is
//! `frap_mesh::gltf_variants::MeshGltfVariants`.

use smol_str::SmolStr;

/// `gltf2_KHR_materials_variants_variant` (a PropertyGroup: `name`, `variant_idx`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GltfVariant {
    /// `name`
    pub name: SmolStr,
    /// `variant_idx`
    pub variant_idx: i32,
}

/// Every variant property of one scene.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneGltfVariants {
    /// `Scene.gltf2_KHR_materials_variants_variants`
    pub variants: Vec<GltfVariant>,
    /// `Scene.gltf2_active_variant`
    pub active_variant: i32,
}
