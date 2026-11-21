// SPDX-License-Identifier: GPL-3.0-or-later
//! The `texCoord` of a texture reference (`blender/exp/material/texture_info.py`,
//! `get_final_material`). Moved out of `images.rs` by M7 W0 (decision log M7W0-4) so WP 7.7
//! owns it (the Attribute-node `texCoord`, deviations §2) while WP 7.3 owns `images.rs`.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `get_final_material` maps each texture's `uv_info` to an index of the mesh:
//! - `Fixed` (a UV Map node, [`UvSource::Named`]): the UV map's index, the active render map's
//!   when the mesh has no map of that name;
//! - `Render` ([`UvSource::ActiveRender`]): `uv_layers.active_render_index`;
//! - `Attribute` (an Attribute node of type GEOMETRY, [`UvSource::Attribute`], WP 7.7): the UV
//!   map of that name, else `attr_indices[name]`, the extra `TEXCOORD_n` the mesh exporter
//!   wrote for the attribute (`primitive_creation_*`: from `tex_coord_max`, the number of
//!   exported UV maps, one per attribute of the material, see
//!   `mesh/writers/texcoord_extra.rs`). [`resolve_attributes`] turns those into
//!   [`UvSource::Index`] per mesh before the material is written.

use frap_material::{LoweredMaterial, TexRef, UvSource};
use frap_mesh::Mesh;
use frap_scene::id::Material;
use smol_str::SmolStr;

use crate::export::ExportSettings;

/// The `TEXCOORD_n` a texture reads on `mesh`: a named UV map's position in the mesh's UV
/// maps, the active-render map's position, or the given index (0 when the map is missing).
pub fn tex_coord(t: &TexRef, mesh: &Mesh) -> u32 {
    let maps = mesh.uv_maps();
    let pos = |name: &str| maps.iter().position(|(n, _)| *n == name);
    let render = || mesh.uv_active_render.as_deref().and_then(pos).unwrap_or(0) as u32;
    match &t.uv {
        // `get_final_material`: a name that is not a UV map uses the render map. An attribute
        // that is not a UV map got its extra index from [`resolve_attributes`]; one the mesh
        // exporter did not write (the attribute is missing: the add-on's `attr_indices` raises
        // `KeyError` there) falls back to the render map.
        UvSource::Named(n) | UvSource::Attribute(n) => pos(n).map_or_else(render, |p| p as u32),
        UvSource::Index(i) => *i,
        _ => render(),
    }
}

/// The attributes a material reads as UVs that are written as extra `TEXCOORD_n` on `mesh`,
/// in index order (`uvmap_attribute_list`: not a UV map of the mesh, present on it; the same
/// list as `texcoord_extra::plan`).
pub fn extra_attributes(mat: &Material, lm: &LoweredMaterial, mesh: &Mesh) -> Vec<SmolStr> {
    let uv_names: Vec<&str> = mesh.uv_maps().into_iter().map(|(n, _)| n).collect();
    crate::export::mesh::writers::texcoord_extra::attribute_uvs(mat, lm)
        .into_iter()
        .filter(|n| !uv_names.contains(&n.as_str()))
        .filter(|n| mesh.attributes().get(n).is_some())
        .collect()
}

/// `attr_indices[name]` for an attribute of `extra` (`tex_coord_max + k`).
pub fn attribute_index(
    name: &str,
    extra: &[SmolStr],
    mesh: &Mesh,
    settings: &ExportSettings,
) -> Option<u32> {
    let base = if settings.export_texcoords {
        mesh.uv_maps().len()
    } else {
        0
    };
    let k = extra.iter().position(|n| n == name)?;
    Some((base + k) as u32)
}

/// Resolves every [`UvSource::Attribute`] texture of `lm` that is not a UV map of `mesh` to
/// the [`UvSource::Index`] of its extra `TEXCOORD_n` (see the module documentation).
pub fn resolve_attributes(
    lm: &mut LoweredMaterial,
    mat: &Material,
    mesh: &Mesh,
    settings: &ExportSettings,
) {
    let maps = mesh.uv_maps();
    let needs = crate::export::materials::textures(lm).iter().any(
        |t| matches!(&t.uv, UvSource::Attribute(n) if !maps.iter().any(|(m, _)| *m == n.as_str())),
    );
    if !needs {
        return;
    }
    let extra = extra_attributes(mat, lm, mesh);
    crate::export::materials::for_each_texture_mut(lm, |t| {
        if let UvSource::Attribute(n) = &t.uv
            && !maps.iter().any(|(m, _)| *m == n.as_str())
            && let Some(i) = attribute_index(n, &extra, mesh, settings)
        {
            t.uv = UvSource::Index(i);
        }
    });
}
