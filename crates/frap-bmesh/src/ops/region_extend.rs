// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.region_extend` (Blender 5.2.4 `bmo_region_extend_exec`, `operators/bmo_utils.cc`),
//! implemented by WP 2.4 (Selection): the core of `mesh.select_more` / `mesh.select_less`
//! (`EDBM_select_more` / `EDBM_select_less`).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.region_extend(bmesh, geom=[], use_contract=False, use_faces=False, use_face_step=False)`.
//!
//! **Create/kill order**: nothing is created or killed; the op only reads topology and returns
//! the elements one ring outside (or, with `use_contract`, on the inside border of) the input
//! region in `geom` (`geom.out`: vertices, then edges, then faces, each in iteration order, as
//! `BMO_slot_buffer_from_enabled_flag` collects them).
//!
//! - Expand, vertices (`use_faces=False`): every input vertex with an edge outside the input
//!   (hidden edges ignored) adds its visible edges and their other vertices; with
//!   `use_face_step` it adds its visible faces (with their edges and vertices) and its wire
//!   edges instead.
//! - Expand, faces (`use_faces=True`): the visible faces sharing an edge (`use_face_step`: a
//!   vertex) with an input face, not in the input.
//! - Contract, vertices: every input vertex with an edge (`use_face_step`: a face, else a wire
//!   edge) outside the input is returned with all its edges.
//! - Contract, faces: every input face with a neighbour across an edge (`use_face_step`: a
//!   vertex) outside the input.
//!
//! Operator type flags: `SELECT_FLUSH | SELECT_VALIDATE` (a top-level call flushes the
//! selection afterwards).

use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};

/// Input slots of `region_extend` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionExtendIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `use_contract`: find the boundary inside the regions, not outside.
    pub use_contract: bool,
    /// `use_faces`: extend from faces instead of edges.
    pub use_faces: bool,
    /// `use_face_step`: step over connected faces.
    pub use_face_step: bool,
}

/// Output slots of `region_extend`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RegionExtendOut {
    /// `geom.out`: the computed boundary geometry (vertices, edges, faces in iteration order).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_region_extend_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::SELECT_FLUSH.union(OpTypeFlag::SELECT_VALIDATE);

/// `SEL_FLAG`: part of the result.
const SEL_FLAG: u16 = 1;
/// `SEL_ORIG`: part of the input.
const SEL_ORIG: u16 = 2;

fn hidden(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    bm.elem_flag(e.into()).contains(ElemFlag::HIDDEN)
}

/// `bmo_face_flag_set_flush`: flags the face, its edges and its vertices.
fn face_flag_set_flush(bm: &mut BMesh, f: FaceId, oflag: u16, value: bool) {
    bm.oflag_set(f, oflag, value);
    for l in bm.face_loops(f) {
        let (v, e) = (bm.lp(l).v, bm.lp(l).e);
        bm.oflag_set(e, oflag, value);
        bm.oflag_set(v, oflag, value);
    }
}

fn input_verts(i: &RegionExtendIn) -> impl Iterator<Item = VertId> + '_ {
    i.geom.iter().filter_map(|g| match g {
        BMElem::Vert(v) => Some(*v),
        _ => None,
    })
}

fn input_faces(i: &RegionExtendIn) -> impl Iterator<Item = FaceId> + '_ {
    i.geom.iter().filter_map(|g| match g {
        BMElem::Face(f) => Some(*f),
        _ => None,
    })
}

// Ported from blender/source/blender/bmesh/operators/bmo_utils.cc (bmo_region_extend_expand).
fn expand(bm: &mut BMesh, i: &RegionExtendIn) {
    if !i.use_faces {
        for v in input_verts(i) {
            let edges = bm.vert_edges(v);
            let found = edges
                .iter()
                .any(|&e| !bm.oflag_test(e, SEL_ORIG) && !hidden(bm, e));
            if !found {
                continue;
            }
            if !i.use_face_step {
                for &e in &edges {
                    if !bm.oflag_test(e, SEL_FLAG) && !hidden(bm, e) {
                        bm.oflag_enable(e, SEL_FLAG);
                        let o = bm.edge_other_vert(e, v);
                        bm.oflag_enable(o, SEL_FLAG);
                    }
                }
            } else {
                for f in bm.vert_faces(v) {
                    if !bm.oflag_test(f, SEL_FLAG) && !hidden(bm, f) {
                        face_flag_set_flush(bm, f, SEL_FLAG, true);
                    }
                }
                // Handle wire edges (when stepping over faces).
                for &e in &edges {
                    if bm.edge_is_wire(e) && !bm.oflag_test(e, SEL_FLAG) && !hidden(bm, e) {
                        bm.oflag_enable(e, SEL_FLAG);
                        let o = bm.edge_other_vert(e, v);
                        bm.oflag_enable(o, SEL_FLAG);
                    }
                }
            }
        }
    } else {
        for f in input_faces(i) {
            for l in bm.face_loops(f) {
                let others = if !i.use_face_step {
                    bm.edge_faces(bm.lp(l).e)
                } else {
                    bm.vert_faces(bm.lp(l).v)
                };
                for fo in others {
                    if !bm.oflag_test(fo, SEL_ORIG | SEL_FLAG) && !hidden(bm, fo) {
                        bm.oflag_enable(fo, SEL_FLAG);
                    }
                }
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_utils.cc (bmo_region_extend_contract).
fn contract(bm: &mut BMesh, i: &RegionExtendIn) {
    if !i.use_faces {
        for v in input_verts(i) {
            let found = if !i.use_face_step {
                bm.vert_edges(v)
                    .into_iter()
                    .any(|e| !bm.oflag_test(e, SEL_ORIG))
            } else {
                bm.vert_faces(v)
                    .into_iter()
                    .any(|f| !bm.oflag_test(f, SEL_ORIG))
                    // Handle wire edges (when stepping over faces).
                    || bm
                        .vert_edges(v)
                        .into_iter()
                        .any(|e| bm.edge_is_wire(e) && !bm.oflag_test(e, SEL_ORIG))
            };
            if found {
                bm.oflag_enable(v, SEL_FLAG);
                for e in bm.vert_edges(v) {
                    bm.oflag_enable(e, SEL_FLAG);
                }
            }
        }
    } else {
        for f in input_faces(i) {
            for l in bm.face_loops(f) {
                let others = if !i.use_face_step {
                    bm.edge_faces(bm.lp(l).e)
                } else {
                    bm.vert_faces(bm.lp(l).v)
                };
                if others.into_iter().any(|fo| !bm.oflag_test(fo, SEL_ORIG)) {
                    bm.oflag_enable(f, SEL_FLAG);
                }
            }
        }
    }
}

/// `BMO_slot_buffer_from_enabled_flag(…, BM_ALL_NOLOOP, SEL_FLAG)`.
fn flagged(bm: &BMesh) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = bm
        .verts
        .ids()
        .map(VertId)
        .filter(|&v| bm.oflag_test(v, SEL_FLAG))
        .map(BMElem::Vert)
        .collect();
    out.extend(
        bm.edges
            .ids()
            .map(EdgeId)
            .filter(|&e| bm.oflag_test(e, SEL_FLAG))
            .map(BMElem::Edge),
    );
    out.extend(
        bm.faces
            .ids()
            .map(FaceId)
            .filter(|&f| bm.oflag_test(f, SEL_FLAG))
            .map(BMElem::Face),
    );
    out
}

/// Runs `region_extend` on `bm`.
pub fn region_extend(bm: &mut BMesh, i: &RegionExtendIn) -> Result<RegionExtendOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "region_extend: dead element {dead:?} in geom"
        )));
    }
    let geom = bm.bmo_exec(TYPE_FLAG, |bm| {
        for &g in &i.geom {
            bm.oflag_enable(g, SEL_ORIG);
        }
        if i.use_contract {
            contract(bm, i);
        } else {
            expand(bm, i);
        }
        flagged(bm)
    });
    Ok(RegionExtendOut { geom })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3 x 1 strip of quads: vertices 0..8 (two rows of four), faces 0..3.
    fn strip() -> BMesh {
        let mut bm = BMesh::new();
        let mut v = Vec::new();
        for y in 0..2 {
            for x in 0..4 {
                v.push(bm.vert_create([x as f32, y as f32, 0.0], None, Default::default()));
            }
        }
        for x in 0..3 {
            let q = [v[x], v[x + 1], v[x + 5], v[x + 4]];
            bm.face_create_verts(&q, None, Default::default(), true)
                .expect("quad");
        }
        bm
    }

    fn ids(out: &[BMElem]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
        let mut r = (Vec::new(), Vec::new(), Vec::new());
        for e in out {
            match e {
                BMElem::Vert(v) => r.0.push(v.0),
                BMElem::Edge(e) => r.1.push(e.0),
                BMElem::Face(f) => r.2.push(f.0),
            }
        }
        r
    }

    #[test]
    fn faces_expand_and_contract() {
        let mut bm = strip();
        let grow = region_extend(
            &mut bm,
            &RegionExtendIn {
                geom: vec![BMElem::Face(FaceId(1))],
                use_faces: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ids(&grow.geom).2, vec![0, 2]);
        let shrink = region_extend(
            &mut bm,
            &RegionExtendIn {
                geom: vec![BMElem::Face(FaceId(0)), BMElem::Face(FaceId(1))],
                use_faces: true,
                use_contract: true,
                ..Default::default()
            },
        )
        .unwrap();
        // Face 0 borders nothing outside across an edge; face 1 borders face 2.
        assert_eq!(ids(&shrink.geom).2, vec![1]);
        // The scratch tool-flag layer is popped.
        assert_eq!(bm.oflags.depth(), 0);
    }

    #[test]
    fn verts_expand_by_edges_or_faces() {
        let mut bm = strip();
        let geom = vec![BMElem::Vert(VertId(0))];
        let by_edges = region_extend(
            &mut bm,
            &RegionExtendIn {
                geom: geom.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let (v, e, f) = ids(&by_edges.geom);
        assert_eq!((v, e.len(), f), (vec![1, 4], 2, vec![]));
        let by_faces = region_extend(
            &mut bm,
            &RegionExtendIn {
                geom,
                use_face_step: true,
                ..Default::default()
            },
        )
        .unwrap();
        let (v, e, f) = ids(&by_faces.geom);
        assert_eq!((v, e.len(), f), (vec![0, 1, 4, 5], 4, vec![0]));
    }

    #[test]
    fn hidden_neighbours_are_skipped() {
        let mut bm = strip();
        bm.face_mut(FaceId(2)).flag.insert(ElemFlag::HIDDEN);
        let out = region_extend(
            &mut bm,
            &RegionExtendIn {
                geom: vec![BMElem::Face(FaceId(1))],
                use_faces: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ids(&out.geom).2, vec![0]);
    }
}
