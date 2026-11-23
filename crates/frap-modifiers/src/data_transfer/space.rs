// SPDX-License-Identifier: GPL-3.0-or-later
//! `SpaceTransform` (the destination → source object space of a transfer) and the automatic
//! best-match transform of `object.data_transfer(use_auto_transform=True)`.
//!
//! Ported from blender/source/blender/blenlib/intern/math_matrix_c.cc
//! (`BLI_space_transform_*`), blender/source/blender/blenlib/intern/math_statistics.cc
//! (`BLI_covariance_m3_v3n`) and blender/source/blender/blenkernel/intern/mesh_remap.cc
//! (`mesh_calc_eigen_matrix`, `BKE_mesh_remap_calc_difference_from_mesh`,
//! `BKE_mesh_remap_find_best_match_from_mesh`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The eigen decomposition (`BLI_eigen_solve_selfadjoint_m3`, Eigen's
//! `SelfAdjointEigenSolver` in Blender) is a cyclic Jacobi solve in `f64` here: eigenvalues
//! ascending and unit eigenvectors as Eigen returns them, equal up to rounding and the sign of
//! each vector (the eight mirrored candidates tried afterwards cover the sign).

use frap_geom::bvh::Nearest;

use super::bvhutils::MeshTree;
use super::math::V3;
use crate::types::weld::util::{invert_m4, mul_m4_m4m4, mul_m4_v3, unit_m4};

/// `float[4][4]` (`m[col][row]`).
type M4 = [[f32; 4]; 4];

/// `SpaceTransform`
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SpaceTransform {
    /// `local2target`
    pub local2target: M4,
    /// `target2local`
    pub target2local: M4,
}

/// `mul_mat3_m4_v3`
fn mul_mat3_m4_v3(m: &M4, v: V3) -> V3 {
    let [x, y, z] = v;
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * z,
        x * m[0][1] + y * m[1][1] + m[2][1] * z,
        x * m[0][2] + y * m[1][2] + m[2][2] * z,
    ]
}

impl SpaceTransform {
    /// `BLI_space_transform_from_matrices(local, target)` (`BLI_SPACE_TRANSFORM_SETUP` with
    /// the two objects' world matrices).
    pub fn from_matrices(local: &M4, target: &M4) -> SpaceTransform {
        let itarget = invert_m4(target);
        let local2target = mul_m4_m4m4(&itarget, local);
        SpaceTransform {
            local2target,
            target2local: invert_m4(&local2target),
        }
    }

    /// `BLI_space_transform_global_from_matrices(local, target)`.
    pub fn global_from_matrices(local: &M4, target: &M4) -> SpaceTransform {
        let ilocal = invert_m4(local);
        let local2target = mul_m4_m4m4(target, &ilocal);
        SpaceTransform {
            local2target,
            target2local: invert_m4(&local2target),
        }
    }

    /// `BLI_space_transform_apply`: a point, local → target.
    pub fn apply(&self, co: V3) -> V3 {
        mul_m4_v3(&self.local2target, co)
    }

    /// `BLI_space_transform_invert`: a point, target → local.
    pub fn invert(&self, co: V3) -> V3 {
        mul_m4_v3(&self.target2local, co)
    }

    /// `BLI_space_transform_apply_normal`: a direction, local → target, normalised.
    pub fn apply_normal(&self, no: V3) -> V3 {
        super::math::normalized(mul_mat3_m4_v3(&self.local2target, no))
    }

    /// `BLI_space_transform_invert_normal`: a direction, target → local, normalised.
    pub fn invert_normal(&self, no: V3) -> V3 {
        super::math::normalized(mul_mat3_m4_v3(&self.target2local, no))
    }
}

/// Applies an optional space transform to a point.
pub(crate) fn apply(st: Option<&SpaceTransform>, co: V3) -> V3 {
    st.map_or(co, |s| s.apply(co))
}

/// Applies an optional space transform to a normal.
pub(crate) fn apply_normal(st: Option<&SpaceTransform>, no: V3) -> V3 {
    st.map_or(no, |s| s.apply_normal(no))
}

/// `BLI_covariance_m3_v3n(cos, true)`: the sample covariance (`m[col][row]`) and the centre.
fn covariance(positions: &[V3]) -> ([[f32; 3]; 3], V3) {
    let mean_fac = 1.0 / positions.len() as f32;
    let mut center = [0.0f32; 3];
    for p in positions {
        for k in 0..3 {
            center[k] += p[k] * mean_fac;
        }
    }
    let covfac = 1.0 / (positions.len() as f32 - 1.0);
    let mut flat = [0.0f32; 9];
    for a in 0..9 {
        let (i, j) = (a / 3, a % 3);
        if j < i {
            continue;
        }
        for p in positions {
            flat[a] += (p[i] - center[i]) * (p[j] - center[j]);
        }
        flat[a] *= covfac;
        if j != i {
            flat[j * 3 + i] = flat[a];
        }
    }
    (
        [
            [flat[0], flat[1], flat[2]],
            [flat[3], flat[4], flat[5]],
            [flat[6], flat[7], flat[8]],
        ],
        center,
    )
}

/// `BLI_eigen_solve_selfadjoint_m3`: ascending eigenvalues and their unit eigenvectors.
fn eigen_selfadjoint(m: &[[f32; 3]; 3]) -> ([f32; 3], [V3; 3]) {
    let mut a = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            a[i][j] = f64::from(m[i][j]);
        }
    }
    let mut v = [[1.0f64, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _sweep in 0..64 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        if off < 1e-30 {
            break;
        }
        for (p, q) in [(0usize, 1usize), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in &mut a {
                let akp = row[p];
                let akq = row[q];
                row[p] = c * akp - s * akq;
                row[q] = s * akp + c * akq;
            }
            let (rp, rq) = (a[p], a[q]);
            for k in 0..3 {
                a[p][k] = c * rp[k] - s * rq[k];
                a[q][k] = s * rp[k] + c * rq[k];
            }
            for row in &mut v {
                let vp = row[p];
                let vq = row[q];
                row[p] = c * vp - s * vq;
                row[q] = s * vp + c * vq;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&x, &y| a[x][x].total_cmp(&a[y][y]));
    let mut vals = [0.0f32; 3];
    let mut vecs = [[0.0f32; 3]; 3];
    for (k, &i) in order.iter().enumerate() {
        vals[k] = a[i][i] as f32;
        vecs[k] = [v[0][i] as f32, v[1][i] as f32, v[2][i] as f32];
    }
    (vals, vecs)
}

/// `ulp_diff_ff`
fn ulp_diff(a: f32, b: f32) -> u32 {
    let (ua, ub) = (a.to_bits(), b.to_bits());
    let sign = 0x8000_0000u32;
    let inf = 0x7f80_0000u32;
    let (a_abs, b_abs) = (ua & !sign, ub & !sign);
    if a_abs > inf || b_abs > inf {
        return u32::MAX;
    }
    if ua & sign == ub & sign {
        a_abs.max(b_abs) - a_abs.min(b_abs)
    } else {
        a_abs + b_abs
    }
}

/// `compare_ff_relative`
fn compare_ff_relative(a: f32, b: f32, max_diff: f32, max_ulps: u32) -> bool {
    (a - b).abs() <= max_diff || ulp_diff(a, b) <= max_ulps
}

/// `sqrtf_signed`
fn sqrtf_signed(f: f32) -> f32 {
    if f >= 0.0 { f.sqrt() } else { -(-f).sqrt() }
}

/// `mesh_calc_eigen_matrix`: the matrix of the "average ellipsoid" of `positions`.
fn eigen_matrix(positions: &[V3]) -> M4 {
    let mut r = unit_m4();
    let (covmat, center) = covariance(positions);
    let (vals, mut vecs) = eigen_selfadjoint(&covmat);
    let eps = f32::EPSILON;
    if compare_ff_relative(vals[0], vals[1], eps, 64) {
        if compare_ff_relative(vals[0], vals[2], eps, 64) {
            vecs = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        } else {
            let (a, b) = super::math::ortho_basis(vecs[2]);
            vecs[0] = a;
            vecs[1] = b;
        }
    } else if compare_ff_relative(vals[0], vals[2], eps, 64) {
        let (a, b) = super::math::ortho_basis(vecs[1]);
        vecs[2] = a;
        vecs[0] = b;
    } else if compare_ff_relative(vals[1], vals[2], eps, 64) {
        let (a, b) = super::math::ortho_basis(vecs[0]);
        vecs[1] = a;
        vecs[2] = b;
    }
    for i in 0..3 {
        let evi = vals[i];
        let evi = if evi < 1e-6 && evi > -1e-6 {
            if evi < 0.0 { -1e-3 } else { 1e-3 }
        } else {
            sqrtf_signed(evi)
        };
        for k in 0..3 {
            r[i][k] = vecs[i][k] * evi;
        }
    }
    r[3] = [center[0], center[1], center[2], 1.0];
    r
}

/// `BKE_mesh_remap_calc_difference_from_mesh` (the lower the better).
fn difference(st: &SpaceTransform, positions_dst: &[V3], tree: &MeshTree<'_>) -> f32 {
    let mut nearest = Nearest::within(f32::MAX);
    let mut result = 0.0f32;
    for &p in positions_dst {
        let co = st.apply(p);
        match super::remap::query_nearest(tree, &mut nearest, co, f32::MAX) {
            Some(hit) => result += 1.0 / (hit + 1.0),
            None => result += 1e-18,
        }
    }
    (positions_dst.len() as f32 / result) - 1.0
}

/// `BKE_mesh_remap_find_best_match_from_mesh`: the transform of the eight mirrored ellipsoid
/// matches that fits the source best.
pub(crate) fn find_best_match(positions_dst: &[V3], positions_src: &[V3]) -> SpaceTransform {
    let mirrors: [[f32; 3]; 7] = [
        [-1.0, 1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, -1.0],
        [1.0, -1.0, 1.0],
        [-1.0, 1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, -1.0],
    ];
    let tree = MeshTree::verts(positions_src, None);
    let mat_src = eigen_matrix(positions_src);
    let mut mat_dst = eigen_matrix(positions_dst);
    let st = SpaceTransform::global_from_matrices(&mat_dst, &mat_src);
    let mut best_match = difference(&st, positions_dst, &tree);
    let mut best_mat_dst = mat_dst;
    for mirr in mirrors {
        for (col, f) in mat_dst.iter_mut().zip(mirr) {
            for x in col.iter_mut().take(3) {
                *x *= f;
            }
        }
        let st = SpaceTransform::global_from_matrices(&mat_dst, &mat_src);
        let m = difference(&st, positions_dst, &tree);
        if m < best_match {
            best_match = m;
            best_mat_dst = mat_dst;
        }
    }
    SpaceTransform::global_from_matrices(&best_mat_dst, &mat_src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_transform_round_trip() {
        let mut local = unit_m4();
        local[3] = [1.0, 2.0, 3.0, 1.0];
        let mut target = unit_m4();
        target[0][0] = 2.0;
        target[3] = [-1.0, 0.0, 0.5, 1.0];
        let st = SpaceTransform::from_matrices(&local, &target);
        // Local (0,0,0) is world (1,2,3), which is target ((1+1)/2, 2, 2.5).
        let p = st.apply([0.0, 0.0, 0.0]);
        assert!((p[0] - 1.0).abs() < 1e-6 && (p[1] - 2.0).abs() < 1e-6);
        assert!((p[2] - 2.5).abs() < 1e-6);
        let back = st.invert(p);
        assert!(back.iter().all(|x| x.abs() < 1e-6));
        let n = st.apply_normal([1.0, 1.0, 0.0]);
        assert!((super::super::math::len(n) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn jacobi_eigen_of_a_diagonal_and_a_rotated_matrix() {
        let (vals, vecs) = eigen_selfadjoint(&[[3.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]]);
        assert_eq!(vals, [1.0, 2.0, 3.0]);
        assert_eq!(vecs[0].map(f32::abs), [0.0, 1.0, 0.0]);
        let (vals, _) = eigen_selfadjoint(&[[2.0, 1.0, 0.0], [1.0, 2.0, 0.0], [0.0, 0.0, 5.0]]);
        for (v, w) in vals.iter().zip([1.0, 3.0, 5.0]) {
            assert!((v - w).abs() < 1e-6, "{vals:?}");
        }
    }

    #[test]
    fn best_match_finds_a_translated_copy() {
        // An irregular point set and the same set moved by (2, 0, 0).
        let src: Vec<V3> = vec![
            [0.0, 0.0, 0.0],
            [3.0, 0.2, 0.1],
            [0.5, 1.5, 0.0],
            [0.3, 0.1, 0.7],
            [2.0, 1.0, 0.3],
        ];
        let dst: Vec<V3> = src.iter().map(|p| [p[0] + 2.0, p[1], p[2]]).collect();
        let st = find_best_match(&dst, &src);
        for (d, s) in dst.iter().zip(&src) {
            let p = st.apply(*d);
            for k in 0..3 {
                assert!((p[k] - s[k]).abs() < 1e-3, "{p:?} vs {s:?}");
            }
        }
    }
}
