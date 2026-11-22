// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_overlap(extend=False)`, tier T1
//! (`tests/golden/cases/uv/select/overlap*.py`). Port of `uv_select_overlap` and
//! `overlap_tri_tri_uv_test` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. Unless `extend`, every UV is deselected first. Every visible
//! face of every mesh in Edit Mode with a UV map is triangulated in UV space (`polyfill` +
//! `beautify`); the faces with a triangle overlapping a triangle of another face (of any of
//! the meshes) are selected with the sticky mode, with `use_uv_select_island` their whole UV
//! islands. Touching corners or edges are no overlap; identical triangles are. Always finishes.
//!
//! The triangles go into a BVH (`BLI_bvhtree_new(n, 0, 4, 6)`, the shared port of
//! `BLI_kdopbvh`), probed with each triangle in turn, Blender's order: a source triangle whose
//! face is already tagged is skipped, the others test `overlap_tri_tri_uv_test(src, dst)`
//! against the leaves their bounds touch, in the tree's traversal order, and stop at the first
//! hit (`max_interactions` 1), tagging both faces. The test is not symmetric in floating point
//! (the intersection point is found on the source's segment), so both the probe direction and
//! the order are Blender's.

mod beautify;
mod heap;

use frap_bmesh::{ElemFlag, FaceId};
use frap_bmesh::ops::contextual_create::connect_islands::isect_seg_seg_v2_point_ex;
use frap_bmesh::ops::contextual_create::kdopbvh::BvhTree;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::linked::LinkedHelper;
use crate::select::{SelAction, Uv, UvSel, op};

/// One UV triangle of a face.
struct Tri {
    ob: usize,
    face: FaceId,
    /// Index of the face among all triangulated faces (of every mesh).
    slot: usize,
    co: [Uv; 3],
}

/// `line_point_side_v2`.
fn line_point_side_v2(l1: Uv, l2: Uv, pt: Uv) -> f32 {
    ((l1[0] - pt[0]) * (l2[1] - pt[1])) - ((l2[0] - pt[0]) * (l1[1] - pt[1]))
}

/// `isect_point_tri_v2 != 0`.
fn isect_point_tri_v2(pt: Uv, v1: Uv, v2: Uv, v3: Uv) -> bool {
    let s12 = line_point_side_v2(v1, v2, pt);
    let s23 = line_point_side_v2(v2, v3, pt);
    let s31 = line_point_side_v2(v3, v1, pt);
    (s12 >= 0.0 && s23 >= 0.0 && s31 >= 0.0) || (s12 <= 0.0 && s23 <= 0.0 && s31 <= 0.0)
}

/// `mid_v2_v2v2v2`.
fn mid3(t: &[Uv; 3]) -> Uv {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
    ]
}

/// `overlap_tri_tri_uv_test`.
pub(crate) fn overlap_tri_tri_uv_test(t1: &[Uv; 3], t2: &[Uv; 3], endpoint_bias: f32) -> bool {
    for (a, b) in [(0, 1), (1, 2), (2, 0)] {
        for (c, d) in [(0, 1), (1, 2), (2, 0)] {
            // Blender tests 8 of the 9 pairs: not (t1[2], t1[0]) × (t2[2], t2[0]).
            if a == 2 && c == 2 {
                continue;
            }
            if isect_seg_seg_v2_point_ex(t1[a], t1[b], t2[c], t2[d], endpoint_bias).is_some() {
                return true;
            }
        }
    }
    if isect_point_tri_v2(mid3(t1), t2[0], t2[1], t2[2]) {
        return true;
    }
    isect_point_tri_v2(mid3(t2), t1[0], t1[1], t1[2])
}

/// The triangles of the visible faces of one mesh; `slots` counts the faces so far.
fn triangles(s: &UvSel, ob: usize, slots: &mut usize, out: &mut Vec<Tri>) {
    for f in s.faces() {
        if !s.face_visible(f) {
            continue;
        }
        let coords: Vec<Uv> = s.face_loops(f).into_iter().map(|l| s.uv(l)).collect();
        let mut tris = frap_geom::polyfill::polyfill_calc(&coords, 0);
        beautify::polyfill_beautify(&coords, &mut tris);
        for t in tris {
            out.push(Tri {
                ob,
                face: f,
                slot: *slots,
                co: [
                    coords[t[0] as usize],
                    coords[t[1] as usize],
                    coords[t[2] as usize],
                ],
            });
        }
        *slots += 1;
    }
}

/// The triangle as the 3D points the BVH takes.
fn tri_3d(t: &Tri) -> [[f32; 3]; 3] {
    t.co.map(|c| [c[0], c[1], 0.0])
}

// Ported from blender/source/blender/editors/uvedit/uvedit_select.cc (uv_select_overlap:
// the BVH, the probe tree and `bvh_overlap_fn`), Blender 5.2.4, GPL-2.0-or-later.
/// The faces (by object) with a triangle overlapping another face's, `faces` faces in all.
fn overlapping(tris: &[Tri], faces: usize, objects: usize) -> Vec<Vec<FaceId>> {
    let mut uv_tree = BvhTree::new(0.0, 4, 6);
    for (i, t) in tris.iter().enumerate() {
        uv_tree.insert(i as i32, &tri_3d(t));
    }
    uv_tree.balance();
    let mut tagged = vec![false; faces];
    let mut out = vec![Vec::new(); objects];
    let mut tag = |t: &Tri, tagged: &mut [bool]| {
        if !tagged[t.slot] {
            tagged[t.slot] = true;
            out[t.ob].push(t.face);
        }
    };
    for (i, src) in tris.iter().enumerate() {
        frap_math::budget::check();
        if tagged[src.slot] {
            continue;
        }
        let mut probe = BvhTree::new(0.0, 4, 6);
        probe.insert(0, &tri_3d(src));
        probe.balance();
        probe.overlap_ex(&uv_tree, 1, false, &mut |_, dst_i, _| {
            let dst_i = dst_i as usize;
            if dst_i == i {
                return false;
            }
            let dst = &tris[dst_i];
            if src.ob == dst.ob && src.face == dst.face {
                return false;
            }
            if overlap_tri_tri_uv_test(&src.co, &dst.co, -1e-4) {
                tag(src, &mut tagged);
                tag(dst, &mut tagged);
                return true;
            }
            false
        });
    }
    out
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SelectOverlap) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let extend = p.extend.value;
    let select_island = ts.select_island_check();
    let objects = op::objects(cx);
    let mut tris = Vec::new();
    let mut slots = 0;
    for (i, &(_, me)) in objects.iter().enumerate() {
        op::read(cx, me, ts, |s| triangles(s, i, &mut slots, &mut tris));
    }
    let tagged = overlapping(&tris, slots, objects.len());
    for (i, &(_, me)) in objects.iter().enumerate() {
        let faces = &tagged[i];
        op::edit(cx, me, ts, |s| {
            s.face_tags_clear();
            if !extend {
                s.deselect_all(SelAction::Deselect);
            }
            if faces.is_empty() {
                return;
            }
            for &f in faces {
                s.set_fflag(f, ElemFlag::TAG, true);
            }
            if select_island {
                let mut helper = LinkedHelper::new();
                for f in s.faces() {
                    if s.fflag(f, ElemFlag::TAG) {
                        helper.face_add(s, f);
                    }
                }
                helper.tag_all(s);
            }
            s.prepare();
            s.flush_from_tag_face(true);
            if s.ts.sync {
                s.select_sync_flush(true);
            } else {
                s.selectmode_flush();
            }
        })?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_is_no_overlap_identical_is() {
        let a = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let b = [[1.0, 0.0], [2.0, 0.0], [1.0, 1.0]];
        let c = [[0.2, 0.2], [1.2, 0.2], [0.2, 1.2]];
        assert!(!overlap_tri_tri_uv_test(&a, &b, -1e-4));
        assert!(overlap_tri_tri_uv_test(&a, &a, -1e-4));
        assert!(overlap_tri_tri_uv_test(&a, &c, -1e-4));
    }
}
