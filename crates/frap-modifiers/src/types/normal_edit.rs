// SPDX-License-Identifier: GPL-3.0-or-later
//! NORMAL_EDIT (`NormalEditModifier`): sets custom normals pointing away from an ellipsoid
//! centre (`mode` RADIAL: the bounds' proportions, or `target`'s scale and position, moved by
//! `offset`) or towards `target` (`mode` DIRECTIONAL; `use_direction_parallel` makes them all
//! parallel), mixed with the current normals (`mix_mode` COPY/ADD/SUB/MUL, `mix_factor`,
//! `vertex_group`, at most `mix_limit` away), flipping faces whose new normals point the other
//! way unless `no_polynors_fix`. DIRECTIONAL without a target is disabled.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_normal_edit.cc (Blender 5.2.4), with
//! `bke::mesh_flip_faces` (blenkernel/intern/mesh_flip_faces.cc), `interp_v3_v3v3_slerp_safe`,
//! `angle_v3v3` and `ortho_v3_v3` (blenlib/intern/math_vector.cc), `interp_dot_slerp`
//! (blenlib/intern/math_rotation_c.cc). The custom-normal encoding is [`custom_normals`].

pub(crate) mod custom_normals;

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, DeformVert};
use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::generated::modifier_settings::NormalEditSettings;
use glam::Mat4;
use smallvec::SmallVec;

use self::custom_normals::{
    MeshView, calc_corners_with_spaces, clnors_for_write, normals_corner_custom_set,
    sharp_edges_for_write, sharp_faces, store, true_normals,
};
use super::smooth::common::{find_weight, get_vgroup};
use crate::prelude::*;

type V3 = [f32; 3];

fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::NormalEdit(n) = s
        && let Some(o) = n.target
    {
        v.push(o);
    }
    v
}

/// `!is_valid_target`.
fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::NormalEdit(n) if n.mode == "DIRECTIONAL" && n.target.is_none() => {
            Some("Invalid target settings")
        }
        _ => None,
    }
}

/// The target object as the modifier sees it.
pub(crate) struct Target {
    /// Its world matrix.
    world: Mat4,
    /// `Object.scale`.
    scale: V3,
}

#[modifier(
    NORMAL_EDIT,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    depends_on = deps,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::NormalEdit(s) = &md.settings else {
        return Ok(mesh);
    };
    // Blender refuses to assign the modified object itself, so a self reference counts as no
    // target.
    let own = cx.object_ref();
    let target = s
        .target
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|r| *r != own)
        .and_then(|r| cx.main().objects.get(r).map(|o| (r, o.scale)))
        .map(|(r, scale)| Target {
            world: cx.object_matrix_world(r),
            scale,
        });
    if s.mode == "DIRECTIONAL" && target.is_none() {
        return Err(ModError::Error("Invalid target settings".into()));
    }
    let ob_world = cx.object_matrix_world(own);
    Ok(normal_edit(s, ob_world, target.as_ref(), mesh))
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// `normalize_v3`; returns the length.
fn normalize(v: &mut V3) -> f32 {
    let d = dot(*v, *v);
    if d > 1.0e-35 {
        let l = d.sqrt();
        let f = 1.0 / l;
        *v = [v[0] * f, v[1] * f, v[2] * f];
        l
    } else {
        *v = [0.0; 3];
        0.0
    }
}

/// `angle_v3v3`.
fn angle_v3v3(a: V3, b: V3) -> f32 {
    let (mut v1, mut v2) = (a, b);
    normalize(&mut v1);
    normalize(&mut v2);
    let safe_asin = |x: f32| libm::asinf(x.clamp(-1.0, 1.0));
    let len = |p: V3, q: V3| {
        let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
        dot(d, d).sqrt()
    };
    if dot(v1, v2) >= 0.0 {
        2.0 * safe_asin(len(v1, v2) / 2.0)
    } else {
        std::f32::consts::PI - 2.0 * safe_asin(len(v1, [-v2[0], -v2[1], -v2[2]]) / 2.0)
    }
}

/// `interp_v3_v3v3_slerp`; `None` for direct opposites.
fn slerp(a: V3, b: V3, t: f32) -> Option<V3> {
    let cosom = dot(a, b);
    if cosom < -1.0 + f32::EPSILON {
        return None;
    }
    let w = if cosom.abs() < 1.0 - 1e-4 {
        let omega = libm::acosf(cosom);
        let sinom = libm::sinf(omega);
        [
            libm::sinf((1.0 - t) * omega) / sinom,
            libm::sinf(t * omega) / sinom,
        ]
    } else {
        [1.0 - t, t]
    };
    Some([
        w[0] * a[0] + w[1] * b[0],
        w[0] * a[1] + w[1] * b[1],
        w[0] * a[2] + w[1] * b[2],
    ])
}

/// `interp_v3_v3v3_slerp_safe`.
fn slerp_safe(a: V3, b: V3, t: f32) -> V3 {
    if let Some(r) = slerp(a, b, t) {
        return r;
    }
    // ortho_v3_v3 (axis_dominant_v3_single).
    let (xn, yn, zn) = (a[0].abs(), a[1].abs(), a[2].abs());
    let axis = if xn > yn {
        if xn > zn { 0 } else { 2 }
    } else if yn > zn {
        1
    } else {
        2
    };
    let mut o = match axis {
        0 => [-a[1] - a[2], a[0], a[0]],
        1 => [a[1], -a[0] - a[2], a[1]],
        _ => [a[2], a[2], -a[0] - a[1]],
    };
    normalize(&mut o);
    if t < 0.5 {
        slerp(a, o, t * 2.0).unwrap_or(a)
    } else {
        slerp(o, b, (t - 0.5) * 2.0).unwrap_or(b)
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_normal_edit.cc (mix_normals).
#[allow(clippy::too_many_arguments)]
fn mix_normals(
    s: &NormalEditSettings,
    vg: Option<(&[DeformVert], u32)>,
    corner_verts: &[i32],
    nos_old: &[V3],
    nos_new: &mut [V3],
) {
    let facs: Option<Vec<f32>> = vg.map(|(dv, index)| {
        corner_verts
            .iter()
            .map(|&v| {
                let w = find_weight(&dv[v as usize], index);
                if s.invert_vertex_group { 1.0 - w } else { w }
            })
            .collect()
    });
    for (i, (new, old)) in nos_new.iter_mut().zip(nos_old).enumerate() {
        let fac = facs.as_ref().map_or(s.mix_factor, |f| f[i] * s.mix_factor);
        match s.mix_mode.as_str() {
            "ADD" => {
                *new = [new[0] + old[0], new[1] + old[1], new[2] + old[2]];
                normalize(new);
            }
            "SUB" => {
                *new = [new[0] - old[0], new[1] - old[1], new[2] - old[2]];
                normalize(new);
            }
            "MUL" => {
                *new = [new[0] * old[0], new[1] * old[1], new[2] * old[2]];
                normalize(new);
            }
            _ => {}
        }
        let t = if s.mix_limit < std::f32::consts::PI {
            fac.min(s.mix_limit / angle_v3v3(*new, *old))
        } else {
            fac
        };
        *new = slerp_safe(*old, *new, t);
    }
}

/// `bke::mesh_flip_faces` on the selected faces (corner attributes reversed, string ones
/// excepted); `extra` columns (the in-flight custom normals) are flipped with them.
fn flip_faces(mesh: &mut Mesh, flip: &[bool], clnors: &mut [[i16; 2]]) {
    if !flip.iter().any(|f| *f) {
        return;
    }
    let offsets = mesh.face_offsets().to_vec();
    let mut cv = mesh.corner_verts().to_vec();
    let mut ce = mesh.corner_edges().to_vec();
    let ranges: Vec<std::ops::Range<usize>> = (0..offsets.len() - 1)
        .filter(|&f| flip[f])
        .map(|f| offsets[f] as usize..offsets[f + 1] as usize)
        .collect();
    for r in &ranges {
        let n = r.len();
        for j in 0..n / 2 {
            let a = r.start + j + 1;
            let b = r.start + n - 1 - j;
            cv.swap(a, b);
            ce.swap(a - 1, b);
        }
    }
    fn rev<T>(v: &mut [T], ranges: &[std::ops::Range<usize>]) {
        for r in ranges {
            v[r.start + 1..r.end].reverse();
        }
    }
    rev(clnors, &ranges);
    let attrs = mesh.attributes_mut();
    for a in attrs.iter_mut() {
        if a.domain != frap_mesh::Domain::Corner
            || a.name == ".corner_vert"
            || a.name == ".corner_edge"
            || a.name == frap_mesh::CUSTOM_NORMAL
        {
            continue;
        }
        match &mut a.data {
            AttrData::Float(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Int(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Int8(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Bool(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Float2(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Float3(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Float4(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::FloatColor(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::ByteColor(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Int16x2(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Int32x2(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Quaternion(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            AttrData::Float4x4(v) => rev(Arc::make_mut(v).as_mut_slice(), &ranges),
            _ => {}
        }
    }
    let edges = mesh.edges().to_vec();
    mesh.set_topology(offsets, cv, edges);
    mesh.attributes_mut().insert(Attribute {
        name: ".corner_edge".into(),
        domain: frap_mesh::Domain::Corner,
        data: AttrData::Int(Arc::new(ce)),
    });
}

// Ported from blender/source/blender/modifiers/intern/MOD_normal_edit.cc (normalEditModifier_do,
// normalEditModifier_do_radial, normalEditModifier_do_directional, generate_vert_coordinates,
// faces_check_flip).
/// The NORMAL_EDIT result of `mesh` for an object at `ob_world`.
pub(crate) fn normal_edit(
    s: &NormalEditSettings,
    ob_world: Mat4,
    target: Option<&Target>,
    mut mesh: Mesh,
) -> Mesh {
    let use_current_clnors = !(s.mix_mode == "COPY"
        && s.mix_factor == 1.0
        && s.vertex_group.is_empty()
        && s.mix_limit == std::f32::consts::PI);
    if mesh.corners_num() == 0 {
        return mesh;
    }
    let Some(mut sharp) = sharp_edges_for_write(&mesh) else {
        return mesh;
    };
    let Some(mut clnors) = clnors_for_write(&mesh) else {
        store(&mut mesh, Some(sharp), None);
        return mesh;
    };
    let (fnor, _, map) = true_normals(&mesh);
    let sf = sharp_faces(&mesh);
    let corner_normals: Option<Vec<V3>> = use_current_clnors.then(|| {
        let view = MeshView {
            positions: mesh.positions(),
            face_offsets: mesh.face_offsets(),
            corner_verts: mesh.corner_verts(),
            corner_edges: mesh.corner_edges(),
            vert_to_face: &map,
            face_normals: &fnor,
            sharp_faces: sf.as_deref(),
        };
        calc_corners_with_spaces(&view, Some(&sharp), Some(&clnors)).0
    });
    let vg = get_vgroup(&mesh, &s.vertex_group);
    let positions = mesh.positions().to_vec();
    let corner_verts = mesh.corner_verts().to_vec();
    let mut nos: Vec<V3> = vec![[0.0; 3]; corner_verts.len()];
    if s.mode == "RADIAL" {
        // generate_vert_coordinates with r_size.
        let mut cos = positions.clone();
        let mut size: V3 = match target {
            Some(t) => t.scale.map(f32::abs),
            None => {
                let mut min = [f32::MAX; 3];
                let mut max = [-f32::MAX; 3];
                for p in &cos {
                    for k in 0..3 {
                        min[k] = min[k].min(p[k]);
                        max[k] = max[k].max(p[k]);
                    }
                }
                [max[0] - min[0], max[1] - min[1], max[2] - min[2]]
            }
        };
        if size == [0.0; 3] {
            size = [1.0; 3];
        } else {
            for c in &mut size {
                *c = c.max(f32::EPSILON);
            }
        }
        let diff = match target {
            Some(t) => {
                let p = ob_world
                    .inverse()
                    .transform_point3(t.world.w_axis.truncate());
                Some([-p.x, -p.y, -p.z])
            }
            None if s.offset != [0.0; 3] => Some([-s.offset[0], -s.offset[1], -s.offset[2]]),
            None => None,
        };
        if let Some(d) = diff {
            for p in &mut cos {
                *p = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            }
        }
        let (a, b, c) = (size[0], size[1], size[2]);
        let m2 = (b * b) / (a * a);
        let n2 = (c * c) / (a * a);
        let mut done = vec![false; cos.len()];
        for (i, &v) in corner_verts.iter().enumerate() {
            let v = v as usize;
            if !done[v] {
                let co = cos[v];
                let x2 = co[0] * co[0];
                let y2 = co[1] * co[1];
                let z2 = co[2] * co[2];
                let a2 = x2 + (y2 / m2) + (z2 / n2);
                let b2 = (m2 * x2) + y2 + (m2 * z2 / n2);
                let c2 = (n2 * x2) + (n2 * y2 / m2) + z2;
                let mut n = [co[0] / a2, co[1] / b2, co[2] / c2];
                normalize(&mut n);
                cos[v] = n;
                done[v] = true;
            }
            nos[i] = cos[v];
        }
    } else {
        let t = target.expect("DIRECTIONAL has a target");
        let mat = ob_world.inverse() * t.world;
        let target_co: V3 = mat.w_axis.truncate().to_array();
        if s.use_direction_parallel {
            let mut no = [
                target_co[0] - s.offset[0],
                target_co[1] - s.offset[1],
                target_co[2] - s.offset[2],
            ];
            normalize(&mut no);
            nos.fill(no);
        } else {
            // generate_vert_coordinates without r_size: centred on the target.
            let p = ob_world
                .inverse()
                .transform_point3(t.world.w_axis.truncate());
            let mut cos: Vec<V3> = positions
                .iter()
                .map(|q| [q[0] - p.x, q[1] - p.y, q[2] - p.z])
                .collect();
            let mut done = vec![false; cos.len()];
            for (i, &v) in corner_verts.iter().enumerate() {
                let v = v as usize;
                if !done[v] {
                    let co = cos[v];
                    let mut n = [
                        target_co[0] - co[0],
                        target_co[1] - co[1],
                        target_co[2] - co[2],
                    ];
                    normalize(&mut n);
                    cos[v] = n;
                    done[v] = true;
                }
                nos[i] = cos[v];
            }
        }
    }
    if let Some(old) = &corner_normals {
        mix_normals(s, vg, &corner_verts, old, &mut nos);
    }
    if !s.no_polynors_fix {
        // faces_check_flip.
        let offsets = mesh.face_offsets().to_vec();
        let mut flip = vec![false; offsets.len() - 1];
        for f in 0..flip.len() {
            let r = offsets[f] as usize..offsets[f + 1] as usize;
            let mut sum = [0.0f32; 3];
            for c in r.clone() {
                sum = [sum[0] + nos[c][0], sum[1] + nos[c][1], sum[2] + nos[c][2]];
            }
            if normalize(&mut sum) == 0.0 {
                continue;
            }
            if dot(fnor[f], sum) < 0.0 {
                nos[r.start + 1..r.end].reverse();
                flip[f] = true;
            }
        }
        flip_faces(&mut mesh, &flip, &mut clnors);
    }
    let (fnor, vnor, map) = true_normals(&mesh);
    let view = MeshView {
        positions: mesh.positions(),
        face_offsets: mesh.face_offsets(),
        corner_verts: mesh.corner_verts(),
        corner_edges: mesh.corner_edges(),
        vert_to_face: &map,
        face_normals: &fnor,
        sharp_faces: sf.as_deref(),
    };
    normals_corner_custom_set(&view, &vnor, false, &mut nos, &mut sharp, &mut clnors);
    store(&mut mesh, Some(sharp), Some(clnors));
    mesh
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    fn settings(p: &serde_json::Value) -> NormalEditSettings {
        let o = p["offset"].as_array().unwrap();
        NormalEditSettings {
            mode: p["mode"].as_str().unwrap().into(),
            offset: [
                o[0].as_f64().unwrap() as f32,
                o[1].as_f64().unwrap() as f32,
                o[2].as_f64().unwrap() as f32,
            ],
            mix_mode: p["mix_mode"].as_str().unwrap().into(),
            mix_factor: p["mix_factor"].as_f64().unwrap() as f32,
            mix_limit: p["mix_limit"].as_f64().unwrap() as f32,
            no_polynors_fix: p["no_polynors_fix"].as_bool().unwrap(),
            vertex_group: p["vertex_group"].as_str().unwrap().into(),
            invert_vertex_group: p["invert_vertex_group"].as_bool().unwrap(),
            target: None,
            use_direction_parallel: false,
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/normal_edit.json`).
    #[test]
    fn matches_blender() {
        let d = load("vgroups/normal_edit.json");
        for case in d["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            eprintln!("normal_edit case {name}");
            let mesh = mesh_from_dump(&case["input"]);
            let s = settings(&case["props"]);
            let out = normal_edit(&s, Mat4::IDENTITY, None, mesh);
            let want = &case["output"];
            assert_topology(&out, want);
            assert_attribute(&out, want, "sharp_edge", 0.0);
            assert_attribute(&out, want, "custom_normal", 0.0);
            assert_corner_normals(&out, want, 2e-4);
        }
    }

    #[test]
    fn slerp_handles_opposites() {
        let r = slerp_safe([0.0, 0.0, 1.0], [0.0, 0.0, -1.0], 0.5);
        assert!((dot(r, r) - 1.0).abs() < 1e-5);
        assert!(r[2].abs() < 1e-5);
        assert!((angle_v3v3([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }
}
