// SPDX-License-Identifier: GPL-3.0-or-later
//! `occlusionTexture` and the ORM image.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO authors):
//! - `__gather_orm_texture`: when the occlusion socket has a texture and metallic and/or
//!   roughness have one, and all those images have the same size (`check_same_size_images`,
//!   `Image.size`), the sockets `(occlusion, roughness, metallic)` (or `(occlusion,
//!   roughness)`, `(occlusion, metallic)`) make one image: R occlusion, G roughness,
//!   B metallic. `occlusionTexture` and `metallicRoughnessTexture` then share that texture
//!   (its sampler is the occlusion Image Texture's; each texture info keeps its own
//!   `texCoord` and transform). Otherwise they are written separately (the add-on logs an
//!   info message, no warning).
//! - `__gather_occlusion_texture`: `strength` from the Mix with white in front of the
//!   socket (`occlusion.rs`), written whenever that Mix exists (`1` included), omitted
//!   otherwise; the ORM sockets are used only when their texture can be written
//!   (`__gather_orm_texture`'s double check, `images::texture_writable`, WP 8.12).

use frap_gltf::json::{Material, OcclusionTextureInfo};
use frap_material::LoweredMaterial;
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use super::num;
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{Role, TexInput, TextureSlot, gather_texture_info_from};

/// The ORM sockets (`__gather_orm_texture`), `None` when occlusion is written alone.
pub fn orm_inputs<'l>(g: &Gather, lm: &'l LoweredMaterial) -> Option<Vec<TexInput<'l>>> {
    let occ = lm.occlusion.as_ref()?;
    let mut v = vec![TexInput {
        tex: occ,
        role: Role::Occlusion,
    }];
    let (m, r) = (lm.metallic.texture.as_ref(), lm.roughness.texture.as_ref());
    if m.is_none() && r.is_none() {
        return None;
    }
    if let Some(tex) = r {
        v.push(TexInput {
            tex,
            role: Role::Roughness,
        });
    }
    if let Some(tex) = m {
        v.push(TexInput {
            tex,
            role: Role::Metallic,
        });
    }
    // `check_same_size_images`
    let base = frap_scene::id::PathBase {
        blend: g.main.filepath.clone(),
        cwd: g.settings.cwd.clone(),
    };
    let mut sizes = v.iter().map(|i| {
        g.main
            .images
            .get(i.tex.image)
            .and_then(|im| im.info(&base))
            .map_or((0, 0), |s| (s.x, s.y))
    });
    let first = sizes.next()?;
    (first != (0, 0) && sizes.all(|s| s == first)).then_some(v)
}

/// Writes `occlusionTexture` (`orm`: the shared ORM sockets).
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
    orm: Option<&[TexInput]>,
    mesh: &Mesh,
    m: &mut Material,
) -> Result<(), ExportError> {
    let Some(occ) = &lm.occlusion else {
        return Ok(());
    };
    let alone = [TexInput {
        tex: occ,
        role: Role::Occlusion,
    }];
    let (inputs, slot) = match orm {
        Some(o) => (o, TextureSlot::MetallicRoughness),
        None => (&alone[..], TextureSlot::Occlusion),
    };
    if let Some(info) = gather_texture_info_from(g, occ, inputs, (r, slot), mesh)? {
        m.occlusion_texture = Some(OcclusionTextureInfo {
            index: info.index,
            // Written whenever a Mix gave it, 1 included (WP 8.12).
            strength: (lm.ext.occlusion_strength_set || lm.occlusion_strength != 1.0)
                .then(|| num(lm.occlusion_strength)),
            tex_coord: info.tex_coord,
            extensions: info.extensions,
            extras: None,
        });
    }
    Ok(())
}
