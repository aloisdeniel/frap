// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_verts` (Blender 5.2.4 `bmo_dissolve_verts_exec`, `operators/
//! bmo_dissolve.cc`), implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_verts(bmesh, verts=[], use_face_split=False, use_boundary_tear=False)`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/bmo_dissolve.cc):
//! with `use_face_split`, every face corner at an input vertex (of more than two edges) whose
//! neighbours are not input vertices is cut off first (`BM_face_split`, a new edge and face
//! per corner, in vertex then corner order); `use_boundary_tear` does the same at boundary
//! vertices and then kills their edges. Wire edges at the input vertices are killed. Then, in
//! input order, the faces around each input vertex (that did not start with exactly two edges)
//! are joined pairwise across each of its edges (`BM_faces_join_pair`: each join **creates**
//! the merged face and frees the two old ones); edges left without faces are killed in edge
//! order, input vertices left between two edges are dissolved into one edge
//! (`BM_vert_collapse_edge`), and vertices left without edges are killed in vertex order.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

pub(crate) mod mods;

use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, VertId};

/// Input slots of `dissolve_verts` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveVertsIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `use_face_split`.
    pub use_face_split: bool,
    /// `use_boundary_tear`.
    pub use_boundary_tear: bool,
}

/// Output slots of `dissolve_verts` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveVertsOut {
    /// Always empty (`bmesh.ops.dissolve_verts` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_dissolve_verts_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const VERT_MARK: u16 = 1;
const VERT_ISGC: u16 = 8;
const VERT_MARK_PAIR: u16 = 4;
const VERT_MARK_TEAR: u16 = 16;
const EDGE_ISGC: u16 = 8;

/// Runs `dissolve_verts` on `bm`.
pub fn dissolve_verts(bm: &mut BMesh, i: &DissolveVertsIn) -> Result<DissolveVertsOut, BmError> {
    if let Some(v) = i.verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!("dissolve_verts: dead vertex {v:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        dissolve_verts_exec(bm, &i.verts, i.use_face_split, i.use_boundary_tear);
    });
    Ok(DissolveVertsOut::default())
}

/// `bm_vert_collapse_edge_and_merge`: `BM_vert_collapse_edge` on an edge-pair vertex, with the
/// two edges' flags merged first (`HIDDEN` only when both are hidden) and the surviving edge
/// hidden (and deselected) when it ends at a hidden vertex.
pub(crate) fn bm_vert_collapse_edge_and_merge(
    bm: &mut BMesh,
    v: VertId,
    do_del: bool,
) -> Option<EdgeId> {
    let (e0, e1) = mods::vert_edge_pair(bm, v)?;
    mods::elem_flag_merge_ex(bm, BMElem::Edge(e0), BMElem::Edge(e1), ElemFlag::HIDDEN);
    let e_kill = bm.vert(v).e?;
    let e_new = mods::vert_collapse_edge(bm, e_kill, v, do_del, true, true);
    if let Some(e_new) = e_new
        && bm.edges.contains(e_new.0)
        && !bm.edge(e_new).flag.contains(ElemFlag::HIDDEN)
    {
        let [a, b] = bm.edge(e_new).v;
        if bm.vert(a).flag.contains(ElemFlag::HIDDEN) || bm.vert(b).flag.contains(ElemFlag::HIDDEN)
        {
            if bm.edge(e_new).flag.contains(ElemFlag::SELECT) {
                bm.edge_select_set_noflush(e_new, false);
            }
            bm.edge_mut(e_new).flag.insert(ElemFlag::HIDDEN);
        }
    }
    e_new
}

/// `bm_face_split(bm, oflag, use_edge_delete)`: at every vertex flagged `oflag` that is not an
/// edge pair, cuts off each corner (of a face with more than three sides) whose neighbouring
/// corners are not flagged; with `use_edge_delete` the vertex's edges are then killed (last
/// vertex first).
pub(crate) fn bm_face_split(bm: &mut BMesh, oflag: u16, use_edge_delete: bool) {
    let mut edge_delete_verts: Vec<VertId> = Vec::new();
    for v in bm.vert_ids() {
        if !bm.verts.contains(v.0) || !bm.oflag_test(v, oflag) {
            continue;
        }
        if mods::vert_is_edge_pair(bm, v) {
            continue;
        }
        for l in bm.vert_loops(v) {
            let f = bm.lp(l).f;
            if bm.face(f).len > 3 {
                let (ln, lp) = (bm.lp(l).next, bm.lp(l).prev);
                if !bm.oflag_test(bm.lp(ln).v, oflag) && !bm.oflag_test(bm.lp(lp).v, oflag) {
                    bm.face_split(f, ln, lp, None, true);
                }
            }
        }
        if use_edge_delete {
            edge_delete_verts.push(v);
        }
    }
    while let Some(v) = edge_delete_verts.pop() {
        // Remove surrounding edges & faces.
        while let Some(e) = bm.vert(v).e {
            bm.edge_kill(e);
        }
    }
}

fn dissolve_verts_exec(bm: &mut BMesh, verts: &[VertId], use_face_split: bool, use_boundary_tear: bool) {
    for &v in verts {
        bm.oflag_enable(v, VERT_MARK | VERT_ISGC);
    }
    if use_face_split {
        bm_face_split(bm, VERT_MARK, false);
    }
    if use_boundary_tear {
        for &v in verts {
            if !mods::vert_is_edge_pair(bm, v)
                && bm.vert_edges(v).into_iter().any(|e| bm.edge_is_boundary(e))
            {
                bm.oflag_enable(v, VERT_MARK_TEAR);
            }
        }
        bm_face_split(bm, VERT_MARK_TEAR, true);
    }

    for &v in verts {
        let mut e_first: Option<EdgeId> = None;
        for l_first in bm.vert_loops(v) {
            let mut l_iter = l_first;
            loop {
                let (lv, le) = (bm.lp(l_iter).v, bm.lp(l_iter).e);
                bm.oflag_enable(lv, VERT_ISGC);
                bm.oflag_enable(le, EDGE_ISGC);
                l_iter = bm.lp(l_iter).next;
                if l_iter == l_first {
                    break;
                }
            }
            e_first = Some(bm.lp(l_first).e);
        }
        // Important: `e_first` won't be deleted.
        if let Some(e_first) = e_first {
            let mut e = e_first;
            loop {
                let e_next = bm.disk_edge_next(e, v);
                if bm.edge_is_wire(e) {
                    bm.edge_kill(e);
                }
                e = e_next;
                if e == e_first {
                    break;
                }
            }
        }
    }

    for &v in verts {
        // Tag here so we avoid feedback loop (checking topology as we edit).
        if mods::vert_is_edge_pair(bm, v) {
            bm.oflag_enable(v, VERT_MARK_PAIR);
        }
    }

    for &v in verts {
        frap_math::budget::check();
        if bm.oflag_test(v, VERT_MARK_PAIR) {
            continue;
        }
        // Merge across every edge that touches `v`. The joins (`do_del = false`) only kill
        // faces, so the disk cycle of `v` does not change while it is walked.
        for e in bm.vert_edges(v) {
            if let Some((l_a, l_b)) = mods::edge_loop_pair(bm, e) {
                mods::faces_join_pair(bm, l_a, l_b, false);
            }
        }
    }

    // Cleanup geometry (BM_faces_join_pair, but it removes geometry we're looping on), so do
    // this in a separate pass instead.
    for e in bm.edge_ids() {
        if bm.edges.contains(e.0) && bm.edge(e).l.is_none() && bm.oflag_test(e, EDGE_ISGC) {
            bm.edge_kill(e);
        }
    }

    // Final cleanup.
    for &v in verts {
        if bm.verts.contains(v.0) && mods::vert_is_edge_pair(bm, v) {
            bm_vert_collapse_edge_and_merge(bm, v, false);
        }
    }

    for v in bm.vert_ids() {
        if bm.verts.contains(v.0) && bm.vert(v).e.is_none() && bm.oflag_test(v, VERT_ISGC) {
            bm.vert_kill(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CreateFlag;

    /// A 2 x 2 grid of quads (9 vertices, centre `v4`).
    fn grid() -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let mut v = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                v.push(bm.vert_create([x as f32, y as f32, 0.0], None, CreateFlag::empty()));
            }
        }
        for y in 0..2 {
            for x in 0..2 {
                let a = y * 3 + x;
                bm.face_create_verts(
                    &[v[a], v[a + 1], v[a + 4], v[a + 3]],
                    None,
                    CreateFlag::empty(),
                    true,
                );
            }
        }
        (bm, v)
    }

    #[test]
    fn dissolving_the_centre_merges_four_quads() {
        let (mut bm, v) = grid();
        dissolve_verts(
            &mut bm,
            &DissolveVertsIn {
                verts: vec![v[4]],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        assert_eq!(bm.verts_num(), 8);
        assert_eq!(bm.faces_num(), 1);
        assert_eq!(bm.edges_num(), 8);
    }

    #[test]
    fn dissolving_a_boundary_edge_pair_vertex_joins_its_edges() {
        let (mut bm, v) = grid();
        dissolve_verts(
            &mut bm,
            &DissolveVertsIn {
                verts: vec![v[1]],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        assert_eq!(bm.verts_num(), 8);
        assert_eq!(bm.faces_num(), 3);
    }

    #[test]
    fn face_split_keeps_the_corners() {
        let (mut bm, v) = grid();
        dissolve_verts(
            &mut bm,
            &DissolveVertsIn {
                verts: vec![v[4]],
                use_face_split: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        assert_eq!(bm.verts_num(), 8);
        // Four corner triangles around a central quad.
        assert_eq!(bm.faces_num(), 5);
    }
}
