// SPDX-License-Identifier: GPL-3.0-or-later
//! Custom properties → `extras` (M3, WP 3.7; stub created by M3 W0). Port target:
//! io_scene_gltf2 5.2.41 `blender/com/extras.py` (`generate_extras`: ID properties, nested
//! groups, arrays, the `RNA_BLACKLIST`), written only with `export_extras` (default False).
//!
//! W0 wiring: `nodes.rs` (objects), `mesh/gather.rs` (meshes), `materials/mod.rs` (materials)
//! and `scenes.rs` (scenes) ask these functions; `None` writes no `extras`.

use frap_gltf::json::Extras;
use frap_mesh::Mesh;
use frap_scene::id::{Material, Object, Scene};

use crate::export::gather::Gather;

/// `extras` of an object's node.
pub fn object(g: &mut Gather, ob: &Object) -> Option<Extras> {
    let _ = (g, ob);
    None
}

/// `extras` of a mesh.
pub fn mesh(g: &mut Gather, mesh: &Mesh) -> Option<Extras> {
    let _ = (g, mesh);
    None
}

/// `extras` of a material.
pub fn material(g: &mut Gather, mat: &Material) -> Option<Extras> {
    let _ = (g, mat);
    None
}

/// `extras` of a scene.
pub fn scene(g: &mut Gather, scene: &Scene) -> Option<Extras> {
    let _ = (g, scene);
    None
}
