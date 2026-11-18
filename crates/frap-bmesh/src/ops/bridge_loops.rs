// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.bridge_loops` (Blender 5.2.4 `bmo_bridge_loops_exec`, `operators/bmo_bridge.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.bridge_loops(bmesh, edges=[], use_pairs=False, use_cyclic=False, use_merge=False, merge_factor=0.0, twist_offset=0)`.
//!
//! **Create/kill order**: the input edges are split into loops (`BM_mesh_edgeloops_find`, in
//! edge order), ordered from the outermost when there are more than two
//! (`BM_mesh_edgeloops_calc_order`), and each consecutive pair is bridged: loop directions
//! are matched (normals, then the winding of attached boundary faces), closed loops are
//! rotated to the best offset (plus `twist_offset`), and one quad per step is created with
//! `BM_face_create_verts` over `(b, b_next, a_next, a)` (missing edges created in
//! `(a, b), (b, b_next), (b_next, a_next), (a_next, a)` order), copying the loop data of the
//! adjacent corners and the face data of the first adjacent face. Loops of different sizes:
//! the shorter one is padded with repeated vertices (triangles), the new faces are
//! triangulated (WP 2.10's `triangulate`) and beautified (`beautify_fill`, ANGLE, restricted
//! to edges across the two loops). With `use_merge` the loops are welded instead (WP 2.7's
//! `weld_verts`, vertex data and positions mixed by `merge_factor`).
//!
//! Errors (`BMO_ERROR_CANCEL`): `Select at least two edge loops`, `Select an even number of
//! loops to bridge pairs`, `Selected loops must have equal edge counts`.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

pub mod edgeloop;

use edgeloop::EdgeLoopStore;

use crate::ops::contextual_create::{bli, construct};
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Input slots of `bridge_loops` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeLoopsIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_pairs`.
    pub use_pairs: bool,
    /// `use_cyclic`.
    pub use_cyclic: bool,
    /// `use_merge`.
    pub use_merge: bool,
    /// `merge_factor`.
    pub merge_factor: f32,
    /// `twist_offset`.
    pub twist_offset: i32,
}

/// Output slots of `bridge_loops`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BridgeLoopsOut {
    /// `faces.out` then `edges.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the bridge faces (mesh order).
    pub faces: Vec<FaceId>,
    /// `edges.out`: the new edges between the loops (mesh order).
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_bridge_loops_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const EDGE_MARK: u16 = 4;
const EDGE_OUT: u16 = 8;
const FACE_OUT: u16 = 16;

/// Runs `bridge_loops` on `bm`. A `BMO_ERROR_CANCEL` is returned as [`BmError::Cancel`] after
/// the operator ran (nothing was changed).
pub fn bridge_loops(bm: &mut BMesh, i: &BridgeLoopsIn) -> Result<BridgeLoopsOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|&&e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("bridge_loops: dead edge {dead:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

/// `bm_bridge_splice_loops` (WP 2.7's `weld_verts`).
fn splice_loops(bm: &mut BMesh, a: &[VertId], b: &[VertId], merge_factor: f32) {
    let mut targetmap = Vec::with_capacity(a.len());
    for (&v_a, &v_b) in a.iter().zip(b) {
        bm.data_interp_from_verts(v_a, v_b, v_b, merge_factor);
        let co = bli::interp_v3_v3v3(bm.vert(v_a).co, bm.vert(v_b).co, merge_factor);
        bm.vert_mut(v_b).co = co;
        targetmap.push((v_a, v_b));
    }
    let _ = crate::ops::weld_verts::weld_verts(
        bm,
        &crate::ops::weld_verts::WeldVertsIn {
            targetmap,
            ..Default::default()
        },
    );
}

/// `bm_vert_loop_pair`.
fn vert_loop_pair(bm: &BMesh, v1: VertId, v2: VertId) -> (Option<LoopId>, Option<LoopId>) {
    let e = bm.edge_exists(v1, v2).expect("bridge loop edge");
    match bm.edge(e).l {
        Some(l) => {
            let ll = bm.lp(l);
            if ll.v == v1 { (Some(l), Some(ll.next)) } else { (Some(ll.next), Some(l)) }
        }
        None => (
            bm.vert_loops(v1).first().copied(),
            bm.vert_loops(v2).first().copied(),
        ),
    }
}

/// `bm_edgeloop_offset_length`.
fn offset_length(bm: &BMesh, a: &[VertId], b: &[VertId], b_start: usize, len_max: f32) -> f32 {
    let mut len = 0.0f32;
    let mut ib = b_start;
    for &va in a {
        len += bli::len_v3v3(bm.vert(va).co, bm.vert(b[ib]).co);
        ib = if ib + 1 < b.len() { ib + 1 } else { 0 };
        if len >= len_max {
            break;
        }
    }
    len
}

/// `bm_bridge_best_rotation`.
fn best_rotation(bm: &BMesh, a: &EdgeLoopStore, b: &mut EdgeLoopStore) {
    let mut len_best = f32::MAX;
    let mut best = None;
    for ib in 0..b.verts.len() {
        let len = offset_length(bm, &a.verts, &b.verts, ib, len_best);
        if len < len_best {
            best = Some(ib);
            len_best = len;
        }
    }
    if let Some(ib) = best {
        b.rotate_first(ib);
    }
}

fn face_edges_tag_out(bm: &mut BMesh, f: FaceId) {
    for e in bm.face_edges(f) {
        bm.oflag_enable(e, EDGE_OUT);
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_bridge.cc (bridge_loop_pair).
fn bridge_loop_pair(
    bm: &mut BMesh,
    mut el_a: EdgeLoopStore,
    mut el_b: EdgeLoopStore,
    use_merge: bool,
    merge_factor: f32,
    twist_offset: i32,
) {
    let eps = 0.00001f32;
    let is_closed = el_a.closed && el_b.closed;
    let mut a_len = el_a.len();
    let mut b_len = el_b.len();
    if a_len < b_len {
        std::mem::swap(&mut a_len, &mut b_len);
        std::mem::swap(&mut el_a, &mut el_b);
    }
    if a_len != b_len {
        construct::faces_flag_disable_all(bm, ElemFlag::TAG);
        construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    }
    let el_dir = bli::sub(el_a.co, el_b.co);
    if is_closed {
        el_a.calc_normal(bm);
        el_b.calc_normal(bm);
    } else {
        let co = |v: VertId| bm.vert(v).co;
        let dir_a_orig = bli::sub(co(el_a.verts[0]), co(*el_a.verts.last().expect("loop")));
        let dir_b_orig = bli::sub(co(el_b.verts[0]), co(*el_b.verts.last().expect("loop")));
        let no = bli::cross(dir_a_orig, el_dir);
        let dir_a = bli::cross(no, el_dir);
        let no = bli::cross(dir_b_orig, el_dir);
        let dir_b = bli::cross(no, el_dir);
        let (test_a, test_b) = if !bli::is_zero_v3(dir_a) && !bli::is_zero_v3(dir_b) {
            (dir_a, dir_b)
        } else {
            (dir_a_orig, dir_b_orig)
        };
        if bli::dot(test_a, test_b) < 0.0 {
            el_b.flip();
        }
        let no = bli::normalize_v3(el_dir).0;
        el_a.calc_normal_aligned(bm, no);
        el_b.calc_normal_aligned(bm, no);
    }
    let dot_a = bli::dot(el_a.no, el_dir);
    let dot_b = bli::dot(el_b.no, el_dir);
    if bli::len_squared_v3(el_dir) < eps || (dot_a.abs() < eps && dot_b.abs() < eps) {
        if bli::dot(el_a.no, el_b.no) < 0.0 {
            el_b.flip();
        }
    } else if (dot_a < 0.0) != (dot_b < 0.0) {
        el_b.flip();
    }
    if !use_merge {
        let no = bli::add(el_a.no, el_b.no);
        if bli::dot(no, el_dir) < 0.0 {
            el_a.flip();
            el_b.flip();
        }
        // Vote on winding (so new face winding is based on existing connected faces).
        if bm.faces_num() > 0 {
            let mut votes = [0i32; 2];
            let mut winding_dir = 1;
            for (k, el) in [&el_a, &el_b].into_iter().enumerate() {
                for idx in 0..el.verts.len() {
                    if let Some(n) = el.next_index(idx) {
                        let (v, vn) = (el.verts[idx], el.verts[n]);
                        if let Some(e) = bm.edge_exists(v, vn)
                            && bm.edge_is_boundary(e)
                        {
                            let l = bm.edge(e).l.expect("boundary edge loop");
                            votes[k] += if bm.lp(l).v == v { winding_dir } else { -winding_dir };
                        }
                    }
                }
                winding_dir = -winding_dir;
            }
            if votes[0] != 0 || votes[1] != 0 {
                let mut flip = [false, false];
                if dot_a.abs() < eps && votes[0] < 0 {
                    flip[0] = !flip[0];
                    votes[0] *= -1;
                }
                if dot_b.abs() < eps && votes[1] < 0 {
                    flip[1] = !flip[1];
                    votes[1] *= -1;
                }
                if votes[0] + votes[1] < 0 {
                    flip[0] = !flip[0];
                    flip[1] = !flip[1];
                }
                if flip[0] {
                    el_a.flip();
                }
                if flip[1] {
                    el_b.flip();
                }
            }
        }
    }
    if a_len > b_len {
        el_b.expand(a_len);
    }
    if is_closed {
        best_rotation(bm, &el_a, &mut el_b);
        if twist_offset != 0 {
            let len_b = el_b.len() as i32;
            // `BLI_rfindlink(lb_b, mod_i(twist_offset, len_b))`: counted from the end.
            let k = bli::mod_i(twist_offset, len_b) as usize;
            let idx = el_b.len() - 1 - k;
            el_b.rotate_first(idx);
        }
    }
    if use_merge {
        splice_loops(bm, &el_a.verts.clone(), &el_b.verts.clone(), merge_factor);
    } else {
        let n = el_a.len();
        let mut ia = 0usize;
        let mut ib = 0usize;
        loop {
            let (ia_next, ib_next) = if is_closed {
                (el_a.next_index(ia).expect("closed"), el_b.next_index(ib).expect("closed"))
            } else {
                match (
                    (ia + 1 < n).then_some(ia + 1),
                    (ib + 1 < el_b.len()).then_some(ib + 1),
                ) {
                    (Some(x), Some(y)) => (x, y),
                    _ => break,
                }
            };
            let v_a = el_a.verts[ia];
            let v_b = el_b.verts[ib];
            let v_a_next = el_a.verts[ia_next];
            let v_b_next = el_b.verts[ib_next];
            let (mut l_a, mut l_a_next, mut l_b, mut l_b_next);
            if v_b != v_b_next {
                (l_a, l_a_next) = vert_loop_pair(bm, v_a, v_a_next);
                (l_b, l_b_next) = vert_loop_pair(bm, v_b, v_b_next);
            } else {
                (l_a, l_a_next) = vert_loop_pair(bm, v_a, v_a_next);
                l_b = bm.vert_loops(v_b).first().copied();
                l_b_next = l_b;
            }
            if l_a.is_some() && l_a_next.is_none() {
                l_a_next = l_a;
            }
            if l_a_next.is_some() && l_a.is_none() {
                l_a = l_a_next;
            }
            if l_b.is_some() && l_b_next.is_none() {
                l_b_next = l_b;
            }
            if l_b_next.is_some() && l_b.is_none() {
                l_b = l_b_next;
            }
            let f_example = l_a
                .map(|l| bm.lp(l).f)
                .or_else(|| l_b.map(|l| bm.lp(l).f));
            let f: Option<FaceId> = if v_b != v_b_next {
                if v_b == v_a_next || v_b == v_a || v_b_next == v_a_next || v_b_next == v_a {
                    None
                } else {
                    let v_arr = [v_b, v_b_next, v_a_next, v_a];
                    match bm.face_exists(&v_arr) {
                        Some(f) => Some(f),
                        None => {
                            let f = bm
                                .face_create_verts(&v_arr, None, CreateFlag::empty(), true)
                                .expect("bridge quad");
                            let ls = bm.face_loops(f);
                            for (src, dst) in [l_b, l_b_next, l_a_next, l_a].into_iter().zip(ls) {
                                if let Some(src) = src {
                                    bm.loop_attrs_copy(src, dst);
                                }
                            }
                            Some(f)
                        }
                    }
                }
            } else if v_b == v_a_next || v_b == v_a {
                None
            } else {
                let v_arr = [v_b, v_a_next, v_a];
                match bm.face_exists(&v_arr) {
                    Some(f) => Some(f),
                    None => {
                        let f = bm
                            .face_create_verts(&v_arr, None, CreateFlag::empty(), true)
                            .expect("bridge triangle");
                        let ls = bm.face_loops(f);
                        for (src, dst) in [l_b, l_a_next, l_a].into_iter().zip(ls) {
                            if let Some(src) = src {
                                bm.loop_attrs_copy(src, dst);
                            }
                        }
                        Some(f)
                    }
                }
            };
            if let Some(f) = f {
                if let Some(fe) = f_example
                    && fe != f
                {
                    bm.face_attrs_copy(fe, f);
                }
                bm.oflag_enable(f, FACE_OUT);
                bm.face_mut(f).flag.insert(ElemFlag::TAG);
                face_edges_tag_out(bm, f);
            }
            if ia_next == 0 {
                break;
            }
            ia = ia_next;
            ib = ib_next;
        }
    }
    if a_len != b_len {
        // Triangulate the tagged faces (WP 2.10's `triangulate`).
        let tag_faces: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
            .collect();
        for &f in &tag_faces {
            bm.face_normal_update(f);
        }
        if let Ok(out) = crate::ops::triangulate::triangulate(
            bm,
            &crate::ops::triangulate::TriangulateIn {
                faces: tag_faces,
                ..Default::default()
            },
        ) {
            for g in out.geom {
                if let BMElem::Face(f) = g
                    && bm.faces.contains(f.0)
                {
                    bm.oflag_enable(f, FACE_OUT);
                    bm.face_mut(f).flag.insert(ElemFlag::TAG);
                }
            }
        }
        // Tag verts on each side so edges only rotate across the two loops.
        for (k, el) in [&el_a, &el_b].into_iter().enumerate() {
            for &v in &el.verts {
                bm.vert_mut(v).flag.set(ElemFlag::TAG, k == 1);
            }
        }
        let faces: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
            .collect();
        for &f in &faces {
            bm.oflag_enable(f, FACE_OUT);
            face_edges_tag_out(bm, f);
        }
        let out = crate::ops::beautify_fill::beautify_fill_inner(
            bm,
            &faces,
            &bm.edge_ids(),
            true,
            crate::ops::beautify_fill::BeautifyMethod::Angle,
        );
        for f in out {
            if bm.faces.contains(f.0) {
                bm.oflag_enable(f, FACE_OUT);
                face_edges_tag_out(bm, f);
            }
        }
    }
    if !use_merge {
        for el in [&el_a, &el_b] {
            for idx in 0..el.verts.len() {
                if let Some(n) = el.next_index(idx)
                    && el.verts[idx] != el.verts[n]
                    && let Some(e) = bm.edge_exists(el.verts[idx], el.verts[n])
                {
                    bm.oflag_disable(e, EDGE_OUT);
                }
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_bridge.cc (bmo_bridge_loops_exec).
fn exec(bm: &mut BMesh, i: &BridgeLoopsIn) -> Result<BridgeLoopsOut, BmError> {
    let use_pairs = i.use_pairs;
    let use_merge = i.use_merge;
    let use_cyclic = i.use_cyclic && !use_merge;
    for &e in &i.edges {
        bm.oflag_enable(e, EDGE_MARK);
    }
    let mut eloops = edgeloop::mesh_edgeloops_find(bm, &|bm, e| bm.oflag_test(e, EDGE_MARK));
    edgeloop::mesh_edgeloops_calc_center(bm, &mut eloops);
    let count = eloops.len();
    if count < 2 {
        return Err(BmError::Cancel("Select at least two edge loops".into()));
    }
    if use_pairs && count % 2 == 1 {
        return Err(BmError::Cancel(
            "Select an even number of loops to bridge pairs".into(),
        ));
    }
    if use_merge {
        let l0 = eloops[0].len();
        if eloops.iter().any(|el| el.len() != l0) {
            return Err(BmError::Cancel(
                "Selected loops must have equal edge counts".into(),
            ));
        }
    }
    if count > 2 {
        if use_pairs {
            edgeloop::mesh_edgeloops_calc_normal(bm, &mut eloops);
        }
        edgeloop::mesh_edgeloops_calc_order(&mut eloops, use_pairs);
    }
    let mut changed = false;
    let mut k = 0usize;
    while k < eloops.len() {
        let next = if k + 1 < eloops.len() {
            k + 1
        } else if use_cyclic && count > 2 {
            0
        } else {
            break;
        };
        bridge_loop_pair(
            bm,
            eloops[k].clone(),
            eloops[next].clone(),
            use_merge,
            i.merge_factor,
            i.twist_offset,
        );
        if use_pairs {
            k += 1;
        }
        k += 1;
        changed = true;
    }
    let mut out = BridgeLoopsOut::default();
    if changed && !use_merge {
        out.faces = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, FACE_OUT))
            .collect();
        out.edges = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| bm.oflag_test(e, EDGE_OUT))
            .collect();
        out.geom = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
        out.geom.extend(out.edges.iter().map(|&e| BMElem::Edge(e)));
    }
    Ok(out)
}
