// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.set_sharpness_by_angle(angle=0.523599, extend=False)`, tier T1
//! (`tests/golden/cases/edit/normals/sharpness_*.py`). Port of `set_sharpness_by_angle_exec`
//! (`editors/mesh/editmesh_set_sharpness_by_angle.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data), each selected edge with exactly two faces becomes
//! sharp when the cosine of the angle between the face normals is at most `cos(angle)`, smooth
//! otherwise; `extend` leaves already-sharp edges sharp. Edges with one face or more than two
//! are left alone. When some edge changed, the custom normal spaces are updated
//! (`BKE_editmesh_lnorspace_update`), which **adds a `custom_normal` layer** (all zero:
//! automatic normals) when the mesh has none, as Blender does ([MESH §0.12]; the mesh's
//! `normals_domain` becomes `CORNER`). The operator always finishes.

use frap_bmesh::ElemFlag;
use frap_geom::normals::dot;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{clnor, edit_meshes_where};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SetSharpnessByAngle) -> OpResult {
    let angle_limit_cos = p.angle.value.cos();
    let extend = p.extend.value;
    for me in edit_meshes_where(cx, |_| true) {
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        for e in bm.edge_ids() {
            let flag = bm.edge(e).flag;
            if !flag.contains(ElemFlag::SELECT) {
                continue;
            }
            let prev_sharp = !flag.contains(ElemFlag::SMOOTH);
            if extend && prev_sharp {
                continue;
            }
            // BM_edge_loop_pair: exactly two faces.
            let Some(l1) = bm.edge(e).l else {
                continue;
            };
            let l2 = bm.lp(l1).radial_next;
            if l2 == l1 || bm.lp(l2).radial_next != l1 {
                continue;
            }
            let n1 = bm.face_calc_normal(bm.lp(l1).f);
            let n2 = bm.face_calc_normal(bm.lp(l2).f);
            let sharp = dot(n1, n2) <= angle_limit_cos;
            bm.edge_mut(e).flag.set(ElemFlag::SMOOTH, !sharp);
            changed = changed || sharp != prev_sharp;
        }
        if changed {
            clnor::update(bm);
        }
    }
    Ok(OpStatus::Finished)
}
