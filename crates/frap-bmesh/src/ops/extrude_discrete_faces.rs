// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.extrude_discrete_faces` (Blender 5.2.4 `bmo_extrude_discrete_faces_exec`,
//! `operators/bmo_extrude.cc`), implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.extrude_discrete_faces(bmesh, faces=[], use_normal_flip=False, use_select_history=False)`.
//!
//! **Create/kill order**: for every input face, in slot order: a copy of the face with new
//! vertices and edges (`BM_face_copy`: its vertices in loop order from `l_first`, then its
//! edges, then the face), then one side quad per corner in loop order, each created with
//! `BM_face_create_verts` (missing edges first, in the order `(v3, v0)`, `(v0, v1)`, …; only
//! the edge between the two original corners exists already). After all faces, the input
//! faces are deleted (`delete context=FACES_ONLY`, in iteration order); their edges and
//! vertices stay, used by the side quads. Freed face slots are left for later creations.
//!
//! `use_normal_flip` is an input slot of Blender's operator that its exec never reads (the side
//! quads always follow the original winding); frap does the same.
//!
//! Operator type flags: `NORMALS_CALC` (the caller flushes the selection).

use crate::ops::delete::{DeleteIn, delete};
use crate::ops::extrude_vert_indiv::history_replace;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, VertId};

/// Input slots of `extrude_discrete_faces` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrudeDiscreteFacesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `use_normal_flip` (unused by Blender's exec).
    pub use_normal_flip: bool,
    /// `use_select_history`.
    pub use_select_history: bool,
}

/// Output slots of `extrude_discrete_faces`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ExtrudeDiscreteFacesOut {
    /// The new faces as elements (same content as `faces`).
    pub geom: Vec<BMElem>,
    /// `faces.out`: the copies of the input faces, in iteration order.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_extrude_discrete_faces_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

const EXT_KEEP: u16 = 2;
const EXT_DEL: u16 = 4;

// Ported from blender/source/blender/bmesh/intern/bmesh_core.cc (bm_face_copy_impl,
// BM_face_copy).
/// `BM_face_copy(bm, f, copy_verts, copy_edges)`: a copy of `f` (custom data and flags of the
/// face and its loops; selection excluded), optionally on new vertices and edges.
pub(crate) fn face_copy(bm: &mut BMesh, f: FaceId, copy_verts: bool, copy_edges: bool) -> FaceId {
    let loops = bm.face_loops(f);
    let n = loops.len();
    let verts: Vec<VertId> = loops
        .iter()
        .map(|&l| {
            let v = bm.lp(l).v;
            if copy_verts {
                let co = bm.vert(v).co;
                bm.vert_create(co, Some(v), CreateFlag::empty())
            } else {
                v
            }
        })
        .collect();
    let edges: Vec<EdgeId> = loops
        .iter()
        .enumerate()
        .map(|(i, &l)| {
            let e = bm.lp(l).e;
            if copy_edges {
                let (v1, v2) = if bm.edge(e).v[0] == verts[i] {
                    (verts[i], verts[(i + 1) % n])
                } else {
                    (verts[(i + 1) % n], verts[i])
                };
                bm.edge_create(v1, v2, Some(e), CreateFlag::empty())
            } else {
                e
            }
        })
        .collect();
    let f_copy = bm
        .face_create(&verts, &edges, None, CreateFlag::SKIP_CD)
        .expect("source face has corners");
    bm.face_attrs_copy(f, f_copy);
    for (ls, ld) in loops.into_iter().zip(bm.face_loops(f_copy)) {
        bm.loop_attrs_copy(ls, ld);
    }
    f_copy
}

/// Runs `extrude_discrete_faces` on `bm`.
pub fn extrude_discrete_faces(bm: &mut BMesh, i: &ExtrudeDiscreteFacesIn) -> Result<ExtrudeDiscreteFacesOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "extrude_discrete_faces: dead face {dead:?} in faces"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        // Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc
        // (bmo_extrude_discrete_faces_exec).
        for &f_org in &i.faces {
            frap_math::budget::check();
            bm.oflag_enable(f_org, EXT_DEL);
            let f_new = face_copy(bm, f_org, true, true);
            bm.oflag_enable(f_new, EXT_KEEP);
            if i.use_select_history {
                history_replace(bm, BMElem::Face(f_org), BMElem::Face(f_new));
            }
            let l_org_first = bm.face(f_org).l_first;
            let mut l_org = l_org_first;
            let mut l_new = bm.face(f_new).l_first;
            loop {
                bm.loop_attrs_copy(l_org, l_new);
                let org_next = bm.lp(l_org).next;
                let new_next = bm.lp(l_new).next;
                let quad = [
                    bm.lp(org_next).v,
                    bm.lp(new_next).v,
                    bm.lp(l_new).v,
                    bm.lp(l_org).v,
                ];
                let f_side = bm
                    .face_create_verts(&quad, Some(f_org), CreateFlag::empty(), true)
                    .expect("four distinct corners");
                let s0 = bm.face(f_side).l_first;
                let s1 = bm.lp(s0).next;
                let s2 = bm.lp(s1).next;
                let s3 = bm.lp(s2).next;
                bm.loop_attrs_copy(org_next, s0);
                bm.loop_attrs_copy(org_next, s1);
                bm.loop_attrs_copy(l_org, s2);
                bm.loop_attrs_copy(l_org, s3);
                if i.use_select_history {
                    let (vo, vn) = (bm.lp(l_org).v, bm.lp(l_new).v);
                    history_replace(bm, BMElem::Vert(vo), BMElem::Vert(vn));
                    let (eo, en) = (bm.lp(l_org).e, bm.lp(l_new).e);
                    history_replace(bm, BMElem::Edge(eo), BMElem::Edge(en));
                }
                l_new = new_next;
                l_org = org_next;
                if l_org == l_org_first {
                    break;
                }
            }
        }
        let del: Vec<BMElem> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, EXT_DEL))
            .map(BMElem::Face)
            .collect();
        delete(
            bm,
            &DeleteIn {
                geom: del,
                context: "FACES_ONLY",
            },
        )?;
        let faces: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, EXT_KEEP))
            .collect();
        Ok(ExtrudeDiscreteFacesOut {
            geom: faces.iter().copied().map(BMElem::Face).collect(),
            faces,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_adjacent_faces_are_extruded_separately() {
        // Two quads sharing an edge: V6 E7 F2 -> each face gets 4 new vertices, 4 cap edges,
        // 4 side edges and 4 side quads; the originals are deleted.
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
        .collect();
        let a = bm
            .face_create_verts(&[v[0], v[1], v[2], v[3]], None, CreateFlag::empty(), true)
            .unwrap();
        let b = bm
            .face_create_verts(&[v[1], v[4], v[5], v[2]], None, CreateFlag::empty(), true)
            .unwrap();
        bm.normals_update();
        bm.select_history.push(BMElem::Face(b));
        let out = extrude_discrete_faces(
            &mut bm,
            &ExtrudeDiscreteFacesIn {
                faces: vec![a, b],
                use_select_history: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((bm.verts_num(), bm.edges_num(), bm.faces_num()), (14, 23, 10));
        assert_eq!(out.faces.len(), 2);
        assert!(!bm.faces.contains(a.0) || !out.faces.contains(&a));
        assert_eq!(bm.select_history, vec![BMElem::Face(out.faces[1])]);
        for e in bm.edge_ids() {
            assert!(bm.edge_face_count(e) <= 2);
        }
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn face_copy_reuses_the_loop_start() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .into_iter()
            .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
            .collect();
        let f = bm
            .face_create_verts(&v, None, CreateFlag::empty(), true)
            .unwrap();
        let c = face_copy(&mut bm, f, true, true);
        let cv: Vec<[f32; 3]> = bm.face_verts(c).iter().map(|&x| bm.vert(x).co).collect();
        let ov: Vec<[f32; 3]> = bm.face_verts(f).iter().map(|&x| bm.vert(x).co).collect();
        assert_eq!(cv, ov);
        // Edges run from the next corner back to the current one (copied vertices never equal
        // the source edge's first vertex).
        assert_eq!(bm.edge(EdgeId(3)).v, [VertId(4), VertId(3)]);
    }
}
