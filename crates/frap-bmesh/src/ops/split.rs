// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.split` (Blender 5.2.4 `bmo_split_exec`, `operators/bmo_dupe.cc`), implemented by
//! WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.split(bmesh, geom=[], dest=None, use_only_faces=False)`.
//!
//! **Create/kill order.** A nested `duplicate` copies `geom` first (vertices, then edges,
//! then faces, in the order of `ops/duplicate.rs`); the active face moves to its copy. Then
//! the input is flagged and deleted with the `FACES` context (`BMO_mesh_delete_oflag_context`:
//! faces in face order, then edges, then vertices no longer used by an unflagged face or
//! edge). With `use_only_faces`, edges whose every face is an input face and vertices whose
//! every edge is flagged are flagged first, so they are deleted too (loose input vertices
//! and edges are then not kept). Freed slots are reused LIFO by later creations: after
//! splitting the top face of the default cube the copy's face sits in slot 6 and slot 5 is
//! free, so the copy is face 5 after conversion.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`. `dest` (another BMesh) is not
//! supported.

use crate::kernel::DeleteContext;
use crate::ops::duplicate::{DuplicateIn, duplicate};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, VertId};

/// Input slots of `split` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplitIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `dest` (another BMesh (unsupported: always this one)).
    pub dest: Option<()>,
    /// `use_only_faces`.
    pub use_only_faces: bool,
}

/// Output slots of `split`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SplitOut {
    /// `geom.out`: the copies (the nested `duplicate`'s `geom.out`).
    pub geom: Vec<BMElem>,
    /// `boundary_map.out`: boundary edges of the input → their copies; an input edge that was
    /// deleted maps from its copy (Blender uses the destination as key and value then).
    pub boundary_map: Vec<(EdgeId, EdgeId)>,
    /// `isovert_map.out`: isolated input vertices → their copies.
    pub isovert_map: Vec<(VertId, VertId)>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_split_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const SPLIT_INPUT: u16 = 1;

/// Runs `split` on `bm`.
pub fn split(bm: &mut BMesh, i: &SplitIn) -> Result<SplitOut, BmError> {
    if i.dest.is_some() {
        return Err(BmError::Invalid(
            "split: 'dest' (another BMesh) is not supported by frap".into(),
        ));
    }
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "split: dead element {dead:?} in geom"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        let dupe = duplicate(
            bm,
            &DuplicateIn {
                geom: i.geom.clone(),
                ..DuplicateIn::default()
            },
        )?;
        if let Some(af) = bm.act_face
            && let Some(&(_, nf)) = dupe.face_map.iter().find(|(s, _)| *s == af)
        {
            bm.act_face = Some(nf);
        }
        for &g in &i.geom {
            bm.oflag_enable(g, SPLIT_INPUT);
        }
        if i.use_only_faces {
            // Make sure to remove edges and verts we don't need.
            for e in bm.edge_ids() {
                let found = bm
                    .edge_faces(e)
                    .into_iter()
                    .any(|f| !bm.oflag_test(f, SPLIT_INPUT));
                if !found {
                    bm.oflag_enable(e, SPLIT_INPUT);
                }
            }
            for v in bm.vert_ids() {
                let found = bm
                    .vert_edges(v)
                    .into_iter()
                    .any(|e| !bm.oflag_test(e, SPLIT_INPUT));
                if !found {
                    bm.oflag_enable(v, SPLIT_INPUT);
                }
            }
        }
        // The outputs are made before the deletion (`boundary_map.out` keys of deleted input
        // edges become their copies), but after `BMO_mesh_delete_oflag_context(DEL_FACES)` has
        // marked what it deletes: Blender builds the map in the delete's `prepare` callback, so
        // input edges still used by a face outside the input keep their own key.
        mark_del_faces(bm, SPLIT_INPUT);
        let boundary_map = dupe
            .boundary_map
            .iter()
            .map(|&(src, dst)| {
                if bm.oflag_test(src, SPLIT_INPUT) {
                    (dst, dst)
                } else {
                    (src, dst)
                }
            })
            .collect();
        let out = SplitOut {
            geom: dupe.geom.clone(),
            boundary_map,
            isovert_map: dupe.isovert_map.clone(),
        };
        bm.delete_oflag_context(SPLIT_INPUT, DeleteContext::Faces);
        Ok(out)
    })
}

/// The marking passes of `BMO_mesh_delete_oflag_context(bm, oflag, DEL_FACES)` (they run
/// before its `prepare` callback): the vertices and edges of flagged faces are flagged, those
/// of unflagged faces and the vertices of unflagged edges are unflagged. Idempotent, so the
/// delete itself marks the same elements again.
// Ported from blender/source/blender/bmesh/intern/bmesh_delete.cc (BMO_mesh_delete_oflag_context).
fn mark_del_faces(bm: &mut BMesh, oflag: u16) {
    for f in bm.face_ids() {
        if bm.oflag_test(f, oflag) {
            for l in bm.face_loops(f) {
                let (v, e) = (bm.lp(l).v, bm.lp(l).e);
                bm.oflag_enable(v, oflag);
                bm.oflag_enable(e, oflag);
            }
        }
    }
    for f in bm.face_ids() {
        if !bm.oflag_test(f, oflag) {
            for l in bm.face_loops(f) {
                let (v, e) = (bm.lp(l).v, bm.lp(l).e);
                bm.oflag_disable(v, oflag);
                bm.oflag_disable(e, oflag);
            }
        }
    }
    for e in bm.edge_ids() {
        if !bm.oflag_test(e, oflag) {
            let [a, b] = bm.edge(e).v;
            bm.oflag_disable(a, oflag);
            bm.oflag_disable(b, oflag);
        }
    }
}
