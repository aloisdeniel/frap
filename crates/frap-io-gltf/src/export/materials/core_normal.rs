// SPDX-License-Identifier: GPL-3.0-or-later
//! `normalTexture` with `scale`.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (`__gather_normal_texture`) and texture_info.py (`__gather_normal_scale`: the Normal Map
//! node's unlinked `Strength`, omitted when 1), glTF-Blender-IO 5.2.41, Apache-2.0, The
//! glTF-Blender-IO authors. The image is the texture's, copied whole.

use frap_gltf::json::{Material, NormalTextureInfo};
use frap_material::LoweredMaterial;
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use super::num;
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{TextureSlot, gather_texture_info};

/// Writes `normalTexture`.
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
    mesh: &Mesh,
    m: &mut Material,
) -> Result<(), ExportError> {
    if let Some(n) = &lm.normal
        && let Some(info) = gather_texture_info(g, &n.texture, (r, TextureSlot::Normal), mesh)?
    {
        m.normal_texture = Some(NormalTextureInfo {
            index: info.index,
            scale: (n.scale != 1.0).then(|| num(n.scale)),
            tex_coord: info.tex_coord,
            extensions: info.extensions,
            extras: None,
        });
    }
    Ok(())
}
