// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.smooth_normals(factor=0.5)`, tier T1
//! (`tests/golden/cases/edit/normals_m8/smooth_normals.py`). Port of
//! `edbm_smooth_normals_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data), each selected custom normal (by select mode and
//! selection history, whole smooth fans) is mixed with the sum of the custom normals of every
//! corner of every face around its vertex (a face using the vertex twice counts twice):
//! `normalize((1 - factor) * own + factor * normalize(sum))`. A normal whose sum or result is
//! shorter than 1e-4 is left unchanged. The `custom_normal` layer is created when missing. The
//! operator always finishes.

use frap_bmesh::BMesh;
use frap_geom::normals::{V3, normalize_v3, scale};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{clnor, edit_meshes_where};

fn smooth_normals(bm: &mut BMesh, factor: f32) {
    let sp = clnor::update(bm);
    let arr = clnor::editdata_array(bm, &sp, false, clnor::Mark::Indiv);
    let mut smooth: Vec<V3> = vec![[0.0; 3]; arr.data.len()];
    for (i, d) in arr.data.iter().enumerate() {
        let v = bm.lp(d.loop_).v;
        for f in bm.vert_faces(v) {
            for l_other in bm.face_loops(f) {
                let n = sp.decode(bm, l_other);
                let s = &mut smooth[i];
                s[0] += n[0];
                s[1] += n[1];
                s[2] += n[2];
            }
        }
    }
    for (d, s) in arr.data.iter().zip(smooth) {
        let (s, len) = normalize_v3(s);
        if len < clnor::CLNORS_VALID_VEC_LEN {
            continue;
        }
        let cur = scale(sp.decode(bm, d.loop_), 1.0 - factor);
        let s = scale(s, factor);
        let (n, len) = normalize_v3([cur[0] + s[0], cur[1] + s[1], cur[2] + s[2]]);
        if len < clnor::CLNORS_VALID_VEC_LEN {
            continue;
        }
        let v = sp.encode(d.loop_, n);
        clnor::set(bm, d.loop_, v);
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SmoothNormals) -> OpResult {
    let factor = p.factor.value;
    for me in edit_meshes_where(cx, |_| true) {
        smooth_normals(cx.bmesh_mut(me)?, factor);
    }
    Ok(OpStatus::Finished)
}
