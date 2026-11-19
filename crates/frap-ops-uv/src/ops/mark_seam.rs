// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.mark_seam(clear=False)`, tier T1 (`tests/golden/cases/uv/seams/mark_*.py`).
//! Port of `uv_mark_seam_exec` (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit` (a mesh in Edit Mode with faces and a UV map). On every mesh in
//! Edit Mode with a UV map (with UV sync selection: one with a selected edge), the edges of the
//! UV-selected edges of visible faces are marked as seams (`clear`: unmarked). With sync
//! selection on (the default) the UV edge selection follows the mesh selection mode: in
//! face mode an edge counts when a face using it at the same UV location is selected, in edge
//! mode when the edge is selected, in vertex mode when both its vertices are. Live unwrap
//! (`use_edge_path_live_unwrap`, off in the factory settings) is not run. Always finishes.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::MarkSeam) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let flag_set = !p.clear.value;
    let ts = UvTs::of(&cx.ctx());
    let objects = uvedit::edit_objects_with_uvs(cx);
    for o in UvOb::gather(cx, &objects) {
        let bm = o.bm.clone();
        if ts.sync && bm.selected_counts().1 == 0 {
            continue;
        }
        let sel = o.sel(ts);
        let mut edges = Vec::new();
        for f in uvedit::faces(&bm) {
            if !sel.face_visible(f) {
                continue;
            }
            for l in bm.face_loops(f) {
                if sel.edge_select(l) {
                    edges.push(bm.lp(l).e);
                }
            }
        }
        if edges.is_empty() {
            continue;
        }
        let bm = cx.bmesh_mut(o.me)?;
        for e in edges {
            bm.edge_mut(e).flag.set(ElemFlag::SEAM, flag_set);
        }
    }
    Ok(OpStatus::Finished)
}
