// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.holes_fill` (Blender 5.2.4 `bmo_holes_fill_exec`, `operators/bmo_fill_holes.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.holes_fill(bmesh, edges=[], sides=0)`.
//!
//! **Create/kill order**: `BM_mesh_edgenet` over the input edges (faces in path-search order,
//! see `contextual_create/edgenet.rs`); with `sides != 0` the new faces with more corners are
//! killed afterwards (mesh order); the rest take the attributes and winding of their
//! neighbours (`face_attribute_fill`), lone ones are oriented by `recalc_face_normals`.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use crate::ops::contextual_create::{attribute_fill, construct, edgenet};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId};

/// Input slots of `holes_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HolesFillIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `sides`.
    pub sides: i32,
}

/// Output slots of `holes_fill`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct HolesFillOut {
    /// `faces.out` as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the new faces (mesh order).
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_holes_fill_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

/// Runs `holes_fill` on `bm`.
pub fn holes_fill(bm: &mut BMesh, i: &HolesFillIn) -> Result<HolesFillOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|&&e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("holes_fill: dead edge {dead:?}")));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

fn tagged_faces(bm: &BMesh) -> Vec<FaceId> {
    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
        .collect()
}

// Ported from blender/source/blender/bmesh/operators/bmo_fill_holes.cc (bmo_holes_fill_exec).
fn exec(bm: &mut BMesh, i: &HolesFillIn) -> HolesFillOut {
    let sides = i.sides.max(0) as u32;
    construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    construct::faces_flag_disable_all(bm, ElemFlag::TAG);
    for &e in &i.edges {
        bm.edge_mut(e).flag.insert(ElemFlag::TAG);
    }
    edgenet::mesh_edgenet(bm, true, true);
    if sides != 0 {
        for f in tagged_faces(bm) {
            if bm.face(f).len > sides {
                bm.face_kill(f);
            }
        }
    }
    let faces = tagged_faces(bm);
    let fail = attribute_fill::face_attribute_fill(bm, &faces, true, true);
    if !fail.is_empty() {
        for &f in &fail {
            bm.face_normal_update(f);
        }
        attribute_fill::recalc_face_normals(bm, &fail);
    }
    HolesFillOut {
        geom: faces.iter().map(|&f| BMElem::Face(f)).collect(),
        faces,
    }
}
