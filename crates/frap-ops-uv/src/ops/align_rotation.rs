// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.align_rotation(method='AUTO', axis='X', correct_aspect=False)`, tier T1
//! (`tests/golden/cases/uv/edit_m8/align_rotation.py`). Port of the Python operator
//! `AlignUVRotation` (`find_rotation_auto`, `find_rotation_edge`, `find_rotation_geometry`,
//! `align_uv_rotation_island`, `align_uv_rotation_bmesh`;
//! `scripts/startup/bl_operators/uvcalc_transform.py`), Blender 5.2.4, GPL-2.0-or-later, with
//! Python's double precision and mathutils' single precision where the script uses them.
//!
//! Poll `context.mode == 'EDIT_MESH'`. Every UV island (`bmesh_linked_uv_islands`: faces
//! sharing a vertex at the same UV, hidden faces included) with a selected face of every mesh
//! in Edit Mode with a UV map is rotated about its bounds centre: `AUTO` aligns the average
//! edge direction (modulo 90°) with the axes, `EDGE` the selected UV edges (modulo 180°),
//! `GEOMETRY` turns the island so that the 3D `axis` points along V. `correct_aspect` reads
//! the image of the Image Editor: frap's virtual one shows none (aspect 1; in Blender a
//! context without an Image Editor raises `AttributeError`). Always finishes.

pub(crate) mod py;

use frap_bmesh::FaceId;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::weld::common;
use crate::select::{UvSel, op};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "correct_aspect reads the image of frap's virtual Image Editor, which shows none (aspect 1); Blender outside an Image Editor raises AttributeError",
    ]
)]
fn exec(cx: &mut OpCtx, p: uv::AlignRotation) -> OpResult {
    let method = p.method.value.identifier();
    let axis = match p.axis.value.identifier() {
        "Y" => 1,
        "Z" => 2,
        _ => 0,
    };
    // `get_aspect_y`: the virtual Image Editor shows no image.
    let aspect_y = 1.0f64;
    let ts = op::ts(cx);
    for (_, me) in common::objects(cx) {
        common::edit(cx, me, ts, |s| {
            let mut changed = false;
            for island in py::linked_uv_islands(s) {
                if py::island_uv_selected(s, &island, method == "EDGE")
                    && align_island(s, &island, method, axis, aspect_y)
                {
                    changed = true;
                }
            }
            ((), changed)
        })?;
    }
    Ok(OpStatus::Finished)
}

/// `find_rotation_auto`.
fn rotation_auto(s: &UvSel, faces: &[FaceId], aspect_y: f64) -> f64 {
    let mut sum_u = 0.0f64;
    let mut sum_v = 0.0f64;
    for &f in faces {
        let loops = s.face_loops(f);
        let mut prev = s.uv(*loops.last().expect("a face has corners"));
        for &l in &loops {
            let uv = s.uv(l);
            let du = f64::from(uv[0]) - f64::from(prev[0]);
            let dv = f64::from(uv[1]) - f64::from(prev[1]);
            let edge_angle = libm::atan2(dv, du * aspect_y) * 4.0;
            sum_u += libm::cos(edge_angle);
            sum_v += libm::sin(edge_angle);
            prev = uv;
        }
    }
    -libm::atan2(sum_v, sum_u) / 4.0
}

/// `find_rotation_edge`.
fn rotation_edge(s: &UvSel, faces: &[FaceId], aspect_y: f64) -> f64 {
    let mut sum_u = 0.0f64;
    let mut sum_v = 0.0f64;
    for &f in faces {
        let loops = s.face_loops(f);
        let last = *loops.last().expect("a face has corners");
        let mut prev = s.uv(last);
        let mut prev_select = py::loop_edge_uv_selected(s, last);
        for &l in &loops {
            let uv = s.uv(l);
            if prev_select {
                let du = f64::from(uv[0]) - f64::from(prev[0]);
                let dv = f64::from(uv[1]) - f64::from(prev[1]);
                let edge_angle = libm::atan2(dv, du * aspect_y) * 2.0;
                sum_u += libm::cos(edge_angle);
                sum_v += libm::sin(edge_angle);
            }
            prev = uv;
            prev_select = py::loop_edge_uv_selected(s, l);
        }
    }
    // Add 90 degrees to align along V (twice, because the angle is halved).
    let (sum_u, sum_v) = (-sum_u, -sum_v);
    -libm::atan2(sum_v, sum_u) / 2.0
}

/// `find_rotation_geometry` (mathutils vectors and matrices are single precision).
fn rotation_geometry(s: &UvSel, faces: &[FaceId], axis: usize, aspect_y: f64) -> f64 {
    let mut sum_u_co = [0.0f32; 3];
    let mut sum_v_co = [0.0f32; 3];
    let aspect = aspect_y as f32;
    for &f in faces {
        let loops = s.face_loops(f);
        let uv0 = s.uv(loops[0]);
        let co0 = s.bm.vert(s.lv(loops[0])).co;
        for fan in 2..loops.len() {
            let a = s.uv(loops[fan - 1]);
            let b = s.uv(loops[fan]);
            let mut d0 = [a[0] - uv0[0], a[1] - uv0[1]];
            let mut d1 = [b[0] - uv0[0], b[1] - uv0[1]];
            d0[0] *= aspect;
            d1[0] *= aspect;
            let mat = invert_safe_m2([[d0[0], d0[1]], [d1[0], d1[1]]]);
            let ca = s.bm.vert(s.lv(loops[fan - 1])).co;
            let cb = s.bm.vert(s.lv(loops[fan])).co;
            let dc0 = [ca[0] - co0[0], ca[1] - co0[1], ca[2] - co0[2]];
            let dc1 = [cb[0] - co0[0], cb[1] - co0[1], cb[2] - co0[2]];
            let cr = [
                dc0[1] * dc1[2] - dc0[2] * dc1[1],
                dc0[2] * dc1[0] - dc0[0] * dc1[2],
                dc0[0] * dc1[1] - dc0[1] * dc1[0],
            ];
            // `Vector.length`: `sqrt(dot_vn_vn(...))` in double, from the last component.
            let dot =
                f64::from(cr[2] * cr[2]) + f64::from(cr[1] * cr[1]) + f64::from(cr[0] * cr[0]);
            let w = dot.sqrt() as f32;
            for k in 0..3 {
                let u = dc0[k] * mat[0][0] + dc1[k] * mat[0][1];
                let v = dc0[k] * mat[1][0] + dc1[k] * mat[1][1];
                sum_u_co[k] += u * w;
                sum_v_co[k] += v * w;
            }
        }
    }
    libm::atan2(f64::from(sum_u_co[axis]), f64::from(sum_v_co[axis]))
}

/// `Matrix.invert_safe` of a 2x2 matrix given by rows (`matrix_invert_safe_internal`).
fn invert_safe_m2(m: [[f32; 2]; 2]) -> [[f32; 2]; 2] {
    let mut m = m;
    let mut det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    if det == 0.0 {
        let eps = 1e-8f32;
        m[0][0] += eps;
        m[1][1] += eps;
        det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if det == 0.0 {
            m = [[1.0, 0.0], [0.0, 1.0]];
            det = 1.0;
        }
    }
    [
        [m[1][1] / det, -m[0][1] / det],
        [-m[1][0] / det, m[0][0] / det],
    ]
}

/// `align_uv_rotation_island`.
fn align_island(s: &mut UvSel, faces: &[FaceId], method: &str, axis: usize, aspect_y: f64) -> bool {
    let angle = match method {
        "EDGE" => rotation_edge(s, faces, aspect_y),
        "GEOMETRY" => rotation_geometry(s, faces, axis, aspect_y),
        _ => rotation_auto(s, faces, aspect_y),
    };
    if angle == 0.0 {
        return false;
    }
    let (mid_u, mid_v) = py::island_uv_bounds_center(s, faces);
    let cos_angle = libm::cos(angle);
    let sin_angle = libm::sin(angle);
    let delta_u = mid_u - cos_angle * mid_u + sin_angle / aspect_y * mid_v;
    let delta_v = mid_v - sin_angle * aspect_y * mid_u - cos_angle * mid_v;
    for &f in faces {
        for l in s.face_loops(f) {
            let pre = s.uv(l);
            let (pu, pv) = (f64::from(pre[0]), f64::from(pre[1]));
            let u = cos_angle * pu - sin_angle / aspect_y * pv + delta_u;
            let v = sin_angle * aspect_y * pu + cos_angle * pv + delta_v;
            s.uv[l.slot()] = [u as f32, v as f32];
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_safe_handles_singular_matrices() {
        let inv = invert_safe_m2([[2.0, 0.0], [0.0, 4.0]]);
        assert_eq!(inv, [[0.5, 0.0], [0.0, 0.25]]);
        let inv = invert_safe_m2([[0.0, 0.0], [0.0, 0.0]]);
        assert!((inv[0][0] / 1e8 - 1.0).abs() < 1e-5);
        assert_eq!(inv[0][1], 0.0);
    }
}
