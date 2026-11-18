// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.rotate_edges` (Blender 5.2.4 `bmo_rotate_edges_exec`,
//! `operators/bmo_rotate_edges.cc`) and `BM_edge_rotate` (`intern/bmesh_mods.cc`), implemented
//! by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.rotate_edges(bmesh, edges=[], use_ccw=False)`.
//!
//! **Create/kill order** of one rotation (`BM_edge_rotate`): the new edge is created first
//! (`BM_edge_create` copying `e`'s data), the two faces are joined (`BM_faces_join`, killing
//! `e` and the second face; the joined face is a new face), then the joined face is split
//! between the new edge's vertices (`BM_face_split`, reusing the new edge); both faces get
//! back the header flags of the faces they replace. Edges whose faces are shared with
//! another input edge are rotated longest first, starting from region boundaries (a
//! `BLI_heap` ordered by `-len²`), so the result does not depend on the input order.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use rustc_hash::FxHashMap;

use crate::ops::beautify_fill::heap::{Heap, HeapNode};
use crate::ops::contextual_create::{bli, construct};
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, LoopId};

/// Input slots of `rotate_edges` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RotateEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_ccw`.
    pub use_ccw: bool,
}

/// Output slots of `rotate_edges`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RotateEdgesOut {
    /// `edges.out` as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out`: the rotated edges (mesh order).
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_rotate_edges_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// `BM_EDGEROT_CHECK_EXISTS`.
pub const CHECK_EXISTS: u8 = 1;
/// `BM_EDGEROT_CHECK_SPLICE`.
pub const CHECK_SPLICE: u8 = 2;
/// `BM_EDGEROT_CHECK_DEGENERATE`.
pub const CHECK_DEGENERATE: u8 = 4;
/// `BM_EDGEROT_CHECK_BEAUTY`.
pub const CHECK_BEAUTY: u8 = 8;

/// `BM_edge_face_pair`: the two faces of a manifold edge.
pub fn edge_face_pair(bm: &BMesh, e: EdgeId) -> Option<(FaceId, FaceId)> {
    let la = bm.edge(e).l?;
    let lb = bm.lp(la).radial_next;
    if la != lb && bm.lp(lb).radial_next == la {
        Some((bm.lp(la).f, bm.lp(lb).f))
    } else {
        None
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mods.cc (BM_edge_rotate_check).
/// `BM_edge_rotate_check`.
pub fn edge_rotate_check(bm: &BMesh, e: EdgeId) -> bool {
    let Some((fa, fb)) = edge_face_pair(bm, e) else {
        return false;
    };
    let [v1, v2] = bm.edge(e).v;
    let la = bm.face_other_vert_loop(fa, v2, v1);
    let lb = bm.face_other_vert_loop(fb, v2, v1);
    match (la, lb) {
        (Some(a), Some(b)) if bm.lp(a).v != bm.lp(b).v => {}
        _ => return false,
    }
    let la = bm.face_other_vert_loop(fa, v1, v2);
    let lb = bm.face_other_vert_loop(fb, v1, v2);
    matches!((la, lb), (Some(a), Some(b)) if bm.lp(a).v != bm.lp(b).v)
}

/// `BM_edge_calc_rotate`: the corners the rotated edge will join.
pub fn edge_calc_rotate(bm: &BMesh, e: EdgeId, ccw: bool) -> Option<(LoopId, LoopId)> {
    let (mut fa, mut fb) = edge_face_pair(bm, e)?;
    let (v1, v2) = construct::edge_ordered_verts(bm, e);
    if !ccw {
        std::mem::swap(&mut fa, &mut fb);
    }
    let l1 = bm.face_other_vert_loop(fb, v2, v1)?;
    let l2 = bm.face_other_vert_loop(fa, v1, v2)?;
    if bm.lp(l1).v == bm.lp(l2).v {
        return None;
    }
    Some((l1, l2))
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mods.cc
// (BM_edge_rotate_check_degenerate).
fn edge_rotate_check_degenerate(bm: &BMesh, e: EdgeId, l1: LoopId, l2: LoopId) -> bool {
    let (v1_old, v2_old) = construct::edge_ordered_verts(bm, e);
    let v1 = bm.lp(l1).v;
    let v2 = bm.lp(l2).v;
    let v1_alt = bm
        .face_other_vert_loop(bm.lp(l1).f, v1_old, v1)
        .map(|l| bm.lp(l).v)
        .unwrap_or(v1);
    let v2_alt = bm
        .face_other_vert_loop(bm.lp(l2).f, v2_old, v2)
        .map(|l| bm.lp(l).v)
        .unwrap_or(v2);
    let co = |v| bm.vert(v).co;
    let ed_dir_old = bli::normalize_v3(bli::sub(co(v1_old), co(v2_old))).0;
    let ed_dir_new = bli::normalize_v3(bli::sub(co(v1), co(v2))).0;
    let ed_dir_v1_old = bli::normalize_v3(bli::sub(co(v1_old), co(v1))).0;
    let ed_dir_v2_old = bli::normalize_v3(bli::sub(co(v2_old), co(v2))).0;
    let ed_dir_v1_new = bli::normalize_v3(bli::sub(co(v1), co(v1_alt))).0;
    let ed_dir_v2_new = bli::normalize_v3(bli::sub(co(v2), co(v2_alt))).0;
    let cross_old = bli::cross(ed_dir_old, ed_dir_v1_old);
    let cross_new = bli::cross(ed_dir_new, ed_dir_v1_new);
    if bli::dot(cross_old, cross_new) < 0.0 {
        return false;
    }
    let cross_old = bli::cross(ed_dir_old, ed_dir_v2_old);
    let cross_new = bli::cross(ed_dir_new, ed_dir_v2_new);
    if bli::dot(cross_old, cross_new) < 0.0 {
        return false;
    }
    let ed_dir_new_flip = bli::negate(ed_dir_new);
    if bli::dot(ed_dir_new, ed_dir_v1_new) > 0.999
        || bli::dot(ed_dir_new_flip, ed_dir_v2_new) > 0.999
    {
        return false;
    }
    true
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mods.cc (BM_edge_rotate,
// BM_faces_join_pair).
/// `BM_edge_rotate(e, ccw, check_flag)`: the new edge, `None` when the rotation is refused.
pub fn edge_rotate(bm: &mut BMesh, e: EdgeId, ccw: bool, check_flag: u8) -> Option<EdgeId> {
    if !edge_rotate_check(bm, e) {
        return None;
    }
    let (l1, l2) = edge_calc_rotate(bm, e, ccw)?;
    let v1 = bm.lp(l1).v;
    let v2 = bm.lp(l2).v;
    if check_flag & CHECK_BEAUTY != 0 {
        let [a, b] = bm.edge(e).v;
        let old = bli::len_squared_v3v3(bm.vert(a).co, bm.vert(b).co);
        let new = bli::len_squared_v3v3(bm.vert(v1).co, bm.vert(v2).co);
        if !matches!(old.partial_cmp(&new), Some(std::cmp::Ordering::Greater)) {
            return None;
        }
    }
    if check_flag & CHECK_EXISTS != 0 && bm.edge_exists(v1, v2).is_some() {
        return None;
    }
    if check_flag & CHECK_DEGENERATE != 0 && !edge_rotate_check_degenerate(bm, e, l1, l2) {
        return None;
    }
    let e_new = bm.edge_create(
        v1,
        v2,
        Some(e),
        if check_flag & CHECK_SPLICE != 0 {
            CreateFlag::NO_DOUBLE
        } else {
            CreateFlag::empty()
        },
    );
    let f1 = bm.lp(l1).f;
    let f2 = bm.lp(l2).f;
    let f_hflag_prev_1 = bm.face(f1).flag;
    let f_hflag_prev_2 = bm.face(f2).flag;
    let f_active_prev = if bm.act_face == Some(f1) {
        1
    } else if bm.act_face == Some(f2) {
        2
    } else {
        0
    };
    let is_flipped = !bm.edge_is_contiguous(e);
    // `BM_faces_join_pair`.
    let la = bm.face_edge_share_loop(f1, e)?;
    let lb = bm.face_edge_share_loop(f2, e)?;
    if bm.lp(la).v == bm.lp(lb).v {
        bm.loop_reverse(bm.lp(lb).f);
    }
    let faces = [bm.lp(la).f, bm.lp(lb).f];
    let (f, f_double) = bm.faces_join(&faces, true, true);
    let f = f?;
    let split = match (
        bm.face_vert_share_loop(f, v1),
        bm.face_vert_share_loop(f, v2),
    ) {
        (Some(a), Some(b)) => bm.face_split(f, a, b, None, true),
        _ => None,
    };
    if split.is_some() {
        if let Some((fa, fb)) = edge_face_pair(bm, e_new) {
            bm.face_mut(fa).flag = f_hflag_prev_1;
            bm.face_mut(fb).flag = f_hflag_prev_2;
            if f_active_prev == 1 {
                bm.act_face = Some(fa);
            } else if f_active_prev == 2 {
                bm.act_face = Some(fb);
            }
            if is_flipped {
                bm.face_normal_flip(fb);
                if ccw {
                    let l = bm.edge(e_new).l.expect("rotated edge has faces");
                    let rn = bm.lp(l).radial_next;
                    bm.edge_mut(e_new).l = Some(rn);
                }
            }
        }
    } else {
        debug_assert!(f_double.is_none());
        return None;
    }
    Some(e_new)
}

/// Runs `rotate_edges` on `bm`.
pub fn rotate_edges(bm: &mut BMesh, i: &RotateEdgesIn) -> Result<RotateEdgesOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|&&e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!(
            "rotate_edges: dead edge {dead:?}"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

const EDGE_OUT: u16 = 1;
const FACE_MARK: u16 = 1;

fn rotate_simple(bm: &mut BMesh, edges: &[EdgeId], check_flag: u8, use_ccw: bool) {
    for &e in edges {
        if bm.edges.contains(e.0)
            && edge_rotate_check(bm, e)
            && let Some(er) = edge_rotate(bm, e, use_ccw, check_flag)
        {
            bm.oflag_enable(er, EDGE_OUT);
        }
    }
}

fn rotate_cost(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    -bli::len_squared_v3v3(bm.vert(a).co, bm.vert(b).co)
}

/// `bm_edge_rotate_is_boundary`.
fn rotate_is_boundary(bm: &BMesh, index: &FxHashMap<EdgeId, i32>, e: EdgeId) -> bool {
    let mut count = 0;
    let first = bm.edge(e).l.expect("manifold edge");
    let mut lr = first;
    loop {
        let mut l_iter = bm.lp(lr).next;
        loop {
            let ei = bm.lp(l_iter).e;
            if index.get(&ei).copied().unwrap_or(-1) != -1 {
                if count == 1 {
                    return false;
                }
                count += 1;
                break;
            }
            l_iter = bm.lp(l_iter).next;
            if l_iter == lr {
                break;
            }
        }
        lr = bm.lp(lr).radial_next;
        if lr == first {
            break;
        }
    }
    true
}

#[derive(Copy, Clone, PartialEq)]
enum Slot {
    None,
    Node(HeapNode),
    Freed,
}

// Ported from blender/source/blender/bmesh/operators/bmo_rotate_edges.cc
// (bm_rotate_edges_shared).
fn rotate_shared(bm: &mut BMesh, edges: &[EdgeId], check_flag: u8, use_ccw: bool) {
    let edges_len = edges.len();
    let mut heap: Heap<EdgeId> = Heap::new();
    let mut table = vec![Slot::None; edges_len];
    let mut index: FxHashMap<EdgeId, i32> = FxHashMap::default();
    for (i, &e) in edges.iter().enumerate() {
        index.insert(e, if bm.edge_is_manifold(e) { i as i32 } else { -1 });
    }
    let mut edges_len_rotate = 0usize;
    let mut pass_type = 0u32; // 0 boundary, 1 all, 2 done
    while pass_type != 2 && edges_len_rotate != edges_len {
        for i in 0..edges_len {
            if table[i] == Slot::Freed {
                continue;
            }
            let e = edges[i];
            let mut ok = index.get(&e).copied().unwrap_or(-1) != -1 && edge_rotate_check(bm, e);
            if ok && pass_type == 0 {
                ok = rotate_is_boundary(bm, &index, e);
            }
            if ok {
                let mut cost = rotate_cost(bm, e);
                if pass_type == 0 {
                    cost = if cost != 0.0 { -1.0 / cost } else { f32::MAX };
                }
                table[i] = Slot::Node(heap.insert(cost, e));
            }
        }
        if heap.is_empty() {
            pass_type += 1;
            continue;
        }
        let prev = edges_len_rotate;
        while !heap.is_empty() {
            frap_math::budget::check();
            let e_best = heap.pop_min();
            let e_best_index = index[&e_best] as usize;
            table[e_best_index] = Slot::None;
            if edge_rotate_check(bm, e_best)
                && let Some(e_rotate) = edge_rotate(bm, e_best, use_ccw, check_flag)
            {
                bm.oflag_enable(e_rotate, EDGE_OUT);
                index.remove(&e_best);
                index.insert(e_rotate, -1);
                table[e_best_index] = Slot::Freed;
                edges_len_rotate += 1;
                let first = bm.edge(e_rotate).l.expect("rotated edge has faces");
                let mut lr = first;
                loop {
                    let mut l_iter = bm.lp(lr).next;
                    loop {
                        let ei = bm.lp(l_iter).e;
                        let eidx = index.get(&ei).copied().unwrap_or(-1);
                        if eidx != -1
                            && table[eidx as usize] == Slot::None
                            && edge_rotate_check(bm, ei)
                        {
                            let cost = rotate_cost(bm, ei);
                            table[eidx as usize] = Slot::Node(heap.insert(cost, ei));
                        }
                        l_iter = bm.lp(l_iter).next;
                        if l_iter == lr {
                            break;
                        }
                    }
                    lr = bm.lp(lr).radial_next;
                    if lr == first {
                        break;
                    }
                }
            }
        }
        if edges_len_rotate == prev {
            pass_type += 1;
            continue;
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_rotate_edges.cc
// (bmo_rotate_edges_exec).
fn exec(bm: &mut BMesh, i: &RotateEdgesIn) -> RotateEdgesOut {
    let edges = &i.edges;
    let is_single = edges.len() == 1;
    let check_flag = if is_single {
        CHECK_EXISTS
    } else {
        CHECK_EXISTS | CHECK_DEGENERATE
    };
    let mut is_simple = true;
    if !is_single {
        'outer: for &e in edges {
            if let Some((fa, fb)) = edge_face_pair(bm, e) {
                for f in [fa, fb] {
                    if bm.oflag_test(f, FACE_MARK) {
                        is_simple = false;
                        break 'outer;
                    }
                    bm.oflag_enable(f, FACE_MARK);
                }
            }
        }
    }
    if is_simple {
        rotate_simple(bm, edges, check_flag, i.use_ccw);
    } else {
        rotate_shared(bm, edges, check_flag, i.use_ccw);
    }
    let out: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, EDGE_OUT))
        .collect();
    RotateEdgesOut {
        geom: out.iter().map(|&e| BMElem::Edge(e)).collect(),
        edges: out,
    }
}
