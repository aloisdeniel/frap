// SPDX-License-Identifier: GPL-3.0-or-later
//! `alphaMode` and `alphaCutoff` from the lowered alpha mode (decided by the node graph, not
//! `blend_method`).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (`__gather_alpha_mode`: OPAQUE is omitted; `__gather_alpha_cutoff`: only with MASK, omitted
//! when 0.5).

use frap_gltf::json::{AlphaMode as GAlphaMode, Material};
use frap_material::{AlphaMode, LoweredMaterial};

use super::num;

/// Writes `alphaMode` / `alphaCutoff`.
pub fn apply(lm: &LoweredMaterial, m: &mut Material) {
    match lm.alpha_mode {
        AlphaMode::Opaque => {}
        AlphaMode::Mask { cutoff } => {
            m.alpha_mode = Some(GAlphaMode::Mask);
            m.alpha_cutoff = (cutoff != 0.5).then(|| num(cutoff));
        }
        AlphaMode::Blend => m.alpha_mode = Some(GAlphaMode::Blend),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::materials::lower::defaults;

    #[test]
    fn omission_rules() {
        let mat = frap_scene::id::Material::new("M");
        let mut lm = defaults(&mat);
        let mut m = Material::default();
        apply(&lm, &mut m);
        assert_eq!((m.alpha_mode, m.alpha_cutoff), (None, None));
        lm.alpha_mode = AlphaMode::Mask { cutoff: 0.5 };
        apply(&lm, &mut m);
        assert_eq!(
            (m.alpha_mode, m.alpha_cutoff),
            (Some(GAlphaMode::Mask), None)
        );
        lm.alpha_mode = AlphaMode::Mask { cutoff: 0.25 };
        apply(&lm, &mut m);
        assert_eq!(m.alpha_cutoff, Some(0.25));
        let mut m = Material::default();
        lm.alpha_mode = AlphaMode::Blend;
        apply(&lm, &mut m);
        assert_eq!(
            (m.alpha_mode, m.alpha_cutoff),
            (Some(GAlphaMode::Blend), None)
        );
    }
}
