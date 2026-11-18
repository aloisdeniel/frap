// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.extrude_vert_indiv` (Blender 5.2.4 `bmo_extrude_vert_indiv_exec`,
//! `operators/bmo_extrude.cc`), implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.extrude_vert_indiv(bmesh, verts=[], use_select_history=False)`.
//!
//! **Create/kill order**: nothing is killed. For every input vertex, in slot order: a copy of
//! the vertex (`BM_vert_create` with the original as example: custom data and flags except
//! the selection), then the wire edge joining them. The edge runs original → copy, except when
//! the original is the end of a wire chain whose only edge starts at it (`v->e->v1 == v`): then
//! copy → original, so extruded chains keep a contiguous direction. New elements take freed
//! slots first (LIFO), then append.
//!
//! With `use_select_history` every history entry naming an input vertex is replaced by its
//! copy (`BM_select_history_map_create`).
//!
//! Operator type flags: `SELECT_FLUSH` (a top-level call flushes the selection, keeping the
//! history).

use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, VertId};

/// Input slots of `extrude_vert_indiv` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrudeVertIndivIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `use_select_history`.
    pub use_select_history: bool,
}

/// Output slots of `extrude_vert_indiv`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ExtrudeVertIndivOut {
    /// The new vertices, then the new edges, in iteration order.
    pub geom: Vec<BMElem>,
    /// `verts.out`: the new vertices, in iteration order.
    pub verts: Vec<VertId>,
    /// `edges.out`: the new wire edges, in iteration order.
    pub edges: Vec<EdgeId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_extrude_vert_indiv_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::SELECT_FLUSH;

const EXT_KEEP: u16 = 2;

/// `BM_vert_is_wire_endpoint`: `v` has exactly one edge and it has no face.
pub(crate) fn vert_is_wire_endpoint(bm: &BMesh, v: VertId) -> bool {
    match bm.vert(v).e {
        Some(e) if bm.edge(e).l.is_none() => bm.disk_edge_next(e, v) == e,
        _ => false,
    }
}

/// Replaces every history entry `from` by `to` (an entry of `BM_select_history_map_create`
/// re-pointed at the new element).
pub(crate) fn history_replace(bm: &mut BMesh, from: BMElem, to: BMElem) {
    for h in &mut bm.select_history {
        if *h == from {
            *h = to;
        }
    }
}

/// Runs `extrude_vert_indiv` on `bm`.
pub fn extrude_vert_indiv(
    bm: &mut BMesh,
    i: &ExtrudeVertIndivIn,
) -> Result<ExtrudeVertIndivOut, BmError> {
    if let Some(dead) = i.verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!(
            "extrude_vert_indiv: dead vertex {dead:?} in verts"
        )));
    }
    let out = bm.bmo_exec(TYPE_FLAG, |bm| {
        // Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc
        // (bmo_extrude_vert_indiv_exec). frap has no skin layer (`CD_MVERT_SKIN`), so the
        // skin-root clean-up has nothing to do.
        for &v_in in &i.verts {
            frap_math::budget::check();
            let co = bm.vert(v_in).co;
            let dupev = bm.vert_create(co, Some(v_in), CreateFlag::empty());
            bm.oflag_enable(dupev, EXT_KEEP);
            if i.use_select_history {
                history_replace(bm, BMElem::Vert(v_in), BMElem::Vert(dupev));
            }
            let (mut a, mut b) = (v_in, dupev);
            // Not essential, but ensures face normals from extruded edges are contiguous.
            if vert_is_wire_endpoint(bm, a) {
                let e = bm.vert(a).e.expect("wire endpoint has an edge");
                if bm.edge(e).v[0] == a {
                    std::mem::swap(&mut a, &mut b);
                }
            }
            let e = bm.edge_create(a, b, None, CreateFlag::empty());
            bm.oflag_enable(e, EXT_KEEP);
        }
        let mut out = ExtrudeVertIndivOut::default();
        for v in bm.vert_ids() {
            if bm.oflag_test(v, EXT_KEEP) {
                out.verts.push(v);
                out.geom.push(BMElem::Vert(v));
            }
        }
        for e in bm.edge_ids() {
            if bm.oflag_test(e, EXT_KEEP) {
                out.edges.push(e);
                out.geom.push(BMElem::Edge(e));
            }
        }
        out
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .into_iter()
            .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
            .collect();
        bm.face_create_verts(&v, None, CreateFlag::empty(), true);
        (bm, v)
    }

    #[test]
    fn each_vertex_gets_a_copy_and_a_wire_edge() {
        let (mut bm, v) = tri();
        let out = extrude_vert_indiv(
            &mut bm,
            &ExtrudeVertIndivIn {
                verts: vec![v[0], v[2]],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.verts, vec![VertId(3), VertId(4)]);
        assert_eq!(out.edges, vec![EdgeId(3), EdgeId(4)]);
        assert_eq!(bm.edge(EdgeId(3)).v, [v[0], VertId(3)]);
        assert_eq!(bm.edge(EdgeId(4)).v, [v[2], VertId(4)]);
        assert_eq!(bm.vert(VertId(4)).co, [0.0, 1.0, 0.0]);
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn wire_endpoint_keeps_chain_direction_and_history_follows() {
        let mut bm = BMesh::new();
        let a = bm.vert_create([0.0; 3], None, CreateFlag::empty());
        let b = bm.vert_create([1.0, 0.0, 0.0], None, CreateFlag::empty());
        bm.edge_create(a, b, None, CreateFlag::empty());
        bm.select_history.push(BMElem::Vert(a));
        let out = extrude_vert_indiv(
            &mut bm,
            &ExtrudeVertIndivIn {
                verts: vec![a],
                use_select_history: true,
            },
        )
        .unwrap();
        let (c, e) = (out.verts[0], out.edges[0]);
        // `a` starts its only wire edge, so the new edge runs copy -> original.
        assert_eq!(bm.edge(e).v, [c, a]);
        assert_eq!(bm.select_history, vec![BMElem::Vert(c)]);
    }

    #[test]
    fn dead_input_is_an_error() {
        let (mut bm, _) = tri();
        let r = extrude_vert_indiv(
            &mut bm,
            &ExtrudeVertIndivIn {
                verts: vec![VertId(9)],
                ..Default::default()
            },
        );
        assert!(r.is_err());
    }
}
