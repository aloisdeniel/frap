// SPDX-License-Identifier: GPL-3.0-or-later
//! The sparse linear algebra the ABF++ and SLIM ports need, on `faer`: Eigen's
//! `SparseMatrix::setFromTriplets` (duplicates summed in insertion order), `SparseLU` for the
//! square systems (`EIG_linear_solver_new` of ABF++, the mean-value initialisation of SLIM)
//! and `SimplicialLDLT` for SLIM's symmetric positive definite proxy system (a sparse
//! Cholesky factorization: the same solution within rounding, a tolerance and not exactness).

use faer::Side;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};

/// A sparse matrix in compressed columns with summed duplicates (`setFromTriplets`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Csc {
    pub(crate) nrows: usize,
    pub(crate) ncols: usize,
    /// Column start offsets (`ncols + 1`).
    pub(crate) col_ptr: Vec<usize>,
    pub(crate) row_idx: Vec<usize>,
    pub(crate) values: Vec<f64>,
}

impl Csc {
    /// `SparseMatrix::setFromTriplets`: entries sorted by column then row, duplicates summed in
    /// the order they were given.
    pub(crate) fn from_triplets(nrows: usize, ncols: usize, trips: &[(usize, usize, f64)]) -> Csc {
        let mut order: Vec<usize> = (0..trips.len()).collect();
        // Stable: duplicates keep their insertion order for the summation.
        order.sort_by_key(|&i| (trips[i].1, trips[i].0));
        let mut col_ptr = vec![0usize; ncols + 1];
        let mut row_idx = Vec::with_capacity(trips.len());
        let mut values = Vec::with_capacity(trips.len());
        let mut last: Option<(usize, usize)> = None;
        for &i in &order {
            let (r, c, v) = trips[i];
            debug_assert!(r < nrows && c < ncols);
            if last == Some((r, c)) {
                *values.last_mut().expect("entry") += v;
            } else {
                row_idx.push(r);
                values.push(v);
                col_ptr[c + 1] += 1;
                last = Some((r, c));
            }
        }
        for c in 0..ncols {
            col_ptr[c + 1] += col_ptr[c];
        }
        Csc {
            nrows,
            ncols,
            col_ptr,
            row_idx,
            values,
        }
    }

    /// The entries of column `c`: `(row, value)`.
    pub(crate) fn col(&self, c: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let (a, b) = (self.col_ptr[c], self.col_ptr[c + 1]);
        self.row_idx[a..b]
            .iter()
            .copied()
            .zip(self.values[a..b].iter().copied())
    }

    /// `coeff(r, c)` (zero when not stored).
    pub(crate) fn coeff(&self, r: usize, c: usize) -> f64 {
        self.col(c)
            .find(|&(row, _)| row == r)
            .map_or(0.0, |(_, v)| v)
    }

    /// `self * x`.
    pub(crate) fn mul_vec(&self, x: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.nrows];
        for (c, &xc) in x.iter().enumerate().take(self.ncols) {
            for (r, v) in self.col(c) {
                out[r] += v * xc;
            }
        }
        out
    }

    fn to_faer(&self) -> Option<SparseColMat<usize, f64>> {
        let mut trips = Vec::with_capacity(self.values.len());
        for c in 0..self.ncols {
            for (r, v) in self.col(c) {
                trips.push(Triplet::new(r, c, v));
            }
        }
        SparseColMat::<usize, f64>::try_new_from_triplets(self.nrows, self.ncols, &trips).ok()
    }
}

/// Solves the square system `m x = b` with a sparse LU (Eigen's `SparseLU`). `None` when the
/// factorization fails or the solution is not finite.
pub(crate) fn lu_solve(m: &Csc, rhs: &[&[f64]]) -> Option<Vec<Vec<f64>>> {
    debug_assert_eq!(m.nrows, m.ncols);
    let n = m.ncols;
    if n == 0 {
        return Some(rhs.iter().map(|_| Vec::new()).collect());
    }
    let mat = m.to_faer()?;
    let lu = mat.sp_lu().ok()?;
    let mut out = Vec::with_capacity(rhs.len());
    for b in rhs {
        let mut x = faer::Mat::<f64>::from_fn(n, 1, |i, _| b[i]);
        lu.solve_in_place(x.as_mut());
        let v: Vec<f64> = (0..n).map(|i| x[(i, 0)]).collect();
        if !v.iter().all(|x| x.is_finite()) {
            return None;
        }
        out.push(v);
    }
    Some(out)
}

/// Solves the symmetric positive definite system `m x = b` (Eigen's `SimplicialLDLT`; only the
/// lower triangle of `m` is read). `None` when the factorization fails.
pub(crate) fn llt_solve(m: &Csc, b: &[f64]) -> Option<Vec<f64>> {
    let n = m.ncols;
    if n == 0 {
        return Some(Vec::new());
    }
    let mut trips = Vec::with_capacity(m.values.len());
    for c in 0..n {
        for (r, v) in m.col(c) {
            if r >= c {
                trips.push(Triplet::new(r, c, v));
            }
        }
    }
    let mat = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &trips).ok()?;
    let llt = mat.sp_cholesky(Side::Lower).ok()?;
    let mut x = faer::Mat::<f64>::from_fn(n, 1, |i, _| b[i]);
    llt.solve_in_place(x.as_mut());
    Some((0..n).map(|i| x[(i, 0)]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triplets_sum_duplicates() {
        let m = Csc::from_triplets(2, 2, &[(0, 0, 1.0), (1, 0, 2.0), (0, 0, 3.0), (1, 1, 5.0)]);
        assert_eq!(m.coeff(0, 0), 4.0);
        assert_eq!(m.coeff(1, 0), 2.0);
        assert_eq!(m.coeff(0, 1), 0.0);
        assert_eq!(m.mul_vec(&[1.0, 1.0]), vec![4.0, 7.0]);
    }

    #[test]
    fn lu_and_llt_solve() {
        // [[4, 1], [1, 3]] x = [1, 2] → x = [1/11, 7/11].
        let m = Csc::from_triplets(2, 2, &[(0, 0, 4.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)]);
        let x = lu_solve(&m, &[&[1.0, 2.0]]).unwrap().remove(0);
        assert!((x[0] - 1.0 / 11.0).abs() < 1e-14 && (x[1] - 7.0 / 11.0).abs() < 1e-14);
        let y = llt_solve(&m, &[1.0, 2.0]).unwrap();
        assert!((y[0] - x[0]).abs() < 1e-14 && (y[1] - x[1]).abs() < 1e-14);
    }

    #[test]
    fn singular_lu_fails() {
        let m = Csc::from_triplets(2, 2, &[(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0)]);
        assert!(lu_solve(&m, &[&[1.0, 2.0]]).is_none());
    }
}
