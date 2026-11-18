// SPDX-License-Identifier: GPL-3.0-or-later
//! CAST (`CastModifier`): moves vertices towards a sphere, a cylinder (no Z) or a cuboid
//! (`cast_type`) by `factor` per enabled axis. The shape is centred on the object's origin, or
//! on `object` (`use_transform` also takes its rotation and scale); `radius` limits the
//! affected vertices; the shape's size is `size` (`use_radius_as_size`: `radius`), or the
//! average distance to the centre (sphere/cylinder) or the symmetric bounds (cuboid) when 0;
//! `vertex_group` scales the factor. Disabled when `factor` is 0 or no axis is enabled.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_cast.cc (Blender 5.2.4).

use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::generated::modifier_settings::CastSettings;
use glam::{Mat4, Vec3};
use smallvec::SmallVec;

use super::smooth::common::{deform, find_weight, get_vgroup};
use crate::prelude::*;

fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Cast(c) = s
        && let Some(o) = c.object
    {
        v.push(o);
    }
    v
}

fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Cast(c) if c.factor == 0.0 || !(c.use_x || c.use_y || c.use_z) => {
            Some("Cast: factor is 0 or no axis is enabled")
        }
        _ => None,
    }
}

#[modifier(
    CAST,
    kind = OnlyDeform,
    flags = ACCEPTS_CVS | ACCEPTS_VERTEX_COS_ONLY | SUPPORTS_EDITMODE,
    depends_on = deps,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Cast(s) = &md.settings else {
        return Ok(mesh);
    };
    let ctrl = control(s.object, cx);
    Ok(deform(mesh, |m, cos| {
        if s.cast_type == "CUBOID" {
            cuboid_do(s, ctrl.as_ref(), m, cos);
        } else {
            sphere_do(s, ctrl.as_ref(), m, cos);
        }
    }))
}

/// The control object's frame in the object's space: `mat` (object → control, with
/// `use_transform`), its inverse, and the control's centre in object space.
pub(crate) struct Control {
    mat: Mat4,
    imat: Mat4,
    center: Vec3,
}

fn control(object: Option<IdAny>, cx: &mut dyn ModEvalCx) -> Option<Control> {
    let ob = object
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|o| cx.main().objects.get(*o).is_some())?;
    let ob_world = cx.object_matrix_world(cx.object_ref());
    let ctrl_world = cx.object_matrix_world(ob);
    // invert_m4_m4(imat, ctrl->object_to_world); mul_m4_m4m4(mat, imat, ob->object_to_world).
    let mat = ctrl_world.inverse() * ob_world;
    let imat = mat.inverse();
    let center = ob_world
        .inverse()
        .transform_point3(ctrl_world.w_axis.truncate());
    Some(Control { mat, imat, center })
}

fn vgroup_fac(
    vg: Option<(&[frap_mesh::DeformVert], u32)>,
    i: usize,
    invert: bool,
    fac_orig: f32,
) -> Option<(f32, f32)> {
    let (dv, index) = vg?;
    let w = find_weight(&dv[i], index);
    let weight = if invert { 1.0 - w } else { w };
    let fac = fac_orig * weight;
    Some((fac, 1.0 - fac))
}

fn len_v3v3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// `normalize_v3` (zero vectors stay zero).
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let d = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if d > 1.0e-35 {
        let l = 1.0 / d.sqrt();
        [v[0] * l, v[1] * l, v[2] * l]
    } else {
        [0.0; 3]
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_cast.cc (sphere_do).
fn sphere_do(s: &CastSettings, ctrl: Option<&Control>, mesh: &Mesh, cos: &mut [[f32; 3]]) {
    let cylinder = s.cast_type == "CYLINDER";
    let (use_x, use_y, use_z) = (s.use_x, s.use_y, s.use_z && !cylinder);
    let fac_orig = s.factor;
    let (mut fac, mut facm) = (fac_orig, 1.0 - fac_orig);
    let center = ctrl.map_or([0.0; 3], |c| c.center.to_array());
    let has_radius = s.radius > f32::EPSILON;
    let vg = if s.vertex_group.is_empty() {
        None
    } else {
        get_vgroup(mesh, &s.vertex_group)
    };
    let mut len = if s.use_radius_as_size {
        s.radius
    } else {
        s.size
    };
    if len <= 0.0 {
        for p in cos.iter() {
            len += len_v3v3(center, *p);
        }
        len /= cos.len() as f32;
        if len == 0.0 {
            len = 10.0;
        }
    }
    for (i, p) in cos.iter_mut().enumerate() {
        let mut tmp = *p;
        if let Some(c) = ctrl {
            if s.use_transform {
                tmp = c.mat.transform_point3(Vec3::from(tmp)).to_array();
            } else {
                tmp = [tmp[0] - center[0], tmp[1] - center[1], tmp[2] - center[2]];
            }
        }
        let mut vec = tmp;
        if cylinder {
            vec[2] = 0.0;
        }
        if has_radius && (vec[0] * vec[0] + vec[1] * vec[1] + vec[2] * vec[2]).sqrt() > s.radius {
            continue;
        }
        if let Some((f, fm)) = vgroup_fac(vg, i, s.invert_vertex_group, fac_orig) {
            if f == 0.0 {
                continue;
            }
            (fac, facm) = (f, fm);
        }
        let vec = normalize(vec);
        if use_x {
            tmp[0] = fac * vec[0] * len + facm * tmp[0];
        }
        if use_y {
            tmp[1] = fac * vec[1] * len + facm * tmp[1];
        }
        if use_z {
            tmp[2] = fac * vec[2] * len + facm * tmp[2];
        }
        if let Some(c) = ctrl {
            if s.use_transform {
                tmp = c.imat.transform_point3(Vec3::from(tmp)).to_array();
            } else {
                tmp = [tmp[0] + center[0], tmp[1] + center[1], tmp[2] + center[2]];
            }
        }
        *p = tmp;
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_cast.cc (cuboid_do).
fn cuboid_do(s: &CastSettings, ctrl: Option<&Control>, mesh: &Mesh, cos: &mut [[f32; 3]]) {
    let fac_orig = s.factor;
    let (mut fac, mut facm) = (fac_orig, 1.0 - fac_orig);
    let center = ctrl.map_or([0.0; 3], |c| c.center.to_array());
    let has_radius = s.radius > f32::EPSILON;
    let vg = if s.vertex_group.is_empty() {
        None
    } else {
        get_vgroup(mesh, &s.vertex_group)
    };
    let (mut min, mut max);
    if s.use_radius_as_size && has_radius {
        min = [-s.radius; 3];
        max = [s.radius; 3];
    } else if !s.use_radius_as_size && s.size > 0.0 {
        min = [-s.size; 3];
        max = [s.size; 3];
    } else {
        min = [f32::MAX; 3];
        max = [-f32::MAX; 3];
        let mut minmax = |v: [f32; 3]| {
            for k in 0..3 {
                if v[k] < min[k] {
                    min[k] = v[k];
                }
                if v[k] > max[k] {
                    max[k] = v[k];
                }
            }
        };
        if ctrl.is_some() {
            minmax(center);
            for p in cos.iter() {
                minmax([p[0] - center[0], p[1] - center[1], p[2] - center[2]]);
            }
        } else {
            for p in cos.iter() {
                minmax(*p);
            }
        }
        for k in 0..3 {
            if min[k].abs() > max[k].abs() {
                max[k] = min[k].abs();
            }
            min[k] = -max[k];
        }
    }
    let bb = |octant: usize| -> [f32; 3] {
        [
            if octant & 1 != 0 { max[0] } else { min[0] },
            if octant & 2 != 0 { max[1] } else { min[1] },
            if octant & 4 != 0 { max[2] } else { min[2] },
        ]
    };
    for (i, p) in cos.iter_mut().enumerate() {
        let mut tmp = *p;
        if let Some(c) = ctrl {
            if s.use_transform {
                tmp = c.mat.transform_point3(Vec3::from(tmp)).to_array();
            } else {
                tmp = [tmp[0] - center[0], tmp[1] - center[1], tmp[2] - center[2]];
            }
        }
        if has_radius
            && (tmp[0].abs() > s.radius || tmp[1].abs() > s.radius || tmp[2].abs() > s.radius)
        {
            continue;
        }
        if let Some((f, fm)) = vgroup_fac(vg, i, s.invert_vertex_group, fac_orig) {
            if f == 0.0 {
                continue;
            }
            (fac, facm) = (f, fm);
        }
        let mut octant = 0;
        if tmp[0] > 0.0 {
            octant += 1;
        }
        if tmp[1] > 0.0 {
            octant += 2;
        }
        if tmp[2] > 0.0 {
            octant += 4;
        }
        let apex = bb(octant);
        let d = [tmp[0] / apex[0], tmp[1] / apex[1], tmp[2] / apex[2]];
        let mut dmax = d[0];
        let mut coord = 0;
        if d[1] > dmax {
            dmax = d[1];
            coord = 1;
        }
        if d[2] > dmax {
            coord = 2;
        }
        if tmp[coord].abs() < f32::EPSILON {
            continue;
        }
        let fbb = apex[coord] / tmp[coord];
        if s.use_x {
            tmp[0] = facm * tmp[0] + fac * tmp[0] * fbb;
        }
        if s.use_y {
            tmp[1] = facm * tmp[1] + fac * tmp[1] * fbb;
        }
        if s.use_z {
            tmp[2] = facm * tmp[2] + fac * tmp[2] * fbb;
        }
        if let Some(c) = ctrl {
            if s.use_transform {
                tmp = c.imat.transform_point3(Vec3::from(tmp)).to_array();
            } else {
                tmp = [tmp[0] + center[0], tmp[1] + center[1], tmp[2] + center[2]];
            }
        }
        *p = tmp;
    }
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    fn settings(p: &serde_json::Value) -> CastSettings {
        let b = |k: &str| p[k].as_bool().unwrap();
        let f = |k: &str| p[k].as_f64().unwrap() as f32;
        CastSettings {
            cast_type: p["cast_type"].as_str().unwrap().into(),
            object: None,
            invert_vertex_group: b("invert_vertex_group"),
            use_x: b("use_x"),
            use_y: b("use_y"),
            use_z: b("use_z"),
            use_radius_as_size: b("use_radius_as_size"),
            use_transform: false,
            factor: f("factor"),
            radius: f("radius"),
            size: f("size"),
            vertex_group: p["vertex_group"].as_str().unwrap().into(),
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/cast.json`).
    #[test]
    fn vertex_group_matches_blender() {
        let d = load("vgroups/cast.json");
        for case in d["cases"].as_array().unwrap() {
            let mesh = mesh_from_dump(&case["input"]);
            let s = settings(&case["props"]);
            let out = deform(mesh, |m, cos| {
                if s.cast_type == "CUBOID" {
                    cuboid_do(&s, None, m, cos);
                } else {
                    sphere_do(&s, None, m, cos);
                }
            });
            assert_positions(&out, &case["output"], 1e-5);
        }
    }

    #[test]
    fn control_object_moves_the_centre() {
        let mut cx = TestCx::new();
        let ctrl = cx.add_object("Empty", Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)));
        let c = control(Some(ctrl.any()), &mut cx).unwrap();
        assert_eq!(c.center, Vec3::new(2.0, 0.0, 0.0));
        assert!(deps(&ModifierSettings::Cast(CastSettings {
            object: Some(ctrl.any()),
            ..CastSettings::default()
        }))
        .contains(&ctrl.any()));
    }
}
