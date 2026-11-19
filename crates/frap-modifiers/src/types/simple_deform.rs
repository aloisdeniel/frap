// SPDX-License-Identifier: GPL-3.0-or-later
//! SIMPLE_DEFORM (`SimpleDeformModifier`): TWIST, BEND, TAPER or STRETCH (`deform_method`)
//! about `deform_axis`, in the object's space or `origin`'s, between `limits` (fractions of the
//! bounds along the deform axis), with `lock_x/y/z` (not BEND) and `vertex_group` weighting.
//!
//! `factor` and `angle` are one DNA field in Blender (`SimpleDeformModifierData.factor`): the
//! UI shows `angle` for TWIST/BEND and `factor` for TAPER/STRETCH. frap stores the two RNA
//! properties separately, so the evaluator reads the one that was changed from the shared
//! default (45°), preferring the method's own property when both were (see
//! [`deform_factor`]).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_simpledeform.cc (Blender 5.2.4) and
//! `BLI_space_transform_from_matrices` (blenlib/intern/math_matrix.cc).

use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::generated::modifier_settings::SimpleDeformSettings;
use glam::{Mat4, Vec3};
use smallvec::SmallVec;

use super::smooth::common::{deform, vgroup_raw, weight_safe};
use crate::prelude::*;

const BEND_EPS: f32 = 0.000001;
/// `DEG2RADF(45.0f)`: the DNA default of `factor` (= `angle`).
const DEFAULT_FACTOR: f32 = std::f32::consts::FRAC_PI_4;

fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::SimpleDeform(d) = s
        && let Some(o) = d.origin
    {
        v.push(o);
    }
    v
}

#[modifier(
    SIMPLE_DEFORM,
    kind = OnlyDeform,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | ACCEPTS_VERTEX_COS_ONLY | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    depends_on = deps,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::SimpleDeform(s) = &md.settings else {
        return Ok(mesh);
    };
    let own = cx.object_ref();
    let transf = s
        .origin
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|o| *o != own && cx.main().objects.get(*o).is_some())
        .map(|o| {
            let local = cx.object_matrix_world(own);
            let target = cx.object_matrix_world(o);
            SpaceTransform::from_matrices(local, target)
        });
    Ok(deform(mesh, |m, cos| {
        simple_deform_do(s, transf.as_ref(), m, cos)
    }))
}

/// `SpaceTransform` (`BLI_space_transform_from_matrices`).
pub(crate) struct SpaceTransform {
    local2target: Mat4,
    target2local: Mat4,
}

impl SpaceTransform {
    fn from_matrices(local: Mat4, target: Mat4) -> SpaceTransform {
        let local2target = target.inverse() * local;
        SpaceTransform {
            local2target,
            target2local: local2target.inverse(),
        }
    }
    fn apply(&self, co: [f32; 3]) -> [f32; 3] {
        self.local2target
            .transform_point3(Vec3::from(co))
            .to_array()
    }
    fn invert(&self, co: [f32; 3]) -> [f32; 3] {
        self.target2local
            .transform_point3(Vec3::from(co))
            .to_array()
    }
}

/// The DNA `factor` the settings stand for: `angle` for TWIST/BEND and `factor` for
/// TAPER/STRETCH, unless only the other property was changed from the shared default.
pub(crate) fn deform_factor(s: &SimpleDeformSettings) -> f32 {
    let angular = matches!(s.deform_method.as_str(), "TWIST" | "BEND");
    let (own, other) = if angular {
        (s.angle, s.factor)
    } else {
        (s.factor, s.angle)
    };
    if own == DEFAULT_FACTOR && other != DEFAULT_FACTOR {
        other
    } else {
        own
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Mode {
    Twist,
    Bend,
    Taper,
    Stretch,
}

/// `axis_map_table`.
const AXIS_MAP: [[usize; 3]; 3] = [[1, 2, 0], [2, 0, 1], [0, 1, 2]];

fn axis_limit(axis: usize, limits: [f32; 2], co: &mut [f32; 3], dcut: &mut [f32; 3]) {
    let mut val = co[axis];
    val = limits[0].max(val);
    val = limits[1].min(val);
    dcut[axis] = co[axis] - val;
    co[axis] = val;
}

fn taper(factor: f32, dcut: [f32; 3], r: &mut [f32; 3]) {
    let (x, y, z) = (r[0], r[1], r[2]);
    let scale = z * factor;
    *r = [x + x * scale, y + y * scale, z];
    add(r, dcut);
}

fn stretch(factor: f32, dcut: [f32; 3], r: &mut [f32; 3]) {
    let (x, y, z) = (r[0], r[1], r[2]);
    let scale = z * z * factor - factor + 1.0;
    *r = [x * scale, y * scale, z * (1.0 + factor)];
    add(r, dcut);
}

fn twist(factor: f32, dcut: [f32; 3], r: &mut [f32; 3]) {
    let (x, y, z) = (r[0], r[1], r[2]);
    let theta = z * factor;
    let (sint, cost) = (libm::sinf(theta), libm::cosf(theta));
    *r = [x * cost - y * sint, x * sint + y * cost, z];
    add(r, dcut);
}

fn bend(factor: f32, axis: usize, dcut: [f32; 3], r: &mut [f32; 3]) {
    let (x, y, z) = (r[0], r[1], r[2]);
    let theta = if axis <= 1 { z * factor } else { x * factor };
    let (sint, cost) = (libm::sinf(theta), libm::cosf(theta));
    // NOTE: the order of operations is Blender's (#85470).
    match axis {
        0 => {
            *r = [
                x,
                y * cost + (1.0 - cost) / factor,
                -(y - 1.0 / factor) * sint,
            ];
            r[0] += dcut[0];
            r[1] += sint * dcut[2];
            r[2] += cost * dcut[2];
        }
        1 => {
            *r = [
                x * cost + (1.0 - cost) / factor,
                y,
                -(x - 1.0 / factor) * sint,
            ];
            r[0] += sint * dcut[2];
            r[1] += dcut[1];
            r[2] += cost * dcut[2];
        }
        _ => {
            *r = [
                -(y - 1.0 / factor) * sint,
                y * cost + (1.0 - cost) / factor,
                z,
            ];
            r[0] += cost * dcut[0];
            r[1] += sint * dcut[0];
            r[2] += dcut[2];
        }
    }
}

fn add(r: &mut [f32; 3], d: [f32; 3]) {
    r[0] += d[0];
    r[1] += d[1];
    r[2] += d[2];
}

// Ported from blender/source/blender/modifiers/intern/MOD_simpledeform.cc
// (SimpleDeformModifier_do, simple_helper).
/// `SimpleDeformModifier_do` on `cos`.
pub(crate) fn simple_deform_do(
    s: &SimpleDeformSettings,
    transf: Option<&SpaceTransform>,
    mesh: &Mesh,
    cos: &mut [[f32; 3]],
) {
    let mode = match s.deform_method.as_str() {
        "BEND" => Mode::Bend,
        "TAPER" => Mode::Taper,
        "STRETCH" => Mode::Stretch,
        _ => Mode::Twist,
    };
    let deform_axis = match s.deform_axis.as_str() {
        "X" => 0,
        "Y" => 1,
        _ => 2,
    };
    // Historically the lock axis, not the deform axis.
    let mut lock = [s.lock_x, s.lock_y, s.lock_z];
    if mode == Mode::Bend {
        lock = [false; 3];
    } else {
        lock[deform_axis] = false;
    }
    let mut limit0 = s.limits[0].clamp(0.0, 1.0);
    limit0 = limit0.min(s.limits[1]);
    let limit1 = s.limits[1];
    let limit_axis = if mode == Mode::Bend {
        if deform_axis <= 1 { 2 } else { 0 }
    } else {
        deform_axis
    };
    let mut lower = f32::MAX;
    let mut upper = -f32::MAX;
    for p in cos.iter() {
        let tmp = match transf {
            Some(t) => t.apply(*p),
            None => *p,
        };
        lower = lower.min(tmp[limit_axis]);
        upper = upper.max(tmp[limit_axis]);
    }
    let smd_limit = [
        lower + (upper - lower) * limit0,
        lower + (upper - lower) * limit1,
    ];
    let smd_factor = deform_factor(s) / f32::EPSILON.max(smd_limit[1] - smd_limit[0]);
    if mode == Mode::Bend && smd_factor.abs() < BEND_EPS {
        return;
    }
    let vg = vgroup_raw(mesh, &s.vertex_group);
    let axis_map = AXIS_MAP[if mode != Mode::Bend { deform_axis } else { 2 }];
    for (i, p) in cos.iter_mut().enumerate() {
        let weight = weight_safe(vg, i, s.invert_vertex_group);
        if weight == 0.0 {
            continue;
        }
        if let Some(t) = transf {
            *p = t.apply(*p);
        }
        let mut co = *p;
        let mut dcut = [0.0f32; 3];
        for (axis, l) in lock.iter().enumerate() {
            if *l {
                axis_limit(axis, [0.0, 0.0], &mut co, &mut dcut);
            }
        }
        axis_limit(limit_axis, smd_limit, &mut co, &mut dcut);
        let mut co_remap = [co[axis_map[0]], co[axis_map[1]], co[axis_map[2]]];
        let dcut_remap = [dcut[axis_map[0]], dcut[axis_map[1]], dcut[axis_map[2]]];
        match mode {
            Mode::Twist => twist(smd_factor, dcut_remap, &mut co_remap),
            Mode::Bend => bend(smd_factor, deform_axis, dcut_remap, &mut co_remap),
            Mode::Taper => taper(smd_factor, dcut_remap, &mut co_remap),
            Mode::Stretch => stretch(smd_factor, dcut_remap, &mut co_remap),
        }
        for k in 0..3 {
            co[axis_map[k]] = co_remap[k];
        }
        // interp_v3_v3v3(vco, vco, co, weight).
        let sw = 1.0 - weight;
        *p = [
            sw * p[0] + weight * co[0],
            sw * p[1] + weight * co[1],
            sw * p[2] + weight * co[2],
        ];
        if let Some(t) = transf {
            *p = t.invert(*p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    #[test]
    fn factor_and_angle_alias() {
        let s = SimpleDeformSettings::default();
        assert_eq!(deform_factor(&s), DEFAULT_FACTOR);
        let s = SimpleDeformSettings {
            factor: 0.5,
            ..SimpleDeformSettings::default()
        };
        // TWIST shows `angle`, but only `factor` was changed: the DNA field is 0.5.
        assert_eq!(deform_factor(&s), 0.5);
        let s = SimpleDeformSettings {
            angle: 1.0,
            factor: 0.5,
            ..SimpleDeformSettings::default()
        };
        assert_eq!(deform_factor(&s), 1.0);
        let s = SimpleDeformSettings {
            deform_method: "TAPER".into(),
            angle: 1.0,
            ..SimpleDeformSettings::default()
        };
        assert_eq!(deform_factor(&s), 1.0);
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/simple_deform.json`).
    #[test]
    fn vertex_group_matches_blender() {
        let d = load("vgroups/simple_deform.json");
        for case in d["cases"].as_array().unwrap() {
            let mesh = mesh_from_dump(&case["input"]);
            let p = &case["props"];
            let lim = p["limits"].as_array().unwrap();
            let s = SimpleDeformSettings {
                deform_method: p["deform_method"].as_str().unwrap().into(),
                deform_axis: p["deform_axis"].as_str().unwrap().into(),
                vertex_group: p["vertex_group"].as_str().unwrap().into(),
                invert_vertex_group: p["invert_vertex_group"].as_bool().unwrap(),
                factor: p["factor"].as_f64().unwrap() as f32,
                angle: p["factor"].as_f64().unwrap() as f32,
                limits: [
                    lim[0].as_f64().unwrap() as f32,
                    lim[1].as_f64().unwrap() as f32,
                ],
                lock_x: p["lock_x"].as_bool().unwrap(),
                lock_y: p["lock_y"].as_bool().unwrap(),
                lock_z: p["lock_z"].as_bool().unwrap(),
                origin: None,
            };
            let out = deform(mesh, |m, cos| simple_deform_do(&s, None, m, cos));
            assert_positions(&out, &case["output"], 1e-5);
        }
    }
}
