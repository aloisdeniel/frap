// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.offset_edgeloops` (Blender 5.2.4 `bmo_offset_edgeloops_exec`), implemented by
//! WP 2.6 (Subdivide and loops).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_offset_edgeloops.cc (Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.offset_edgeloops(bmesh, edges=[], use_cap_endpoint=False)`.
//!
//! **Create/kill order**: the end vertices of the input edges are gathered in input order
//! (`v1` then `v2`, each once) and those using only input edges are dropped (a swap-remove
//! that skips the moved entry, as Blender's `STACK_REMOVE` loop does). For each remaining
//! vertex, every other edge around it (disk order) is split next to the vertex
//! (`BM_edge_split` at the far end with factor 1: one vertex, one edge each; the new vertex
//! sits on the vertex, the edge slide moves it). Then, per vertex and corner, the corners
//! between two new vertices are cut off (`BM_face_split`: one edge, one face), and n-gon
//! corners are connected two or more steps back (`BM_face_split_n`, which also creates and
//! kills a temporary copy of the face). Without `use_cap_endpoint` the new 2-valence vertices
//! next to loop ends are joined away (`bmesh_kernel_join_edge_kill_vert`).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use crate::ops::unsubdivide::mods;
use crate::types::{ElemFlag, ElemTypes, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, VertId};

/// Input slots of `offset_edgeloops` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OffsetEdgeloopsIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_cap_endpoint`.
    pub use_cap_endpoint: bool,
}

/// Output slots of `offset_edgeloops`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct OffsetEdgeloopsOut {
    /// `edges.out` as elements (the new loop edges, iteration order).
    pub geom: Vec<BMElem>,
    /// `edges.out`: the new loop edges, in iteration order.
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_offset_edgeloops_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const ELE_NEW: u16 = 1;
const ELE_VERT_ENDPOINT: u16 = 2;

/// Debugging offset of Blender's source (0: the new vertices sit on the old ones).
const OFFSET: f32 = 0.0;

fn is_new(bm: &BMesh, v: VertId) -> bool {
    bm.index_get(BMElem::Vert(v)) == -1
}

fn tagged_edge(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e).flag.contains(ElemFlag::TAG)
}

/// `bm_face_split_walk_back`: connects `l_src->next` back to the first new vertex before
/// `l_src`, through the positions of the old corners in between.
fn face_split_walk_back(bm: &mut BMesh, l_src: crate::LoopId) -> Option<crate::LoopId> {
    let mut l_dst = bm.lp(l_src).prev;
    let mut num = 0usize;
    while !is_new(bm, bm.lp(bm.lp(l_dst).prev).v) {
        l_dst = bm.lp(l_dst).prev;
        num += 1;
    }
    let mut cos = vec![[0.0f32; 3]; num];
    let mut l = bm.lp(l_src).prev;
    let mut i = 0usize;
    while !is_new(bm, bm.lp(bm.lp(l).prev).v) {
        cos[num - (i + 1)] = bm.vert(bm.lp(l).v).co;
        l = bm.lp(l).prev;
        i += 1;
    }
    let f = bm.lp(l_src).f;
    let l_a = bm.lp(l).prev;
    let l_b = bm.lp(l_src).next;
    mods::face_split_n(bm, f, l_a, l_b, &cos).map(|(_, l_new)| l_new)
}

/// Runs `offset_edgeloops` on `bm`.
pub fn offset_edgeloops(bm: &mut BMesh, i: &OffsetEdgeloopsIn) -> Result<OffsetEdgeloopsOut, BmError> {
    for &e in &i.edges {
        if !bm.edges.contains(e.0) {
            return Err(BmError::Invalid(format!("edges: no edge {}", e.0)));
        }
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

fn exec(bm: &mut BMesh, i: &OffsetEdgeloopsIn) -> OffsetEdgeloopsOut {
    let use_cap_endpoint = i.use_cap_endpoint;
    let mut v_edges_max = 0usize;

    // Only so new vertices can be detected (index -1).
    bm.index.dirty |= ElemTypes::VERT;
    bm.elem_index_ensure(ElemTypes::VERT);
    // `BM_mesh_elem_hflag_disable_all(bm, BM_VERT | BM_EDGE | BM_FACE, BM_ELEM_TAG, false)`.
    for v in bm.vert_ids() {
        bm.vert_mut(v).flag.remove(ElemFlag::TAG);
    }
    for e in bm.edge_ids() {
        bm.edge_mut(e).flag.remove(ElemFlag::TAG);
    }
    for f in bm.face_ids() {
        bm.face_mut(f).flag.remove(ElemFlag::TAG);
    }

    let mut verts: Vec<VertId> = Vec::with_capacity(i.edges.len() * 2);
    for &e in &i.edges {
        bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        for v in bm.edge(e).v {
            if !bm.vert(v).flag.contains(ElemFlag::TAG) {
                bm.vert_mut(v).flag.insert(ElemFlag::TAG);
                verts.push(v);
            }
        }
    }

    // Remove the vertices only used by tagged edges (only boundary vertices are interesting).
    // `STACK_REMOVE` swaps the last entry in and the loop moves on without testing it.
    let mut k = 0;
    while k < verts.len() {
        let mut flag = 0u8;
        for e in bm.vert_edges(verts[k]) {
            flag |= if tagged_edge(bm, e) { 1 } else { 2 };
            if flag == 3 {
                break;
            }
        }
        if flag != 3 {
            verts.swap_remove(k);
        }
        k += 1;
    }

    // Possible but unlikely: no mixed vertices.
    if verts.is_empty() {
        return OffsetEdgeloopsOut::default();
    }

    // Main loop: split the untagged edges next to each vertex.
    for &v in &verts {
        frap_math::budget::check();
        let mut v_edges_num = 0usize;
        let mut v_edges_num_untag = 0usize;
        for e in bm.vert_edges(v) {
            if !tagged_edge(bm, e) {
                for l in bm.edge_loops(e) {
                    let f = bm.lp(l).f;
                    bm.face_mut(f).flag.insert(ElemFlag::TAG);
                }
                let v_other = bm.edge_other_vert(e, v);
                bm.edge_split(e, v_other, 1.0 - OFFSET);
            } else {
                v_edges_num_untag += 1;
            }
            v_edges_num += 1;
        }
        if v_edges_num_untag == 1 {
            bm.oflag_enable(v, ELE_VERT_ENDPOINT);
        }
        v_edges_max = v_edges_max.max(v_edges_num);
    }

    for &v in &verts {
        frap_math::budget::check();
        for l in bm.vert_loops(v) {
            let f = bm.lp(l).f;
            if !bm.face(f).flag.contains(ElemFlag::TAG) || bm.face(f).len == 3 {
                continue;
            }
            let ll = bm.lp(l).clone();
            if is_new(bm, bm.lp(ll.next).v) && is_new(bm, bm.lp(ll.prev).v) {
                if (use_cap_endpoint || !bm.oflag_test(v, ELE_VERT_ENDPOINT))
                    && !mods::face_split_check_double_face(bm, ll.prev, ll.next, 3)
                    && let Some((_, l_new)) = bm.face_split(f, ll.prev, ll.next, None, true)
                {
                    let e = bm.lp(l_new).e;
                    bm.oflag_enable(e, ELE_NEW);
                }
            } else if bm.face(f).len > 4
                && tagged_edge(bm, ll.e) != tagged_edge(bm, bm.lp(ll.prev).e)
                && is_new(bm, bm.lp(ll.next).v)
            {
                let l_pp = bm.lp(ll.prev).prev;
                if is_new(bm, bm.lp(l_pp).v) {
                    if !mods::face_split_check_double_face(bm, l_pp, ll.next, 4)
                        && let Some((_, l_new)) = bm.face_split(f, l_pp, ll.next, None, true)
                    {
                        let e = bm.lp(l_new).e;
                        bm.oflag_enable(e, ELE_NEW);
                        bm.face_mut(f).flag.remove(ElemFlag::TAG);
                    }
                } else {
                    // Walk backwards.
                    if let Some(mut l_new) = face_split_walk_back(bm, l) {
                        loop {
                            let e = bm.lp(l_new).e;
                            bm.oflag_enable(e, ELE_NEW);
                            l_new = bm.lp(l_new).next;
                            if !mods::vert_is_edge_pair(bm, bm.lp(l_new).v) {
                                break;
                            }
                        }
                    }
                    let f = bm.lp(l).f;
                    bm.face_mut(f).flag.remove(ElemFlag::TAG);
                }
            }
        }
    }

    if !use_cap_endpoint {
        for &v in &verts {
            let mut varr: Vec<VertId> = Vec::with_capacity(v_edges_max);
            for e in bm.vert_edges(v) {
                let v_other = bm.edge_other_vert(e, v);
                if is_new(bm, v_other) && mods::vert_is_edge_pair(bm, v_other) {
                    // Defer the join to avoid looping over data being removed.
                    bm.vert_mut(v_other).e = Some(e);
                    varr.push(v_other);
                }
            }
            while let Some(vo) = varr.pop() {
                let e = bm.vert(vo).e.expect("pushed with its edge");
                bm.join_edge_kill_vert(e, vo, true, false, false, true);
            }
        }
    }

    let edges: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, ELE_NEW))
        .collect();
    OffsetEdgeloopsOut {
        geom: edges.iter().map(|&e| BMElem::Edge(e)).collect(),
        edges,
    }
}
