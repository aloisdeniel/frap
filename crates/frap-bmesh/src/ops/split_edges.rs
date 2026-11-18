// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.split_edges` (Blender 5.2.4 `bmo_split_edges_exec`,
//! `operators/bmo_split_edges.cc`, and `BM_mesh_edgesplit`, `tools/bmesh_edgesplit.cc`),
//! implemented by WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.split_edges(bmesh, edges=[], verts=[], use_verts=False)`.
//!
//! **Create/kill order.** Every input edge is tagged (`BM_ELEM_TAG`); with `use_verts` the
//! input vertices are tagged too (an edge with neither vertex tagged gets both), otherwise
//! both vertices of every tagged edge. Then the edges are walked in slot order, **including
//! edges created behind the cursor** (a mempool iterator sees them); for each tagged edge
//! its tagged vertices are untagged and separated (`BM_vert_separate_hflag`): first every
//! tagged edge around the vertex (disk order from `v->e`) that has several faces is split
//! into one edge per face (`bmesh_kernel_edge_separate`, new edges appended to the disk),
//! then the vertex is split into one vertex per remaining edge fan (new vertices take freed
//! slots first), and edges that ended up joining the same vertices are spliced back
//! (`bmesh_kernel_vert_separate__cleanup`, which frees edge slots). No face is created or
//! killed. `edges.out` are the tagged edges (the input ones and their copies), edge order.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, VertId};

/// Input slots of `split_edges` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplitEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `use_verts`.
    pub use_verts: bool,
}

/// Output slots of `split_edges`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SplitEdgesOut {
    /// `edges.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out`: the old and new edges from splitting, edge order.
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_split_edges_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

// Ported from blender/source/blender/bmesh/intern/bmesh_core.cc (BM_vert_separate_hflag).
/// `BM_vert_separate_hflag`: separates `v` along every edge carrying `hflag`; returns `v`
/// followed by the new vertices (most recent first).
pub fn vert_separate_hflag(
    bm: &mut BMesh,
    v: VertId,
    hflag: ElemFlag,
    copy_select: bool,
) -> Vec<VertId> {
    // The disk cycle as it is before separating: Blender moves `e_first` to the first edge
    // the separation appends, which ends the walk exactly where the original cycle ends.
    let edges: Vec<EdgeId> = bm
        .vert_edges(v)
        .into_iter()
        .filter(|&e| bm.edge(e).flag.contains(hflag))
        .collect();
    bm.vert_separate(v, &edges, copy_select)
}

// Ported from blender/source/blender/bmesh/tools/bmesh_edgesplit.cc (BM_mesh_edgesplit).
/// `BM_mesh_edgesplit(bm, use_verts, tag_only, copy_select)`: splits the tagged edges (all
/// edges unless `tag_only`). With `copy_select` the new vertices are selected like the ones
/// they come from and inserted into the selection history after them.
pub fn mesh_edgesplit(bm: &mut BMesh, use_verts: bool, tag_only: bool, copy_select: bool) {
    let use_ese = copy_select && !bm.select_history.is_empty();
    if !tag_only {
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
        if use_verts {
            for v in bm.vert_ids() {
                bm.vert_mut(v).flag.insert(ElemFlag::TAG);
            }
        }
    }
    let tagged = |bm: &BMesh, v: VertId| bm.vert(v).flag.contains(ElemFlag::TAG);
    if use_verts {
        // Prevent one edge having both verts unflagged.
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::TAG) {
                let [a, b] = bm.edge(e).v;
                if !tagged(bm, a) && !tagged(bm, b) {
                    bm.vert_mut(a).flag.insert(ElemFlag::TAG);
                    bm.vert_mut(b).flag.insert(ElemFlag::TAG);
                }
            }
        }
    } else {
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::TAG) {
                let [a, b] = bm.edge(e).v;
                bm.vert_mut(a).flag.insert(ElemFlag::TAG);
                bm.vert_mut(b).flag.insert(ElemFlag::TAG);
            }
        }
    }
    // `BM_ITER_MESH` over the edge mempool: edges created (or slots reused) behind the cursor
    // are visited too.
    let mut s = 0u32;
    while (s as usize) < bm.edges.capacity() {
        if bm.edges.contains(s) && bm.edge(EdgeId(s)).flag.contains(ElemFlag::TAG) {
            frap_math::budget::check();
            for i in 0..2 {
                // The edge may have been spliced away by the previous vertex.
                if !bm.edges.contains(s) {
                    break;
                }
                let v = bm.edge(EdgeId(s)).v[i];
                if !tagged(bm, v) {
                    continue;
                }
                bm.vert_mut(v).flag.remove(ElemFlag::TAG);
                let pos = use_ese
                    .then(|| {
                        bm.select_history
                            .iter()
                            .position(|&h| h == BMElem::Vert(v))
                    })
                    .flatten();
                let vtar = vert_separate_hflag(bm, v, ElemFlag::TAG, copy_select);
                if let Some(p) = pos {
                    // `BM_select_history_store_after_notest` for each new vertex.
                    for &vn in vtar.iter().skip(1) {
                        bm.select_history.insert(p + 1, BMElem::Vert(vn));
                    }
                }
            }
        }
        s += 1;
    }
}

/// Runs `split_edges` on `bm`.
pub fn split_edges(bm: &mut BMesh, i: &SplitEdgesIn) -> Result<SplitEdgesOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!(
            "split_edges: dead edge {dead:?} in edges"
        )));
    }
    if let Some(dead) = i.verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!(
            "split_edges: dead vertex {dead:?} in verts"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| {
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        for &e in &i.edges {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
        if i.use_verts {
            for &v in &i.verts {
                bm.vert_mut(v).flag.insert(ElemFlag::TAG);
            }
        }
        mesh_edgesplit(bm, i.use_verts, true, false);
        let mut out = SplitEdgesOut::default();
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::TAG) {
                out.edges.push(e);
                out.geom.push(BMElem::Edge(e));
            }
        }
        out
    }))
}
