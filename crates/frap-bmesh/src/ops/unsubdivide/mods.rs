// SPDX-License-Identifier: GPL-3.0-or-later
//! The `bmesh_mods.cc` / `bmesh_query.cc` / `bmesh_core.cc` helpers `unsubdivide` and
//! `offset_edgeloops` need beyond the kernel of `crate::kernel`: vertex collapse and dissolve,
//! the duplicate-face checks of face splits, `BM_face_copy` and `BM_face_split_n`.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_mods.cc, bmesh_query.cc,
//! bmesh_core.cc and bmesh_construct.cc (Blender 5.2.4, GPL-2.0-or-later). Allocation and
//! kill order follow Blender's (the pools are LIFO, so later allocations land in the slots
//! freed here).

use crate::types::{CreateFlag, ElemFlag};
use crate::{BMesh, EdgeId, FaceId, LoopId, VertId};

/// `BM_vert_is_edge_pair`: exactly two edges use `v`.
pub(crate) fn vert_is_edge_pair(bm: &BMesh, v: VertId) -> bool {
    match bm.vert(v).e {
        Some(e) => {
            let e_other = bm.disk_edge_next(e, v);
            e_other != e && bm.disk_edge_next(e_other, v) == e
        }
        None => false,
    }
}

/// `BM_edge_share_face_check`.
pub(crate) fn edge_share_face_check(bm: &BMesh, e1: EdgeId, e2: EdgeId) -> bool {
    if bm.edge(e1).l.is_none() || bm.edge(e2).l.is_none() {
        return false;
    }
    bm.edge_loops(e1)
        .into_iter()
        .any(|l| bm.edge_in_face(e2, bm.lp(l).f))
}

/// `BM_face_pair_overlap_check_subset_same_winding` / `…_swap_winding`: the vertices from
/// `l_a` to `l_a_end` (forward) equal those from `l_b` (forward, or backward when `swap`).
fn overlap_check_subset(bm: &BMesh, l_a: LoopId, l_a_end: LoopId, l_b: LoopId, swap: bool) -> bool {
    let (mut a, mut b) = (l_a, l_b);
    loop {
        if bm.lp(b).v != bm.lp(a).v {
            return false;
        }
        if a == l_a_end {
            return true;
        }
        a = bm.lp(a).next;
        b = if swap { bm.lp(b).prev } else { bm.lp(b).next };
    }
}

/// `BM_face_exists_subset_from_face(l_a, l_b, f_len)`: a face of `f_len` corners over the
/// vertices from `l_a` to `l_b` (forward) already exists.
fn face_exists_subset_from_face(bm: &BMesh, l_a: LoopId, l_b: LoopId, f_len: usize) -> bool {
    let (va, vb) = (bm.lp(l_a).v, bm.lp(l_b).v);
    let Some(e_exists) = bm.edge_exists(va, vb) else {
        return false;
    };
    if bm.edge(e_exists).l.is_none() {
        return false;
    }
    for lr in bm.edge_loops(e_exists) {
        if bm.face(bm.lp(lr).f).len as usize != f_len {
            continue;
        }
        let swap_winding = bm.lp(lr).v == va;
        let (beg, end) = if swap_winding {
            (lr, bm.lp(lr).next)
        } else {
            (bm.lp(lr).next, lr)
        };
        let found = if swap_winding {
            overlap_check_subset(bm, bm.lp(l_a).next, bm.lp(l_b).prev, bm.lp(beg).prev, true)
        } else {
            overlap_check_subset(bm, bm.lp(l_a).next, bm.lp(l_b).prev, bm.lp(beg).next, false)
        };
        let _ = end;
        if found {
            return true;
        }
    }
    false
}

/// `BM_face_split_check_double_face(l_a, l_b, f_len)`: splitting the face between `l_a` and
/// `l_b` would create a face that already exists (on either side).
pub(crate) fn face_split_check_double_face(
    bm: &BMesh,
    l_a: LoopId,
    l_b: LoopId,
    f_len: usize,
) -> bool {
    let flen = bm.face(bm.lp(l_a).f).len as usize;
    let side_b_len = (flen + 2).wrapping_sub(f_len);
    face_exists_subset_from_face(bm, l_a, l_b, f_len)
        || face_exists_subset_from_face(bm, l_b, l_a, side_b_len)
}

/// `BM_face_exists_multi(varr, earr, len)`.
fn face_exists_multi(bm: &mut BMesh, varr: &[VertId], earr: &[EdgeId]) -> bool {
    let it = ElemFlag::INTERNAL_TAG;
    for i in 0..varr.len() {
        for l in bm.edge_loops(earr[i]) {
            let f = bm.lp(l).f;
            bm.face_mut(f).flag.remove(it);
            for v in bm.face_verts(f) {
                bm.vert_mut(v).flag.remove(it);
            }
        }
        for e in bm.vert_edges(varr[i]) {
            bm.edge_mut(e).flag.remove(it);
        }
    }
    for i in 0..varr.len() {
        bm.vert_mut(varr[i]).flag.insert(it);
        bm.edge_mut(earr[i]).flag.insert(it);
    }
    // 1) Tag all faces connected to edges whose vertices are all boundary vertices.
    let mut tot_tag = 0;
    for &e in earr {
        for l in bm.edge_loops(e) {
            let f = bm.lp(l).f;
            if !bm.face(f).flag.contains(it)
                && bm
                    .face_verts(f)
                    .into_iter()
                    .all(|v| bm.vert(v).flag.contains(it))
            {
                bm.face_mut(f).flag.insert(it);
                tot_tag += 1;
            }
        }
    }
    let mut ok = false;
    if tot_tag != 0 {
        // 2) Non-boundary edges using boundary vertices must have two tagged faces. Blender
        // reuses the edge iterator for the inner face loop, so the edge walk of a vertex ends
        // after the first such edge.
        ok = true;
        'outer: for &v in varr {
            for e in bm.vert_edges(v) {
                let [a, b] = bm.edge(e).v;
                if !bm.edge(e).flag.contains(it)
                    && bm.vert(a).flag.contains(it)
                    && bm.vert(b).flag.contains(it)
                {
                    let tot_face_tag = bm
                        .edge_loops(e)
                        .into_iter()
                        .filter(|&l| bm.face(bm.lp(l).f).flag.contains(it))
                        .count();
                    if tot_face_tag != 2 {
                        ok = false;
                        break 'outer;
                    }
                    break;
                }
            }
        }
    }
    for i in 0..varr.len() {
        bm.vert_mut(varr[i]).flag.remove(it);
        bm.edge_mut(earr[i]).flag.remove(it);
    }
    ok
}

/// `BM_face_exists_multi_edge(earr, len)`: a face uses exactly the closed edge loop `earr`.
pub(crate) fn face_exists_multi_edge(bm: &mut BMesh, earr: &[EdgeId]) -> bool {
    let it = ElemFlag::INTERNAL_TAG;
    for &e in earr {
        let [a, b] = bm.edge(e).v;
        bm.vert_mut(a).flag.insert(it);
        bm.vert_mut(b).flag.insert(it);
    }
    let mut varr = Vec::with_capacity(earr.len());
    for &e in earr {
        for v in bm.edge(e).v {
            if bm.vert(v).flag.contains(it) {
                bm.vert_mut(v).flag.remove(it);
                varr.push(v);
            }
        }
    }
    if varr.len() != earr.len() {
        return false;
    }
    face_exists_multi(bm, &varr, earr)
}

/// `BM_faces_join_pair(l_a, l_b, do_del)`: joins the faces of two loops of one edge.
pub(crate) fn faces_join_pair(
    bm: &mut BMesh,
    l_a: LoopId,
    l_b: LoopId,
    do_del: bool,
) -> (Option<FaceId>, Option<FaceId>) {
    if bm.lp(l_a).v == bm.lp(l_b).v {
        let fb = bm.lp(l_b).f;
        bm.loop_reverse(fb);
    }
    let faces = [bm.lp(l_a).f, bm.lp(l_b).f];
    bm.faces_join(&faces, do_del, true)
}

/// `BM_vert_collapse_faces(e_kill, v_kill, fac, do_del, join_faces, kill_degenerate_faces,
/// kill_duplicate_faces)`: removes the 2-valence vertex `v_kill`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn vert_collapse_faces(
    bm: &mut BMesh,
    e_kill: EdgeId,
    v_kill: VertId,
    fac: f32,
    do_del: bool,
    join_faces: bool,
    kill_degenerate_faces: bool,
    kill_duplicate_faces: bool,
) -> Option<EdgeId> {
    let tv = bm.edge_other_vert(e_kill, v_kill);
    // First modify the face loop data.
    if bm.edge(e_kill).l.is_some() {
        let w = [1.0 - fac, fac];
        for l in bm.edge_loops(e_kill) {
            let ll = bm.lp(l).clone();
            if ll.v == tv && bm.lp(ll.next).v == v_kill {
                let (tvloop, kvloop) = (l, ll.next);
                bm.ldata.interp(&[kvloop.0, tvloop.0], &w, kvloop.0);
            }
        }
    }
    // Now interpolate the vertex data.
    bm.data_interp_from_verts(v_kill, tv, v_kill, fac);

    let e2 = bm.disk_edge_next(e_kill, v_kill);
    let tv2 = bm.edge_other_vert(e2, v_kill);
    if join_faces {
        let faces = bm.vert_faces(v_kill);
        let mut e_new = None;
        if faces.len() >= 2 {
            let (f2, f_double) = bm.faces_join(&faces, true, true);
            if let Some(f2) = f2 {
                if kill_duplicate_faces && let Some(fd) = f_double {
                    bm.face_kill(fd);
                }
                if let (Some(l_a), Some(l_b)) = (
                    bm.face_vert_share_loop(f2, tv),
                    bm.face_vert_share_loop(f2, tv2),
                ) && let Some((_, l_new)) = bm.face_split(f2, l_a, l_b, None, false)
                {
                    e_new = Some(bm.lp(l_new).e);
                }
            }
        }
        e_new
    } else {
        bm.join_edge_kill_vert(
            e_kill,
            v_kill,
            do_del,
            true,
            kill_degenerate_faces,
            kill_duplicate_faces,
        )
    }
}

/// `BM_vert_collapse_edge(e_kill, v_kill, do_del, kill_degenerate_faces,
/// kill_duplicate_faces)`: collapses the 2-valence vertex without joining faces.
pub(crate) fn vert_collapse_edge(
    bm: &mut BMesh,
    e_kill: EdgeId,
    v_kill: VertId,
    do_del: bool,
    kill_degenerate_faces: bool,
    kill_duplicate_faces: bool,
) -> Option<EdgeId> {
    vert_collapse_faces(
        bm,
        e_kill,
        v_kill,
        1.0,
        do_del,
        false,
        kill_degenerate_faces,
        kill_duplicate_faces,
    )
}

/// `BM_vert_dissolve`.
pub(crate) fn vert_dissolve(bm: &mut BMesh, v: VertId) -> bool {
    let len = bm.vert_edge_count_at_most(v, 3);
    if len == 1 {
        bm.vert_kill(v);
        return true;
    }
    if !bm.vert_is_manifold(v) {
        let Some(e) = bm.vert(v).e else {
            bm.vert_kill(v);
            return true;
        };
        if bm.edge(e).l.is_none() {
            if len == 2 {
                return vert_collapse_edge(bm, e, v, true, true, true).is_some();
            }
            return false;
        }
        return false;
    }
    if len == 2 && bm.vert_face_count(v) == 1 {
        // Boundary vertex on a face.
        let e = bm.vert(v).e.expect("vertex has edges");
        return vert_collapse_edge(bm, e, v, true, true, true).is_some();
    }
    disk_dissolve(bm, v)
}

/// `BM_disk_dissolve`.
pub(crate) fn disk_dissolve(bm: &mut BMesh, v: VertId) -> bool {
    if !bm.vert_is_manifold(v) {
        return false;
    }
    let (mut keepedge, mut baseedge) = (None, None);
    let mut len = 0;
    let mut e_last = None;
    if let Some(ve) = bm.vert(v).e {
        let mut e = ve;
        loop {
            e = bm.disk_edge_next(e, v);
            if !edge_share_face_check(bm, e, ve) {
                keepedge = Some(e);
                baseedge = Some(ve);
                break;
            }
            len += 1;
            if e == ve {
                break;
            }
        }
        e_last = Some(e);
    }

    if keepedge.is_none() && len == 3 {
        let Some(e) = e_last else {
            return false;
        };
        let Some(el) = bm.edge(e).l else {
            return false;
        };
        let rn = bm.lp(el).radial_next;
        let (f, _) = faces_join_pair(bm, el, rn, true);
        if f.is_none() {
            return false;
        }
        let Some(ve) = bm.vert(v).e else {
            return false;
        };
        return vert_collapse_faces(bm, ve, v, 1.0, true, false, true, true).is_some();
    }
    if keepedge.is_none() && len == 2 {
        let Some(ve) = bm.vert(v).e else {
            return false;
        };
        let Some(e) = vert_collapse_faces(bm, ve, v, 1.0, true, true, true, true) else {
            return false;
        };
        // Handle two-valence.
        if let Some(el) = bm.edge(e).l {
            let rn = bm.lp(el).radial_next;
            if el != rn && faces_join_pair(bm, el, rn, true).0.is_none() {
                return false;
            }
        }
        return true;
    }

    if let (Some(keepedge), Some(baseedge)) = (keepedge, baseedge) {
        let mut done = false;
        while !done {
            done = true;
            let Some(ve) = bm.vert(v).e else {
                return false;
            };
            let mut e = ve;
            loop {
                let mut joined = false;
                if bm.edge_is_manifold(e) && e != baseedge && e != keepedge {
                    let el = bm.edge(e).l.expect("manifold edge");
                    let rn = bm.lp(el).radial_next;
                    let (f, _) = faces_join_pair(bm, el, rn, true);
                    if f.is_none() {
                        return false;
                    }
                    joined = true;
                }
                if joined {
                    done = false;
                    break;
                }
                e = bm.disk_edge_next(e, v);
                if e == ve {
                    break;
                }
            }
        }
        // Collapse the vertex; the base edge can be a boundary of a manifold.
        let join = !bm.edge_is_boundary(baseedge);
        let Some(e) = vert_collapse_faces(bm, baseedge, v, 1.0, true, join, true, true) else {
            return false;
        };
        if let Some(el) = bm.edge(e).l {
            let rn = bm.lp(el).radial_next;
            if el != rn && faces_join_pair(bm, el, rn, true).0.is_none() {
                return false;
            }
        }
    }
    true
}

/// `BM_face_copy(bm, f, copy_verts = true, copy_edges = true)`: a detached copy of `f` with
/// its own vertices (created in loop order) and edges.
pub(crate) fn face_copy_detached(bm: &mut BMesh, f: FaceId) -> FaceId {
    let loops = bm.face_loops(f);
    let n = loops.len();
    let mut verts = Vec::with_capacity(n);
    for &l in &loops {
        let v = bm.lp(l).v;
        let co = bm.vert(v).co;
        verts.push(bm.vert_create(co, Some(v), CreateFlag::empty()));
    }
    let mut edges = Vec::with_capacity(n);
    for (i, &l) in loops.iter().enumerate() {
        let e = bm.lp(l).e;
        let (v1, v2) = if bm.edge(e).v[0] == bm.lp(l).v {
            (verts[i], verts[(i + 1) % n])
        } else {
            (verts[(i + 1) % n], verts[i])
        };
        edges.push(bm.edge_create(v1, v2, Some(e), CreateFlag::empty()));
    }
    let f_copy = bm
        .face_create(&verts, &edges, None, CreateFlag::SKIP_CD)
        .expect("a face has corners");
    bm.face_attrs_copy(f, f_copy);
    let copy_loops = bm.face_loops(f_copy);
    for (&l, &lc) in loops.iter().zip(&copy_loops) {
        bm.loop_attrs_copy(l, lc);
    }
    f_copy
}

/// `BM_face_split_n(f, l_a, l_b, cos, n)`: splits `f` between `l_a` and `l_b` with `cos.len()`
/// new vertices along the new edge (from `l_b`'s vertex); returns the new face and the new
/// face's loop on the first piece of the new edge.
pub(crate) fn face_split_n(
    bm: &mut BMesh,
    f: FaceId,
    l_a: LoopId,
    l_b: LoopId,
    cos: &[[f32; 3]],
) -> Option<(FaceId, LoopId)> {
    let n = cos.len();
    let adjacent = bm.lp(l_a).next == l_b || bm.lp(l_a).prev == l_b;
    if (n == 0 && adjacent) || bm.lp(l_a).f != bm.lp(l_b).f {
        return None;
    }
    let v_b = bm.lp(l_b).v;
    let f_tmp = face_copy_detached(bm, f);
    let r = bm.split_face_make_edge(f, l_a, l_b, None, false);
    if let Some((_, l_new)) = r {
        let mut e = bm.lp(l_new).e;
        for co in cos {
            let (v_new, e_new) = bm.split_edge_make_vert(v_b, e);
            bm.vert_mut(v_new).co = *co;
            // Interpolate the loop data of the corners at `v_new` from the original face.
            for e_iter in [e, e_new] {
                for l in bm.edge_loops(e_iter) {
                    if bm.lp(l).v == v_new {
                        bm.loop_interp_from_face(l, f_tmp, true);
                    }
                }
            }
            e = e_new;
        }
    }
    bm.face_verts_kill(f_tmp);
    r
}
