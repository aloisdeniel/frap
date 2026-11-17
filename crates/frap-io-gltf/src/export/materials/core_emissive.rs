// SPDX-License-Identifier: GPL-3.0-or-later
//! `emissiveFactor`, `emissiveTexture` and `KHR_materials_emissive_strength`.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/extensions/
//! emission.py (`export_emission_factor`: colour × strength, `[1, 1, 1]` when only a texture
//! is linked, omitted when all zero; `export_emission_strength_extension`: written when a
//! channel exceeds 1, not required) and materials.py (`gather_material`: the factor divided by
//! the strength in that case). The arithmetic is Python's (doubles of the float32 inputs).

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_emissive_strength::KhrMaterialsEmissiveStrength;
use frap_gltf::json::Material;
use frap_material::LoweredMaterial;
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{TextureSlot, gather_texture_info};

/// The emissive factor and the strength extension value (`None` when not needed).
pub fn factor(lm: &LoweredMaterial) -> (Option<[f64; 3]>, Option<f64>) {
    let strength = f64::from(lm.emissive_strength);
    let mut f = lm.emissive.factor.map(|c| f64::from(c) * strength);
    if lm.emissive.texture.is_some() && f == [0.0; 3] {
        f = [1.0; 3];
    }
    if f == [0.0; 3] {
        return (None, None);
    }
    if f.iter().any(|c| *c > 1.0) {
        let s = f.iter().copied().fold(f64::MIN, f64::max);
        (Some(f.map(|c| c / s)), Some(s))
    } else {
        (Some(f), None)
    }
}

/// Writes the emission members (gathering the texture in member order).
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
    mesh: &Mesh,
    m: &mut Material,
) -> Result<(), ExportError> {
    let (f, strength) = factor(lm);
    m.emissive_factor = f;
    if let Some(t) = &lm.emissive.texture {
        m.emissive_texture = gather_texture_info(g, t, (r, TextureSlot::Emissive), mesh)?;
    }
    if let Some(s) = strength {
        m.extensions.set(Some(KhrMaterialsEmissiveStrength {
            emissive_strength: Some(s),
            ..KhrMaterialsEmissiveStrength::default()
        }));
        g.use_extension(KhrMaterialsEmissiveStrength::NAME, false);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::materials::lower::defaults;

    #[test]
    fn strength_rules() {
        let mat = frap_scene::id::Material::new("M");
        let mut lm = defaults(&mat);
        // Principled default: strength 0 → nothing.
        lm.emissive.factor = [1.0, 1.0, 1.0];
        lm.emissive_strength = 0.0;
        assert_eq!(factor(&lm), (None, None));
        // Within [0, 1]: factor only.
        lm.emissive.factor = [0.5, 0.25, 1.0];
        lm.emissive_strength = 1.0;
        assert_eq!(factor(&lm), (Some([0.5, 0.25, 1.0]), None));
        // Research run: colour (1, 0.5, 0.2) at strength 5 → factor (1, 0.5, 0.2), strength 5.
        lm.emissive.factor = [1.0, 0.5, 0.2];
        lm.emissive_strength = 5.0;
        let (f, s) = factor(&lm);
        assert_eq!(s, Some(5.0));
        let f = f.unwrap();
        assert_eq!(f[0], 1.0);
        assert_eq!(f[1], 0.5);
        assert!((f[2] - 0.2).abs() < 1e-7);
    }
}
