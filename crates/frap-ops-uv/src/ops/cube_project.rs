// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.cube_project(cube_size=1.0, correct_aspect=True, clip_to_bounds=False,
//! scale_to_bounds=False)`, tier T1 (`tests/golden/cases/uv/project/cube_*.py`). Port of
//! `cube_project_exec`, `uvedit_unwrap_cube_project` and `uv_map_clip_correct`
//! (`editors/uvedit/uvedit_unwrap_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvmap`. For every mesh in Edit Mode with a selected face (a UV map is
//! added when missing), each selected face is projected on the axis plane its normal faces
//! most: `uv = 0.5 + (co - centre) / cube_size`, the centre being the transform pivot
//! (`tool_settings.transform_pivot_point`, of the virtual 3D View, D-10). Without
//! `cube_size` it is the largest dimension of the selected faces' bounds — computed for the
//! first object and then, as in Blender, reused for the others. Then per-face aspect
//! correction (`correct_aspect`, each face's material image), `clip_to_bounds` or
//! `scale_to_bounds`. Always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::CubeProject) -> OpResult {
    uvedit::check_uvmap(cx)?;
    let ts = UvTs::of(&cx.ctx());
    let objects = uvedit::edit_objects(cx);
    let mut size_is_set = p.cube_size.is_set;
    let mut size_init = p.cube_size.value;
    for (ob_index, &(ob, me)) in objects.iter().enumerate() {
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
        let (center, bounds) = uvedit::transform_center(&cx.ctx(), ob, &bm);
        let mut cube_size = size_init;
        if !size_is_set {
            let d = [
                bounds[1][0] - bounds[0][0],
                bounds[1][1] - bounds[0][1],
                bounds[1][2] - bounds[0][2],
            ];
            cube_size = d[0].max(d[1]).max(d[2]);
            if ob_index == 0 {
                // "This doesn't fit well with, multiple objects": the property is set now.
                size_init = cube_size;
                size_is_set = true;
            }
        }
        for f in uvedit::faces(&bm) {
            if !bm.face(f).flag.contains(frap_bmesh::ElemFlag::SELECT) {
                continue;
            }
            let loops = bm.face_loops(f);
            let cos: Vec<[f32; 3]> = loops.iter().map(|&l| uvedit::loop_co(&bm, l)).collect();
            let mut out = vec![[0.0f32; 2]; loops.len()];
            frap_uv::project::cube_project_face(bm.face(f).no, &cos, center, cube_size, &mut out);
            for (l, uv) in loops.iter().zip(out) {
                o.uvs[l.slot()] = uv;
            }
        }
        let base = uvedit::path_base(cx);
        let mut one = [o];
        uvedit::clip_correct(
            cx.main(),
            &base,
            &mut one,
            ts,
            uvedit::clip_props(
                p.correct_aspect.value,
                p.clip_to_bounds.value,
                p.scale_to_bounds.value,
            ),
            true,
            false,
        );
        let [o] = one;
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}
