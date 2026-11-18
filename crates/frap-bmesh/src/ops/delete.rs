// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.delete` (Blender 5.2.4 `bmo_delete_exec`, `operators/bmo_dupe.cc`), implemented
//! by WP 2.1 (BMesh core).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.delete(bmesh, geom=[], context='VERTS')`.
//!
//! **Create/kill order** (`BMO_mesh_delete_oflag_context`, `bmesh_delete.cc`): nothing is
//! created. The input is flagged `DEL_INPUT`; then, per context: `VERTS` kills flagged
//! vertices in vertex order (`BM_vert_kill`: each kills its edges in disk order, each edge its
//! faces in radial order); `EDGES` flags the edges' vertices, kills flagged edges in edge
//! order, then the flagged vertices left loose; `EDGES_FACES` kills flagged edges;
//! `FACES_ONLY` kills flagged faces; `FACES`/`FACES_KEEP_BOUNDARY` flag the faces' vertices
//! and edges, unflag those still used by unflagged faces (and by unflagged edges; boundary
//! edges stay with `KEEP_BOUNDARY`), then kill faces, edges and vertices in that order;
//! `TAGGED_ONLY` kills flagged faces, edges, vertices. Freed slots are reused LIFO by later
//! creations ([MESH App. B.6]).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE` (a top-level call
//! updates normals and flushes the selection afterwards).

use crate::kernel::DeleteContext;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError};

/// Input slots of `delete` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `context` (enum identifier).
    pub context: &'static str,
}

impl Default for DeleteIn {
    fn default() -> Self {
        DeleteIn {
            geom: Vec::new(),
            context: "VERTS",
        }
    }
}

/// Output slots of `delete` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DeleteOut {
    /// Always empty (`bmesh.ops.delete` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_delete_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// Runs `delete` on `bm`.
pub fn delete(bm: &mut BMesh, i: &DeleteIn) -> Result<DeleteOut, BmError> {
    let Some(ctx) = DeleteContext::from_identifier(i.context) else {
        return Err(BmError::Invalid(format!(
            "delete: unknown context '{}' (expected VERTS, EDGES, FACES_ONLY, EDGES_FACES, \
             FACES, FACES_KEEP_BOUNDARY or TAGGED_ONLY)",
            i.context
        )));
    };
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!("delete: dead element {dead:?} in geom")));
    }
    const DEL_INPUT: u16 = 1;
    bm.bmo_exec(TYPE_FLAG, |bm| {
        for &g in &i.geom {
            bm.oflag_enable(g, DEL_INPUT);
        }
        bm.delete_oflag_context(DEL_INPUT, ctx);
    });
    Ok(DeleteOut::default())
}
