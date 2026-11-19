// SPDX-License-Identifier: GPL-3.0-or-later
//! WEIGHTED_NORMAL (`WeightedNormalModifier`): sets custom normals weighted by face area
//! (`mode` FACE_AREA), corner angle (CORNER_ANGLE) or both (FACE_AREA_WITH_ANGLE): larger
//! values dominate according to `weight` (50 = equal), values within `thresh` count as equal;
//! `keep_sharp` keeps one normal per smooth fan, `use_face_influence` lets the stronger
//! `__mod_weightednormals_faceweight` faces win, and `vertex_group` limits the vertices.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_weighted_normal.cc (Blender 5.2.4)
//! with `face_area_calc` / `face_angles_calc` (blenkernel/intern/mesh_evaluate.cc). The
//! custom-normal encoding is `normal_edit::custom_normals`. Blender sorts the weights with the
//! unstable `qsort`; frap sorts stably, which only changes the summation order of equal
//! weights.

use frap_geom::normals::V3;
use frap_mesh::{AttrData, Attribute, Domain};
use frap_scene::generated::modifier_settings::WeightedNormalSettings;

use super::normal_edit::custom_normals::{
    MeshView, calc_corners_with_spaces, clnors_for_write, normals_corner_custom_set,
    sharp_edges_for_write, sharp_faces, store, true_normals,
};
use super::smooth::common::{get_vgroup, has_group};
use crate::prelude::*;

/// `CLNORS_VALID_VEC_LEN`.
const CLNORS_VALID_VEC_LEN: f32 = 1e-6;
/// `FACE_STRENGTH_WEAK`.
const FACE_STRENGTH_WEAK: i32 = -16384;
/// `MOD_WEIGHTEDNORMALS_FACEWEIGHT_CDLAYER_ID`.
pub(crate) const FACEWEIGHT_LAYER: &str = "__mod_weightednormals_faceweight";

#[modifier(
    WEIGHTED_NORMAL,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::WeightedNormal(s) = &md.settings else {
        return Ok(mesh);
    };
    Ok(weighted_normal(s, mesh))
}

/// `WeightedNormalDataAggregateItem`.
#[derive(Copy, Clone)]
struct Item {
    normal: V3,
    loops_num: i32,
    curr_val: f32,
    curr_strength: i32,
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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

/// `bke::mesh::face_area_calc`.
fn face_area(positions: &[V3], verts: &[i32]) -> f32 {
    let p = |i: usize| positions[verts[i] as usize];
    if verts.len() == 3 {
        let n1 = sub(p(0), p(1));
        let n2 = sub(p(1), p(2));
        let n = [
            n1[1] * n2[2] - n1[2] * n2[1],
            n1[2] * n2[0] - n1[0] * n2[2],
            n1[0] * n2[1] - n1[1] * n2[0],
        ];
        return dot(n, n).sqrt() * 0.5;
    }
    let n = frap_geom::normals::newell_cross(positions, verts);
    dot(n, n).sqrt() * 0.5
}

/// `angle_normalized_v3v3`.
fn angle_normalized(v1: V3, v2: V3) -> f32 {
    let safe_asin = |x: f32| libm::asinf(x.clamp(-1.0, 1.0));
    let len = |a: V3| dot(a, a).sqrt();
    if dot(v1, v2) >= 0.0 {
        2.0 * safe_asin(len(sub(v1, v2)) / 2.0)
    } else {
        std::f32::consts::PI
            - 2.0 * safe_asin(len([v1[0] + v2[0], v1[1] + v2[1], v1[2] + v2[2]]) / 2.0)
    }
}

/// `bke::mesh::face_angles_calc`.
fn face_angles(positions: &[V3], verts: &[i32]) -> Vec<f32> {
    let n = verts.len();
    let p = |i: usize| positions[verts[i] as usize];
    let mut angles = vec![0.0f32; n];
    let mut i_this = n - 1;
    let mut nor_prev = sub(p(i_this - 1), p(i_this));
    normalize(&mut nor_prev);
    for i_next in 0..n {
        let mut nor_next = sub(p(i_this), p(i_next));
        normalize(&mut nor_next);
        angles[i_this] = angle_normalized(nor_prev, nor_next);
        nor_prev = nor_next;
        i_this = i_next;
    }
    angles
}

// Ported from blender/source/blender/modifiers/intern/MOD_weighted_normal.cc (modify_mesh,
// wn_face_area, wn_corner_angle, wn_face_with_angle, apply_weights_vertex_normal,
// aggregate_item_normal, check_item_face_strength).
/// The WEIGHTED_NORMAL result of `mesh`.
pub(crate) fn weighted_normal(s: &WeightedNormalSettings, mut mesh: Mesh) -> Mesh {
    let mut weight = s.weight as f32 / 50.0;
    if s.weight == 100 {
        weight = i16::MAX as f32;
    } else if s.weight == 1 {
        weight = 1.0 / i16::MAX as f32;
    } else if (weight - 1.0) * 25.0 > 1.0 {
        weight = (weight - 1.0) * 25.0;
    }
    let vg = get_vgroup(&mesh, &s.vertex_group);
    let Some(mut sharp) = sharp_edges_for_write(&mesh) else {
        return mesh;
    };
    let Some(mut clnors) = clnors_for_write(&mesh) else {
        store(&mut mesh, Some(sharp), None);
        return mesh;
    };
    let face_strength: Option<Vec<i32>> = match mesh.attributes().get(FACEWEIGHT_LAYER) {
        Some(Attribute {
            domain: Domain::Face,
            data: AttrData::Int(v),
            ..
        }) => Some(v.to_vec()),
        _ => None,
    };
    let use_face_influence = s.use_face_influence && face_strength.is_some();
    let (fnor, vnor, map) = true_normals(&mesh);
    let sf = sharp_faces(&mesh);
    let positions = mesh.positions();
    let offsets = mesh.face_offsets();
    let cv = mesh.corner_verts();
    let view = MeshView {
        positions,
        face_offsets: offsets,
        corner_verts: cv,
        corner_edges: mesh.corner_edges(),
        vert_to_face: &map,
        face_normals: &fnor,
        sharp_faces: sf.as_deref(),
    };
    let nfaces = offsets.len() - 1;
    let face_range = |f: usize| offsets[f] as usize..offsets[f + 1] as usize;
    let mut corner_to_face = vec![0usize; cv.len()];
    for f in 0..nfaces {
        for c in face_range(f) {
            corner_to_face[c] = f;
        }
    }

    // The (value, index) pairs, sorted by decreasing value.
    let mut mode_pair: Vec<(f32, usize)> = match s.mode.as_str() {
        "FACE_AREA" => (0..nfaces)
            .map(|f| (face_area(positions, &cv[face_range(f)]), f))
            .collect(),
        mode => {
            let mut v = Vec::with_capacity(cv.len());
            for f in 0..nfaces {
                let r = face_range(f);
                let area = if mode == "FACE_AREA_WITH_ANGLE" {
                    face_area(positions, &cv[r.clone()])
                } else {
                    1.0
                };
                let angles = face_angles(positions, &cv[r.clone()]);
                for (k, c) in r.enumerate() {
                    let a = std::f32::consts::PI - angles[k];
                    v.push((
                        if mode == "FACE_AREA_WITH_ANGLE" {
                            a * area
                        } else {
                            a
                        },
                        c,
                    ));
                }
            }
            v
        }
    };
    mode_pair.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let keep_sharp = s.keep_sharp;
    let start_item = Item {
        normal: [0.0; 3],
        loops_num: 0,
        curr_val: 0.0,
        curr_strength: FACE_STRENGTH_WEAK,
    };
    let mut corner_normals: Vec<V3> = Vec::new();
    let mut corner_space: Vec<usize> = Vec::new();
    let mut items: Vec<Item> = if keep_sharp {
        let (cn, spaces) = calc_corners_with_spaces(&view, Some(&sharp), Some(&clnors));
        corner_normals = cn;
        corner_space = spaces.corner_space;
        vec![start_item; spaces.spaces.len()]
    } else {
        vec![start_item; positions.len()]
    };
    let mut cached = [0.0f32; 128];
    let mut aggregate = |item: &mut Item, mv: usize, face: usize, val: f32| {
        if let Some((dv, index)) = vg {
            let of_group = has_group(&dv[mv], index);
            if of_group == s.invert_vertex_group {
                return;
            }
        }
        if use_face_influence {
            let strength = face_strength.as_ref().expect("checked")[face];
            if strength > item.curr_strength {
                item.curr_strength = strength;
                item.curr_val = 0.0;
                item.loops_num = 0;
                item.normal = [0.0; 3];
            }
            if strength != item.curr_strength {
                return;
            }
        }
        if item.curr_val == 0.0 {
            item.curr_val = val;
        }
        if (item.curr_val - val).abs() > s.thresh {
            item.loops_num += 1;
            item.curr_val = val;
        }
        let n = item.loops_num;
        let inv = if (n as usize) < cached.len() {
            if cached[n as usize] == 0.0 {
                cached[n as usize] = 1.0 / libm::powf(weight, n as f32);
            }
            cached[n as usize]
        } else {
            1.0 / libm::powf(weight, n as f32)
        };
        let f = val * inv;
        let fno = fnor[face];
        item.normal = [
            item.normal[0] + fno[0] * f,
            item.normal[1] + fno[1] * f,
            item.normal[2] + fno[2] * f,
        ];
    };
    if s.mode == "FACE_AREA" {
        for &(val, face) in &mode_pair {
            for corner in face_range(face) {
                let mv = cv[corner] as usize;
                let it = if keep_sharp { corner_space[corner] } else { mv };
                aggregate(&mut items[it], mv, face, val);
            }
        }
    } else {
        for &(val, corner) in &mode_pair {
            let mv = cv[corner] as usize;
            let it = if keep_sharp { corner_space[corner] } else { mv };
            aggregate(&mut items[it], mv, corner_to_face[corner], val);
        }
    }
    for it in &mut items {
        if normalize(&mut it.normal) < CLNORS_VALID_VEC_LEN {
            it.normal = [0.0; 3];
        }
    }
    if keep_sharp {
        for (c, n) in corner_normals.iter_mut().enumerate() {
            let it = &items[corner_space[c]];
            if it.normal != [0.0; 3] {
                *n = it.normal;
            }
        }
        normals_corner_custom_set(
            &view,
            &vnor,
            false,
            &mut corner_normals,
            &mut sharp,
            &mut clnors,
        );
    } else if vg.is_none() {
        let mut vert_normals: Vec<V3> = vec![[0.0; 3]; positions.len()];
        for &v in cv {
            vert_normals[v as usize] = items[v as usize].normal;
        }
        normals_corner_custom_set(
            &view,
            &vnor,
            true,
            &mut vert_normals,
            &mut sharp,
            &mut clnors,
        );
    } else {
        let (mut cn, _) = calc_corners_with_spaces(&view, Some(&sharp), Some(&clnors));
        for (c, n) in cn.iter_mut().enumerate() {
            let it = &items[cv[c] as usize];
            if it.normal != [0.0; 3] {
                *n = it.normal;
            }
        }
        normals_corner_custom_set(&view, &vnor, false, &mut cn, &mut sharp, &mut clnors);
    }
    store(&mut mesh, Some(sharp), Some(clnors));
    mesh
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    fn settings(p: &serde_json::Value) -> WeightedNormalSettings {
        WeightedNormalSettings {
            weight: p["weight"].as_i64().unwrap() as i32,
            mode: p["mode"].as_str().unwrap().into(),
            thresh: p["thresh"].as_f64().unwrap() as f32,
            keep_sharp: p["keep_sharp"].as_bool().unwrap(),
            vertex_group: p["vertex_group"].as_str().unwrap().into(),
            invert_vertex_group: p["invert_vertex_group"].as_bool().unwrap(),
            use_face_influence: false,
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/weighted_normal.json`).
    #[test]
    fn matches_blender() {
        let d = load("vgroups/weighted_normal.json");
        for case in d["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            eprintln!("weighted_normal case {name}");
            let mesh = mesh_from_dump(&case["input"]);
            let s = settings(&case["props"]);
            let out = weighted_normal(&s, mesh);
            let want = &case["output"];
            assert_topology(&out, want);
            assert_attribute(&out, want, "sharp_edge", 0.0);
            assert_attribute(&out, want, "custom_normal", 0.0);
            assert_corner_normals(&out, want, 2e-4);
        }
    }

    #[test]
    fn face_angles_of_a_square() {
        let p = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        for a in face_angles(&p, &[0, 1, 2, 3]) {
            assert!((a - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        }
        assert!((face_area(&p, &[0, 1, 2, 3]) - 1.0).abs() < 1e-6);
    }
}
