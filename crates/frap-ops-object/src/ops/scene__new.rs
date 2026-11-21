// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.new(type='NEW')`, tier T0 (`tests/golden/cases/scenes/*.py`; M3 WP 3.10).
//! Port of `scene_new_exec`, `ED_scene_add` and `scene_add` (`editors/scene/scene_edit.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): the new scene (`frap_scene::scenes::copy_scene`: `NEW`
//! a default scene named `Scene`, `EMPTY` the settings only, `LINK_COPY` shared collections
//! and objects, `FULL_COPY` everything duplicated) becomes the window's scene
//! (`WM_window_set_active_scene`). `FULL_COPY` first writes the meshes in Edit Mode back to
//! their data (`ED_editors_flush_edits`), so the copies get the edits; the objects stay in Edit
//! Mode in the old scene and the copies are in Object Mode (verified on 5.2.4). Poll
//! `WM_operator_winactive` (frap always has its window).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_scene::scenes::{SceneCopy, copy_scene};
use frap_spec_ops_object::scene::{self, new::Type};

/// `ED_editors_flush_edits`: the meshes in Edit Mode get their edit-mode state (the edit mode
/// itself goes on).
fn flush_edits(cx: &mut OpCtx) {
    let views: Vec<(IdRef<MeshId>, std::sync::Arc<frap_mesh::Mesh>)> = {
        let (host, _) = cx.split_eval();
        let edit = host.edit();
        host.main()
            .meshes
            .iter()
            .filter(|(r, _)| edit.meshes.contains_key(&r.key()))
            .map(|(r, _)| (r, host.mesh_view(r)))
            .collect()
    };
    for (me, view) in views {
        if let Ok(m) = cx.mesh_mut(me) {
            *m = (*view).clone();
        }
    }
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    notes = ["the new scene becomes bpy.context.window.scene; result.scene is the new scene"]
)]
fn exec(cx: &mut OpCtx, p: scene::New) -> OpResult {
    let ty = match p.r#type.value {
        Type::New => SceneCopy::New,
        Type::Empty => SceneCopy::Empty,
        Type::LinkCopy => SceneCopy::LinkCopy,
        Type::FullCopy => SceneCopy::FullCopy,
    };
    if ty == SceneCopy::FullCopy {
        flush_edits(cx);
    }
    let src = cx.ctx().win.scene;
    let new = {
        let (main, log) = cx.main_mut();
        let new = copy_scene(main, src, ty, log);
        log.context_changed = true;
        new
    };
    let mut win = cx.ctx().win.clone();
    frap_scene::window::set_scene(cx.main(), &mut win, new).map_err(OpError::Runtime)?;
    let old = cx.ctx().win.clone();
    cx.main_mut().0.shown_depsgraphs.remember(&old);
    *cx.win_mut() = win;
    // The new scene's depsgraph is evaluated at its frame by the closing boundary
    // (`ED_scene_change_update`; verified on 5.2.4 with `LINK_COPY`).
    cx.mark_time_dirty();
    cx.result("scene", new.any());
    Ok(OpStatus::Finished)
}
