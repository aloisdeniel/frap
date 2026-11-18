// SPDX-License-Identifier: GPL-3.0-or-later
//! LAPLACIANSMOOTH (`LaplacianSmoothModifier`): implicit Laplacian smoothing. Each iteration
//! solves `(I + λL) x = co` once per axis with cotangent weights on interior vertices
//! (`lambda_factor`, scaled by the ring area unless `use_normalized`) and an edge-length
//! umbrella operator along borders (`lambda_border`), weighted by `vertex_group`; vertices next
//! to a degenerate edge or face stay fixed. A negative λ moves the vertices away from the
//! solution. `use_volume_preserve` rescales about the centroid to keep the volume. Disabled
//! when no axis is enabled.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_laplaciansmooth.cc (Blender 5.2.4)
//! with the `area_tri_v3`, `cotangent_tri_weight_v3` and `volume_tetrahedron_signed_v3`
//! helpers of blenlib/intern/math_geom.cc; the linear solve is described in [`solver`].

pub(crate) mod solver;

use frap_scene::generated::modifier_settings::LaplacianSmoothSettings;

use self::solver::Csr;
use super::smooth::common::{deform, find_weight, get_vgroup};
use crate::prelude::*;

fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Laplaciansmooth(s) if !(s.use_x || s.use_y || s.use_z) => {
            Some("Smooth Laplacian: no axis is enabled")
        }
        _ => None,
    }
}

#[modifier(
    LAPLACIANSMOOTH,
    kind = OnlyDeform,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Laplaciansmooth(s) = &md.settings else {
        return Ok(mesh);
    };
    if mesh.verts_num() == 0 {
        return Ok(mesh);
    }
    Ok(deform(mesh, |m, cos| laplacian_smooth_do(s, m, cos)))
}

type V3 = [f32; 3];

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

/// `area_tri_v3`.
fn area_tri(v1: V3, v2: V3, v3: V3) -> f32 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    len(cross(n1, n2)) * 0.5
}

/// `cotangent_tri_weight_v3`.
fn cotangent_tri_weight(v1: V3, v2: V3, v3: V3) -> f32 {
    let a = sub(v2, v1);
    let b = sub(v3, v1);
    let c_len = len(cross(a, b));
    if c_len > f32::EPSILON {
        (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / c_len
    } else {
        0.0
    }
}

/// `volume_tetrahedron_signed_v3`.
fn volume_tetrahedron_signed(v1: V3, v2: V3, v3: V3, v4: V3) -> f32 {
    let m = [sub(v1, v2), sub(v2, v3), sub(v3, v4)];
    (m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
        + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1]))
        / 6.0
}

/// `LaplacianSystem`.
struct System {
    eweights: Vec<f32>,
    fweights: Vec<V3>,
    ring_areas: Vec<f32>,
    vlengths: Vec<f32>,
    vweights: Vec<f32>,
    ne_fa_num: Vec<i16>,
    ne_ed_num: Vec<i16>,
    zerola: Vec<bool>,
    min_area: f32,
    vert_centroid: V3,
}

/// Iterates `(prev, curr, next)` corners of a face the way Blender's loops do: starting with
/// `curr` = last corner, `next` = first.
fn face_triples(s: usize, e: usize) -> impl Iterator<Item = (usize, usize, usize)> {
    let mut prev = e - 2;
    let mut curr = e - 1;
    (s..e).map(move |next| {
        let t = (prev, curr, next);
        prev = curr;
        curr = next;
        t
    })
}

fn compute_volume(center: V3, cos: &[V3], mesh: &Mesh) -> f32 {
    let cv = mesh.corner_verts();
    let mut vol = 0.0f32;
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        let first = r.start;
        for c in first + 2..r.end {
            vol += volume_tetrahedron_signed(
                center,
                cos[cv[first] as usize],
                cos[cv[c - 1] as usize],
                cos[cv[c] as usize],
            );
        }
    }
    vol.abs()
}

// Ported from blender/source/blender/modifiers/intern/MOD_laplaciansmooth.cc
// (init_laplacian_matrix).
fn init_laplacian_matrix(sys: &mut System, mesh: &Mesh, cos: &[V3]) {
    for (i, e) in mesh.edges().iter().enumerate() {
        let (a, b) = (e[0] as usize, e[1] as usize);
        sys.ne_ed_num[a] += 1;
        sys.ne_ed_num[b] += 1;
        let mut w1 = len(sub(cos[a], cos[b]));
        if w1 < sys.min_area {
            sys.zerola[a] = true;
            sys.zerola[b] = true;
        } else {
            w1 = 1.0 / w1;
        }
        sys.eweights[i] = w1;
    }
    let cv = mesh.corner_verts();
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        for (prev, curr, next) in face_triples(r.start, r.end) {
            let (vp, vc, vn) = (cv[prev] as usize, cv[curr] as usize, cv[next] as usize);
            let (v_prev, v_curr, v_next) = (cos[vp], cos[vc], cos[vn]);
            sys.ne_fa_num[vc] += 1;
            let areaf = area_tri(v_prev, v_curr, v_next);
            if areaf < sys.min_area {
                sys.zerola[vc] = true;
            }
            sys.ring_areas[vp] += areaf;
            sys.ring_areas[vc] += areaf;
            sys.ring_areas[vn] += areaf;
            let w1 = cotangent_tri_weight(v_curr, v_next, v_prev) / 2.0;
            let w2 = cotangent_tri_weight(v_next, v_prev, v_curr) / 2.0;
            let w3 = cotangent_tri_weight(v_prev, v_curr, v_next) / 2.0;
            sys.fweights[curr][0] += w1;
            sys.fweights[curr][1] += w2;
            sys.fweights[curr][2] += w3;
            sys.vweights[vc] += w2 + w3;
            sys.vweights[vn] += w1 + w3;
            sys.vweights[vp] += w1 + w2;
        }
    }
    for (i, e) in mesh.edges().iter().enumerate() {
        let (a, b) = (e[0] as usize, e[1] as usize);
        if sys.ne_ed_num[a] != sys.ne_fa_num[a] && sys.ne_ed_num[b] != sys.ne_fa_num[b] {
            sys.vlengths[a] += sys.eweights[i];
            sys.vlengths[b] += sys.eweights[i];
        }
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_laplaciansmooth.cc
// (fill_laplacian_matrix).
fn fill_laplacian_matrix(sys: &System, mesh: &Mesh, t: &mut Vec<(usize, usize, f64)>) {
    let cv = mesh.corner_verts();
    let ring = |v: usize| sys.ne_ed_num[v] == sys.ne_fa_num[v] && !sys.zerola[v];
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        for (prev, curr, next) in face_triples(r.start, r.end) {
            let (vp, vc, vn) = (cv[prev] as usize, cv[curr] as usize, cv[next] as usize);
            let fw = sys.fweights[curr];
            if ring(vc) {
                t.push((vc, vn, (fw[2] * sys.vweights[vc]) as f64));
                t.push((vc, vp, (fw[1] * sys.vweights[vc]) as f64));
            }
            if ring(vn) {
                t.push((vn, vc, (fw[2] * sys.vweights[vn]) as f64));
                t.push((vn, vp, (fw[0] * sys.vweights[vn]) as f64));
            }
            if ring(vp) {
                t.push((vp, vc, (fw[1] * sys.vweights[vp]) as f64));
                t.push((vp, vn, (fw[0] * sys.vweights[vp]) as f64));
            }
        }
    }
    for (i, e) in mesh.edges().iter().enumerate() {
        let (a, b) = (e[0] as usize, e[1] as usize);
        if sys.ne_ed_num[a] != sys.ne_fa_num[a]
            && sys.ne_ed_num[b] != sys.ne_fa_num[b]
            && !sys.zerola[a]
            && !sys.zerola[b]
        {
            t.push((a, b, (sys.eweights[i] * sys.vlengths[a]) as f64));
            t.push((b, a, (sys.eweights[i] * sys.vlengths[b]) as f64));
        }
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_laplaciansmooth.cc
// (laplaciansmoothModifier_do, validate_solution, volume_preservation).
/// `laplaciansmoothModifier_do` on `cos`.
pub(crate) fn laplacian_smooth_do(s: &LaplacianSmoothSettings, mesh: &Mesh, cos: &mut [V3]) {
    let n = cos.len();
    let mut sys = System {
        eweights: vec![0.0; mesh.edges_num()],
        fweights: vec![[0.0; 3]; mesh.corners_num()],
        ring_areas: vec![0.0; n],
        vlengths: vec![0.0; n],
        vweights: vec![0.0; n],
        ne_fa_num: vec![0; n],
        ne_ed_num: vec![0; n],
        zerola: vec![false; n],
        min_area: 0.00001,
        vert_centroid: [0.0; 3],
    };
    let vg = get_vgroup(mesh, &s.vertex_group);
    init_laplacian_matrix(&mut sys, mesh, cos);
    let lambda = s.lambda_factor;
    let lambda_border = s.lambda_border;
    let mut matrix: Option<Option<Csr>> = None;
    for iter in 0..s.iterations.max(0) {
        frap_math::budget::check();
        if iter == 0 {
            for p in cos.iter() {
                for (c, v) in sys.vert_centroid.iter_mut().zip(p) {
                    *c += v;
                }
            }
            let f = 1.0 / n as f32;
            sys.vert_centroid = [
                sys.vert_centroid[0] * f,
                sys.vert_centroid[1] * f,
                sys.vert_centroid[2] * f,
            ];
        }
        let rhs: Vec<Vec<f64>> = (0..3)
            .map(|k| cos.iter().map(|p| p[k] as f64).collect())
            .collect();
        if matrix.is_none() {
            let mut t: Vec<(usize, usize, f64)> = Vec::new();
            for i in 0..n {
                let wpaint = match vg {
                    Some((dv, index)) => {
                        let w = find_weight(&dv[i], index);
                        if s.invert_vertex_group { 1.0 - w } else { w }
                    }
                    None => 1.0,
                };
                if !sys.zerola[i] {
                    let ring = sys.ne_ed_num[i] == sys.ne_fa_num[i];
                    if s.use_normalized {
                        let w = sys.vweights[i];
                        sys.vweights[i] = if w == 0.0 {
                            0.0
                        } else {
                            -lambda.abs() * wpaint / w
                        };
                        let w = sys.vlengths[i];
                        sys.vlengths[i] = if w == 0.0 {
                            0.0
                        } else {
                            -lambda_border.abs() * wpaint * 2.0 / w
                        };
                        let d = if ring {
                            1.0 + lambda.abs() * wpaint
                        } else {
                            1.0 + lambda_border.abs() * wpaint * 2.0
                        };
                        t.push((i, i, d as f64));
                    } else {
                        let w = sys.vweights[i] * sys.ring_areas[i];
                        sys.vweights[i] = if w == 0.0 {
                            0.0
                        } else {
                            -lambda.abs() * wpaint / (4.0 * w)
                        };
                        let w = sys.vlengths[i];
                        sys.vlengths[i] = if w == 0.0 {
                            0.0
                        } else {
                            -lambda_border.abs() * wpaint * 2.0 / w
                        };
                        let d = if ring {
                            1.0 + lambda.abs() * wpaint / (4.0 * sys.ring_areas[i])
                        } else {
                            1.0 + lambda_border.abs() * wpaint * 2.0
                        };
                        t.push((i, i, d as f64));
                    }
                } else {
                    t.push((i, i, 1.0));
                }
            }
            fill_laplacian_matrix(&sys, mesh, &mut t);
            matrix = Some(Some(Csr::from_triplets(n, t)));
        }
        let Some(Some(m)) = &matrix else {
            continue;
        };
        let Some(x) = m.solve(&rhs) else {
            // `EIG_linear_solver_solve` failed: the positions are left as they are.
            continue;
        };
        // validate_solution.
        let preserve = s.use_volume_preserve;
        let vini = if preserve {
            compute_volume(sys.vert_centroid, cos, mesh)
        } else {
            0.0
        };
        for (i, p) in cos.iter_mut().enumerate() {
            if sys.zerola[i] {
                continue;
            }
            let lam = if sys.ne_ed_num[i] == sys.ne_fa_num[i] {
                if lambda >= 0.0 { 1.0 } else { -1.0 }
            } else if lambda_border >= 0.0 {
                1.0
            } else {
                -1.0
            };
            let axes = [s.use_x, s.use_y, s.use_z];
            for k in 0..3 {
                if axes[k] {
                    p[k] += lam * (x[k][i] as f32 - p[k]);
                }
            }
        }
        if preserve {
            let vend = compute_volume(sys.vert_centroid, cos, mesh);
            if vend != 0.0 {
                let beta = libm::powf(vini / vend, 1.0 / 3.0);
                let c = sys.vert_centroid;
                let axes = [s.use_x, s.use_y, s.use_z];
                for p in cos.iter_mut() {
                    for k in 0..3 {
                        if axes[k] {
                            p[k] = (p[k] - c[k]) * beta + c[k];
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    fn settings(p: &serde_json::Value) -> LaplacianSmoothSettings {
        let b = |k: &str| p[k].as_bool().unwrap();
        let f = |k: &str| p[k].as_f64().unwrap() as f32;
        LaplacianSmoothSettings {
            use_x: b("use_x"),
            use_y: b("use_y"),
            use_z: b("use_z"),
            use_volume_preserve: b("use_volume_preserve"),
            use_normalized: b("use_normalized"),
            lambda_factor: f("lambda_factor"),
            lambda_border: f("lambda_border"),
            iterations: p["iterations"].as_i64().unwrap() as i32,
            vertex_group: p["vertex_group"].as_str().unwrap().into(),
            invert_vertex_group: b("invert_vertex_group"),
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/laplaciansmooth.json`).
    #[test]
    fn matches_blender() {
        let d = load("vgroups/laplaciansmooth.json");
        for case in d["cases"].as_array().unwrap() {
            let mesh = mesh_from_dump(&case["input"]);
            let s = settings(&case["props"]);
            let out = deform(mesh, |m, cos| laplacian_smooth_do(&s, m, cos));
            assert_positions(&out, &case["output"], 2e-5);
        }
    }

    #[test]
    fn helpers() {
        assert_eq!(area_tri([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), 0.5);
        // 90° at v1: cot = 0.
        assert_eq!(
            cotangent_tri_weight([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            0.0
        );
        let v = volume_tetrahedron_signed(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        );
        assert!((v.abs() - 1.0 / 6.0).abs() < 1e-7);
    }
}
