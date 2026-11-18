// SPDX-License-Identifier: GPL-3.0-or-later
//! The sparse square solve behind Blender's `EIG_linear_least_squares_solver` as
//! LAPLACIANSMOOTH uses it (intern/eigen/intern/linear_solver.cc): the matrix is assembled from
//! triplets (duplicates summed, `setFromTriplets`) and Blender solves the normal equations
//! `MᵀM x = Mᵀb` with Eigen's `SparseLU` in double precision. For the square, non-singular
//! matrices this modifier builds that is the solution of `M x = b`, which is computed here in
//! double precision with BiCGSTAB (Jacobi-preconditioned) to a relative residual of 1e-13, and
//! with a dense partial-pivot LU when the iteration does not converge. A singular matrix
//! (`SparseLU` failure) returns `None`, as `EIG_linear_solver_solve` returns false.

/// A square sparse matrix in compressed rows.
pub(crate) struct Csr {
    n: usize,
    offsets: Vec<usize>,
    cols: Vec<usize>,
    vals: Vec<f64>,
}

impl Csr {
    /// Builds an `n × n` matrix from `(row, col, value)` triplets, summing duplicates.
    pub fn from_triplets(n: usize, mut t: Vec<(usize, usize, f64)>) -> Csr {
        t.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let mut offsets = vec![0usize; n + 1];
        let mut cols: Vec<usize> = Vec::with_capacity(t.len());
        let mut vals: Vec<f64> = Vec::with_capacity(t.len());
        let mut rows: Vec<usize> = Vec::with_capacity(t.len());
        for (r, c, v) in t {
            if let (Some(&lr), Some(&lc)) = (rows.last(), cols.last())
                && lr == r
                && lc == c
            {
                *vals.last_mut().expect("non-empty") += v;
                continue;
            }
            rows.push(r);
            cols.push(c);
            vals.push(v);
        }
        for &r in &rows {
            offsets[r + 1] += 1;
        }
        for i in 0..n {
            offsets[i + 1] += offsets[i];
        }
        Csr {
            n,
            offsets,
            cols,
            vals,
        }
    }

    fn mul(&self, x: &[f64], out: &mut [f64]) {
        for (r, o) in out.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in self.offsets[r]..self.offsets[r + 1] {
                s += self.vals[k] * x[self.cols[k]];
            }
            *o = s;
        }
    }

    fn diag(&self) -> Vec<f64> {
        let mut d = vec![0.0; self.n];
        for (r, dr) in d.iter_mut().enumerate() {
            for k in self.offsets[r]..self.offsets[r + 1] {
                if self.cols[k] == r {
                    *dr += self.vals[k];
                }
            }
        }
        d
    }

    /// Solves `M x = b` for every right-hand side; `None` when the matrix is singular.
    pub fn solve(&self, rhs: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
        let mut out = Vec::with_capacity(rhs.len());
        for b in rhs {
            match self.bicgstab(b) {
                Some(x) => out.push(x),
                None => return self.dense_solve(rhs),
            }
        }
        Some(out)
    }

    fn bicgstab(&self, b: &[f64]) -> Option<Vec<f64>> {
        let n = self.n;
        let dot = |a: &[f64], c: &[f64]| a.iter().zip(c).map(|(x, y)| x * y).sum::<f64>();
        let bnorm = dot(b, b).sqrt();
        if bnorm == 0.0 {
            return Some(vec![0.0; n]);
        }
        let d = self.diag();
        if d.iter().any(|v| *v == 0.0 || !v.is_finite()) {
            return None;
        }
        let pre = |v: &[f64], out: &mut [f64]| {
            for i in 0..n {
                out[i] = v[i] / d[i];
            }
        };
        let tol = 1e-13 * bnorm;
        let mut x = vec![0.0; n];
        pre(b, &mut x);
        let mut r = vec![0.0; n];
        self.mul(&x, &mut r);
        for i in 0..n {
            r[i] = b[i] - r[i];
        }
        let r0 = r.clone();
        let (mut rho, mut alpha, mut omega) = (1.0f64, 1.0f64, 1.0f64);
        let mut v = vec![0.0; n];
        let mut p = vec![0.0; n];
        let mut y = vec![0.0; n];
        let mut s = vec![0.0; n];
        let mut z = vec![0.0; n];
        let mut t = vec![0.0; n];
        for _ in 0..(10 * n + 200) {
            frap_math::budget::check();
            if dot(&r, &r).sqrt() <= tol {
                return Some(x);
            }
            let rho_new = dot(&r0, &r);
            if rho_new == 0.0 || !rho_new.is_finite() {
                return None;
            }
            let beta = (rho_new / rho) * (alpha / omega);
            rho = rho_new;
            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }
            pre(&p, &mut y);
            self.mul(&y, &mut v);
            let r0v = dot(&r0, &v);
            if r0v == 0.0 || !r0v.is_finite() {
                return None;
            }
            alpha = rho / r0v;
            for i in 0..n {
                s[i] = r[i] - alpha * v[i];
            }
            if dot(&s, &s).sqrt() <= tol {
                for i in 0..n {
                    x[i] += alpha * y[i];
                }
                return Some(x);
            }
            pre(&s, &mut z);
            self.mul(&z, &mut t);
            let tt = dot(&t, &t);
            if tt == 0.0 || !tt.is_finite() {
                return None;
            }
            omega = dot(&t, &s) / tt;
            for i in 0..n {
                x[i] += alpha * y[i] + omega * z[i];
                r[i] = s[i] - omega * t[i];
            }
            if omega == 0.0 {
                return None;
            }
        }
        None
    }

    /// Dense LU with partial pivoting (fallback for small systems).
    fn dense_solve(&self, rhs: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
        let n = self.n;
        if n > 4000 {
            return None;
        }
        let mut a = vec![0.0f64; n * n];
        for r in 0..n {
            for k in self.offsets[r]..self.offsets[r + 1] {
                a[r * n + self.cols[k]] += self.vals[k];
            }
        }
        let mut perm: Vec<usize> = (0..n).collect();
        let scale = a.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        for c in 0..n {
            frap_math::budget::check();
            let (mut best, mut piv) = (0.0f64, c);
            for r in c..n {
                let v = a[r * n + c].abs();
                if v > best {
                    best = v;
                    piv = r;
                }
            }
            if best <= scale * 1e-14 {
                return None;
            }
            if piv != c {
                for k in 0..n {
                    a.swap(c * n + k, piv * n + k);
                }
                perm.swap(c, piv);
            }
            let inv = 1.0 / a[c * n + c];
            for r in c + 1..n {
                let f = a[r * n + c] * inv;
                if f == 0.0 {
                    continue;
                }
                a[r * n + c] = f;
                for k in c + 1..n {
                    a[r * n + k] -= f * a[c * n + k];
                }
            }
        }
        let mut out = Vec::with_capacity(rhs.len());
        for b in rhs {
            let mut y: Vec<f64> = perm.iter().map(|&p| b[p]).collect();
            for r in 0..n {
                for k in 0..r {
                    y[r] -= a[r * n + k] * y[k];
                }
            }
            for r in (0..n).rev() {
                for k in r + 1..n {
                    y[r] -= a[r * n + k] * y[k];
                }
                y[r] /= a[r * n + r];
            }
            out.push(y);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_small_system_with_duplicates() {
        // [[4, 1], [2, 3]] with the 4 given as 3 + 1.
        let m = Csr::from_triplets(
            2,
            vec![(0, 0, 3.0), (0, 1, 1.0), (1, 0, 2.0), (1, 1, 3.0), (0, 0, 1.0)],
        );
        let x = m.solve(&[vec![1.0, 2.0]]).unwrap();
        assert!((x[0][0] - 0.1).abs() < 1e-12);
        assert!((x[0][1] - 0.6).abs() < 1e-12);
        let d = m.dense_solve(&[vec![1.0, 2.0]]).unwrap();
        assert!((d[0][0] - 0.1).abs() < 1e-12);
    }

    #[test]
    fn singular_is_none() {
        let m = Csr::from_triplets(2, vec![(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0)]);
        assert!(m.solve(&[vec![1.0, 2.0]]).is_none());
    }
}
