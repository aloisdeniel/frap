// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.smooth_laplacian_vert` (Blender 5.2.4 `bmo_smooth_laplacian_vert_exec`,
//! `operators/bmo_smooth_laplacian.cc`), implemented by WP 8.3.3.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.smooth_laplacian_vert(bmesh, verts=[], lambda_factor=0.0, lambda_border=0.0,
//! use_x=False, use_y=False, use_z=False, preserve_volume=False)`.
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_smooth_laplacian.cc (Blender 5.2.4,
//! GPL-2.0-or-later). One implicit Laplacian step (Desbrun et al.): cotangent weights over the
//! **selected** faces, inverse edge lengths over the unselected boundary edges, the input
//! vertices as the unknowns of a least-squares system (`EIG_linear_least_squares_solver`,
//! [`solver`]) whose other variables are locked at 0, then a validation that keeps a vertex
//! in place when one of its edges would grow past 1.8× or shrink below 0.15× (measured
//! against the solution, 0 for locked vertices, as in Blender), and an optional uniform
//! rescale that restores the mesh volume. Nothing happens on a mesh without faces or when the
//! factorization fails. The face normals are not used (only positions).
//!
//! **Create/kill order**: nothing is created or killed. Operator type flags: `NORMALS_CALC`.

use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, ElemFlag, ElemTypes, VertId};

use super::triangulate::math::{area_tri_v3, len_v3};

pub(crate) mod solver;

use solver::LeastSquares;

/// Input slots of `smooth_laplacian_vert` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmoothLaplacianIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `lambda_factor`.
    pub lambda_factor: f32,
    /// `lambda_border`.
    pub lambda_border: f32,
    /// `use_x`.
    pub use_x: bool,
    /// `use_y`.
    pub use_y: bool,
    /// `use_z`.
    pub use_z: bool,
    /// `preserve_volume`.
    pub preserve_volume: bool,
}

/// Output slots of `smooth_laplacian_vert` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SmoothLaplacianOut {
    /// Always empty.
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_smooth_laplacian_vert_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

const MAX_EDGE_PERCENTAGE: f32 = 1.8;
const MIN_EDGE_PERCENTAGE: f32 = 0.15;
const MIN_AREA: f32 = 0.00001;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `cotangent_tri_weight_v3`.
fn cotangent_tri_weight_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    let a = sub(v2, v1);
    let b = sub(v3, v1);
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let c_len = len_v3(c);
    if c_len > f32::EPSILON {
        (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / c_len
    } else {
        0.0
    }
}

/// `BM_mesh_calc_volume(bm, false)` (double precision, `|Σ p1 · (p2 × p3)| / 6` over each
/// face's tessellation).
pub(crate) fn mesh_calc_volume(bm: &BMesh) -> f64 {
    let mut vol = 0.0f64;
    for f in bm.face_ids() {
        let (loops, tris) = super::intersect::query::face_calc_tessellation(bm, f);
        let mut fv = 0.0f64;
        for t in tris {
            let p = |k: usize| {
                let c = bm.vert(bm.lp(loops[t[k] as usize]).v).co;
                [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]
            };
            let (p1, p2, p3) = (p(0), p(1), p(2));
            let cross = [
                p2[1] * p3[2] - p2[2] * p3[1],
                p2[2] * p3[0] - p2[0] * p3[2],
                p2[0] * p3[1] - p2[1] * p3[0],
            ];
            fv += p1[0] * cross[0] + p1[1] * cross[1] + p1[2] * cross[2];
        }
        vol += (1.0 / 6.0) * fv;
    }
    vol.abs()
}

/// `vert_is_boundary`: a boundary edge or an unselected face around `v`.
fn vert_is_boundary(bm: &BMesh, v: VertId) -> bool {
    if bm.vert_edges(v).into_iter().any(|e| bm.edge_is_boundary(e)) {
        return true;
    }
    bm.vert_faces(v)
        .into_iter()
        .any(|f| !bm.face(f).flag.contains(ElemFlag::SELECT))
}

/// Runs `smooth_laplacian_vert` on `bm`.
pub fn smooth_laplacian_vert(
    bm: &mut BMesh,
    i: &SmoothLaplacianIn,
) -> Result<SmoothLaplacianOut, BmError> {
    if let Some(dead) = i.verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!(
            "smooth_laplacian_vert: dead vertex {dead:?} in verts"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        run(bm, i);
        Ok(SmoothLaplacianOut::default())
    })
}

fn run(bm: &mut BMesh, i: &SmoothLaplacianIn) {
    if bm.faces_num() == 0 {
        return;
    }
    bm.elem_index_ensure(ElemTypes::VERT);
    let totvert = bm.verts_num();
    let edges = bm.edge_ids();
    let numloops = bm.loops_num();
    let vidx = |bm: &BMesh, v: VertId| bm.index_get(BMElem::Vert(v)) as usize;

    let mut eweights = vec![0.0f32; edges.len()];
    let mut fweights = vec![[0.0f32; 3]; numloops];
    let mut ring_areas = vec![0.0f32; totvert];
    let mut vlengths = vec![0.0f32; totvert];
    let mut vweights = vec![0.0f32; totvert];
    let mut zerola = vec![false; totvert];

    let mut sys = LeastSquares::new(totvert, totvert);
    for k in 0..totvert {
        sys.lock(k);
    }
    for &v in &i.verts {
        let m = vidx(bm, v);
        let co = bm.vert(v).co;
        sys.unlock(m);
        for (rhs, c) in co.iter().enumerate() {
            sys.set(rhs, m, f64::from(*c));
        }
    }

    // `init_laplacian_matrix`.
    for (ei, &e) in edges.iter().enumerate() {
        if bm.edge(e).flag.contains(ElemFlag::SELECT) || !bm.edge_is_boundary(e) {
            continue;
        }
        let [a, b] = bm.edge(e).v;
        let (v1, v2) = (bm.vert(a).co, bm.vert(b).co);
        let (idv1, idv2) = (vidx(bm, a), vidx(bm, b));
        let mut w1 = len_v3(sub(v1, v2));
        if w1 > MIN_AREA {
            w1 = 1.0 / w1;
            eweights[ei] = w1;
            vlengths[idv1] += w1;
            vlengths[idv2] += w1;
        } else {
            zerola[idv1] = true;
            zerola[idv2] = true;
        }
    }
    let faces = bm.face_ids();
    let mut l_curr_index = 0usize;
    for &f in &faces {
        let loops = bm.face_loops(f);
        if !bm.face(f).flag.contains(ElemFlag::SELECT) {
            l_curr_index += loops.len();
            continue;
        }
        for &l in &loops {
            frap_math::budget::check();
            let lp = bm.lp(l);
            let (vp, vc, vn) = (bm.lp(lp.prev).v, lp.v, bm.lp(lp.next).v);
            let (vi_prev, vi_curr, vi_next) = (vidx(bm, vp), vidx(bm, vc), vidx(bm, vn));
            let (co_prev, co_curr, co_next) = (bm.vert(vp).co, bm.vert(vc).co, bm.vert(vn).co);
            let areaf = area_tri_v3(co_prev, co_curr, co_next);
            if areaf < MIN_AREA {
                zerola[vi_curr] = true;
            }
            ring_areas[vi_prev] += areaf;
            ring_areas[vi_curr] += areaf;
            ring_areas[vi_next] += areaf;
            let w1 = cotangent_tri_weight_v3(co_curr, co_next, co_prev) / 2.0;
            let w2 = cotangent_tri_weight_v3(co_next, co_prev, co_curr) / 2.0;
            let w3 = cotangent_tri_weight_v3(co_prev, co_curr, co_next) / 2.0;
            fweights[l_curr_index][0] += w1;
            fweights[l_curr_index][1] += w2;
            fweights[l_curr_index][2] += w3;
            vweights[vi_prev] += w1 + w2;
            vweights[vi_curr] += w2 + w3;
            vweights[vi_next] += w1 + w3;
            l_curr_index += 1;
        }
    }

    let lambda_factor = i.lambda_factor;
    let lambda_border = i.lambda_border;
    for &v in &i.verts {
        let m = vidx(bm, v);
        let co = bm.vert(v).co;
        for (rhs, c) in co.iter().enumerate() {
            sys.rhs_add(rhs, m, f64::from(*c));
        }
        // A non-zero ring area: the vertex is in a selected face (wire edges become NaN
        // otherwise, #89214).
        if !zerola[m] && ring_areas[m] != 0.0 {
            let w = vweights[m] * ring_areas[m];
            vweights[m] = if w == 0.0 {
                0.0
            } else {
                -lambda_factor / (4.0 * w)
            };
            let w = vlengths[m];
            vlengths[m] = if w == 0.0 {
                0.0
            } else {
                -lambda_border * 2.0 / w
            };
            if !vert_is_boundary(bm, v) {
                sys.matrix_add(m, m, f64::from(1.0 + lambda_factor / (4.0 * ring_areas[m])));
            } else {
                sys.matrix_add(m, m, f64::from(1.0 + lambda_border * 2.0));
            }
        } else {
            sys.matrix_add(m, m, 1.0);
        }
    }

    // `fill_laplacian_matrix`.
    let ok = |bm: &BMesh, zerola: &[bool], v: VertId| -> bool {
        !zerola[vidx(bm, v)] && !vert_is_boundary(bm, v)
    };
    let mut l_curr_index = 0usize;
    for &f in &faces {
        let loops = bm.face_loops(f);
        if !bm.face(f).flag.contains(ElemFlag::SELECT) {
            l_curr_index += loops.len();
            continue;
        }
        let l0 = bm.lp(loops[0]);
        let mut vi_prev = vidx(bm, bm.lp(l0.prev).v);
        let mut vi_curr = vidx(bm, l0.v);
        let mut ok_prev = ok(bm, &zerola, bm.lp(l0.prev).v);
        let mut ok_curr = ok(bm, &zerola, l0.v);
        for &l in &loops {
            let vn = bm.lp(bm.lp(l).next).v;
            let vi_next = vidx(bm, vn);
            let ok_next = ok(bm, &zerola, vn);
            let fw = fweights[l_curr_index];
            if ok_prev {
                sys.matrix_add(vi_prev, vi_curr, f64::from(fw[1] * vweights[vi_prev]));
                sys.matrix_add(vi_prev, vi_next, f64::from(fw[0] * vweights[vi_prev]));
            }
            if ok_curr {
                sys.matrix_add(vi_curr, vi_next, f64::from(fw[2] * vweights[vi_curr]));
                sys.matrix_add(vi_curr, vi_prev, f64::from(fw[1] * vweights[vi_curr]));
            }
            if ok_next {
                sys.matrix_add(vi_next, vi_curr, f64::from(fw[2] * vweights[vi_next]));
                sys.matrix_add(vi_next, vi_prev, f64::from(fw[0] * vweights[vi_next]));
            }
            vi_prev = vi_curr;
            vi_curr = vi_next;
            ok_prev = ok_curr;
            ok_curr = ok_next;
            l_curr_index += 1;
        }
    }
    for (ei, &e) in edges.iter().enumerate() {
        if bm.edge(e).flag.contains(ElemFlag::SELECT) || !bm.edge_is_boundary(e) {
            continue;
        }
        let [a, b] = bm.edge(e).v;
        let (idv1, idv2) = (vidx(bm, a), vidx(bm, b));
        if !zerola[idv1] && !zerola[idv2] {
            sys.matrix_add(idv1, idv2, f64::from(eweights[ei] * vlengths[idv1]));
            sys.matrix_add(idv2, idv1, f64::from(eweights[ei] * vlengths[idv2]));
        }
    }

    if !sys.solve() {
        return;
    }

    // `validate_solution`.
    for &e in &edges {
        let [a, b] = bm.edge(e).v;
        let (idv1, idv2) = (vidx(bm, a), vidx(bm, b));
        let (vi1, vi2) = (bm.vert(a).co, bm.vert(b).co);
        let ve1 = [
            sys.get(0, idv1) as f32,
            sys.get(1, idv1) as f32,
            sys.get(2, idv1) as f32,
        ];
        let ve2 = [
            sys.get(0, idv2) as f32,
            sys.get(1, idv2) as f32,
            sys.get(2, idv2) as f32,
        ];
        let leni = len_v3(sub(vi1, vi2));
        let lene = len_v3(sub(ve1, ve2));
        if lene > leni * MAX_EDGE_PERCENTAGE || lene < leni * MIN_EDGE_PERCENTAGE {
            zerola[idv1] = true;
            zerola[idv2] = true;
        }
    }
    let vini = if i.preserve_volume {
        mesh_calc_volume(bm) as f32
    } else {
        0.0
    };
    for &v in &i.verts {
        let m = vidx(bm, v);
        if zerola[m] {
            continue;
        }
        let co = &mut bm.vert_mut(v).co;
        if i.use_x {
            co[0] = sys.get(0, m) as f32;
        }
        if i.use_y {
            co[1] = sys.get(1, m) as f32;
        }
        if i.use_z {
            co[2] = sys.get(2, m) as f32;
        }
    }
    if i.preserve_volume {
        let vend = mesh_calc_volume(bm) as f32;
        // `volume_preservation`.
        if vend != 0.0 {
            let beta = libm::powf(vini / vend, 1.0 / 3.0);
            for &v in &i.verts {
                let co = &mut bm.vert_mut(v).co;
                if i.use_x {
                    co[0] *= beta;
                }
                if i.use_y {
                    co[1] *= beta;
                }
                if i.use_z {
                    co[2] *= beta;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cotangent_weight_of_a_right_angle_is_zero() {
        assert_eq!(
            cotangent_tri_weight_v3([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            0.0
        );
        // 45 degrees: cot = 1.
        let w = cotangent_tri_weight_v3([0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]);
        assert!((w - 1.0).abs() < 1e-6);
    }
}
