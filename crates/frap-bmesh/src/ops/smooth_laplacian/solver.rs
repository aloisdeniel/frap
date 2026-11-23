// SPDX-License-Identifier: GPL-3.0-or-later
//! `EIG_linear_least_squares_solver_*` as `bmo_smooth_laplacian_vert` uses it: `rows × vars`
//! with three right-hand sides, variables locked or unlocked before the matrix is built.
//!
//! Ported from blender/intern/eigen/intern/linear_solver.cc (Blender 5.2.4,
//! GPL-2.0-or-later): coefficients on a locked column go to that variable's list (moved to the
//! right-hand side times the variable's value at solve time), the others to `M` (rows by
//! their raw index, columns by the unlocked index); duplicates are summed. Blender factors
//! `MᵀM` with Eigen's `SparseLU` and solves `MᵀM x = Mᵀb` in double precision; frap factors the
//! same `MᵀM` with `faer`'s sparse LU (partial pivoting), so the solution agrees to double
//! rounding, far below the T2 tolerance of 1e-5 on the `f32` positions.

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use std::collections::HashMap;

/// One variable (`LinearSolver::Variable`): value per right-hand side, lock, unlocked index,
/// and the coefficients that reference it while locked.
#[derive(Clone, Debug, Default)]
struct Variable {
    value: [f64; 3],
    locked: bool,
    index: usize,
    a: Vec<(usize, f64)>,
}

/// `LinearSolver` (least squares, three right-hand sides).
pub(crate) struct LeastSquares {
    num_rows: usize,
    variables: Vec<Variable>,
    constructing: bool,
    n: usize,
    triplets: Vec<(usize, usize, f64)>,
    b: [Vec<f64>; 3],
}

impl LeastSquares {
    /// `EIG_linear_least_squares_solver_new(num_rows, num_columns, 3)`.
    pub(crate) fn new(num_rows: usize, num_columns: usize) -> Self {
        LeastSquares {
            num_rows,
            variables: vec![Variable::default(); num_columns],
            constructing: false,
            n: 0,
            triplets: Vec::new(),
            b: [Vec::new(), Vec::new(), Vec::new()],
        }
    }
    /// `EIG_linear_solver_variable_lock`.
    pub(crate) fn lock(&mut self, i: usize) {
        self.variables[i].locked = true;
    }
    /// `EIG_linear_solver_variable_unlock`.
    pub(crate) fn unlock(&mut self, i: usize) {
        self.variables[i].locked = false;
    }
    /// `EIG_linear_solver_variable_set`.
    pub(crate) fn set(&mut self, rhs: usize, i: usize, v: f64) {
        self.variables[i].value[rhs] = v;
    }
    /// `EIG_linear_solver_variable_get`.
    pub(crate) fn get(&self, rhs: usize, i: usize) -> f64 {
        self.variables[i].value[rhs]
    }
    /// `linear_solver_ensure_matrix_construct`.
    fn ensure_construct(&mut self) {
        if self.constructing {
            return;
        }
        let mut n = 0;
        for v in &mut self.variables {
            if v.locked {
                v.index = usize::MAX;
            } else {
                v.index = n;
                n += 1;
            }
        }
        let m = if self.num_rows == 0 { n } else { self.num_rows };
        self.n = n;
        self.b = [vec![0.0; m], vec![0.0; m], vec![0.0; m]];
        self.constructing = true;
    }
    /// `EIG_linear_solver_matrix_add`.
    pub(crate) fn matrix_add(&mut self, row: usize, col: usize, value: f64) {
        self.ensure_construct();
        if self.variables[col].locked {
            self.variables[col].a.push((row, value));
        } else {
            let c = self.variables[col].index;
            self.triplets.push((row, c, value));
        }
    }
    /// `EIG_linear_solver_right_hand_side_add` (least squares: by raw row).
    pub(crate) fn rhs_add(&mut self, rhs: usize, row: usize, value: f64) {
        self.ensure_construct();
        self.b[rhs][row] += value;
    }

    /// `EIG_linear_solver_solve`: `false` when the factorization fails (the variables keep
    /// their values).
    pub(crate) fn solve(&mut self) -> bool {
        self.ensure_construct();
        let m = self.b[0].len();
        let n = self.n;
        if m == 0 || n == 0 {
            return true;
        }
        // `setFromTriplets`: duplicates summed, rows of `M`.
        let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); m];
        for &(r, c, v) in &self.triplets {
            match rows[r].iter_mut().find(|e| e.0 == c) {
                Some(e) => e.1 += v,
                None => rows[r].push((c, v)),
            }
        }
        // `MtM = Mᵀ M`.
        let mut mtm: HashMap<(usize, usize), f64> = HashMap::with_capacity(n * 8);
        for row in &rows {
            frap_math::budget::check_n(row.len().pow(2).min(u32::MAX as usize) as u32);
            for &(ci, vi) in row {
                for &(cj, vj) in row {
                    *mtm.entry((ci, cj)).or_insert(0.0) += vi * vj;
                }
            }
        }
        let mut trips: Vec<Triplet<usize, usize, f64>> = mtm
            .into_iter()
            .map(|((r, c), v)| Triplet::new(r, c, v))
            .collect();
        trips.sort_unstable_by_key(|t| (t.col, t.row));
        let Ok(mat) = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &trips) else {
            return false;
        };
        frap_math::budget::check_now();
        let Ok(lu) = mat.sp_lu() else {
            return false;
        };
        frap_math::budget::check_now();
        let mut x = [vec![0.0; n], vec![0.0; n], vec![0.0; n]];
        for (rhs, xr) in x.iter_mut().enumerate() {
            // Modify for locked variables.
            let mut b = self.b[rhs].clone();
            for v in &self.variables {
                if v.locked {
                    for &(r, a) in &v.a {
                        b[r] -= a * v.value[rhs];
                    }
                }
            }
            let mut mtb = faer::Mat::<f64>::zeros(n, 1);
            for (r, row) in rows.iter().enumerate() {
                for &(c, v) in row {
                    mtb[(c, 0)] += v * b[r];
                }
            }
            lu.solve_in_place(mtb.as_mut());
            for i in 0..n {
                let s = mtb[(i, 0)];
                if !s.is_finite() {
                    return false;
                }
                xr[i] = s;
            }
        }
        for v in &mut self.variables {
            if !v.locked {
                for (rhs, xr) in x.iter().enumerate() {
                    v.value[rhs] = xr[v.index];
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_columns_move_to_the_right_hand_side() {
        // x0 + 2 x1 = 5 with x1 locked at 1.5: x0 = 2; second row empty.
        let mut s = LeastSquares::new(2, 2);
        s.lock(1);
        s.set(0, 1, 1.5);
        s.matrix_add(0, 0, 1.0);
        s.matrix_add(0, 1, 2.0);
        s.rhs_add(0, 0, 5.0);
        assert!(s.solve());
        assert!((s.get(0, 0) - 2.0).abs() < 1e-12);
        assert_eq!(s.get(0, 1), 1.5);
    }
}
