// SPDX-License-Identifier: GPL-3.0-or-later
//! ABF++ (`p_abf_*` and `p_chart_abf_solve` of `uv_parametrizer.cc`, `method='ANGLE_BASED'`):
//! angle-based flattening of a chart's triangles. The angles start at the 3D angles (scaled so
//! that they sum to 2π around interior vertices) and Newton iterations on the Lagrangian with
//! the triangle, planarity and length (sine product) constraints move them, solving the
//! reduced `2 × #interior` system of the ABF++ paper each step with a sparse LU (Eigen's
//! `SparseLU` in Blender). The flattened angles go to LSCM (`PChart::abf_alpha`), which builds
//! the UVs from them; a chart whose system does not converge in 20 iterations, or whose matrix
//! is singular, falls back to the 3D angles (plain LSCM), as Blender does.
//!
//! The system state is single precision as Blender's (`float` arrays), the solve in double.
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc (Blender 5.2.4).

use crate::lscm::triangle_angles;
use crate::param::{NIL, PVERT_INTERIOR, ParamMesh};
use crate::slim::sparse::{Csc, lu_solve};

/// `ABF_MAX_ITER`.
const ABF_MAX_ITER: usize = 20;

/// `PAbfSystem`.
struct AbfSystem {
    ninterior: usize,
    alpha: Vec<f32>,
    beta: Vec<f32>,
    sine: Vec<f32>,
    cosine: Vec<f32>,
    weight: Vec<f32>,
    b_alpha: Vec<f32>,
    b_triangle: Vec<f32>,
    b_interior: Vec<f32>,
    lambda_triangle: Vec<f32>,
    lambda_planar: Vec<f32>,
    lambda_length: Vec<f32>,
    j2dt: Vec<[f32; 3]>,
    bstar: Vec<f32>,
    dstar: Vec<f32>,
}

impl AbfSystem {
    /// `p_abf_setup_system`.
    fn new(ninterior: usize, nfaces: usize, nangles: usize) -> AbfSystem {
        AbfSystem {
            ninterior,
            alpha: vec![0.0; nangles],
            beta: vec![0.0; nangles],
            sine: vec![0.0; nangles],
            cosine: vec![0.0; nangles],
            weight: vec![0.0; nangles],
            b_alpha: vec![0.0; nangles],
            b_triangle: vec![0.0; nfaces],
            b_interior: vec![0.0; 2 * ninterior],
            lambda_triangle: vec![0.0; nfaces],
            lambda_planar: vec![0.0; ninterior],
            lambda_length: vec![1.0; ninterior],
            j2dt: vec![[0.0; 3]; nangles],
            bstar: vec![0.0; nfaces],
            dstar: vec![0.0; nfaces],
        }
    }

    /// `p_abf_compute_sines`.
    fn compute_sines(&mut self) {
        for i in 0..self.alpha.len() {
            let angle = f64::from(self.alpha[i]);
            self.sine[i] = libm::sin(angle) as f32;
            self.cosine[i] = libm::cos(angle) as f32;
        }
    }

    /// `p_abf_adjust_alpha`.
    fn adjust_alpha(&mut self, id: usize, dlambda1: f32, pre: f32) {
        let mut alpha = self.alpha[id];
        let dalpha = self.b_alpha[id] - dlambda1;
        alpha += dalpha / self.weight[id] - pre;
        self.alpha[id] = alpha.clamp(0.0, core::f32::consts::PI);
    }
}

impl ParamMesh {
    /// The angle index (`e->u.id`) of an edge.
    #[inline]
    fn aid(&self, e: u32) -> usize {
        self.edges[e as usize].id as usize
    }

    /// `p_abf_compute_sin_product`.
    fn abf_sin_product(&self, sys: &AbfSystem, v: u32, aid: i64) -> f32 {
        let mut sin1 = 1.0f32;
        let mut sin2 = 1.0f32;
        let start = self.verts[v as usize].edge;
        let mut e = start;
        loop {
            let e1 = self.edges[e as usize].next;
            let e2 = self.edges[e1 as usize].next;
            let (id1, id2) = (self.aid(e1), self.aid(e2));
            if aid == id1 as i64 {
                // A derivative for this angle: cos, and drop the other part.
                sin1 *= sys.cosine[id1];
                sin2 = 0.0;
            } else {
                sin1 *= sys.sine[id1];
            }
            if aid == id2 as i64 {
                sin1 = 0.0;
                sin2 *= sys.cosine[id2];
            } else {
                sin2 *= sys.sine[id2];
            }
            e = self.edges[e2 as usize].pair;
            if e == NIL || e == start {
                break;
            }
        }
        sin1 - sin2
    }

    fn interior(&self, v: u32) -> bool {
        self.verts[v as usize].flag & PVERT_INTERIOR != 0
    }

    fn vid(&self, v: u32) -> usize {
        self.verts[v as usize].id as usize
    }

    /// `p_abf_compute_grad_alpha`.
    fn abf_grad_alpha(&self, sys: &AbfSystem, f: u32, e: u32) -> f32 {
        let v = self.edges[e as usize].vert;
        let en = self.edges[e as usize].next;
        let v1 = self.edges[en as usize].vert;
        let v2 = self.edges[self.edges[en as usize].next as usize].vert;
        let id = self.aid(e);
        let mut deriv = (sys.alpha[id] - sys.beta[id]) * sys.weight[id];
        deriv += sys.lambda_triangle[self.faces[f as usize].id as usize];
        if self.interior(v) {
            deriv += sys.lambda_planar[self.vid(v)];
        }
        if self.interior(v1) {
            let product = self.abf_sin_product(sys, v1, id as i64);
            deriv += sys.lambda_length[self.vid(v1)] * product;
        }
        if self.interior(v2) {
            let product = self.abf_sin_product(sys, v2, id as i64);
            deriv += sys.lambda_length[self.vid(v2)] * product;
        }
        deriv
    }

    /// `p_abf_compute_gradient`.
    fn abf_gradient(&self, sys: &mut AbfSystem, faces: &[u32], verts: &[u32]) -> f32 {
        let mut norm = 0.0f32;
        for &f in faces {
            let [e1, e2, e3] = self.face_edges(f);
            let galpha1 = self.abf_grad_alpha(sys, f, e1);
            let galpha2 = self.abf_grad_alpha(sys, f, e2);
            let galpha3 = self.abf_grad_alpha(sys, f, e3);
            sys.b_alpha[self.aid(e1)] = -galpha1;
            sys.b_alpha[self.aid(e2)] = -galpha2;
            sys.b_alpha[self.aid(e3)] = -galpha3;
            norm += galpha1 * galpha1 + galpha2 * galpha2 + galpha3 * galpha3;
            let gtriangle =
                sys.alpha[self.aid(e1)] + sys.alpha[self.aid(e2)] + sys.alpha[self.aid(e3)]
                    - core::f32::consts::PI;
            sys.b_triangle[self.faces[f as usize].id as usize] = -gtriangle;
            norm += gtriangle * gtriangle;
        }
        for &v in verts {
            if !self.interior(v) {
                continue;
            }
            let mut gplanar = (-2.0 * core::f64::consts::PI) as f32;
            let start = self.verts[v as usize].edge;
            let mut e = start;
            loop {
                gplanar += sys.alpha[self.aid(e)];
                e = self.wheel_edge_next(e);
                if e == NIL || e == start {
                    break;
                }
            }
            let vid = self.vid(v);
            sys.b_interior[vid] = -gplanar;
            norm += gplanar * gplanar;
            let glength = self.abf_sin_product(sys, v, -1);
            sys.b_interior[sys.ninterior + vid] = -glength;
            norm += glength * glength;
        }
        norm
    }

    /// `p_abf_matrix_invert`.
    fn abf_matrix_invert(&self, sys: &mut AbfSystem, faces: &[u32]) -> bool {
        let ninterior = sys.ninterior;
        let nvar = 2 * ninterior;
        let mut b = vec![0.0f64; nvar];
        for (i, bi) in b.iter_mut().enumerate() {
            *bi += f64::from(sys.b_interior[i]);
        }
        let mut trips: Vec<(usize, usize, f64)> = Vec::with_capacity(faces.len() * 27);
        for &f in faces {
            frap_math::budget::check();
            let [e1, e2, e3] = self.face_edges(f);
            let (a1, a2, a3) = (self.aid(e1), self.aid(e2), self.aid(e3));
            let fid = self.faces[f as usize].id as usize;
            let [v1, v2, v3] = self.face_verts(f);

            let wi1 = 1.0f32 / sys.weight[a1];
            let wi2 = 1.0f32 / sys.weight[a2];
            let wi3 = 1.0f32 / sys.weight[a3];

            // bstar1 = (J1*dInv*bAlpha - bTriangle)
            let mut bb = sys.b_alpha[a1] * wi1;
            bb += sys.b_alpha[a2] * wi2;
            bb += sys.b_alpha[a3] * wi3;
            bb -= sys.b_triangle[fid];

            // si = J1*d*J1t
            let si = 1.0f32 / (wi1 + wi2 + wi3);

            // J1t*si*bstar1 - bAlpha
            let beta = [
                bb * si - sys.b_alpha[a1],
                bb * si - sys.b_alpha[a2],
                bb * si - sys.b_alpha[a3],
            ];

            sys.bstar[fid] = bb;
            sys.dstar[fid] = si;

            let w = [
                [si - sys.weight[a1], si, si],
                [si, si - sys.weight[a2], si],
                [si, si, si - sys.weight[a3]],
            ];

            let mut vid = [-1i64; 6];
            let mut j2 = [[0.0f32; 3]; 3];
            let mut row1 = [0.0f32; 6];
            let mut row2 = [0.0f32; 6];
            let mut row3 = [0.0f32; 6];

            if self.interior(v1) {
                let id = self.vid(v1);
                vid[0] = id as i64;
                vid[3] = (ninterior + id) as i64;
                j2[0][0] = wi1;
                j2[1][0] = self.abf_sin_product(sys, v1, a2 as i64) * wi2;
                j2[2][0] = self.abf_sin_product(sys, v1, a3 as i64) * wi3;
                sys.j2dt[a1][0] = j2[0][0];
                sys.j2dt[a2][0] = j2[1][0];
                sys.j2dt[a3][0] = j2[2][0];
                b[id] += f64::from(j2[0][0] * beta[0]);
                b[ninterior + id] += f64::from(j2[1][0] * beta[1] + j2[2][0] * beta[2]);
                row1[0] = j2[0][0] * w[0][0];
                row2[0] = j2[0][0] * w[1][0];
                row3[0] = j2[0][0] * w[2][0];
                row1[3] = j2[1][0] * w[0][1] + j2[2][0] * w[0][2];
                row2[3] = j2[1][0] * w[1][1] + j2[2][0] * w[1][2];
                row3[3] = j2[1][0] * w[2][1] + j2[2][0] * w[2][2];
            }
            if self.interior(v2) {
                let id = self.vid(v2);
                vid[1] = id as i64;
                vid[4] = (ninterior + id) as i64;
                j2[0][1] = self.abf_sin_product(sys, v2, a1 as i64) * wi1;
                j2[1][1] = wi2;
                j2[2][1] = self.abf_sin_product(sys, v2, a3 as i64) * wi3;
                sys.j2dt[a1][1] = j2[0][1];
                sys.j2dt[a2][1] = j2[1][1];
                sys.j2dt[a3][1] = j2[2][1];
                b[id] += f64::from(j2[1][1] * beta[1]);
                b[ninterior + id] += f64::from(j2[0][1] * beta[0] + j2[2][1] * beta[2]);
                row1[1] = j2[1][1] * w[0][1];
                row2[1] = j2[1][1] * w[1][1];
                row3[1] = j2[1][1] * w[2][1];
                row1[4] = j2[0][1] * w[0][0] + j2[2][1] * w[0][2];
                row2[4] = j2[0][1] * w[1][0] + j2[2][1] * w[1][2];
                row3[4] = j2[0][1] * w[2][0] + j2[2][1] * w[2][2];
            }
            if self.interior(v3) {
                let id = self.vid(v3);
                vid[2] = id as i64;
                vid[5] = (ninterior + id) as i64;
                j2[0][2] = self.abf_sin_product(sys, v3, a1 as i64) * wi1;
                j2[1][2] = self.abf_sin_product(sys, v3, a2 as i64) * wi2;
                j2[2][2] = wi3;
                sys.j2dt[a1][2] = j2[0][2];
                sys.j2dt[a2][2] = j2[1][2];
                sys.j2dt[a3][2] = j2[2][2];
                b[id] += f64::from(j2[2][2] * beta[2]);
                b[ninterior + id] += f64::from(j2[0][2] * beta[0] + j2[1][2] * beta[1]);
                row1[2] = j2[2][2] * w[0][2];
                row2[2] = j2[2][2] * w[1][2];
                row3[2] = j2[2][2] * w[2][2];
                row1[5] = j2[0][2] * w[0][0] + j2[1][2] * w[0][1];
                row2[5] = j2[0][2] * w[1][0] + j2[1][2] * w[1][1];
                row3[5] = j2[0][2] * w[2][0] + j2[1][2] * w[2][1];
            }

            for i in 0..3 {
                if vid[i] < 0 {
                    continue;
                }
                let r = vid[i] as usize;
                for j in 0..6 {
                    if vid[j] < 0 {
                        continue;
                    }
                    let c = vid[j] as usize;
                    let rows = [row1[j], row2[j], row3[j]];
                    for (k, &rv) in rows.iter().enumerate() {
                        let row = if i == k { r } else { r + ninterior };
                        trips.push((row, c, f64::from(j2[k][i] * rv)));
                    }
                }
            }
        }

        let m = Csc::from_triplets(nvar, nvar, &trips);
        let Some(mut x) = lu_solve(&m, &[&b]) else {
            return false;
        };
        let x = x.remove(0);
        let get = |i: usize| x[i] as f32;

        for &f in faces {
            let [e1, e2, e3] = self.face_edges(f);
            let (a1, a2, a3) = (self.aid(e1), self.aid(e2), self.aid(e3));
            let fid = self.faces[f as usize].id as usize;
            let [v1, v2, v3] = self.face_verts(f);
            let mut pre = [0.0f32; 3];
            if self.interior(v1) {
                let id = self.vid(v1);
                let (x1, x2) = (get(id), get(ninterior + id));
                pre[0] += sys.j2dt[a1][0] * x1;
                pre[1] += sys.j2dt[a2][0] * x2;
                pre[2] += sys.j2dt[a3][0] * x2;
            }
            if self.interior(v2) {
                let id = self.vid(v2);
                let (x1, x2) = (get(id), get(ninterior + id));
                pre[0] += sys.j2dt[a1][1] * x2;
                pre[1] += sys.j2dt[a2][1] * x1;
                pre[2] += sys.j2dt[a3][1] * x2;
            }
            if self.interior(v3) {
                let id = self.vid(v3);
                let (x1, x2) = (get(id), get(ninterior + id));
                pre[0] += sys.j2dt[a1][2] * x2;
                pre[1] += sys.j2dt[a2][2] * x2;
                pre[2] += sys.j2dt[a3][2] * x1;
            }
            let mut dlambda1 = pre[0] + pre[1] + pre[2];
            dlambda1 = sys.dstar[fid] * (sys.bstar[fid] - dlambda1);
            sys.lambda_triangle[fid] += dlambda1;
            sys.adjust_alpha(a1, dlambda1, pre[0]);
            sys.adjust_alpha(a2, dlambda1, pre[1]);
            sys.adjust_alpha(a3, dlambda1, pre[2]);
        }
        for i in 0..ninterior {
            sys.lambda_planar[i] += x[i] as f32;
            sys.lambda_length[i] += x[ninterior + i] as f32;
        }
        true
    }

    /// `p_vert_interior`: the vertex is not on a boundary.
    pub(crate) fn vert_interior(&self, v: u32) -> bool {
        let e = self.verts[v as usize].edge;
        self.edges[e as usize].pair != NIL
    }

    /// `p_chart_abf_solve`: flattens the chart's angles into `abf_alpha`. `false` when the
    /// system cannot be solved (LSCM then uses the 3D angles).
    pub(crate) fn chart_abf_solve(&mut self, c: usize) -> bool {
        let limit = if self.charts[c].nfaces > 100 {
            1.0f32
        } else {
            0.001f32
        };
        let verts = self.chart_vert_list(c);
        let faces = self.chart_face_list(c);

        // Set up the ids.
        let mut ninterior = 0usize;
        for &v in &verts {
            if self.vert_interior(v) {
                self.verts[v as usize].flag |= PVERT_INTERIOR;
                self.verts[v as usize].id = ninterior as i32;
                ninterior += 1;
            } else {
                self.verts[v as usize].flag &= !PVERT_INTERIOR;
            }
        }
        let mut nangles = 0i32;
        for (fi, &f) in faces.iter().enumerate() {
            self.faces[f as usize].id = fi as i32;
            for e in self.face_edges(f) {
                self.edges[e as usize].id = nangles;
                nangles += 1;
            }
        }
        let mut sys = AbfSystem::new(ninterior, faces.len(), nangles as usize);

        // Initial angles.
        for &f in &faces {
            let [e1, e2, e3] = self.face_edges(f);
            let [v1, v2, v3] = self.face_verts(f);
            let (a1, a2, a3) = triangle_angles(
                self.verts[v1 as usize].co,
                self.verts[v2 as usize].co,
                self.verts[v3 as usize].co,
            );
            for (e, a) in [(e1, a1), (e2, a2), (e3, a3)] {
                let id = self.aid(e);
                sys.alpha[id] = a as f32;
                sys.beta[id] = a as f32;
                sys.weight[id] = (2.0 / (a * a)) as f32;
            }
        }

        for &v in &verts {
            if !self.interior(v) {
                continue;
            }
            let start = self.verts[v as usize].edge;
            let mut anglesum = 0.0f32;
            let mut e = start;
            loop {
                anglesum += sys.beta[self.aid(e)];
                e = self.wheel_edge_next(e);
                if e == NIL || e == start {
                    break;
                }
            }
            let scale = if anglesum == 0.0 {
                0.0
            } else {
                2.0f32 * core::f32::consts::PI / anglesum
            };
            let mut e = start;
            loop {
                let id = self.aid(e);
                let value = sys.beta[id] * scale;
                sys.beta[id] = value;
                sys.alpha[id] = value;
                e = self.wheel_edge_next(e);
                if e == NIL || e == start {
                    break;
                }
            }
        }

        if sys.ninterior > 0 {
            sys.compute_sines();
            let mut i = 0;
            while i < ABF_MAX_ITER {
                frap_math::budget::check();
                let norm = self.abf_gradient(&mut sys, &faces, &verts);
                if norm < limit {
                    break;
                }
                if !self.abf_matrix_invert(&mut sys, &faces) {
                    // "ABF failed to invert matrix"
                    return false;
                }
                sys.compute_sines();
                i += 1;
            }
            if i == ABF_MAX_ITER {
                // "ABF maximum iterations reached"
                return false;
            }
        }

        self.charts[c].abf_alpha = Some(sys.alpha);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::param::{ConstructOpts, ParamFace, ParamMesh};

    /// A 3 × 3 grid of quads (4 × 4 vertices) bent along x: one interior-rich chart.
    fn bent_grid(pm: &mut ParamMesh) {
        let n = 4usize;
        let co = |i: usize, j: usize| -> [f32; 3] {
            let x = i as f32 / 3.0;
            [x, j as f32 / 3.0, 0.4 * libm::sinf(x * 3.0)]
        };
        let mut slot = 0u32;
        let mut key = 0u64;
        for i in 0..n - 1 {
            for j in 0..n - 1 {
                let ids = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                let vkeys: Vec<u64> = ids.iter().map(|&(a, b)| (a * n + b) as u64).collect();
                let cos: Vec<[f32; 3]> = ids.iter().map(|&(a, b)| co(a, b)).collect();
                let uvi: Vec<u32> = (0..4).map(|k| slot + k).collect();
                slot += 4;
                let uv = vec![[0.0f32; 2]; 4];
                let pin = vec![false; 4];
                let sel = vec![true; 4];
                let f = ParamFace::new(key, &vkeys, &cos, &uvi, &uv, &pin, &sel);
                key += 1;
                pm.face_add(&f);
            }
        }
    }

    #[test]
    fn abf_keeps_angle_sums_and_flattens() {
        let mut pm = ParamMesh::new();
        bent_grid(&mut pm);
        pm.construct_end(ConstructOpts::new(false, false));
        assert_eq!(pm.chart_count(), 1);
        assert!(pm.chart_abf_solve(0));
        let alpha = pm.charts[0].abf_alpha.clone().unwrap();
        // Triangles sum to π, interior vertices to 2π.
        for t in alpha.chunks(3) {
            let s: f32 = t.iter().sum();
            assert!((s - core::f32::consts::PI).abs() < 1e-3, "{s}");
        }
        for v in pm.chart_vert_list(0) {
            if !pm.vert_interior(v) {
                continue;
            }
            let start = pm.verts[v as usize].edge;
            let mut e = start;
            let mut s = 0.0f32;
            loop {
                s += alpha[pm.edges[e as usize].id as usize];
                e = pm.wheel_edge_next(e);
                if e == super::NIL || e == start {
                    break;
                }
            }
            assert!((s - 2.0 * core::f32::consts::PI).abs() < 1e-3, "{s}");
        }
    }

    #[test]
    fn abf_unwrap_is_developable_flat() {
        // A bent strip is developable: ABF++ + LSCM must give the same edge lengths as 3D.
        let mut pm = ParamMesh::new();
        bent_grid(&mut pm);
        pm.construct_end(ConstructOpts::new(false, false));
        pm.lscm_begin(false, true, false);
        let c = pm.lscm_solve();
        pm.lscm_end();
        assert_eq!(c.changed, 1);
        let mut ratio = None;
        for e in pm.chart_edge_list(0) {
            let a = pm.edges[e as usize].vert;
            let b = pm.edges[pm.edges[e as usize].next as usize].vert;
            let l3 = crate::param::bli::len_v3v3(pm.verts[a as usize].co, pm.verts[b as usize].co);
            let l2 = crate::param::bli::len_v2v2(pm.verts[a as usize].uv, pm.verts[b as usize].uv);
            let r = l2 / l3;
            let r0 = *ratio.get_or_insert(r);
            assert!((r - r0).abs() < 2e-3 * r0, "{r} vs {r0}");
        }
    }
}
