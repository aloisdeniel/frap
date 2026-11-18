// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.flip_quad_tessellation()`, tier T1 (`tests/golden/cases/edit/misc/
//! flip_quad_tessellation.py`). Port of `edbm_flip_quad_tessellation_exec`
//! (`editors/mesh/editmesh_tools.cc`) and `bmo_flip_quad_tessellation_exec`
//! (`bmesh/operators/bmo_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with a selected face: every selected quad starts one corner
//! later (`l_first = l_first->next`), which moves its tessellation diagonal from corners 0–2
//! to 1–3 (looptris and glTF export follow the corner order). Other faces are unchanged; the
//! selection is kept. The bmesh operator has no file of its own in frap (it is three lines);
//! its type flags (`NORMALS_CALC`) are applied here.

use frap_bmesh::ElemTypes;
use frap_bmesh::types::OpTypeFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

/// `BMO_OPTYPE_FLAG_*` of `bmo_flip_quad_tessellation_def`.
const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::FlipQuadTessellation) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (_, _, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::selected_faces(bm);
        bm.bmo_exec(TYPE_FLAG, |bm| {
            let mut changed = false;
            for f in faces {
                if bm.face(f).len == 4 {
                    let next = bm.lp(bm.face(f).l_first).next;
                    bm.face_mut(f).l_first = next;
                    changed = true;
                }
            }
            if changed {
                bm.index.dirty |= ElemTypes::LOOP;
            }
        });
    }
    Ok(OpStatus::Finished)
}
