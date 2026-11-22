// SPDX-License-Identifier: GPL-3.0-or-later
//! Setting SLIM up for one chart (`MatrixTransfer::setup_slim_data`): the geometry of the
//! transfer matrices (`GeometryData`), per-face importance weights, the initial map (the outer
//! boundary on a circle and mean value coordinates inside, `uv_initializer.cpp`), its fit to
//! the pins (`least_squares_relocator.cpp`) and the surface-area compensation
//! (`area_compensation.cpp`).
//!
//! Ported from blender/intern/slim/intern/{geometry_data_retrieval.cpp, uv_initializer.cpp,
//! least_squares_relocator.cpp, area_compensation.cpp, slim_matrix_transfer.cpp}
//! (Blender 5.2.4).

use super::MatrixTransferChart;
use super::solver::{SlimData, doublearea2, doublearea3, slim_precompute};
use super::sparse::{Csc, lu_solve};
use super::svd::{Mat2, det, mul, svd2, transpose};

/// `MatrixTransfer`'s options.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct TransferOptions {
    pub(crate) use_weights: bool,
    pub(crate) weight_influence: f64,
    pub(crate) reflection_mode: i32,
    pub(crate) skip_initialization: bool,
}

/// `compute_angle`.
fn compute_angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let na = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let nb = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
    libm::acos(dot / (na * nb))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `map_vertices_to_convex_border`.
fn convex_border(n: usize) -> Vec<[f64; 2]> {
    let pi = libm::atan(1.0) * 4.0;
    let angle = 2.0 * pi / n as f64;
    (0..n)
        .map(|i| {
            let x = libm::cos(angle * i as f64);
            let y = libm::sin(angle * i as f64);
            [(x * 0.5) + 0.5, (y * 0.5) + 0.5]
        })
        .collect()
}

/// `mvc` (`convex_border_parameterization` with `Method::MVC`).
fn mvc(
    f: &[[usize; 3]],
    v: &[[f64; 3]],
    e: &[[usize; 2]],
    el: &[f64],
    nb: usize,
    bnd_uv: &[[f64; 2]],
    uv: &mut [[f64; 2]],
) {
    let nv = uv.len();
    // `find_vertex_to_its_angles_correspondence`.
    let mut trips = Vec::with_capacity(f.len() * 6);
    for t in f {
        let (i1, i2, i3) = (t[0], t[1], t[2]);
        let angle1 = compute_angle(sub(v[i2], v[i1]), sub(v[i3], v[i1]));
        let angle2 = compute_angle(sub(v[i3], v[i2]), sub(v[i1], v[i2]));
        let angle3 = compute_angle(sub(v[i1], v[i3]), sub(v[i2], v[i3]));
        trips.push((i1, 2 * i2, angle1));
        trips.push((i1, 2 * i3 + 1, angle1));
        trips.push((i2, 2 * i1 + 1, angle2));
        trips.push((i2, 2 * i3, angle2));
        trips.push((i3, 2 * i1, angle3));
        trips.push((i3, 2 * i2 + 1, angle3));
    }
    let angles = Csc::from_triplets(nv, 2 * nv, &trips);

    let n_unknowns = nv - nb;
    let n_knowns = nb;
    let mut int_trips = Vec::new();
    let mut bnd_trips = Vec::new();
    for (k, edge) in e.iter().enumerate() {
        let (first, second) = (edge[0], edge[1]);
        if first < n_knowns {
            continue;
        }
        let row = first - n_knowns;
        let angle1 = angles.coeff(first, 2 * second);
        let angle2 = angles.coeff(first, 2 * second + 1);
        let mut w = libm::tan(angle1 / 2.0) + libm::tan(angle2 / 2.0);
        w /= el[k];
        int_trips.push((row, row, w));
        if second >= n_knowns {
            int_trips.push((row, second - n_knowns, -w));
        } else {
            bnd_trips.push((row, second, w));
        }
    }
    // Rows divided by their diagonal.
    let mut diag = vec![0.0f64; n_unknowns];
    let aint0 = Csc::from_triplets(n_unknowns, n_unknowns, &int_trips);
    for (i, d) in diag.iter_mut().enumerate() {
        *d = aint0.coeff(i, i);
    }
    let mut aint = aint0;
    for c in 0..aint.ncols {
        for k in aint.col_ptr[c]..aint.col_ptr[c + 1] {
            aint.values[k] /= diag[aint.row_idx[k]];
        }
    }
    let mut abnd = Csc::from_triplets(n_unknowns, n_knowns, &bnd_trips);
    for c in 0..abnd.ncols {
        for k in abnd.col_ptr[c]..abnd.col_ptr[c + 1] {
            abnd.values[k] /= diag[abnd.row_idx[k]];
        }
    }
    let rhs: Vec<Vec<f64>> = (0..2)
        .map(|i| {
            let z: Vec<f64> = (0..n_knowns).map(|zi| bnd_uv[zi][i]).collect();
            abnd.mul_vec(&z)
        })
        .collect();
    let sol = lu_solve(&aint, &[&rhs[0], &rhs[1]]);
    for (k, p) in uv.iter_mut().enumerate() {
        if k < n_knowns {
            *p = bnd_uv[k];
        } else {
            *p = match &sol {
                Some(s) => [s[0][k - n_knowns], s[1][k - n_knowns]],
                None => [f64::NAN, f64::NAN],
            };
        }
    }
}

/// `count_flips`: triangles with a negative determinant in the map.
fn count_flips(f: &[[usize; 3]], uv: &[[f64; 2]]) -> usize {
    f.iter()
        .filter(|t| {
            let (a, b, c) = (uv[t[0]], uv[t[1]], uv[t[2]]);
            // Determinant of [[a0, b0, c0], [a1, b1, c1], [1, 1, 1]].
            let det = a[0] * (b[1] - c[1]) - b[0] * (a[1] - c[1]) + c[0] * (a[1] - b[1]);
            det < 0.0
        })
        .count()
}

/// `compute_surface_area` of a map.
fn map_area(v: &[[f64; 2]], f: &[[usize; 3]]) -> f64 {
    doublearea2(v, f).iter().sum::<f64>() / 2.0
}

/// `correct_map_surface_area_if_necessary`.
pub(crate) fn correct_map_surface_area_if_necessary(s: &mut SlimData) {
    if !s.valid {
        return;
    }
    let corrected = s.expected_surface_area_of_resulting_map != 0.0;
    if !(corrected && s.b.is_empty()) {
        return;
    }
    let area = map_area(&s.v_o, &s.f);
    if area == 0.0 {
        return;
    }
    let ratio = area / s.expected_surface_area_of_resulting_map;
    let sq = (ratio / 1.0).sqrt();
    for p in &mut s.v_o {
        p[0] /= sq;
        p[1] /= sq;
    }
}

/// `correct_mesh_surface_area_if_necessary`.
fn correct_mesh_surface_area_if_necessary(s: &mut SlimData) {
    if !(s.skip_initialization || !s.b.is_empty()) {
        return;
    }
    let mut area_map = map_area(&s.v_o, &s.f);
    if area_map == 0.0 {
        return;
    }
    if area_map < 0.0 {
        area_map *= -1.0;
    }
    s.expected_surface_area_of_resulting_map = area_map;
    let area3d = doublearea3(&s.v, &s.f).iter().sum::<f64>() / 2.0;
    let ratio = area3d / area_map;
    let sq = (ratio / 1.0).sqrt();
    for p in &mut s.v {
        for x in p.iter_mut() {
            *x /= sq;
        }
    }
}

fn centroid(p: &[[f64; 2]]) -> [f64; 2] {
    let (mut x, mut y) = (0.0f64, 0.0f64);
    for q in p {
        x += q[0];
    }
    for q in p {
        y += q[1];
    }
    [x / p.len() as f64, y / p.len() as f64]
}

fn apply_transformation(s: &mut SlimData, t: &Mat2) {
    for p in &mut s.v_o {
        *p = [
            t[0][0] * p[0] + t[0][1] * p[1],
            t[1][0] * p[0] + t[1][1] * p[1],
        ];
    }
}

fn apply_translation(s: &mut SlimData, t: [f64; 2]) {
    for p in &mut s.v_o {
        *p = [t[0] + p[0], t[1] + p[1]];
    }
}

fn translation_1_pin(s: &SlimData) -> [f64; 2] {
    let p = s.v_o[s.b[0]];
    [s.bc[0][0] - p[0], s.bc[0][1] - p[1]]
}

/// `transform_initialization_if_necessary` (`transform_initialized_map`).
fn transform_initialization_if_necessary(s: &mut SlimData) {
    if s.skip_initialization || s.b.is_empty() {
        return;
    }
    match s.b.len() {
        1 => {
            let t = translation_1_pin(s);
            apply_translation(s, t);
        }
        2 => {
            // `compute_transformation_matrix2_pins`.
            let mut dp = [s.bc[0][0] - s.bc[1][0], s.bc[0][1] - s.bc[1][1]];
            let (a, b) = (s.v_o[s.b[0]], s.v_o[s.b[1]]);
            let mut di = [a[0] - b[0], a[1] - b[1]];
            let ndp = (dp[0] * dp[0] + dp[1] * dp[1]).sqrt();
            let ndi = (di[0] * di[0] + di[1] * di[1]).sqrt();
            let scale = ndp / ndi;
            if ndp > 0.0 {
                dp = [dp[0] / ndp, dp[1] / ndp];
            }
            if ndi > 0.0 {
                di = [di[0] / ndi, di[1] / ndi];
            }
            let cos_angle = dp[0] * di[0] + dp[1] * di[1];
            let sin_angle = (1.0 - cos_angle * cos_angle).sqrt();
            let rot = [[cos_angle, -sin_angle], [sin_angle, cos_angle]];
            let id = [[scale, 0.0], [0.0, scale]];
            let t = mul(&id, &rot);
            apply_transformation(s, &t);
            let tr = translation_1_pin(s);
            apply_translation(s, tr);
        }
        _ => {
            let flip_allowed = s.reflection_mode == 0;
            // `comput_least_squares_rotation_scale_only`.
            let init: Vec<[f64; 2]> = s.b.iter().map(|&i| s.v_o[i]).collect();
            let c_init = centroid(&init);
            let c_pins = centroid(&s.bc);
            let ci: Vec<[f64; 2]> = init
                .iter()
                .map(|p| [p[0] - c_init[0], p[1] - c_init[1]])
                .collect();
            let cp: Vec<[f64; 2]> =
                s.bc.iter()
                    .map(|p| [p[0] - c_pins[0], p[1] - c_pins[1]])
                    .collect();
            // `centered_initialized_pinsᵀ * centeredpins`.
            let mut m = [[0.0f64; 2]; 2];
            for r in 0..2 {
                for c in 0..2 {
                    let mut acc = 0.0;
                    for k in 0..ci.len() {
                        acc += ci[k][r] * cp[k][c];
                    }
                    m[r][c] = acc;
                }
            }
            let (u, _, v) = svd2(&m);
            let vu_t = mul(&v, &transpose(&u));
            let mut sv: Mat2 = [[1.0, 0.0], [0.0, 1.0]];
            if det(&vu_t) < 0.0 {
                if flip_allowed {
                    // `flip_input_geometry`.
                    for t in &mut s.f {
                        t.swap(0, 2);
                    }
                } else {
                    sv[1][1] = det(&vu_t);
                }
            }
            // `compute_least_squares_scaling`: argmin_t |a t - p|.
            let (mut ata, mut atp) = (0.0f64, 0.0f64);
            for k in 0..ci.len() {
                ata += ci[k][0] * ci[k][0];
                atp += ci[k][0] * cp[k][0];
            }
            for k in 0..ci.len() {
                ata += ci[k][1] * ci[k][1];
                atp += ci[k][1] * cp[k][1];
            }
            let t0 = if ata != 0.0 { (atp / ata).abs() } else { 0.0 };
            let scaling = [[t0, 0.0], [0.0, t0]];
            let t = mul(&mul(&mul(&scaling, &v), &sv), &transpose(&u));
            let tc = [
                t[0][0] * c_init[0] + t[0][1] * c_init[1],
                t[1][0] * c_init[0] + t[1][1] * c_init[1],
            ];
            let tr = [c_pins[0] - tc[0], c_pins[1] - tc[1]];
            apply_transformation(s, &t);
            apply_translation(s, tr);
        }
    }
}

/// `create_weights_per_face`.
fn create_weights_per_face(s: &mut SlimData) {
    if !s.valid {
        return;
    }
    if !s.with_weighted_parameterization {
        s.weight_per_face_map = vec![1.0; s.f.len()];
        return;
    }
    let sign = if s.weight_influence >= 0.0 { 1.0 } else { -1.0 };
    let max_factor = s.weight_influence.abs() + 1.0;
    s.weight_per_face_map =
        s.f.iter()
            .map(|t| {
                let (w1, w2, w3) = (s.weightmap[t[0]], s.weightmap[t[1]], s.weightmap[t[2]]);
                let mean = (w1 + w2 + w3) / 3.0;
                let flipped_mean = 1.0 - mean;
                libm::pow(max_factor, sign * 2.0 * (f64::from(flipped_mean) - 0.5)) as f32
            })
            .collect();
}

/// `MatrixTransfer::setup_slim_data` (`GeometryData::construct_slim_data`): `None` when the
/// chart cannot be parametrized.
pub(crate) fn setup_slim_data(chart: &MatrixTransferChart, opts: &TransferOptions) -> SlimData {
    let mut s = SlimData::new();
    if !chart.succeeded {
        s.valid = false;
        return s;
    }
    let nv = chart.verts_num;
    let nf = chart.faces_num;
    let ne2 = chart.edges_num + chart.boundary_vertices_num;
    let nb = chart.boundary_vertices_num;

    let v: Vec<[f64; 3]> = (0..nv)
        .map(|i| {
            [
                chart.v_matrices[i],
                chart.v_matrices[nv + i],
                chart.v_matrices[2 * nv + i],
            ]
        })
        .collect();
    let uv: Vec<[f64; 2]> = (0..nv)
        .map(|i| [chart.uv_matrices[i], chart.uv_matrices[nv + i]])
        .collect();
    let f: Vec<[usize; 3]> = (0..nf)
        .map(|i| {
            [
                chart.f_matrices[i] as usize,
                chart.f_matrices[nf + i] as usize,
                chart.f_matrices[2 * nf + i] as usize,
            ]
        })
        .collect();
    let e: Vec<[usize; 2]> = (0..ne2)
        .map(|i| {
            [
                chart.e_matrices[i] as usize,
                chart.e_matrices[ne2 + i] as usize,
            ]
        })
        .collect();
    let el = &chart.el_vectors[..ne2];

    // `retrieve_pinned_vertices(fixed_boundary = false)`: the explicitly pinned vertices.
    let np = chart.pinned_vertices_num;
    let b: Vec<usize> = chart.p_matrices[..np].iter().map(|&i| i as usize).collect();
    let bc: Vec<[f64; 2]> = (0..np)
        .map(|i| [chart.pp_matrices[2 * i], chart.pp_matrices[2 * i + 1]])
        .collect();

    // `has_valid_preinitialized_map`.
    s.skip_initialization = opts.skip_initialization && count_flips(&f, &uv) == 0;
    s.weight_influence = opts.weight_influence;
    s.reflection_mode = opts.reflection_mode;
    s.with_weighted_parameterization = opts.use_weights;
    // `set_geometry_data_matrices`.
    s.v = v;
    s.f = f;
    s.b = b;
    s.bc = bc;
    s.v_o = uv.clone();
    s.old_uvs = uv;
    s.weightmap = chart.w_vectors[..nv].to_vec();
    create_weights_per_face(&mut s);

    s.soft_const_p = 10.0e100;

    // `initialize_if_needed`.
    if !s.skip_initialization {
        let bnd_uv = convex_border(nb);
        if s.v_o.len() == nb {
            s.v_o = bnd_uv;
        } else {
            let mut v_o = vec![[0.0f64; 2]; nv];
            mvc(&s.f, &s.v, &e, el, nb, &bnd_uv, &mut v_o);
            s.v_o = v_o;
        }
    }
    transform_initialization_if_necessary(&mut s);
    correct_mesh_surface_area_if_necessary(&mut s);
    slim_precompute(&mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convex_border_is_a_circle_in_the_unit_square() {
        let b = convex_border(4);
        let expect = [[1.0, 0.5], [0.5, 1.0], [0.0, 0.5], [0.5, 0.0]];
        for (p, q) in b.iter().zip(expect) {
            assert!((p[0] - q[0]).abs() < 1e-15 && (p[1] - q[1]).abs() < 1e-15);
        }
    }

    #[test]
    fn mvc_puts_a_symmetric_centre_at_the_centre() {
        // A square fan around one interior vertex (4): boundary 0..4, centre 4.
        let v = vec![
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let f = vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]];
        let e: Vec<[usize; 2]> = vec![[4, 0], [4, 1], [4, 2], [4, 3]];
        let el = vec![1.0; 4];
        let bnd = convex_border(4);
        let mut uv = vec![[0.0; 2]; 5];
        mvc(&f, &v, &e, &el, 4, &bnd, &mut uv);
        assert!((uv[4][0] - 0.5).abs() < 1e-12 && (uv[4][1] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn flips_are_counted() {
        let f = vec![[0, 1, 2]];
        assert_eq!(count_flips(&f, &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]), 0);
        assert_eq!(count_flips(&f, &[[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]]), 1);
    }
}
