// SPDX-License-Identifier: GPL-3.0-or-later
//! The `texCoord` of a texture reference (`blender/exp/material/texture_info.py`,
//! `get_final_material`). Moved out of `images.rs` by M7 W0 (decision log M7W0-4) so WP 7.7
//! owns it (the Attribute-node `texCoord`, deviations §2) while WP 7.3 owns `images.rs`.

use frap_material::{TexRef, UvSource};
use frap_mesh::Mesh;

/// The `TEXCOORD_n` a texture reads on `mesh`: a named UV map's position in the mesh's UV
/// maps, the active-render map's position, or the given index (0 when the map is missing).
pub fn tex_coord(t: &TexRef, mesh: &Mesh) -> u32 {
    let maps = mesh.uv_maps();
    let pos = |name: &str| maps.iter().position(|(n, _)| *n == name);
    let render = || mesh.uv_active_render.as_deref().and_then(pos).unwrap_or(0) as u32;
    match &t.uv {
        // `get_final_material`: a name that is not a UV map uses the render map.
        UvSource::Named(n) => pos(n).map_or_else(render, |p| p as u32),
        UvSource::Index(i) => *i,
        _ => render(),
    }
}
