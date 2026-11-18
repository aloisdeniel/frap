// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.region_to_loop()`, tier T1 (`tests/golden/cases/edit/select/region_to_loop*.py`).
//! Port of `edbm_region_to_loop_exec`, `EDBM_selectmode_disable_multi` and
//! `EDBM_selectmode_disable` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode with selected faces, the selection is replaced by the boundary edges
//! of the selected regions (edges used by both selected and unselected faces, or by a single
//! selected face); the selection history is cleared. When an edge was selected, face select
//! mode is switched off on every mesh in Edit Mode (to edge mode when it was the only mode),
//! and `tool_settings.mesh_select_mode` follows, so the edge-only selection stays consistent.

use frap_bmesh::{EdgeId, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use rustc_hash::FxHashSet;

use super::select_more::common;

/// `EDBM_selectmode_disable_multi(C, disable, fallback)` on every mesh in Edit Mode, then
/// `scene->toolsettings->selectmode` (the last mesh that changed sets it, as in Blender).
pub(crate) fn selectmode_disable_multi(
    cx: &mut OpCtx,
    disable: SelectMode,
    fallback: SelectMode,
) -> Result<bool, OpError> {
    let mut changed = false;
    let mut last: Option<SelectMode> = None;
    for (_, me) in common::edit_meshes(cx) {
        let Some(cur) = common::read(cx, me, |bm| bm.select_mode) else {
            continue;
        };
        if !cur.intersects(disable) {
            continue;
        }
        let mode = if cur == disable {
            fallback
        } else {
            cur.difference(disable)
        };
        cx.bmesh_mut(me)?.selectmode_set(mode);
        last = Some(mode);
        changed = true;
    }
    if let Some(mode) = last {
        let scene = cx.ctx().win.scene;
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.tool_settings.mesh_select_mode = [
                mode.contains(SelectMode::VERT),
                mode.contains(SelectMode::EDGE),
                mode.contains(SelectMode::FACE),
            ];
        }
        log.context_changed = true;
    }
    Ok(changed)
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::RegionToLoop) -> OpResult {
    let mut changed = false;
    for (_, me) in common::edit_meshes(cx) {
        let any_face = common::read(cx, me, |bm| bm.selected_counts().2 != 0).unwrap_or(false);
        if !any_face {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let mut tag: FxHashSet<EdgeId> = FxHashSet::default();
        for f in bm.face_ids() {
            for l1 in bm.face_loops(f) {
                let e = bm.lp(l1).e;
                let faces = bm.edge_faces(e);
                let tot = faces.len();
                let totsel = faces
                    .into_iter()
                    .filter(|&x| common::selected(bm, x))
                    .count();
                if (tot != totsel && totsel > 0) || (totsel == 1 && tot == 1) {
                    tag.insert(e);
                }
            }
        }
        common::deselect_all(bm);
        for e in bm.edge_ids() {
            if tag.contains(&e) {
                bm.edge_select_set(e, true);
                changed = true;
            }
        }
    }
    if changed {
        // Out of face-only select mode, so an edge-only selection is not an inconsistent state.
        selectmode_disable_multi(cx, SelectMode::FACE, SelectMode::EDGE)?;
    }
    Ok(OpStatus::Finished)
}
