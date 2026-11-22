// SPDX-License-Identifier: GPL-3.0-or-later
//! The SLIM solver ("Scalable Locally Injective Maps", Rabinovich et al. 2016) as Blender's
//! `intern/slim` runs it: the symmetric Dirichlet energy, per-triangle closest rotations and
//! weights from a 2 × 2 polar SVD (local step), the weighted proxy (global step: a sparse
//! symmetric system with a proximal term and soft pin constraints), and the flip-avoiding
//! line search between the current and the proxy's map. With pins the global scale invariance
//! factor rescales the gradient operators every iteration.
//!
//! The proxy system is factored with a sparse Cholesky (Eigen's `SimplicialLDLT` in Blender):
//! equal solutions within rounding, so results agree within a tolerance (WP 8.1: 1e-4).
//!
//! Ported from blender/intern/slim/intern/{slim.cpp, flip_avoiding_line_search.cpp,
//! doublearea.cpp} (Blender 5.2.4; `slim.cpp` derives from libigl, MPL-2.0).

use super::SlimFailed;
use super::sparse::{Csc, llt_solve};
use super::svd::{Mat2, det, mul, svd2, transpose};

/// One row of a surface gradient operator: `(column, value)` for the triangle's vertices.
type GradRow = [(usize, f64); 3];

/// `SLIMData` (only the symmetric Dirichlet energy, the one Blender uses).
#[derive(Clone, Debug, Default)]
pub(crate) struct SlimData {
    pub(crate) valid: bool,
    /// Vertex positions (#V × 3).
    pub(crate) v: Vec<[f64; 3]>,
    /// Triangles (#F × 3).
    pub(crate) f: Vec<[usize; 3]>,
    /// Soft-constrained (pinned) vertices.
    pub(crate) b: Vec<usize>,
    /// Their target positions.
    pub(crate) bc: Vec<[f64; 2]>,
    pub(crate) soft_const_p: f64,
    pub(crate) exp_factor: f64,
    pub(crate) reflection_mode: i32,
    pub(crate) skip_initialization: bool,
    pub(crate) expected_surface_area_of_resulting_map: f64,
    /// The map (#V × 2).
    pub(crate) v_o: Vec<[f64; 2]>,
    pub(crate) old_uvs: Vec<[f64; 2]>,
    pub(crate) with_weighted_parameterization: bool,
    pub(crate) weightmap: Vec<f32>,
    pub(crate) weight_per_face_map: Vec<f32>,
    pub(crate) weight_influence: f64,
    pub(crate) global_scale_invariance_factor: f64,
    pub(crate) energy: f64,
    /// Triangle areas.
    m: Vec<f64>,
    mesh_area: f64,
    v_num: usize,
    f_num: usize,
    proximal_p: f64,
    /// `Ji` rows: `[D1 u, D2 u, D1 v, D2 v]`.
    ji: Vec<[f64; 4]>,
    /// `Ri` rows: the closest rotations, column-major `[r00, r10, r01, r11]`.
    ri: Vec<[f64; 4]>,
    /// `W_11, W_12, W_21, W_22`.
    w: Vec<[f64; 4]>,
    /// Rows of the surface gradient operators `Dx`, `Dy` (three entries, column order).
    dx: Vec<GradRow>,
    dy: Vec<GradRow>,
    has_pre_calc: bool,
}

impl SlimData {
    pub(crate) fn new() -> SlimData {
        SlimData {
            valid: true,
            global_scale_invariance_factor: 1.0,
            ..SlimData::default()
        }
    }
}

/// `doublearea(V, F)` of a 3D mesh: twice the triangle areas.
pub(crate) fn doublearea3(v: &[[f64; 3]], f: &[[usize; 3]]) -> Vec<f64> {
    f.iter()
        .map(|t| {
            let mut s = 0.0f64;
            for d in 0..3 {
                let (x, y) = (d, (d + 1) % 3);
                let rx = v[t[0]][x] - v[t[2]][x];
                let sx = v[t[1]][x] - v[t[2]][x];
                let ry = v[t[0]][y] - v[t[2]][y];
                let sy = v[t[1]][y] - v[t[2]][y];
                let dbl = rx * sy - ry * sx;
                s += dbl * dbl;
            }
            s.sqrt()
        })
        .collect()
}

/// `doublearea(V, F)` of a 2D map: twice the signed triangle areas.
pub(crate) fn doublearea2(v: &[[f64; 2]], f: &[[usize; 3]]) -> Vec<f64> {
    f.iter()
        .map(|t| {
            let rx = v[t[0]][0] - v[t[2]][0];
            let sx = v[t[1]][0] - v[t[2]][0];
            let ry = v[t[0]][1] - v[t[2]][1];
            let sy = v[t[1]][1] - v[t[2]][1];
            rx * sy - ry * sx
        })
        .collect()
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn scale3(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn div3(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] / s, a[1] / s, a[2] / s]
}

/// Eigen's `normalized()`.
fn normalized(a: [f64; 3]) -> [f64; 3] {
    let z = dot3(a, a);
    if z > 0.0 { div3(a, z.sqrt()) } else { a }
}

/// `polar_svd`: `(R, U, S)` with `R` the closest rotation (never a reflection).
fn polar_svd(a: &Mat2) -> (Mat2, Mat2, [f64; 2]) {
    let (u, s, v) = svd2(a);
    let mut r = mul(&u, &transpose(&v));
    if det(&r) < 0.0 {
        let mut w = v;
        w[0][1] = -w[0][1];
        w[1][1] = -w[1][1];
        r = mul(&u, &transpose(&w));
    }
    (r, u, s)
}

impl SlimData {
    /// `grad` + `local_basis` + `compute_surface_gradient_matrix`: the rows of `Dx`, `Dy`.
    fn surface_gradient(&mut self) {
        let nf = self.f.len();
        self.dx = Vec::with_capacity(nf);
        self.dy = Vec::with_capacity(nf);
        for t in &self.f {
            let (i1, i2, i3) = (t[0], t[1], t[2]);
            let (p1, p2, p3) = (self.v[i1], self.v[i2], self.v[i3]);
            // `grad`.
            let v32 = sub3(p3, p2);
            let v13 = sub3(p1, p3);
            let v21 = sub3(p2, p1);
            let n = cross3(v32, v13);
            let dbl_a = dot3(n, n).sqrt();
            let u = div3(n, dbl_a);
            let norm21 = dot3(v21, v21).sqrt();
            let norm13 = dot3(v13, v13).sqrt();
            let mut eperp21 = cross3(u, v21);
            eperp21 = div3(eperp21, dot3(eperp21, eperp21).sqrt());
            eperp21 = scale3(eperp21, norm21 / dbl_a);
            let mut eperp13 = cross3(u, v13);
            eperp13 = div3(eperp13, dot3(eperp13, eperp13).sqrt());
            eperp13 = scale3(eperp13, norm13 / dbl_a);
            // G(r, F1) = eperp13[r], G(r, F0) = -eperp13[r] - eperp21[r], G(r, F2) = eperp21[r]
            // (`setFromTriplets` sums the two F0 entries in that order).
            let g = |d: usize| -> [(usize, f64); 3] {
                [
                    (i2, eperp13[d]),
                    (i1, -eperp13[d] + -eperp21[d]),
                    (i3, eperp21[d]),
                ]
            };
            let (gx, gy, gz) = (g(0), g(1), g(2));
            // `local_basis`.
            let b1 = normalized(sub3(p2, p1));
            let tt = sub3(p3, p1);
            let b3 = normalized(cross3(b1, tt));
            let b2 = normalized(cross3(b1, b3));
            let f1 = b1;
            let f2 = [-b2[0], -b2[1], -b2[2]];
            let combine = |fb: [f64; 3]| -> [(usize, f64); 3] {
                let mut row = [(0usize, 0.0f64); 3];
                for k in 0..3 {
                    // (diag(a) Dx + diag(b) Dy) + diag(c) Dz.
                    row[k] = (gx[k].0, fb[0] * gx[k].1 + fb[1] * gy[k].1 + fb[2] * gz[k].1);
                }
                row.sort_by_key(|e| e.0);
                row
            };
            self.dx.push(combine(f1));
            self.dy.push(combine(f2));
        }
    }

    /// `compute_jacobians` (weighted or not) into `ji`.
    fn compute_jacobians(&mut self, uv: &[[f64; 2]]) {
        let nf = self.f.len();
        self.ji.resize(nf, [0.0; 4]);
        for i in 0..nf {
            let mut j = [0.0f64; 4];
            for &(c, val) in &self.dx[i] {
                j[0] += val * uv[c][0];
                j[2] += val * uv[c][1];
            }
            for &(c, val) in &self.dy[i] {
                j[1] += val * uv[c][0];
                j[3] += val * uv[c][1];
            }
            if self.with_weighted_parameterization {
                let w = f64::from(self.weight_per_face_map[i]);
                for x in &mut j {
                    *x *= w;
                }
            }
            self.ji[i] = j;
        }
    }

    /// `update_weights_and_closest_rotations` (symmetric Dirichlet).
    fn update_weights_and_closest_rotations(&mut self, uv: &[[f64; 2]]) {
        self.compute_jacobians(uv);
        let eps = 1e-8;
        let nf = self.f.len();
        self.w.resize(nf, [0.0; 4]);
        self.ri.resize(nf, [0.0; 4]);
        for i in 0..nf {
            let j = self.ji[i];
            let ji: Mat2 = [[j[0], j[1]], [j[2], j[3]]];
            let (ri, ui, sing) = polar_svd(&ji);
            let (s1, s2) = (sing[0], sing[1]);
            let s1_g = 2.0 * (s1 - libm::pow(s1, -3.0));
            let s2_g = 2.0 * (s2 - libm::pow(s2, -3.0));
            let mut m_sing_new = [
                (s1_g / (2.0 * (s1 - 1.0))).sqrt(),
                (s2_g / (2.0 * (s2 - 1.0))).sqrt(),
            ];
            if (s1 - 1.0).abs() < eps {
                m_sing_new[0] = 1.0;
            }
            if (s2 - 1.0).abs() < eps {
                m_sing_new[1] = 1.0;
            }
            let d = [[m_sing_new[0], 0.0], [0.0, m_sing_new[1]]];
            let mat_w = mul(&mul(&ui, &d), &transpose(&ui));
            self.w[i] = [mat_w[0][0], mat_w[0][1], mat_w[1][0], mat_w[1][1]];
            self.ri[i] = [ri[0][0], ri[1][0], ri[0][1], ri[1][1]];
        }
    }

    /// `pre_calc`.
    fn pre_calc(&mut self) {
        if self.has_pre_calc {
            return;
        }
        self.surface_gradient();
        self.has_pre_calc = true;
    }

    /// `build_linear_system` + `buildRhs` + `add_soft_constraints`: `(L, rhs)`.
    fn build_linear_system(&self) -> (Csc, Vec<f64>) {
        let vn = self.v_num;
        let nf = self.f_num;
        let n = 2 * vn;
        let mut trips: Vec<(usize, usize, f64)> = Vec::with_capacity(nf * 84 + n);
        let mut rhs = vec![0.0f64; n];
        for i in 0..nf {
            let weight = f64::from(self.weight_per_face_map[i]);
            let [w11, w12, w21, w22] = self.w[i];
            let r = self.ri[i];
            let m = self.m[i];
            // Rows of A (formula (35)) with their right-hand side (formula (36)).
            let blocks: [(&GradRow, f64, f64, f64); 4] = [
                (&self.dx[i], w11, w12, w11 * r[0] + w12 * r[1]),
                (&self.dy[i], w11, w12, w11 * r[2] + w12 * r[3]),
                (&self.dx[i], w21, w22, w21 * r[0] + w22 * r[1]),
                (&self.dy[i], w21, w22, w21 * r[2] + w22 * r[3]),
            ];
            for (d, wa, wb, f_rhs) in blocks {
                let mut row = [(0usize, 0.0f64); 6];
                for k in 0..3 {
                    let (c, val) = d[k];
                    row[k] = (c, weight * val * wa);
                    row[3 + k] = (vn + c, weight * val * wb);
                }
                for &(ca, va) in &row {
                    rhs[ca] += va * (m * f_rhs);
                    for &(cb, vb) in &row {
                        if ca >= cb {
                            trips.push((ca, cb, va * m * vb));
                        }
                    }
                }
            }
        }
        for (k, x) in rhs.iter_mut().enumerate() {
            let uv = if k < vn {
                self.v_o[k][0]
            } else {
                self.v_o[k - vn][1]
            };
            *x += self.proximal_p * uv;
            trips.push((k, k, self.proximal_p));
        }
        for d in 0..2 {
            for (i, &v_idx) in self.b.iter().enumerate() {
                rhs[d * vn + v_idx] += self.soft_const_p * self.bc[i][d];
                trips.push((d * vn + v_idx, d * vn + v_idx, self.soft_const_p));
            }
        }
        (Csc::from_triplets(n, n, &trips), rhs)
    }

    /// `solve_weighted_arap`: the proxy map.
    fn solve_weighted_arap(&self) -> Result<Vec<[f64; 2]>, SlimFailed> {
        let (l, rhs) = self.build_linear_system();
        let mut uc = llt_solve(&l, &rhs).ok_or(SlimFailed)?;
        // A pinned unknown's row is dominated by the penalty (1e101): in Eigen's LDLᵀ its
        // value is the penalised right-hand side times the inverse of the penalised diagonal
        // (`m_diag.asDiagonal().inverse() * dest`; the other terms vanish below its
        // precision), so the pin lands on `fl(fl(p bc) fl(1 / p))`, often `bc` exactly. The
        // Cholesky factor divides twice by sqrt(p) and lands elsewhere within an ulp, which the
        // soft-constraint energy (p |bc - x|², ~1e69 for one ulp) turns into the noise that
        // decides the line search. Reproduce the LDLᵀ value.
        let vn = self.v_num;
        for d in 0..2 {
            for &v_idx in &self.b {
                let k = d * vn + v_idx;
                uc[k] = rhs[k] * (1.0 / l.coeff(k, k));
            }
        }
        if !uc.iter().all(|x| x.is_finite()) {
            return Err(SlimFailed);
        }
        Ok((0..vn).map(|i| [uc[i], uc[vn + i]]).collect())
    }

    /// `compute_energy` (+ the soft constraint energy); fills `singular_values` when given.
    fn compute_energy(&mut self, v_new: &[[f64; 2]], mut sing: Option<&mut Vec<f64>>) -> f64 {
        self.compute_jacobians(v_new);
        let nf = self.f_num;
        if let Some(s) = sing.as_deref_mut() {
            s.resize(2 * nf, 0.0);
        }
        let mut energy = 0.0f64;
        for i in 0..nf {
            let j = self.ji[i];
            let (_, _, sv) = polar_svd(&[[j[0], j[1]], [j[2], j[3]]]);
            let (s1, s2) = (sv[0], sv[1]);
            energy += self.m[i]
                * (libm::pow(s1, 2.0)
                    + libm::pow(s1, -2.0)
                    + libm::pow(s2, 2.0)
                    + libm::pow(s2, -2.0));
            if let Some(s) = sing.as_deref_mut() {
                s[i] = s1;
                s[i + nf] = s2;
            }
        }
        let mut e = 0.0f64;
        for (i, &bi) in self.b.iter().enumerate() {
            let d0 = self.bc[i][0] - v_new[bi][0];
            let d1 = self.bc[i][1] - v_new[bi][1];
            e += self.soft_const_p * (d0 * d0 + d1 * d1);
        }
        energy + e
    }
}

/// `slim_precompute`.
pub(crate) fn slim_precompute(data: &mut SlimData) {
    debug_assert!(data.valid);
    data.v_num = data.v.len();
    data.f_num = data.f.len();
    data.proximal_p = 0.0001;
    data.m = doublearea3(&data.v, &data.f)
        .into_iter()
        .map(|a| a / 2.0)
        .collect();
    data.mesh_area = data.m.iter().sum();
    data.exp_factor = 1.0;
    data.pre_calc();
    let v_o = data.v_o.clone();
    data.energy = data.compute_energy(&v_o, None) / data.mesh_area;
}

/// `computeGlobalScaleInvarianceFactor`.
fn global_scale_invariance_factor(sing: &[f64], areas: &[f64]) -> f64 {
    let nf = sing.len() / 2;
    let (mut s1, mut s2, mut t1, mut t2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..nf {
        let (a, b) = (sing[i], sing[nf + i]);
        s1 += a * a * areas[i];
        s2 += b * b * areas[i];
        t1 += (1.0 / (a * a)) * areas[i];
        t2 += (1.0 / (b * b)) * areas[i];
    }
    let a = s1 + s2;
    let b = t1 + t2;
    let x = (b / a).sqrt().sqrt();
    1.0 / x
}

/// `get_smallest_pos_quad_zero`.
fn smallest_pos_quad_zero(a: f64, b: f64, c: f64) -> Result<f64, SlimFailed> {
    let (mut t1, mut t2);
    if a != 0.0 {
        let delta_in = b * b - 4.0 * a * c;
        if delta_in < 0.0 {
            return Ok(f64::INFINITY);
        }
        let delta = delta_in.sqrt();
        t1 = (-b + delta) / (2.0 * a);
        t2 = (-b - delta) / (2.0 * a);
    } else {
        // Blender's formula (`-b / c`, not `-c / b`).
        t1 = -b / c;
        t2 = t1;
    }
    if !t1.is_finite() || !t2.is_finite() {
        return Err(SlimFailed);
    }
    let tmp_n = t1.min(t2);
    t1 = t1.max(t2);
    t2 = tmp_n;
    if t1 == t2 {
        // The orientation flips twice: it does not flip.
        return Ok(f64::INFINITY);
    }
    if t1 > 0.0 {
        if t2 > 0.0 {
            return Ok(t2);
        }
        return Ok(t1);
    }
    Ok(f64::INFINITY)
}

/// `compute_max_step_from_singularities`.
fn max_step_from_singularities(
    uv: &[[f64; 2]],
    f: &[[usize; 3]],
    d: &[[f64; 2]],
) -> Result<f64, SlimFailed> {
    let mut max_step = f64::INFINITY;
    for t in f {
        let (v1, v2, v3) = (t[0], t[1], t[2]);
        let (u11, u12, u21, u22, u31, u32) = (
            uv[v1][0], uv[v1][1], uv[v2][0], uv[v2][1], uv[v3][0], uv[v3][1],
        );
        let (v11, v12, v21, v22, v31, v32) =
            (d[v1][0], d[v1][1], d[v2][0], d[v2][1], d[v3][0], d[v3][1]);
        let a = v11 * v22 - v12 * v21 - v11 * v32 + v12 * v31 + v21 * v32 - v22 * v31;
        let b = u11 * v22 - u12 * v21 - u21 * v12 + u22 * v11 - u11 * v32 + u12 * v31 + u31 * v12
            - u32 * v11
            + u21 * v32
            - u22 * v31
            - u31 * v22
            + u32 * v21;
        let c = u11 * u22 - u12 * u21 - u11 * u32 + u12 * u31 + u21 * u32 - u22 * u31;
        max_step = max_step.min(smallest_pos_quad_zero(a, b, c)?);
    }
    Ok(max_step)
}

/// `slim_solve(data, iter_num)`: `iter_num` iterations on `data.v_o`.
pub(crate) fn slim_solve(data: &mut SlimData, iter_num: i32) -> Result<(), SlimFailed> {
    debug_assert!(data.valid);
    let pins = !data.b.is_empty();
    let mut sing: Vec<f64> = Vec::new();
    if pins {
        let v_o = data.v_o.clone();
        data.energy = data.compute_energy(&v_o, Some(&mut sing)) / data.mesh_area;
    }
    for _ in 0..iter_num.max(0) {
        frap_math::budget::check();
        let cur = data.v_o.clone();
        // Solve the weighted proxy.
        data.update_weights_and_closest_rotations(&cur);
        let dest = data.solve_weighted_arap()?;

        // `flip_avoiding_line_search`.
        let d: Vec<[f64; 2]> = dest
            .iter()
            .zip(&cur)
            .map(|(a, b)| [a[0] - b[0], a[1] - b[1]])
            .collect();
        let min_step = max_step_from_singularities(&cur, &data.f, &d)?;
        let mut step = 1.0f64.min(min_step * 0.8);
        // `line_search` (the current energy is given when positive).
        let cur_energy = data.energy * data.mesh_area;
        let old_energy = if cur_energy > 0.0 {
            cur_energy
        } else {
            data.compute_energy(&cur, if pins { Some(&mut sing) } else { None })
        };
        let mut x = cur;
        let mut new_energy = old_energy;
        let mut it = 0;
        while new_energy >= old_energy && it < 12 {
            let new_x: Vec<[f64; 2]> = x
                .iter()
                .zip(&d)
                .map(|(p, dd)| [p[0] + step * dd[0], p[1] + step * dd[1]])
                .collect();
            let e = data.compute_energy(&new_x, if pins { Some(&mut sing) } else { None });
            if e >= old_energy {
                step /= 2.0;
            } else {
                x = new_x;
                new_energy = e;
            }
            it += 1;
        }
        data.v_o = x;
        data.energy = new_energy / data.mesh_area;

        if pins {
            data.global_scale_invariance_factor = global_scale_invariance_factor(&sing, &data.m);
            let f = data.global_scale_invariance_factor;
            for row in data.dx.iter_mut().chain(data.dy.iter_mut()) {
                for e in row.iter_mut() {
                    e.1 /= f;
                }
            }
            let v_o = data.v_o.clone();
            data.energy = data.compute_energy(&v_o, Some(&mut sing)) / data.mesh_area;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit square split into two triangles, mapped with a stretch.
    fn square() -> SlimData {
        let mut d = SlimData::new();
        d.v = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        d.f = vec![[0, 1, 2], [0, 2, 3]];
        d.v_o = vec![[0.0, 0.0], [2.0, 0.0], [2.0, 0.5], [0.0, 0.5]];
        d.weight_per_face_map = vec![1.0; 2];
        d.soft_const_p = 1e101;
        d
    }

    #[test]
    fn energy_is_four_at_isometry() {
        let mut d = square();
        d.v_o = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        slim_precompute(&mut d);
        // Symmetric Dirichlet: s² + s⁻² = 2 per singular value at s = 1.
        assert!((d.energy - 4.0).abs() < 1e-12, "{}", d.energy);
    }

    #[test]
    fn iterations_reduce_energy_towards_isometry() {
        let mut d = square();
        slim_precompute(&mut d);
        let e0 = d.energy;
        slim_solve(&mut d, 20).unwrap();
        assert!(d.energy < e0);
        assert!(d.energy - 4.0 < 1e-3, "{}", d.energy);
        let l = |a: usize, b: usize| {
            let (p, q) = (d.v_o[a], d.v_o[b]);
            ((p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])).sqrt()
        };
        assert!((l(0, 1) - 1.0).abs() < 1e-2 && (l(1, 2) - 1.0).abs() < 1e-2);
    }

    #[test]
    fn quad_zero_matches_blender_cases() {
        assert_eq!(smallest_pos_quad_zero(1.0, -3.0, 2.0).unwrap(), 1.0);
        assert_eq!(
            smallest_pos_quad_zero(1.0, 0.0, 1.0).unwrap(),
            f64::INFINITY
        );
        assert!(smallest_pos_quad_zero(0.0, 0.0, 0.0).is_err());
        assert_eq!(
            smallest_pos_quad_zero(0.0, 0.0, 1.0).unwrap(),
            f64::INFINITY
        );
    }
}
