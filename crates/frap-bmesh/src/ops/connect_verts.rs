// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.connect_verts` (Blender 5.2.4 `bmo_connect_verts_exec`,
//! `operators/bmo_connect.cc`), implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.connect_verts(bmesh, verts=[], faces_exclude=[], check_degenerate=False)`.
//!
//! **Create/kill order**: faces using the input vertices are collected in the order the
//! vertices list them (`BM_LOOPS_OF_VERT`, triangles skipped) and processed from the last
//! collected (a link stack). In each face, consecutive non-adjacent input corners (plus the
//! wrap-around pair) are checked (`BM_face_splits_check_legal` with `check_degenerate`, else
//! `BM_face_splits_check_optimal`) and split in order with `BM_face_split`, each split
//! continuing in the new face. Duplicate faces a split may create are killed afterwards.
//!
//! The submodules hold `connect_verts_concave` (`concave.rs`) and `connect_verts_nonplanar`
//! (`nonplanar.rs`), which have no owner file of their own (rule 22.1).
//!
//! Errors: `Could not connect vertices` (`BMO_ERROR_FATAL`).
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC | SELECT_FLUSH`.

pub mod concave;
pub mod nonplanar;

use crate::ops::contextual_create::bli::{self, ISECT_LINE_LINE_CROSS};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, LoopId, VertId};

/// Input slots of `connect_verts` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectVertsIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `faces_exclude`.
    pub faces_exclude: Vec<FaceId>,
    /// `check_degenerate`.
    pub check_degenerate: bool,
}

/// Output slots of `connect_verts`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConnectVertsOut {
    /// `edges.out` as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out`: the new edges and the existing edges between input vertices (mesh order).
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_connect_verts_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const VERT_INPUT: u16 = 1;
const EDGE_OUT: u16 = 1;
const EDGE_OUT_ADJ: u16 = 2;
const FACE_TAG: u16 = 2;
const FACE_EXCLUDE: u16 = 4;

/// `BM_loop_is_adjacent`.
pub fn loop_is_adjacent(bm: &BMesh, a: LoopId, b: LoopId) -> bool {
    bm.lp(a).next == b || bm.lp(a).prev == b
}

/// `float2` `math::normalize`.
fn normalize2(v: [f32; 2]) -> [f32; 2] {
    let l = v[0] * v[0] + v[1] * v[1];
    if l > 1.0e-35 {
        let l = l.sqrt();
        [v[0] / l, v[1] / l]
    } else {
        [0.0, 0.0]
    }
}

/// `angle_signed_v2v2_pos`: `[0, 2π]`.
fn angle_signed_v2v2_pos(v1: [f32; 2], v2: [f32; 2]) -> f32 {
    let perp_dot = (v1[1] * v2[0]) - (v1[0] * v2[1]);
    let angle = libm::atan2f(perp_dot, v1[0] * v2[0] + v1[1] * v2[1]);
    if angle < 0.0 {
        (angle as f64 + std::f64::consts::PI * 2.0) as f32
    } else {
        angle
    }
}

/// `is_poly_convex_v2`.
fn is_poly_convex_v2(verts: &[[f32; 2]]) -> bool {
    let nr = verts.len();
    let mut sign_flag = 0u32;
    let mut co_prev = verts[nr - 1];
    let mut dir_prev = [verts[nr - 2][0] - co_prev[0], verts[nr - 2][1] - co_prev[1]];
    for &co_curr in verts {
        let dir_curr = [co_prev[0] - co_curr[0], co_prev[1] - co_curr[1]];
        let cross = dir_prev[0] * dir_curr[1] - dir_prev[1] * dir_curr[0];
        if cross < 0.0 {
            sign_flag |= 1;
        } else if cross > 0.0 {
            sign_flag |= 2;
        }
        if sign_flag == 3 {
            return false;
        }
        dir_prev = dir_curr;
        co_prev = co_curr;
    }
    true
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc (BM_face_splits_check_legal).
/// `BM_face_splits_check_legal`: clears (`None`) the splits that leave the face or cross.
pub fn face_splits_check_legal(bm: &BMesh, f: FaceId, loops: &mut [Option<[LoopId; 2]>]) {
    let m = bli::axis_dominant_v3_to_m3(bm.face(f).no);
    let fl = bm.face_loops(f);
    let mut proj: Vec<[f32; 2]> = Vec::with_capacity(fl.len());
    let mut center = [0.0f32; 2];
    for &l in &fl {
        let p = bli::mul_v2_m3v3(&m, bm.vert(bm.lp(l).v).co);
        center[0] += p[0];
        center[1] += p[1];
        proj.push(p);
    }
    if is_poly_convex_v2(&proj) {
        return;
    }
    let inv = 1.0 / fl.len() as f32;
    center = [center[0] * inv, center[1] * inv];
    for p in &mut proj {
        p[0] -= center[0];
        p[1] -= center[1];
    }
    let idx = |l: LoopId| fl.iter().position(|&x| x == l).expect("loop of face");
    let edgeverts: Vec<[usize; 2]> = loops
        .iter()
        .map(|p| match p {
            Some([a, b]) => [idx(*a), idx(*b)],
            None => [usize::MAX, usize::MAX],
        })
        .collect();
    let n = fl.len();
    for lp in loops.iter_mut() {
        let Some(pair) = *lp else {
            continue;
        };
        let co_pair = [proj[idx(pair[0])], proj[idx(pair[1])]];
        if co_pair[0] == co_pair[1] {
            continue;
        }
        let pair_dir = normalize2([co_pair[1][0] - co_pair[0][0], co_pair[1][1] - co_pair[0][1]]);
        for side in 0..2 {
            let co = co_pair[side];
            let mut l_prev = bm.lp(pair[side]).prev;
            let mut l_next = bm.lp(pair[side]).next;
            let limit_init = n as i32 - 3;
            let mut limit = limit_init;
            while co == proj[idx(l_prev)] && {
                let c = limit > 0;
                limit -= 1;
                c
            } {
                l_prev = bm.lp(l_prev).prev;
            }
            limit = limit_init;
            while co == proj[idx(l_next)] && {
                let c = limit > 0;
                limit -= 1;
                c
            } {
                l_next = bm.lp(l_next).next;
            }
            let co_prev = proj[idx(l_prev)];
            let co_next = proj[idx(l_next)];
            let dir_other = if side == 0 { pair_dir } else { [-pair_dir[0], -pair_dir[1]] };
            let dir_prev = normalize2([co_prev[0] - co[0], co_prev[1] - co[1]]);
            let dir_next = normalize2([co_next[0] - co[0], co_next[1] - co[1]]);
            if angle_signed_v2v2_pos(dir_prev, dir_other) > angle_signed_v2v2_pos(dir_prev, dir_next) {
                *lp = None;
                break;
            }
        }
    }
    let share = |a: [usize; 2], b: [usize; 2]| a[0] == b[0] || a[0] == b[1] || a[1] == b[0] || a[1] == b[1];
    let mut i_prev = n - 1;
    for i in 0..n {
        let f_edge = [i_prev, i];
        for j in 0..loops.len() {
            if loops[j].is_some()
                && !share(f_edge, edgeverts[j])
                && bli::isect_seg_seg_v2(
                    proj[f_edge[0]],
                    proj[f_edge[1]],
                    proj[edgeverts[j][0]],
                    proj[edgeverts[j][1]],
                ) == ISECT_LINE_LINE_CROSS
            {
                loops[j] = None;
            }
        }
        i_prev = i;
    }
    for i in 0..loops.len() {
        if loops[i].is_some() {
            for j in (i + 1)..loops.len() {
                if loops[j].is_some()
                    && !share(edgeverts[i], edgeverts[j])
                    && bli::isect_seg_seg_v2(
                        proj[edgeverts[i][0]],
                        proj[edgeverts[i][1]],
                        proj[edgeverts[j][0]],
                        proj[edgeverts[j][1]],
                    ) == ISECT_LINE_LINE_CROSS
                {
                    loops[i] = None;
                    break;
                }
            }
        }
    }
}

/// `BM_face_calc_normal_subset`: Newell normal of the corners `l_first..=l_last`.
pub fn face_calc_normal_subset(bm: &BMesh, l_first: LoopId, l_last: LoopId) -> ([f32; 3], f32) {
    let mut n = [0.0f32; 3];
    let l_term = bm.lp(l_last).next;
    let mut v_prev = bm.vert(bm.lp(l_last).v).co;
    let mut l = l_first;
    loop {
        let v_curr = bm.vert(bm.lp(l).v).co;
        bli::add_newell_cross_v3_v3v3(&mut n, v_prev, v_curr);
        v_prev = v_curr;
        l = bm.lp(l).next;
        if l == l_term {
            break;
        }
    }
    bli::normalize_v3(n)
}

/// `bm_face_calc_split_dot`.
fn face_calc_split_dot(bm: &BMesh, l_a: LoopId, l_b: LoopId) -> f32 {
    let (n0, a) = face_calc_normal_subset(bm, l_a, l_b);
    let (n1, b) = face_calc_normal_subset(bm, l_b, l_a);
    if a != 0.0 && b != 0.0 {
        bli::dot(n0, n1)
    } else {
        -1.0
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc
// (BM_vert_pair_share_face_by_angle).
/// `BM_vert_pair_share_face_by_angle`: the face both vertices use whose split is flattest.
pub fn vert_pair_share_face_by_angle(
    bm: &BMesh,
    v_a: VertId,
    v_b: VertId,
    allow_adjacent: bool,
) -> Option<(FaceId, LoopId, LoopId)> {
    if bm.vert(v_a).e.is_none() || bm.vert(v_b).e.is_none() {
        return None;
    }
    let mut cur: Option<(FaceId, LoopId, LoopId)> = None;
    let mut dot_best = -1.0f32;
    for l_a in bm.vert_loops(v_a) {
        let f = bm.lp(l_a).f;
        let Some(l_b) = bm.face_vert_share_loop(f, v_b) else {
            continue;
        };
        if !allow_adjacent && loop_is_adjacent(bm, l_a, l_b) {
            continue;
        }
        match cur {
            None => cur = Some((f, l_a, l_b)),
            Some((_, ca, cb)) => {
                if dot_best == -1.0 {
                    dot_best = face_calc_split_dot(bm, ca, cb);
                }
                let dot = face_calc_split_dot(bm, l_a, l_b);
                if dot > dot_best {
                    dot_best = dot;
                    cur = Some((f, l_a, l_b));
                }
            }
        }
    }
    cur
}

/// `BM_face_splits_check_optimal`.
pub fn face_splits_check_optimal(bm: &BMesh, f: FaceId, loops: &mut [Option<[LoopId; 2]>]) {
    for lp in loops.iter_mut() {
        if let Some([a, b]) = *lp {
            let best = vert_pair_share_face_by_angle(bm, bm.lp(a).v, bm.lp(b).v, false);
            if best.map(|x| x.0) != Some(f) {
                *lp = None;
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_connect.cc (bm_face_connect_verts).
fn face_connect_verts(bm: &mut BMesh, mut f: FaceId, check_degenerate: bool) -> i32 {
    let input = |bm: &BMesh, v: VertId| bm.oflag_test(v, VERT_INPUT);
    let mut loops_split: Vec<Option<[LoopId; 2]>> = Vec::new();
    let mut l_tag_prev: Option<LoopId> = None;
    let mut l_tag_first: Option<LoopId> = None;
    for l in bm.face_loops(f) {
        let ll = bm.lp(l);
        if input(bm, ll.v)
            && (!input(bm, bm.lp(ll.prev).v) || !input(bm, bm.lp(ll.next).v))
        {
            let Some(prev) = l_tag_prev else {
                l_tag_prev = Some(l);
                l_tag_first = Some(l);
                continue;
            };
            if !loop_is_adjacent(bm, prev, l) {
                let e = bm.edge_exists(bm.lp(prev).v, ll.v);
                if e.is_none_or(|e| !bm.oflag_test(e, EDGE_OUT)) {
                    loops_split.push(Some([prev, l]));
                }
            }
            l_tag_prev = Some(l);
        }
    }
    if loops_split.is_empty() {
        return 0;
    }
    let (first, last) = (l_tag_first.expect("tagged"), l_tag_prev.expect("tagged"));
    if !loop_is_adjacent(bm, first, last) && loops_split[0] != Some([first, last]) {
        loops_split.push(Some([first, last]));
    }
    if check_degenerate {
        face_splits_check_legal(bm, f, &mut loops_split);
    } else {
        face_splits_check_optimal(bm, f, &mut loops_split);
    }
    let verts_pair: Vec<[VertId; 2]> = loops_split
        .iter()
        .flatten()
        .map(|p| [bm.lp(p[0]).v, bm.lp(p[1]).v])
        .collect();
    let mut deferred: Vec<[Option<LoopId>; 2]> = Vec::new();
    let mut result = 1;
    for vp in verts_pair {
        let edge_exists = bm.edge_exists(vp[0], vp[1]).is_some();
        let split = match (bm.face_vert_share_loop(f, vp[0]), bm.face_vert_share_loop(f, vp[1])) {
            (Some(a), Some(b)) => {
                let r = bm.face_split(f, a, b, None, edge_exists);
                if edge_exists {
                    let mut d: Option<[Option<LoopId>; 2]> = None;
                    for (j, l) in [a, b].into_iter().enumerate() {
                        if bm.face_find_double(bm.lp(l).f).is_some() {
                            let e = d.get_or_insert([None, None]);
                            e[j] = Some(l);
                        }
                    }
                    if let Some(d) = d {
                        deferred.push(d);
                    }
                }
                r
            }
            _ => None,
        };
        let Some((f_new, l_new)) = split else {
            result = -1;
            break;
        };
        f = f_new;
        let e = bm.lp(l_new).e;
        bm.oflag_enable(e, EDGE_OUT);
    }
    for d in deferred {
        for l in d.into_iter().flatten() {
            if bm.loops.contains(l.0) {
                let lf = bm.lp(l).f;
                if bm.faces.contains(lf.0) {
                    bm.face_kill(lf);
                }
            }
        }
    }
    result
}

/// Runs `connect_verts` on `bm`. `Could not connect vertices` is a [`BmError::Fatal`] raised
/// after the operator ran.
pub fn connect_verts(bm: &mut BMesh, i: &ConnectVertsIn) -> Result<ConnectVertsOut, BmError> {
    if let Some(dead) = i.verts.iter().find(|&&v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!("connect_verts: dead vertex {dead:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

// Ported from blender/source/blender/bmesh/operators/bmo_connect.cc (bmo_connect_verts_exec).
fn exec(bm: &mut BMesh, i: &ConnectVertsIn) -> Result<ConnectVertsOut, BmError> {
    for &f in &i.faces_exclude {
        bm.oflag_enable(f, FACE_EXCLUDE);
    }
    let mut faces: Vec<FaceId> = Vec::new();
    for &v in &i.verts {
        bm.oflag_enable(v, VERT_INPUT);
        for l in bm.vert_loops(v) {
            let f = bm.lp(l).f;
            if !bm.oflag_test(f, FACE_EXCLUDE) && !bm.oflag_test(f, FACE_TAG) {
                bm.oflag_enable(f, FACE_TAG);
                if bm.face(f).len > 3 {
                    faces.push(f);
                }
            }
            let ll = bm.lp(l).clone();
            if bm.oflag_test(bm.lp(ll.prev).v, VERT_INPUT) {
                let pe = bm.lp(ll.prev).e;
                bm.oflag_enable(pe, EDGE_OUT_ADJ);
            }
            if bm.oflag_test(bm.lp(ll.next).v, VERT_INPUT) {
                bm.oflag_enable(ll.e, EDGE_OUT_ADJ);
            }
        }
    }
    let mut failed = false;
    while let Some(f) = faces.pop() {
        if face_connect_verts(bm, f, i.check_degenerate) == -1 {
            failed = true;
        }
    }
    let edges: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, EDGE_OUT | EDGE_OUT_ADJ))
        .collect();
    if failed {
        return Err(BmError::Fatal("Could not connect vertices".into()));
    }
    Ok(ConnectVertsOut {
        geom: edges.iter().map(|&e| BMElem::Edge(e)).collect(),
        edges,
    })
}
