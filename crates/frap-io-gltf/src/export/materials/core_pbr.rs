// SPDX-License-Identifier: GPL-3.0-or-later
//! `pbrMetallicRoughness`: base colour factor and texture, metallic and roughness factors, and
//! the metallic-roughness texture (packed with occlusion into an ORM image when
//! `core_occlusion.rs` says so).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/
//! pbr_metallic_roughness.py (glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO
//! authors): `__gather_base_color_factor` (RGB clamped to `[0, 1]` plus the alpha factor,
//! omitted when `[1, 1, 1, 1]`), `__gather_base_color_texture` (the Base Color and Alpha
//! sockets that have a texture, the first one primary), `__gather_metallic_factor` /
//! `__gather_roughness_factor` (omitted when 1), `__gather_metallic_roughness_texture`
//! (`(metallic, roughness)`, `(roughness,)` or `(metallic,)` sockets, the first primary; the
//! ORM sockets when occlusion shares the image). Member order: `baseColorTexture` before
//! `metallicRoughnessTexture`. An empty object is dropped.

use frap_gltf::json::{Material, PbrMetallicRoughness};
use frap_material::{LoweredMaterial, TexRef};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use super::num;
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{
    Role, TexInput, TextureSlot, gather_texture_info, gather_texture_info_from,
};

/// `baseColorFactor` (`None` when it equals the glTF default).
pub fn base_color_factor(lm: &LoweredMaterial) -> Option<[f64; 4]> {
    let f = lm.base_color.factor;
    let c = |x: f32| num(x.clamp(0.0, 1.0));
    let rgba = [c(f[0]), c(f[1]), c(f[2]), num(f[3])];
    (rgba != [1.0; 4]).then_some(rgba)
}

/// `metallicFactor` / `roughnessFactor` (`None` when 1).
pub fn scalar_factor(x: f32) -> Option<f64> {
    (x != 1.0).then(|| num(x))
}

/// The metallic-roughness sockets (`texture_input`): `(metallic, roughness)`, `(roughness,)`
/// or `(metallic,)`, with their roles.
pub fn metallic_roughness_inputs(lm: &LoweredMaterial) -> Vec<TexInput<'_>> {
    let m = lm.metallic.texture.as_ref().map(|tex| TexInput {
        tex,
        role: Role::Metallic,
    });
    let r = lm.roughness.texture.as_ref().map(|tex| TexInput {
        tex,
        role: Role::Roughness,
    });
    m.into_iter().chain(r).collect()
}

/// Writes `pbrMetallicRoughness`; `orm` holds the ORM sockets when occlusion, roughness and
/// metallic share one image.
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
    orm: Option<&[TexInput]>,
    mesh: &Mesh,
    m: &mut Material,
) -> Result<(), ExportError> {
    let mut pbr = PbrMetallicRoughness {
        base_color_factor: base_color_factor(lm),
        metallic_factor: scalar_factor(lm.metallic.factor),
        roughness_factor: scalar_factor(lm.roughness.factor),
        ..PbrMetallicRoughness::default()
    };
    if let Some(t) = &lm.base_color.texture {
        pbr.base_color_texture = match &lm.ext.base_alpha {
            // The alpha packed into the base colour image (WP 8.12, `__get_image_data_mapping`
            // with the Base Color and Alpha sockets): the colour image whole, then the alpha.
            Some(pa) => {
                let alpha = TexInput {
                    tex: &pa.texture,
                    role: if pa.rgb_to_bw {
                        Role::AlphaBw
                    } else {
                        Role::Alpha
                    },
                };
                let full = TexInput {
                    tex: t,
                    role: Role::Full,
                };
                let inputs: Vec<TexInput> = if pa.texture == *t {
                    vec![alpha]
                } else {
                    vec![full, alpha]
                };
                gather_texture_info_from(g, t, &inputs, (r, TextureSlot::BaseColor), mesh)?
            }
            None => gather_texture_info(g, t, (r, TextureSlot::BaseColor), mesh)?,
        };
    }
    let inputs = metallic_roughness_inputs(lm);
    if let Some(primary) = inputs.first().map(|i| i.tex) {
        let primary: &TexRef = primary;
        pbr.metallic_roughness_texture = gather_texture_info_from(
            g,
            primary,
            orm.unwrap_or(&inputs),
            (r, TextureSlot::MetallicRoughness),
            mesh,
        )?;
    }
    if pbr != PbrMetallicRoughness::default() {
        m.pbr_metallic_roughness = Some(pbr);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::materials::lower::defaults;

    #[test]
    fn factor_omission_rules() {
        let mat = frap_scene::id::Material::new("M");
        let mut lm = defaults(&mat);
        assert_eq!(base_color_factor(&lm), None);
        // Principled default: (0.8, 0.8, 0.8, 1) written as float32 widened.
        lm.base_color.factor = [0.8, 0.8, 0.8, 1.0];
        assert_eq!(
            base_color_factor(&lm),
            Some([
                0.800_000_011_920_929,
                0.800_000_011_920_929,
                0.800_000_011_920_929,
                1.0
            ])
        );
        // RGB clamped, alpha not.
        lm.base_color.factor = [2.0, -1.0, 0.5, 0.5];
        assert_eq!(base_color_factor(&lm), Some([1.0, 0.0, 0.5, 0.5]));
        assert_eq!(scalar_factor(1.0), None);
        assert_eq!(scalar_factor(0.0), Some(0.0));
        assert_eq!(scalar_factor(0.5), Some(0.5));
    }
}
