// SPDX-License-Identifier: GPL-3.0-or-later
//! The Python helpers of `uv.align_rotation` and `uv.randomize_uv_transform`: the UV-island
//! search of `bpy_extras.bmesh_utils.bmesh_linked_uv_islands` and the selection tests of
//! `uvcalc_transform.py` (`is_face_uv_selected_fn_from_context`,
//! `is_loop_edge_uv_selected_fn_from_context`), with mathutils' `Vector` equality.
//!
//! Ported from blender/scripts/modules/bpy_extras/bmesh_utils.py,
//! blender/scripts/startup/bl_operators/uvcalc_transform.py and
//! blender/source/blender/python/mathutils/mathutils.cc (`EXPP_FloatsAreEqual`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! `bm.uv_select_sync_valid` is false at the start of every frap operator (deviation D-111),
//! so with `use_uv_select_sync` the `sync_invalid` tests apply (the mesh selection).

use frap_bmesh::{ElemFlag, FaceId, LoopId};

use crate::select::{Uv, UvSel};

/// `EXPP_FloatsAreEqual(af, bf, max_diff)`: equal within `max_diff` ulps.
pub(crate) fn floats_are_equal(af: f32, bf: f32, max_diff: i32) -> bool {
    let ai = af.to_bits() as i32;
    let bi = bf.to_bits() as i32;
    let test = (ai ^ bi) >> 31;
    let diff = (ai ^ (test & 0x7fff_ffff)).wrapping_sub(bi);
    let v1 = max_diff.wrapping_add(diff);
    let v2 = max_diff.wrapping_sub(diff);
    (v1 | v2) >= 0
}

/// `Vector.__eq__` (`EXPP_VectorsAreEqual(a, b, 2, 1)`).
pub(crate) fn uv_eq(a: Uv, b: Uv) -> bool {
    floats_are_equal(a[0], b[0], 1) && floats_are_equal(a[1], b[1], 1)
}

/// `match_uv`.
fn match_uv(s: &UvSel, face: FaceId, vert: frap_bmesh::VertId, uv: Uv) -> bool {
    for l in s.face_loops(face) {
        if s.lv(l) == vert {
            return uv_eq(uv, s.uv(l));
        }
    }
    false
}

/// `bmesh_linked_uv_islands`: the faces of every UV island (hidden faces included), each
/// island in discovery order.
pub(crate) fn linked_uv_islands(s: &UvSel) -> Vec<Vec<FaceId>> {
    let mut result = Vec::new();
    let mut used = vec![false; s.bm.faces.capacity()];
    for seed in s.faces() {
        if used[seed.slot()] {
            continue;
        }
        used[seed.slot()] = true;
        let mut island = vec![seed];
        let mut stack = vec![seed];
        while let Some(current) = stack.pop() {
            for l in s.face_loops(current) {
                let v = s.lv(l);
                let uv = s.uv(l);
                for f in s.bm.vert_faces(v) {
                    if f == current || used[f.slot()] {
                        continue;
                    }
                    if !match_uv(s, f, v, uv) {
                        continue;
                    }
                    used[f.slot()] = true;
                    island.push(f);
                    stack.push(f);
                }
            }
        }
        result.push(island);
    }
    result
}

/// `is_face_uv_selected_fn_from_context(scene, bm)(face, any_edge)`.
pub(crate) fn face_uv_selected(s: &UvSel, face: FaceId, any_edge: bool) -> bool {
    let fl = s.bm.face(face).flag;
    if fl.contains(ElemFlag::HIDDEN) {
        return false;
    }
    if s.ts.sync {
        // `is_face_uv_selected_for_uv_select_sync_invalid`.
        if fl.contains(ElemFlag::SELECT) {
            return true;
        }
        return any_edge
            && s.face_loops(face)
                .into_iter()
                .any(|l| s.eflag(s.le(l), ElemFlag::SELECT));
    }
    // `is_face_uv_selected_for_uv_select_no_sync`.
    if fl.contains(ElemFlag::SELECT) {
        if fl.contains(ElemFlag::SELECT_UV) {
            return true;
        }
        if any_edge
            && s.face_loops(face)
                .into_iter()
                .any(|l| s.lflag(l, ElemFlag::SELECT_UV_EDGE))
        {
            return true;
        }
    }
    false
}

/// `is_loop_edge_uv_selected_fn_from_context(scene, bm)(loop)`.
pub(crate) fn loop_edge_uv_selected(s: &UvSel, l: LoopId) -> bool {
    let fl = s.bm.face(s.lf(l)).flag;
    if fl.contains(ElemFlag::HIDDEN) {
        return false;
    }
    if s.ts.sync {
        return s.eflag(s.le(l), ElemFlag::SELECT);
    }
    fl.contains(ElemFlag::SELECT) && s.lflag(l, ElemFlag::SELECT_UV_EDGE)
}

/// `is_island_uv_selected`.
pub(crate) fn island_uv_selected(s: &UvSel, island: &[FaceId], any_edge: bool) -> bool {
    island.iter().any(|&f| face_uv_selected(s, f, any_edge))
}

/// `island_uv_bounds_center` (Python floats).
pub(crate) fn island_uv_bounds_center(s: &UvSel, island: &[FaceId]) -> (f64, f64) {
    let mut minmax = [1e30f64, 1e30, -1e30, -1e30];
    for &f in island {
        for l in s.face_loops(f) {
            let uv = s.uv(l);
            let (u, v) = (f64::from(uv[0]), f64::from(uv[1]));
            minmax[0] = py_min(minmax[0], u);
            minmax[1] = py_min(minmax[1], v);
            minmax[2] = py_max(minmax[2], u);
            minmax[3] = py_max(minmax[3], v);
        }
    }
    ((minmax[0] + minmax[2]) / 2.0, (minmax[1] + minmax[3]) / 2.0)
}

/// Python `min(a, b)`: `b` only when `b < a`.
fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// Python `max(a, b)`: `b` only when `b > a`.
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_equality_allows_one_ulp() {
        let a = 0.5f32;
        let b = f32::from_bits(a.to_bits() + 1);
        let c = f32::from_bits(a.to_bits() + 2);
        assert!(floats_are_equal(a, b, 1));
        assert!(!floats_are_equal(a, c, 1));
        assert!(floats_are_equal(0.0, -0.0, 1));
        assert!(!floats_are_equal(1.0, -1.0, 1));
        assert!(uv_eq([0.25, 0.75], [0.25, 0.75]));
    }
}
