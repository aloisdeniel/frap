// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_repeat(steps=10, offset=(0, 0, 0), scale_offset=1.0)`, tier T1
//! (`tests/golden/cases/edit/extrude/repeat*.py`). Port of `edbm_extrude_repeat_exec`
//! (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! `steps` times: the selection (vertices, edges and faces, whatever the select mode) is
//! extruded as a region (no mirror clipping) and its vertices are moved by `offset ·
//! scale_offset`, given in world space and converted to each object's space (the inverse of
//! the object's 3 × 3 world matrix). In 5.x `offset` is a vector; when it is not set, the
//! direction is the view's: Blender normalises the third column of the 3D View's inverse
//! perspective matrix, which for frap's virtual perspective view (D-10) is the direction from
//! the viewpoint towards the world origin.

use frap_bmesh::{BMesh, ElemFlag, VertId};
use frap_math::glam::{DMat4, Mat3, Vec3};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

/// `normalize_v3_v3(offset, rv3d->persinv[2])`: for a perspective window matrix the third
/// column of `inverse(winmat · viewmat)` is the viewpoint's position divided by the negative
/// `winmat[3][2]`, so the offset points from the viewpoint towards the origin.
fn view_offset(cx: &OpCtx) -> [f32; 3] {
    let vm: DMat4 = cx.ctx().win.view3d.view_matrix.as_dmat4();
    let cam = vm.inverse().w_axis.truncate();
    let len = cam.length();
    if len > 0.0 {
        (-cam / len).as_vec3().to_array()
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// `bmesh.ops.translate(verts=<selected>, vec=off)`: `mul_m4_v3` with a translation matrix.
fn translate_selected(bm: &mut BMesh, off: [f32; 3]) {
    let verts: Vec<VertId> = bm
        .verts
        .iter()
        .filter(|(_, v)| v.flag.contains(ElemFlag::SELECT) && !v.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| VertId(i))
        .collect();
    for v in verts {
        let c = bm.vert(v).co;
        bm.vert_mut(v).co = [
            c[0] * 1.0 + c[1] * 0.0 + c[2] * 0.0 + off[0],
            c[0] * 0.0 + c[1] * 1.0 + c[2] * 0.0 + off[1],
            c[0] * 0.0 + c[1] * 0.0 + c[2] * 1.0 + off[2],
        ];
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::ExtrudeRepeat) -> OpResult {
    let steps = p.steps.value.max(0);
    let scale_offset = p.scale_offset.value;
    let mut offset = if p.offset.is_set {
        p.offset.value
    } else {
        view_offset(cx)
    };
    for c in &mut offset {
        *c *= scale_offset;
    }
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        // `copy_m3_m4`, `invert_m3`, `mul_v3_m3v3`.
        let tmat = Mat3::from_mat4(world_matrix(&cx.main().objects, ob));
        let tmat = if tmat.determinant() != 0.0 {
            tmat.inverse()
        } else {
            tmat
        };
        let off = (tmat * Vec3::from(offset)).to_array();
        let bm = cx.bmesh_mut(me)?;
        for _ in 0..steps {
            frap_math::budget::check();
            common::extrude_ex(bm, common::HType::ALL, false, false, Vec::new(), true)?;
            // The translate op runs top-level: it updates the normals.
            translate_selected(bm, off);
            bm.normals_update();
        }
        bm.normals_update();
    }
    Ok(OpStatus::Finished)
}
