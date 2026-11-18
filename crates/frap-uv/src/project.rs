// SPDX-License-Identifier: GPL-3.0-or-later
//! Projection helpers of `uvedit_unwrap_ops.cc` and `BLI_math_geom` on plain arrays (WP 3.3):
//! cube projection by dominant axis, `map_to_tube`/`map_to_sphere` with the seam fix-up
//! `uv_map_mirror`, the projection frame of `uv_map_transform` (headless: no 3D view, so the
//! view matrix is the identity) and the face walk of `uv_sphere_project` /
//! `uv_cylinder_project` (with the optional flood fill across non-seam edges), plus the
//! `uv_map_clip_correct` steps. `frap-ops-uv` (WP 3.8) supplies the mesh through
//! [`ProjectMesh`] and writes the corner UVs.
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_unwrap_ops.cc and
//! blender/source/blender/blenlib/intern/math_geom.cc (Blender 5.2.4).

use crate::param::bli::{M3, V3, mul_m3_v3};

/// `snap_coordinate`: maps `u` into the closed unit interval, snapping values within a quarter
/// texel of 65536 to the bounds.
fn snap_coordinate(u: f32) -> f32 {
    let mut u = u;
    if u < 0.0 {
        u += 1.0;
    }
    let epsilon = 0.25f32 / 65536.0;
    if u < epsilon {
        return 0.0;
    }
    if 1.0 - epsilon < u {
        return 1.0;
    }
    u
}

/// `2 * M_PI` as Blender's `float(2.0f * M_PI)`.
#[inline]
fn two_pi_f() -> f32 {
    (2.0f64 * core::f64::consts::PI) as f32
}

/// `map_to_tube` returning whether the point is "regular" (not on the cylinder's axis).
pub fn map_to_tube_ex(co: [f32; 3]) -> ([f32; 2], bool) {
    let [x, y, z] = co;
    let (u, regular) = if x * x + y * y < 1e-6 * 1e-6 {
        (0.5, false)
    } else {
        (snap_coordinate(libm::atan2f(x, -y) / two_pi_f()), true)
    };
    ([u, (z + 1.0) / 2.0], regular)
}

/// `map_to_tube`: cylindrical UV of a point already in the projection frame.
pub fn map_to_tube(co: [f32; 3]) -> [f32; 2] {
    map_to_tube_ex(co).0
}

/// `map_to_sphere` returning whether the point is "regular" (not on the pole axis).
pub fn map_to_sphere_ex(co: [f32; 3]) -> ([f32; 2], bool) {
    let [x, y, z] = co;
    let epsilon = 0.25f32 / 65536.0;
    let len_xy = (x * x + y * y).sqrt();
    let (u, regular) = if len_xy <= z.abs() * epsilon {
        (0.5, false)
    } else {
        (snap_coordinate(libm::atan2f(x, -y) / two_pi_f()), true)
    };
    let v = snap_coordinate(libm::atan2f(len_xy, -z) / core::f32::consts::PI);
    ([u, v], regular)
}

/// `map_to_sphere`: spherical UV of a point already in the projection frame.
pub fn map_to_sphere(co: [f32; 3]) -> [f32; 2] {
    map_to_sphere_ex(co).0
}

/// `uv_map_mirror`: moves the corners of a face that wraps across the seam (`u` spans more
/// than half) so the face stays on one side, then, with `fan`, spreads the corners that sit
/// on a pole (`regular[i] == false`) to the `u` of their regular neighbours.
pub fn uv_map_mirror(face_uvs: &mut [[f32; 2]], regular: &[bool], fan: bool) {
    let n = face_uvs.len();
    let mut right_u = -1.0e30f32;
    for uv in face_uvs.iter_mut() {
        if uv[0] >= 1.0 {
            uv[0] -= 1.0;
        }
        right_u = right_u.max(uv[0]);
    }
    let mut left_u = 1.0e30f32;
    for uv in face_uvs.iter() {
        if right_u <= uv[0] + 0.5 {
            left_u = left_u.min(uv[0]);
        }
    }
    for uv in face_uvs.iter_mut() {
        if uv[0] + 0.5 < right_u && 2.0 * uv[0] + 1.0 < left_u + right_u {
            uv[0] += 1.0;
        }
    }
    if !fan {
        return;
    }
    let mut minmax_u = [1.0e30f32, -1.0e30f32];
    let mut pole_count = 0;
    for i in 0..n {
        if regular[i] {
            minmax_u[0] = minmax_u[0].min(face_uvs[i][0]);
            minmax_u[1] = minmax_u[1].max(face_uvs[i][0]);
        } else {
            pole_count += 1;
        }
    }
    if pole_count == 0 || pole_count == n {
        return;
    }
    for i in 0..n {
        if regular[i] {
            continue;
        }
        let mut u = 0.0f32;
        let mut sum = 0.0f32;
        let i_plus = (i + 1) % n;
        let i_minus = (i + n - 1) % n;
        if regular[i_plus] {
            u += face_uvs[i_plus][0];
            sum += 1.0;
        }
        if regular[i_minus] {
            u += face_uvs[i_minus][0];
            sum += 1.0;
        }
        if sum == 0.0 {
            u += minmax_u[0] + minmax_u[1];
            sum += 2.0;
        }
        face_uvs[i][0] = u / sum;
    }
}

/// `axis_dominant_v3`: the two axes of the plane most facing `axis`.
pub fn axis_dominant_v3(axis: [f32; 3]) -> (usize, usize) {
    let xn = axis[0].abs();
    let yn = axis[1].abs();
    let zn = axis[2].abs();
    if zn >= xn && zn >= yn {
        (0, 1)
    } else if yn >= xn && yn >= zn {
        (0, 2)
    } else {
        (1, 2)
    }
}

/// `uvedit_unwrap_cube_project` for one face: projects `cos` (object space) on the plane of
/// the face normal's dominant axis, `uv = 0.5 + (co - loc) / cube_size` (a zero `cube_size`
/// counts as 1).
pub fn cube_project_face(
    normal: [f32; 3],
    cos: &[[f32; 3]],
    loc: [f32; 3],
    cube_size: f32,
    out: &mut [[f32; 2]],
) {
    let cube_size = if cube_size == 0.0 { 1.0 } else { cube_size };
    let (cox, coy) = axis_dominant_v3(normal);
    for (co, uv) in cos.iter().zip(out.iter_mut()) {
        *uv = [
            0.5 + ((co[cox] - loc[cox]) / cube_size),
            0.5 + ((co[coy] - loc[coy]) / cube_size),
        ];
    }
}

/// `uv_map_transform`'s `direction`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ProjectDirection {
    /// `VIEW_ON_EQUATOR` (the operators' default).
    #[default]
    ViewOnEquator,
    /// `VIEW_ON_POLES`
    ViewOnPoles,
    /// `ALIGN_TO_OBJECT`
    AlignToObject,
}

/// `uv_map_transform`'s `align`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ProjectAlign {
    /// `POLAR_ZX` (default)
    #[default]
    PolarZx,
    /// `POLAR_ZY`
    PolarZy,
}

/// Column-major `float[4][4]` (`m[col][row]`).
pub type M4 = [[f32; 4]; 4];

/// The identity 4×4 matrix.
pub const M4_IDENTITY: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `mul_m4_m4m4` (SSE2 summation order: `(b0·a0 + b1·a1) + (b2·a2 + b3·a3)`).
fn mul_m4_m4m4(a: &M4, b: &M4) -> M4 {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for k in 0..4 {
            r[i][k] =
                (b[i][0] * a[0][k] + b[i][1] * a[1][k]) + (b[i][2] * a[2][k] + b[i][3] * a[3][k]);
        }
    }
    r
}

/// `uv_map_rotation_matrix_ex(result, rv3d, ob, upangledeg, sideangledeg, radius, offset=0)`:
/// `view` is `rv3d->viewmat` (`None` headless: identity), `object_to_world` the object matrix.
pub fn uv_map_rotation_matrix(
    view: Option<&M4>,
    object_to_world: &M4,
    upangledeg: f32,
    sideangledeg: f32,
    radius: f32,
) -> M4 {
    let mut viewmatrix = view.copied().unwrap_or(M4_IDENTITY);
    viewmatrix[3][0] = 0.0;
    viewmatrix[3][1] = 0.0;
    viewmatrix[3][2] = 0.0;
    let mut rotobj = *object_to_world;
    rotobj[3] = [0.0, 0.0, 0.0, 0.0];

    let pi = core::f32::consts::PI;
    let sideangle = pi * (sideangledeg + 180.0) / 180.0;
    let mut rotside = [[0.0f32; 4]; 4];
    rotside[0][0] = libm::cosf(sideangle);
    rotside[0][1] = -libm::sinf(sideangle);
    rotside[1][0] = libm::sinf(sideangle);
    rotside[1][1] = libm::cosf(sideangle);
    rotside[2][2] = 1.0;

    let upangle = pi * upangledeg / 180.0;
    let mut rotup = [[0.0f32; 4]; 4];
    rotup[1][1] = libm::cosf(upangle) / radius;
    rotup[1][2] = -libm::sinf(upangle) / radius;
    rotup[2][1] = libm::sinf(upangle) / radius;
    rotup[2][2] = libm::cosf(upangle) / radius;
    rotup[0][0] = 1.0 / radius;

    let s = mul_m4_m4m4(&rotup, &rotside);
    let t = mul_m4_m4m4(&s, &viewmatrix);
    mul_m4_m4m4(&t, &rotobj)
}

/// `uv_map_transform`: the 3×3 projection frame of `sphere_project` / `cylinder_project`
/// (`radius` is 1 for the sphere, which has no such property).
pub fn uv_map_transform(
    direction: ProjectDirection,
    align: ProjectAlign,
    radius: f32,
    view: Option<&M4>,
    object_to_world: &M4,
) -> M3 {
    if direction == ProjectDirection::AlignToObject {
        let mut m = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        if align == ProjectAlign::PolarZy {
            m[0][0] = 0.0;
            m[0][1] = 1.0;
            m[1][0] = -1.0;
            m[1][1] = 0.0;
        }
        return m;
    }
    let up = if direction == ProjectDirection::ViewOnEquator {
        90.0
    } else {
        0.0
    };
    let side = if (align == ProjectAlign::PolarZy) == (direction == ProjectDirection::ViewOnEquator)
    {
        90.0
    } else {
        0.0
    };
    let m4 = uv_map_rotation_matrix(view, object_to_world, up, side, radius);
    [
        [m4[0][0], m4[0][1], m4[0][2]],
        [m4[1][0], m4[1][1], m4[1][2]],
        [m4[2][0], m4[2][1], m4[2][2]],
    ]
}

/// The mesh a curved projection walks (`BMesh` faces, loops and edges in iteration order).
pub trait ProjectMesh {
    /// Number of faces.
    fn face_count(&self) -> usize;
    /// The corner (loop) indices of face `f`, from its first loop.
    fn face_corners(&self, f: usize) -> core::ops::Range<usize>;
    /// The (object space) position of the vertex of corner `c`.
    fn corner_co(&self, c: usize) -> [f32; 3];
    /// The edge of corner `c` (`l->e`).
    fn corner_edge(&self, c: usize) -> usize;
    /// `BM_ELEM_SEAM` of edge `e`.
    fn edge_is_seam(&self, e: usize) -> bool;
    /// The faces of edge `e` in `BM_FACES_OF_EDGE` order (radial cycle from `e->l`).
    fn edge_faces(&self, e: usize, out: &mut Vec<usize>);
    /// `BM_ELEM_SELECT` of face `f`.
    fn face_selected(&self, f: usize) -> bool;
}

/// Which curved projection to run.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CurvedProjection {
    /// `uv.sphere_project` (`map_to_sphere`).
    Sphere,
    /// `uv.cylinder_project` (`map_to_tube`).
    Cylinder,
}

/// `uv_sphere_project` / `uv_cylinder_project` for one start face: returns the largest `u`.
#[allow(clippy::too_many_arguments)]
fn curved_project_face(
    mesh: &impl ProjectMesh,
    kind: CurvedProjection,
    efa_init: usize,
    center: V3,
    rotmat: &M3,
    fan: bool,
    use_seams: bool,
    branch_init: f32,
    tag: &mut [bool],
    uvs: &mut [[f32; 2]],
) -> f32 {
    let mut max_u = 0.0f32;
    if use_seams && tag[efa_init] {
        return max_u;
    }
    let mut stack: Vec<(usize, f32)> = vec![(efa_init, branch_init)];
    let mut faces_of_edge = Vec::new();
    let mut regular: Vec<bool> = Vec::new();
    while let Some((efa, branch)) = stack.pop() {
        frap_math::budget::check();
        if use_seams {
            if tag[efa] {
                continue;
            }
            tag[efa] = true;
        }
        if !mesh.face_selected(efa) {
            continue;
        }
        let corners = mesh.face_corners(efa);
        regular.clear();
        for c in corners.clone() {
            let co = mesh.corner_co(c);
            let pv = mul_m3_v3(
                rotmat,
                [co[0] - center[0], co[1] - center[1], co[2] - center[2]],
            );
            let (mut uv, reg) = match kind {
                CurvedProjection::Sphere => map_to_sphere_ex(pv),
                CurvedProjection::Cylinder => map_to_tube_ex(pv),
            };
            regular.push(reg);
            if use_seams {
                // Move the UV to the correct branch.
                uv[0] += (branch - 0.5 - uv[0]).ceil();
                max_u = max_u.max(uv[0]);
            }
            uvs[c] = uv;
            if !use_seams {
                continue;
            }
            let e = mesh.corner_edge(c);
            if mesh.edge_is_seam(e) {
                continue;
            }
            faces_of_edge.clear();
            mesh.edge_faces(e, &mut faces_of_edge);
            for &f2 in &faces_of_edge {
                if !tag[f2] {
                    stack.push((f2, uv[0]));
                }
            }
        }
        uv_map_mirror(&mut uvs[corners], &regular, fan);
    }
    max_u
}

/// `sphere_project_exec` / `cylinder_project_exec` minus the clip/correct step: projects every
/// selected face (`uvs` is indexed by corner), `fan` = `pole='FAN'`, `use_seams` = `seam`.
pub fn curved_project(
    mesh: &impl ProjectMesh,
    kind: CurvedProjection,
    center: [f32; 3],
    rotmat: &M3,
    fan: bool,
    use_seams: bool,
    uvs: &mut [[f32; 2]],
) {
    let mut tag = vec![false; mesh.face_count()];
    let mut island_offset = 0.0f32;
    for f in 0..mesh.face_count() {
        if kind == CurvedProjection::Cylinder && !mesh.face_selected(f) {
            continue;
        }
        let max_u = curved_project_face(
            mesh,
            kind,
            f,
            center,
            rotmat,
            fan,
            use_seams,
            island_offset + 0.5,
            &mut tag,
            uvs,
        );
        island_offset = max_u.max(island_offset).ceil();
    }
}

/// `uv_map_transform_calc_bounds`: bounds of the selected faces' vertices; the centre is the
/// default projection centre without a 3D view (`V3D_AROUND_CENTER_BOUNDS`).
pub fn selected_bounds(mesh: &impl ProjectMesh) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [-f32::MAX; 3];
    for f in 0..mesh.face_count() {
        if !mesh.face_selected(f) {
            continue;
        }
        for c in mesh.face_corners(f) {
            let co = mesh.corner_co(c);
            for k in 0..3 {
                if co[k] < min[k] {
                    min[k] = co[k];
                }
                if co[k] > max[k] {
                    max[k] = co[k];
                }
            }
        }
    }
    (min, max)
}

/// The midpoint of [`selected_bounds`] (`mid_v3_v3v3`).
pub fn bounds_center(min: [f32; 3], max: [f32; 3]) -> [f32; 3] {
    [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ]
}

/// `shrink_loop_uv_by_aspect_ratio` (`correct_aspect`, `aspect_y != 1`).
pub fn shrink_uvs_by_aspect_ratio(uvs: &mut [[f32; 2]], aspect_y: f32) {
    if aspect_y == 1.0 {
        return;
    }
    for uv in uvs {
        if aspect_y > 1.0 {
            uv[0] = uv[0] / aspect_y + (0.5 - 0.5 / aspect_y);
        } else {
            uv[1] = uv[1] * aspect_y + (0.5 - 0.5 * aspect_y);
        }
    }
}

/// `clip_to_bounds`: clamps every UV to the unit square.
pub fn clip_uvs(uvs: &mut [[f32; 2]]) {
    for uv in uvs {
        uv[0] = uv[0].clamp(0.0, 1.0);
        uv[1] = uv[1].clamp(0.0, 1.0);
    }
}

/// `scale_to_bounds`: rescales the UVs to fill the unit square.
pub fn scale_uvs_to_bounds(uvs: &mut [[f32; 2]]) {
    let mut min = [f32::MAX; 2];
    let mut max = [-f32::MAX; 2];
    for uv in uvs.iter() {
        for k in 0..2 {
            min[k] = min[k].min(uv[k]);
            max[k] = max[k].max(uv[k]);
        }
    }
    let mut dx = max[0] - min[0];
    let mut dy = max[1] - min[1];
    if dx > 0.0 {
        dx = 1.0 / dx;
    }
    if dy > 0.0 {
        dy = 1.0 / dy;
    }
    if dx == 1.0 && dy == 1.0 && min[0] == 0.0 && min[1] == 0.0 {
        return;
    }
    for uv in uvs {
        uv[0] = (uv[0] - min[0]) * dx;
        uv[1] = (uv[1] - min[1]) * dy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tube_and_sphere_basics() {
        assert_eq!(map_to_tube([0.0, -1.0, 1.0]), [0.0, 1.0]);
        assert_eq!(map_to_tube([1.0, 0.0, -1.0]), [0.25, 0.0]);
        assert_eq!(map_to_tube_ex([0.0, 0.0, 0.3]), ([0.5, 0.65], false));
        assert_eq!(map_to_sphere([0.0, 0.0, -1.0]), [0.5, 0.0]);
        assert_eq!(map_to_sphere([0.0, 0.0, 1.0]), [0.5, 1.0]);
        assert_eq!(map_to_sphere([1.0, 0.0, 0.0]), [0.25, 0.5]);
    }

    #[test]
    fn mirror_moves_wrapped_corners() {
        let mut uvs = [[0.9, 0.0], [0.1, 0.0], [0.1, 1.0], [0.9, 1.0]];
        uv_map_mirror(&mut uvs, &[true; 4], false);
        assert_eq!(uvs, [[0.9, 0.0], [1.1, 0.0], [1.1, 1.0], [0.9, 1.0]]);
    }

    #[test]
    fn fan_spreads_poles() {
        let mut uvs = [[0.5, 1.0], [0.2, 0.8], [0.4, 0.8]];
        uv_map_mirror(&mut uvs, &[false, true, true], true);
        assert!((uvs[0][0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn cube_projection_of_a_top_face() {
        let mut out = [[0.0; 2]; 2];
        cube_project_face(
            [0.0, 0.0, 1.0],
            &[[-1.0, -1.0, 1.0], [1.0, 1.0, 1.0]],
            [0.0; 3],
            2.0,
            &mut out,
        );
        assert_eq!(out, [[0.0, 0.0], [1.0, 1.0]]);
    }

    #[test]
    fn scale_to_bounds_fills_unit_square() {
        let mut uvs = [[0.5, 0.5], [1.5, 2.5]];
        scale_uvs_to_bounds(&mut uvs);
        assert_eq!(uvs, [[0.0, 0.0], [1.0, 1.0]]);
    }
}
