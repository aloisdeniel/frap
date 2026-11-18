// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.extrude_edge_only` (Blender 5.2.4 `bmo_extrude_edge_only_exec`,
//! `operators/bmo_extrude.cc`), implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.extrude_edge_only(bmesh, edges=[], use_normal_flip=False, use_select_history=False)`.
//!
//! **Create/kill order**: nothing is killed. The input edges and their vertices are copied by
//! `bmesh.ops.duplicate` (vertices, then edges, in iteration order; see `duplicate.rs`). Then,
//! for every copied edge (`boundary_map.out`), one quad joining the original and the copy is
//! created with `BM_face_create_verts`, which first creates the two missing side edges in the
//! order `(v3, v0)`, `(v1, v2)` of the quad's corners. The quad is wound like the face of the
//! original edge (unless `use_normal_flip`) and takes the loop data of that face
//! (`bm_extrude_copy_face_loop_attributes`).
//!
//! **Order of the side quads (deviation from Blender's run-to-run order):** Blender iterates
//! the duplicate's `boundary_map.out`, a `GHash` keyed by element pointers, so with two or more
//! input edges the order in which the side edges and quads are created changes from one run of
//! Blender to the next (memory addresses). frap creates them in the order the duplicate copied
//! the edges (edge iteration order), which is one of Blender's possible orders. Goldens with
//! several input edges therefore compare edges and faces order-free (`order=canonical`).
//!
//! Operator type flags: `NORMALS_CALC`.

use crate::ops::duplicate::{DuplicateIn, duplicate};
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};

/// Input slots of `extrude_edge_only` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrudeEdgeOnlyIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_normal_flip`.
    pub use_normal_flip: bool,
    /// `use_select_history`.
    pub use_select_history: bool,
}

/// Output slots of `extrude_edge_only`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ExtrudeEdgeOnlyOut {
    /// `geom.out`: the new vertices, edges (the copies of the input edges) and side faces, in
    /// iteration order (vertices, then edges, then faces).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_extrude_edge_only_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

const EXT_INPUT: u16 = 1;
const EXT_KEEP: u16 = 2;

// Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc
// (bm_extrude_copy_face_loop_attributes).
/// Copies the face attributes and the loop data of the face on the other side of `f`'s first
/// edge (the edge it was extruded from) onto the quad `f`: corners 0 and 3 from the other
/// face's corner at the first vertex, corners 1 and 2 from its corner at the second vertex.
/// The copy is never hidden. Nothing happens when the first edge has no other face.
pub(crate) fn copy_face_loop_attributes(bm: &mut BMesh, f: FaceId) {
    let l0 = bm.face(f).l_first;
    let l1 = bm.lp(l0).next;
    let l2 = bm.lp(l1).next;
    let l3 = bm.lp(l2).next;
    if bm.lp(l0).radial_next == l0 {
        return;
    }
    let e = bm.lp(l0).e;
    let lo0 = bm.edge_other_loop(e, l0);
    let lo1 = bm.edge_other_loop(e, l1);
    let fo = bm.lp(lo0).f;
    bm.face_attrs_copy(fo, f);
    bm.face_mut(f).flag.remove(ElemFlag::HIDDEN);
    bm.loop_attrs_copy(lo0, l0);
    bm.loop_attrs_copy(lo0, l3);
    bm.loop_attrs_copy(lo1, l1);
    bm.loop_attrs_copy(lo1, l2);
}

/// Every live element carrying the tool flag `oflag`: vertices, then edges, then faces
/// (`BMO_slot_buffer_from_enabled_flag(..., BM_ALL_NOLOOP, oflag)`).
pub(crate) fn all_with_oflag(bm: &BMesh, oflag: u16) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = bm
        .vert_ids()
        .into_iter()
        .filter(|&v| bm.oflag_test(v, oflag))
        .map(BMElem::Vert)
        .collect();
    out.extend(
        bm.edge_ids()
            .into_iter()
            .filter(|&e| bm.oflag_test(e, oflag))
            .map(BMElem::Edge),
    );
    out.extend(
        bm.face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, oflag))
            .map(BMElem::Face),
    );
    out
}

/// Runs `extrude_edge_only` on `bm`.
pub fn extrude_edge_only(
    bm: &mut BMesh,
    i: &ExtrudeEdgeOnlyIn,
) -> Result<ExtrudeEdgeOnlyOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!(
            "extrude_edge_only: dead edge {dead:?} in edges"
        )));
    }
    let out = bm.bmo_exec(TYPE_FLAG, |bm| -> Result<ExtrudeEdgeOnlyOut, BmError> {
        // Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc
        // (bmo_extrude_edge_only_exec). No skin layer in frap (`CD_MVERT_SKIN`).
        for &e in &i.edges {
            let [a, b] = bm.edge(e).v;
            bm.oflag_enable(e, EXT_INPUT);
            bm.oflag_enable(a, EXT_INPUT);
            bm.oflag_enable(b, EXT_INPUT);
        }
        // "duplicate geom=%fve": flagged vertices, then flagged edges, in iteration order.
        let mut geom: Vec<BMElem> = bm
            .vert_ids()
            .into_iter()
            .filter(|&v| bm.oflag_test(v, EXT_INPUT))
            .map(BMElem::Vert)
            .collect();
        geom.extend(
            bm.edge_ids()
                .into_iter()
                .filter(|&e| bm.oflag_test(e, EXT_INPUT))
                .map(BMElem::Edge),
        );
        let dupe = duplicate(
            bm,
            &DuplicateIn {
                geom,
                use_select_history: i.use_select_history,
                ..Default::default()
            },
        )?;
        for &(e, e_new) in &dupe.boundary_map {
            frap_math::budget::check();
            let [ev1, ev2] = bm.edge(e).v;
            let [nv1, nv2] = bm.edge(e_new).v;
            let edge_normal_flip = !bm.edge(e).l.is_some_and(|l| ev1 != bm.lp(l).v);
            let f_verts: [VertId; 4] = if edge_normal_flip == i.use_normal_flip {
                [ev1, ev2, nv2, nv1]
            } else {
                [ev2, ev1, nv1, nv2]
            };
            // Not sure what to do about the example face (Blender passes none).
            let f = bm
                .face_create_verts(&f_verts, None, CreateFlag::empty(), true)
                .expect("four distinct corners");
            copy_face_loop_attributes(bm, f);
            let e_keep = if bm.oflag_test(e, EXT_INPUT) {
                e_new
            } else {
                e
            };
            bm.oflag_enable(f, EXT_KEEP);
            bm.oflag_enable(e_keep, EXT_KEEP);
            let [k1, k2] = bm.edge(e_keep).v;
            bm.oflag_enable(k1, EXT_KEEP);
            bm.oflag_enable(k2, EXT_KEEP);
        }
        Ok(ExtrudeEdgeOnlyOut {
            geom: all_with_oflag(bm, EXT_KEEP),
        })
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit quad in the XY plane, wound counter-clockwise (normal +Z).
    pub(crate) fn quad() -> (BMesh, Vec<VertId>, FaceId) {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
        .collect();
        let f = bm
            .face_create_verts(&v, None, CreateFlag::empty(), true)
            .unwrap();
        bm.normals_update();
        (bm, v, f)
    }

    #[test]
    fn boundary_edge_gets_a_quad_wound_like_its_face() {
        let (mut bm, v, f) = quad();
        let e = bm.edge_exists(v[0], v[1]).unwrap();
        let out = extrude_edge_only(
            &mut bm,
            &ExtrudeEdgeOnlyIn {
                edges: vec![e],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((bm.verts_num(), bm.edges_num(), bm.faces_num()), (6, 7, 2));
        let f_new = FaceId(1);
        assert_eq!(
            out.geom,
            vec![
                BMElem::Vert(VertId(4)),
                BMElem::Vert(VertId(5)),
                BMElem::Edge(EdgeId(4)),
                BMElem::Face(f_new)
            ]
        );
        // The shared edge is used in opposite directions by the two faces (contiguous).
        assert!(bm.edge_is_contiguous(e));
        assert_eq!(bm.face(f_new).flag, bm.face(f).flag);
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn normal_flip_reverses_the_quad() {
        let (mut bm, v, _) = quad();
        let e = bm.edge_exists(v[0], v[1]).unwrap();
        extrude_edge_only(
            &mut bm,
            &ExtrudeEdgeOnlyIn {
                edges: vec![e],
                use_normal_flip: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!bm.edge_is_contiguous(e));
    }
}
