// SPDX-License-Identifier: GPL-3.0-or-later
//! CORRECTIVE_SMOOTH (`CorrectiveSmoothModifier`, "delta mush"): smooths the deformed
//! vertices, then adds back the detail the same smoothing removes from the rest shape, stored
//! per corner in the corner's tangent space (`factor`, `iterations`, `scale`, `smooth_type`
//! `SIMPLE` or `LENGTH_WEIGHTED`, `use_pin_boundary`, `use_only_smooth`, `vertex_group`).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_correctivesmooth.cc (Blender 5.2.4,
//! GPL-2.0-or-later). The rest shape is `rest_source='ORCO'` (the object's own mesh, without
//! shape keys or modifiers). `rest_source='BIND'` needs `object.correctivesmooth_bind`, which
//! frap does not implement (OUT): such a modifier has no bind data and reports Blender's
//! "Bind data required" (as an unbound one does in Blender). Blender caches the deltas between
//! evaluations; frap recomputes them, with the same result. In Edit Mode the rest shape is the
//! edit mesh's coordinates, as in Blender.

use frap_geom::normals::safe_acos_approx;
use frap_math::matrix::normalize_v3;
use frap_mesh::DeformVert;
use frap_rna::{IdRef, MeshId};
use frap_scene::generated::modifier_settings::CorrectiveSmoothSettings;

use super::warp::shared::{vgroup, vgroup_weight};
use crate::prelude::*;

type V3 = [f32; 3];
type M3 = [[f32; 3]; 3];

#[modifier(
    CORRECTIVE_SMOOTH,
    kind = OnlyDeform,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::CorrectiveSmooth(s) = &md.settings else {
        return Ok(mesh);
    };
    let ob = cx.object();
    // The edit mesh's coordinates in Edit Mode (`BKE_editmesh_vert_coords_alloc_orco`), else
    // the object's mesh.
    let rest: Option<Vec<V3>> = if s.rest_source == "ORCO" && ob.object_type() == "MESH" {
        match cx.edit_mesh() {
            Some(em) => Some(em.positions().to_vec()),
            None => ob
                .data
                .and_then(IdRef::<MeshId>::from_any)
                .and_then(|m| cx.main().meshes.get(m))
                .map(|m| m.positions().to_vec()),
        }
    } else {
        None
    };
    let is_mesh = ob.object_type() == "MESH";
    corrective_smooth(s, is_mesh, rest.as_deref(), &mut mesh)?;
    Ok(mesh)
}

// Ported from blender/source/blender/modifiers/intern/MOD_correctivesmooth.cc
// (correctivesmooth_modifier_do).
/// Smooths `mesh` in place against the rest positions `rest` (`ORCO`; `None` for `BIND`,
/// which frap cannot bind). `is_mesh`: the object is a mesh object.
pub(crate) fn corrective_smooth(
    s: &CorrectiveSmoothSettings,
    is_mesh: bool,
    rest: Option<&[V3]>,
    mesh: &mut Mesh,
) -> Result<(), ModError> {
    let (defgrp, dvert) = vgroup(mesh, &s.vertex_group);
    let dvert: Option<(u32, Vec<DeformVert>)> = defgrp.zip(dvert.map(<[_]>::to_vec));
    let dvert = dvert.as_ref().map(|(g, d)| (*g, d.as_slice()));
    let mut cos = mesh.positions().to_vec();
    if s.use_only_smooth {
        smooth_verts(s, mesh, dvert, &mut cos);
        *mesh.positions_mut() = cos;
        return Ok(());
    }
    let rest = if s.rest_source == "BIND" {
        return Err(ModError::Error("Bind data required".into()));
    } else if !is_mesh {
        return Err(ModError::Error("Object is not a mesh".into()));
    } else {
        let rest = rest.unwrap_or(&[]);
        if rest.len() != cos.len() {
            return Err(ModError::Error(format!(
                "Original vertex count mismatch: {} to {}",
                rest.len(),
                cos.len()
            )));
        }
        rest
    };
    let deltas = calc_deltas(s, mesh, dvert, rest);
    // The actual delta mush.
    smooth_verts(s, mesh, dvert, &mut cos);
    let corner_verts = mesh.corner_verts();
    let mut tangent_spaces = vec![[[0.0f32; 3]; 3]; corner_verts.len()];
    let mut weights = vec![0.0f32; corner_verts.len()];
    let mut weights_per_vertex = vec![0.0f32; cos.len()];
    calc_tangent_spaces(
        mesh,
        &cos,
        &mut tangent_spaces,
        Some((&mut weights, &mut weights_per_vertex)),
    );
    for (l, &v) in corner_verts.iter().enumerate() {
        let v = v as usize;
        let weight = weights[l] / weights_per_vertex[v];
        // Catches zero and divide by zero.
        if weight.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            continue;
        }
        let mut delta = mul_v3_m3v3(&tangent_spaces[l], deltas[l]);
        for d in &mut delta {
            *d *= weight;
        }
        for k in 0..3 {
            cos[v][k] += delta[k] * s.scale;
        }
    }
    *mesh.positions_mut() = cos;
    Ok(())
}

/// `mesh_get_weights` / `mesh_get_boundaries`: the per-vertex smoothing weights (`None`
/// without a group and boundary pinning).
fn smooth_weights(
    s: &CorrectiveSmoothSettings,
    mesh: &Mesh,
    dvert: Option<(u32, &[DeformVert])>,
    n: usize,
) -> Option<Vec<f32>> {
    if dvert.is_none() && !s.use_pin_boundary {
        return None;
    }
    let mut w: Vec<f32> = match dvert {
        Some((g, dv)) => (0..n)
            .map(|i| vgroup_weight(dv, i, g, s.invert_vertex_group))
            .collect(),
        None => vec![1.0; n],
    };
    if s.use_pin_boundary {
        // Flag boundary edges so only boundaries are set to 1.
        let mut boundaries = vec![0u8; mesh.edges_num()];
        for &e in mesh.corner_edges() {
            let b = &mut boundaries[e as usize];
            *b |= b.wrapping_add(1);
        }
        for (e, edge) in mesh.edges().iter().enumerate() {
            if boundaries[e] == 1 {
                w[edge[0] as usize] = 0.0;
                w[edge[1] as usize] = 0.0;
            }
        }
    }
    Some(w)
}

/// `smooth_verts`
fn smooth_verts(
    s: &CorrectiveSmoothSettings,
    mesh: &Mesh,
    dvert: Option<(u32, &[DeformVert])>,
    cos: &mut [V3],
) {
    let weights = smooth_weights(s, mesh, dvert, cos.len());
    let iterations = s.iterations.max(0) as u32;
    if s.smooth_type == "LENGTH_WEIGHTED" {
        smooth_iter_length_weight(s.factor, mesh, cos, weights.as_deref(), iterations);
    } else {
        smooth_iter_simple(s.factor, mesh, cos, weights.as_deref(), iterations);
    }
}

/// `smooth_iter__simple`: the average of the neighbours.
fn smooth_iter_simple(
    lambda: f32,
    mesh: &Mesh,
    cos: &mut [V3],
    weights: Option<&[f32]>,
    iterations: u32,
) {
    let edges = mesh.edges();
    let mut delta = vec![[0.0f32; 3]; cos.len()];
    let mut div = vec![0.0f32; cos.len()];
    for e in edges {
        div[e[0] as usize] += 1.0;
        div[e[1] as usize] += 1.0;
    }
    // `lambda` and the smoothing weight are folded in here.
    for (i, d) in div.iter_mut().enumerate() {
        let inv = if *d != 0.0 { 1.0 / *d } else { 1.0 };
        *d = match weights {
            None => lambda * inv,
            Some(w) => w[i] * lambda * inv,
        };
    }
    for _ in 0..iterations {
        for e in edges {
            let (a, b) = (e[0] as usize, e[1] as usize);
            let dir = sub(cos[b], cos[a]);
            for k in 0..3 {
                delta[a][k] += dir[k];
                delta[b][k] -= dir[k];
            }
        }
        for (i, co) in cos.iter_mut().enumerate() {
            for k in 0..3 {
                co[k] += delta[i][k] * div[i];
            }
            delta[i] = [0.0; 3];
        }
    }
}

/// `smooth_iter__length_weight`: neighbours weighted by edge length.
fn smooth_iter_length_weight(
    lambda: f32,
    mesh: &Mesh,
    cos: &mut [V3],
    weights: Option<&[f32]>,
    iterations: u32,
) {
    let eps = f32::EPSILON * 10.0;
    // About half as strong as the simple smooth: double it for consistent behaviour.
    let lambda = lambda * 2.0;
    let edges = mesh.edges();
    let mut delta = vec![[0.0f32; 3]; cos.len()];
    let mut length_sum = vec![0.0f32; cos.len()];
    let mut count = vec![0.0f32; cos.len()];
    for e in edges {
        count[e[0] as usize] += 1.0;
        count[e[1] as usize] += 1.0;
    }
    for _ in 0..iterations {
        for e in edges {
            let (a, b) = (e[0] as usize, e[1] as usize);
            let mut dir = sub(cos[b], cos[a]);
            let dist = len(dir);
            // Weight by distance.
            for d in &mut dir {
                *d *= dist;
            }
            for k in 0..3 {
                delta[a][k] += dir[k];
                delta[b][k] -= dir[k];
            }
            length_sum[a] += dist;
            length_sum[b] += dist;
        }
        for (i, co) in cos.iter_mut().enumerate() {
            let div = length_sum[i] * count[i];
            if div > eps {
                let f = match weights {
                    None => lambda / div,
                    Some(w) => (lambda * w[i]) / div,
                };
                for k in 0..3 {
                    co[k] += delta[i][k] * f;
                }
            }
            delta[i] = [0.0; 3];
            length_sum[i] = 0.0;
        }
    }
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len(a: V3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `mul_v3_m3v3`: `M · v` (`m[col][row]`).
fn mul_v3_m3v3(m: &M3, v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

/// `calc_tangent_loop`: an orthogonal frame from the two edge directions of a corner;
/// `false` when the corner has no influence.
fn calc_tangent_loop(prev: V3, next: V3, ts: &mut M3) -> bool {
    let lim = f32::EPSILON * 10.0;
    if (0..3).all(|k| (prev[k] - next[k]).abs() <= lim) {
        // No weight: any value.
        *ts = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        return false;
    }
    ts[2] = normalize_v3(cross(prev, next)).0;
    // Orthogonal, with `ts[2]` as the basis.
    let sum = [prev[0] + next[0], prev[1] + next[1], prev[2] + next[2]];
    ts[0] = normalize_v3(sum).0;
    ts[1] = cross(ts[2], ts[0]);
    true
}

/// `calc_tangent_spaces`: a frame per corner and, when asked, its weight (the corner's angle)
/// and the sum of weights per vertex.
fn calc_tangent_spaces(
    mesh: &Mesh,
    cos: &[V3],
    spaces: &mut [M3],
    mut weights: Option<(&mut [f32], &mut [f32])>,
) {
    let cv = mesh.corner_verts();
    if let Some((_, per_vertex)) = weights.as_mut() {
        per_vertex.fill(0.0);
    }
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        let term = r.end;
        let mut prev = term - 2;
        let mut curr = term - 1;
        let mut next = r.start;
        let mut dir_prev = normalize_v3(sub(cos[cv[prev] as usize], cos[cv[curr] as usize])).0;
        while next != term {
            let dir_next = normalize_v3(sub(cos[cv[curr] as usize], cos[cv[next] as usize])).0;
            if calc_tangent_loop(dir_prev, dir_next, &mut spaces[curr]) {
                if let Some((w, per_vertex)) = weights.as_mut() {
                    let d = dir_next[0] * dir_prev[0]
                        + dir_next[1] * dir_prev[1]
                        + dir_next[2] * dir_prev[2];
                    let weight = safe_acos_approx(d).abs();
                    w[curr] = weight;
                    per_vertex[cv[curr] as usize] += weight;
                }
            } else if let Some((w, _)) = weights.as_mut() {
                w[curr] = 0.0;
            }
            dir_prev = dir_next;
            prev = curr;
            curr = next;
            next += 1;
        }
        let _ = prev;
    }
}

/// `invert_m3_m3`: the adjoint over the determinant; `None` for a zero determinant.
fn invert_m3(m: &M3) -> Option<M3> {
    let (m00, m01, m02) = (m[0][0], m[0][1], m[0][2]);
    let (m10, m11, m12) = (m[1][0], m[1][1], m[1][2]);
    let (m20, m21, m22) = (m[2][0], m[2][1], m[2][2]);
    let mut r = [
        [
            m11 * m22 - m12 * m21,
            -m01 * m22 + m02 * m21,
            m01 * m12 - m02 * m11,
        ],
        [
            -m10 * m22 + m12 * m20,
            m00 * m22 - m02 * m20,
            -m00 * m12 + m02 * m10,
        ],
        [
            m10 * m21 - m11 * m20,
            -m00 * m21 + m01 * m20,
            m00 * m11 - m01 * m10,
        ],
    ];
    let det = m00 * (m11 * m22 - m12 * m21) - m10 * (m01 * m22 - m02 * m21)
        + m20 * (m01 * m12 - m02 * m11);
    if det == 0.0 {
        return None;
    }
    let inv = 1.0 / det;
    for c in &mut r {
        for v in c.iter_mut() {
            *v *= inv;
        }
    }
    Some(r)
}

/// `calc_deltas`: the rest shape's detail per corner, in the tangent space of the smoothed
/// rest shape.
fn calc_deltas(
    s: &CorrectiveSmoothSettings,
    mesh: &Mesh,
    dvert: Option<(u32, &[DeformVert])>,
    rest: &[V3],
) -> Vec<V3> {
    let mut smooth = rest.to_vec();
    smooth_verts(s, mesh, dvert, &mut smooth);
    let cv = mesh.corner_verts();
    let mut spaces = vec![[[0.0f32; 3]; 3]; cv.len()];
    calc_tangent_spaces(mesh, &smooth, &mut spaces, None);
    cv.iter()
        .enumerate()
        .map(|(l, &v)| {
            let v = v as usize;
            let delta = sub(rest[v], smooth[v]);
            let ts = &spaces[l];
            let imat = invert_m3(ts).unwrap_or_else(|| {
                // `transpose_m3_m3`
                let mut t = [[0.0f32; 3]; 3];
                for (i, ti) in t.iter_mut().enumerate() {
                    for (j, tij) in ti.iter_mut().enumerate() {
                        *tij = ts[j][i];
                    }
                }
                t
            });
            mul_v3_m3v3(&imat, delta)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere() -> Mesh {
        frap_mesh::primitives::uv_sphere::uv_sphere(
            &frap_mesh::primitives::uv_sphere::UvSphere::default(),
        )
    }

    #[test]
    fn undeformed_mesh_is_restored() {
        // Smoothing then adding the rest deltas back gives the rest shape.
        let mut m = sphere();
        let rest = m.positions().to_vec();
        let s = CorrectiveSmoothSettings::default();
        corrective_smooth(&s, true, Some(&rest), &mut m).unwrap();
        for (a, b) in m.positions().iter().zip(&rest) {
            for k in 0..3 {
                assert!((a[k] - b[k]).abs() < 1e-4, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn errors_match_blender() {
        let mut m = sphere();
        let s = CorrectiveSmoothSettings {
            rest_source: "BIND".into(),
            ..CorrectiveSmoothSettings::default()
        };
        let e = corrective_smooth(&s, true, None, &mut m).unwrap_err();
        assert_eq!(e.to_string(), "Bind data required");
        let s = CorrectiveSmoothSettings::default();
        let e = corrective_smooth(&s, true, Some(&[[0.0; 3]]), &mut m).unwrap_err();
        assert_eq!(
            e.to_string(),
            format!("Original vertex count mismatch: 1 to {}", m.verts_num())
        );
    }
}
