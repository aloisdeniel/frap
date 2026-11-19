// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_emissive_strength` (ARCHITECTURE §16.2): glTF clamps `emissiveFactor` to
//! [0, 1], so an emission brighter than 1 is split into a factor and a strength.
//!
//! Lowering ports the strength split of `gather_material` and
//! `export_emission_strength_extension` (`io_scene_gltf2/blender/exp/material/materials.py`,
//! `extensions/emission.py`, glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO
//! authors): when a channel of the emissive factor (colour × strength, from `emission.rs`)
//! exceeds 1, the largest channel becomes `emissive_strength` and the factor is divided by
//! it; otherwise the strength stays 1 and no extension is written. Computed in f64 like
//! Python.
//!
//! Raising is part of `emission.rs` (the importer's `emission()` sets the factor and the
//! strength together: the strength times a grey factor goes into `Emission Strength`).

use crate::{LowerCx, LoweredMaterial, MaterialPattern};

fn lower(_: &LowerCx, out: &mut LoweredMaterial) {
    if out.unlit {
        return;
    }
    let strength = f64::from(out.emissive_strength);
    let f = out.emissive.factor.map(|c| f64::from(c) * strength);
    let max = f.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if max > 1.0 {
        out.emissive.factor = f.map(|c| (c / max) as f32);
        out.emissive_strength = max as f32;
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "emission_strength",
    order: 11,
    lower,
    raise: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_above_one() {
        let mut lm = LoweredMaterial::gltf_default();
        let mat = frap_scene::id::Material::new("M");
        let tree = frap_nodes::NodeTree::default_material();
        let images = frap_scene::Main::default();
        let cx = LowerCx::new(&tree, &mat, &images);
        // Research run: colour (1, 0.5, 0.2) at strength 5 → factor (1, 0.5, 0.2), strength 5.
        lm.emissive.factor = [5.0, 2.5, 1.0];
        lower(&cx, &mut lm);
        assert_eq!(lm.emissive.factor, [1.0, 0.5, 0.2]);
        assert_eq!(lm.emissive_strength, 5.0);
        // Within [0, 1]: unchanged.
        let mut lm = LoweredMaterial::gltf_default();
        lm.emissive.factor = [0.5, 1.0, 0.0];
        lower(&cx, &mut lm);
        assert_eq!((lm.emissive.factor, lm.emissive_strength), ([0.5, 1.0, 0.0], 1.0));
    }
}
