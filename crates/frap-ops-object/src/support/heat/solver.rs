// SPDX-License-Identifier: GPL-3.0-or-later
//! The least-squares `LinearSolver` of `intern/eigen/intern/linear_solver.cc` as bone heat uses
//! it (`EIG_linear_least_squares_solver_new(0, verts_num, 1)`: one right-hand side, no locked
//! variables): the square matrix `M` is built from triplets once (duplicates summed, as
//! `setFromTriplets`), the normal equations `MᵀM x = Mᵀb` are factored once at the first solve
//! and reused for every bone.
//!
//! Blender factors `MᵀM` with Eigen's `SparseLU`; frap with `faer`'s sparse LU (partial
//! pivoting). Both work in `f64` on the same `f32`-rounded entries, so on a regular system the
//! solutions agree to far below the 1e-4 weight tolerance. Eigen reports failure when a
//! pivot column is exactly zero (`pivotL`: `pivmax <= 0`); frap when the matrix is symbolically
//! singular or the solution is not finite. Both fail on NaN coordinates.
//!
//! Not matched (deviations §2): a mesh part none of whose vertices sees a bone (fully
//! occluded, `H = 0` on the whole part) makes its Laplacian block singular, and whether Eigen
//! then meets an *exactly* zero pivot depends on its COLAMD ordering and the rounding of its
//! supernodal kernels. A C++ program linked against Eigen 3.4, fed frap's triplets, reproduces
//! Blender 5.2.4 on every probe: an axis-aligned cube or plane hat above a box fails (Blender
//! warns and writes no weight on the whole mesh), a shifted smaller cube, a rotated cube or an
//! icosphere hat succeeds (the box gets its weights, the hat none). `faer` meets no exact
//! zero on any of them, so frap always takes the second outcome; matching the first needs a
//! bit-exact port of Eigen's `SparseLU` (golden `armature/heat/occluded_part` pins the case
//! where both succeed).
//!
//! Ported from blender/intern/eigen/intern/linear_solver.cc (Blender 5.2.4,
//! GPL-2.0-or-later).

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum State {
    MatrixConstruct,
    MatrixSolved,
}

/// `LinearSolver` in least-squares mode with one right-hand side.
pub(crate) struct LeastSquares {
    state: State,
    n: usize,
    triplets: Vec<(usize, usize, f64)>,
    /// `M` by rows (merged), kept for `Mᵀb`.
    rows: Vec<Vec<(usize, f64)>>,
    b: Vec<f64>,
    x: Vec<f64>,
    lu: Option<faer::sparse::linalg::solvers::Lu<usize, f64>>,
    factored: bool,
}

impl LeastSquares {
    /// `EIG_linear_least_squares_solver_new(0, n, 1)`
    pub(crate) fn new(n: usize) -> LeastSquares {
        LeastSquares {
            state: State::MatrixConstruct,
            n,
            triplets: Vec::with_capacity(n * 3),
            rows: Vec::new(),
            b: vec![0.0; n],
            x: vec![0.0; n],
            lu: None,
            factored: false,
        }
    }

    /// `EIG_linear_solver_matrix_add`
    pub(crate) fn matrix_add(&mut self, row: usize, col: usize, value: f64) {
        if self.state == State::MatrixSolved {
            return;
        }
        self.triplets.push((row, col, value));
    }

    /// `EIG_linear_solver_right_hand_side_add`
    pub(crate) fn rhs_add(&mut self, index: usize, value: f64) {
        self.b[index] += value;
    }

    /// `EIG_linear_solver_variable_get`
    pub(crate) fn get(&self, index: usize) -> f64 {
        self.x[index]
    }

    /// `M`'s triplets merged by row, then `MᵀM` and its LU factorization.
    fn factorize(&mut self) -> bool {
        let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.n];
        for &(r, c, v) in &self.triplets {
            let row = &mut rows[r];
            if let Some(e) = row.iter_mut().find(|e| e.0 == c) {
                e.1 += v;
            } else {
                row.push((c, v));
            }
        }
        self.triplets = Vec::new();
        let mut mtm: HashMap<(usize, usize), f64> = HashMap::with_capacity(self.n * 8);
        for row in &rows {
            for &(ci, vi) in row {
                for &(cj, vj) in row {
                    *mtm.entry((ci, cj)).or_insert(0.0) += vi * vj;
                }
            }
        }
        self.rows = rows;
        let mut trips: Vec<Triplet<usize, usize, f64>> = mtm
            .into_iter()
            .map(|((r, c), v)| Triplet::new(r, c, v))
            .collect();
        trips.sort_unstable_by_key(|t| (t.col, t.row));
        let Ok(mat) = SparseColMat::<usize, f64>::try_new_from_triplets(self.n, self.n, &trips)
        else {
            return false;
        };
        match mat.sp_lu() {
            Ok(lu) => {
                self.lu = Some(lu);
                true
            }
            Err(_) => false,
        }
    }

    /// `EIG_linear_solver_solve`: solves for the current right-hand side, which is cleared
    /// afterwards. Returns `false` when the factorization or the solution failed (the
    /// variables keep their previous values).
    pub(crate) fn solve(&mut self) -> bool {
        if self.n == 0 {
            return true;
        }
        if self.state == State::MatrixConstruct {
            self.factored = self.factorize();
            self.state = State::MatrixSolved;
        }
        let mut result = self.factored;
        if let (true, Some(lu)) = (result, &self.lu) {
            let mut mtb = faer::Mat::<f64>::zeros(self.n, 1);
            for (r, row) in self.rows.iter().enumerate() {
                let br = self.b[r];
                for &(c, v) in row {
                    mtb[(c, 0)] += v * br;
                }
            }
            lu.solve_in_place(mtb.as_mut());
            if (0..self.n).all(|i| mtb[(i, 0)].is_finite()) {
                for i in 0..self.n {
                    self.x[i] = mtb[(i, 0)];
                }
            } else {
                result = false;
            }
        }
        self.b.iter_mut().for_each(|b| *b = 0.0);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_small_nonsymmetric_system() {
        // [2 1; 1 3] x = [3; 5] → x = [0.8, 1.4]
        let mut s = LeastSquares::new(2);
        s.matrix_add(0, 0, 1.0);
        s.matrix_add(0, 0, 1.0);
        s.matrix_add(0, 1, 1.0);
        s.matrix_add(1, 0, 1.0);
        s.matrix_add(1, 1, 3.0);
        s.rhs_add(0, 3.0);
        s.rhs_add(1, 5.0);
        assert!(s.solve());
        assert!((s.get(0) - 0.8).abs() < 1e-12);
        assert!((s.get(1) - 1.4).abs() < 1e-12);
        // The factorization is reused; the right-hand side was cleared.
        s.matrix_add(0, 0, 100.0);
        s.rhs_add(1, 3.0);
        assert!(s.solve());
        assert!((s.get(0) + 0.6).abs() < 1e-12);
        assert!((s.get(1) - 1.2).abs() < 1e-12);
    }

    #[test]
    fn singular_systems_fail() {
        let mut s = LeastSquares::new(2);
        s.matrix_add(0, 0, 1.0);
        s.matrix_add(0, 1, -1.0);
        s.matrix_add(1, 0, -1.0);
        s.matrix_add(1, 1, 1.0);
        s.rhs_add(0, 1.0);
        assert!(!s.solve());
        let mut nan = LeastSquares::new(1);
        nan.matrix_add(0, 0, f64::NAN);
        nan.rhs_add(0, 1.0);
        assert!(!nan.solve());
        assert_eq!(nan.get(0), 0.0);
        assert!(LeastSquares::new(0).solve());
    }
}
