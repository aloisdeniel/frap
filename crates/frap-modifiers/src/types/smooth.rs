// SPDX-License-Identifier: GPL-3.0-or-later
//! SMOOTH (`SmoothModifier`): `iterations` passes moving every vertex towards the average of
//! the midpoints of its edges by `factor`, per enabled axis (`use_x/y/z`), weighted by
//! `vertex_group` (`invert_vertex_group`). Disabled when `factor` is 0 or no axis is enabled.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_smooth.cc (Blender 5.2.4).

pub(crate) mod common;

use frap_scene::generated::modifier_settings::SmoothSettings;

use self::common::{deform, find_weight, get_vgroup};
use crate::prelude::*;

fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Smooth(s) if s.factor == 0.0 || !(s.use_x || s.use_y || s.use_z) => {
            Some("Smooth: factor is 0 or no axis is enabled")
        }
        _ => None,
    }
}

#[modifier(
    SMOOTH,
    kind = OnlyDeform,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | SUPPORTS_EDITMODE,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Smooth(s) = &md.settings else {
        return Ok(mesh);
    };
    Ok(deform(mesh, |m, cos| smooth_do(s, m, cos)))
}

// Ported from blender/source/blender/modifiers/intern/MOD_smooth.cc (smoothModifier_do).
/// `smoothModifier_do` on `cos` (the mesh's positions).
pub(crate) fn smooth_do(s: &SmoothSettings, mesh: &Mesh, cos: &mut [[f32; 3]]) {
    let n = cos.len();
    let mut acc = vec![[0.0f32; 3]; n];
    let mut count = vec![0u32; n];
    let fac_new = s.factor;
    let fac_orig = 1.0 - fac_new;
    let vg = get_vgroup(mesh, &s.vertex_group);
    let edges = mesh.edges();
    for j in 0..s.iterations.max(0) {
        frap_math::budget::check();
        if j != 0 {
            acc.fill([0.0; 3]);
            count.fill(0);
        }
        for e in edges {
            let (a, b) = (e[0] as usize, e[1] as usize);
            let mid = [
                0.5 * (cos[a][0] + cos[b][0]),
                0.5 * (cos[a][1] + cos[b][1]),
                0.5 * (cos[a][2] + cos[b][2]),
            ];
            for v in [a, b] {
                count[v] += 1;
                acc[v][0] += mid[0];
                acc[v][1] += mid[1];
                acc[v][2] += mid[2];
            }
        }
        for i in 0..n {
            if count[i] > 0 {
                let f = 1.0 / count[i] as f32;
                acc[i] = [acc[i][0] * f, acc[i][1] * f, acc[i][2] * f];
            }
            let new = acc[i];
            let (f_new, f_orig) = match vg {
                Some((dv, index)) => {
                    let w = find_weight(&dv[i], index);
                    let f_vgroup = if s.invert_vertex_group { 1.0 - w } else { w };
                    if f_vgroup <= 0.0 {
                        continue;
                    }
                    let f_new = f_vgroup * fac_new;
                    (f_new, 1.0 - f_new)
                }
                None => (fac_new, fac_orig),
            };
            let co = &mut cos[i];
            if s.use_x {
                co[0] = f_orig * co[0] + f_new * new[0];
            }
            if s.use_y {
                co[1] = f_orig * co[1] + f_new * new[1];
            }
            if s.use_z {
                co[2] = f_orig * co[2] + f_new * new[2];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::common::testing::*;
    use super::*;

    #[test]
    fn disabled_without_factor_or_axes() {
        let s = SmoothSettings {
            factor: 0.0,
            ..SmoothSettings::default()
        };
        assert!(is_disabled(&ModifierSettings::Smooth(s)).is_some());
        let s = SmoothSettings {
            use_x: false,
            use_y: false,
            use_z: false,
            ..SmoothSettings::default()
        };
        assert!(is_disabled(&ModifierSettings::Smooth(s)).is_some());
        assert!(is_disabled(&ModifierSettings::Smooth(SmoothSettings::default())).is_none());
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/`, written by its
    /// `dump.py`): vertex-group weighted smoothing.
    #[test]
    fn vertex_group_matches_blender() {
        let d = load("vgroups/smooth.json");
        for case in d["cases"].as_array().unwrap() {
            let mesh = mesh_from_dump(&case["input"]);
            let p = &case["props"];
            let s = SmoothSettings {
                factor: p["factor"].as_f64().unwrap() as f32,
                iterations: p["iterations"].as_i64().unwrap() as i32,
                vertex_group: p["vertex_group"].as_str().unwrap().into(),
                invert_vertex_group: p["invert_vertex_group"].as_bool().unwrap(),
                use_x: p["use_x"].as_bool().unwrap(),
                use_y: p["use_y"].as_bool().unwrap(),
                use_z: p["use_z"].as_bool().unwrap(),
            };
            let out = deform(mesh, |m, cos| smooth_do(&s, m, cos));
            assert_positions(&out, &case["output"], 1e-5);
        }
    }
}
