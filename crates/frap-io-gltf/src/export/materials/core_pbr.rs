// SPDX-License-Identifier: GPL-3.0-or-later
//! `pbrMetallicRoughness`: base colour factor and texture, metallic and roughness factors, and
//! a metallic-roughness texture used as is.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/
//! pbr_metallic_roughness.py (`__gather_base_color_factor`: RGB clamped to `[0, 1]` plus the
//! alpha factor, omitted when `[1, 1, 1, 1]`; `__gather_metallic_factor` /
//! `__gather_roughness_factor`: omitted when 1; `__gather_metallic_roughness_texture`), and
//! image.py (`__get_image_data_mapping`: a texture plugged straight into Metallic/Roughness
//! uses the glTF channels B/G). Member order: `baseColorTexture` before
//! `metallicRoughnessTexture`. An empty object is dropped.

use frap_gltf::json::{Material, PbrMetallicRoughness};
use frap_material::{ChannelSel, LoweredMaterial};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use super::num;
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{TextureSlot, gather_texture_info};

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

/// Writes `pbrMetallicRoughness`.
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
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
        pbr.base_color_texture = gather_texture_info(g, t, (r, TextureSlot::BaseColor), mesh)?;
    }
    // A metallic-roughness texture is written when the image already holds metallic in B and
    // roughness in G (one image, or only one of the two channels textured).
    let (mt, rt) = (lm.metallic.texture.as_ref(), lm.roughness.texture.as_ref());
    let as_is = |t: &frap_material::TexRef, want: ChannelSel| t.channel == want;
    let mr = match (mt, rt) {
        (None, None) => None,
        (Some(a), Some(b))
            if a.image == b.image && as_is(a, ChannelSel::B) && as_is(b, ChannelSel::G) =>
        {
            Some(a)
        }
        (Some(a), None) if as_is(a, ChannelSel::B) => Some(a),
        (None, Some(b)) if as_is(b, ChannelSel::G) => Some(b),
        _ => {
            g.warn(format!(
                "material '{}': metallic/roughness textures need channel packing, not exported yet (M3)",
                m.name.as_deref().unwrap_or("")
            ));
            None
        }
    };
    if let Some(t) = mr {
        pbr.metallic_roughness_texture =
            gather_texture_info(g, t, (r, TextureSlot::MetallicRoughness), mesh)?;
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
