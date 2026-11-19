// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.reset()`, tier T1 (`tests/golden/cases/uv/reset/*.py`). Port of `reset_exec`
//! (`editors/uvedit/uvedit_unwrap_ops.cc`) and `ED_mesh_uv_loop_reset` / `reset_uv_map`
//! (`editors/mesh/mesh_data.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvmap`. For every mesh in Edit Mode with a selected face (a UV map is
//! added when missing), each selected face of the active UV map gets the default layout from
//! its first corner: triangles `(0,0) (1,0) (1,1)`, quads `(0,0) (1,0) (1,1) (0,1)`, n-gons
//! on a circle (`(0.5 sin t + 0.5, 0.5 cos t + 0.5)`, `t` stepping by 2π/n from 0). Always
//! finishes.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UvOb};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: uv::Reset) -> OpResult {
    uvedit::check_uvmap(cx)?;
    for (ob, me) in uvedit::edit_objects(cx) {
        if !uvedit::edit_bm(cx, me).is_some_and(|bm| uvedit::has_face_selection(&bm)) {
            continue;
        }
        if !uvedit::ensure_uvs(cx, me)? {
            continue;
        }
        let Some(mut o) = UvOb::gather(cx, &[(ob, me)]).pop() else {
            continue;
        };
        let bm = o.bm.clone();
        for f in uvedit::faces(&bm) {
            if !bm.face(f).flag.contains(ElemFlag::SELECT) {
                continue;
            }
            let loops = bm.face_loops(f);
            for (l, uv) in loops.iter().zip(uvedit::reset_face_uvs(loops.len())) {
                o.uvs[l.slot()] = uv;
            }
        }
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::uvedit::reset_face_uvs;

    #[test]
    fn default_layouts() {
        assert_eq!(reset_face_uvs(3), vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]);
        assert_eq!(reset_face_uvs(4)[3], [0.0, 1.0]);
        let ngon = reset_face_uvs(6);
        assert_eq!(ngon[0], [0.5, 1.0]);
        assert!((ngon[3][1] - 0.0).abs() < 1e-6);
        assert_eq!(reset_face_uvs(2), vec![[0.0, 0.0]; 2]);
    }
}
