// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.object_as_camera()`, tier T0 (`tests/golden/cases/gltf/lights_cameras/
//! object_as_camera.py`). Port of `view3d_setobjectascamera_exec`
//! (`editors/space_view3d/view3d_view.cc`, Blender 5.2.4, GPL-2.0-or-later) with frap's
//! virtual 3D View (D-10): the view is scene-locked, so the active object (of any type, as in
//! Blender) becomes `scene.camera`; without an active object nothing changes and the operator
//! still finishes. The view switching to camera perspective and the smooth-view animation
//! have no headless counterpart. Poll `ED_operator_rv3d_user_region_poll`: the virtual 3D View
//! always passes it.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: view3d::ObjectAsCamera) -> OpResult {
    let (scene, ob) = {
        let c = cx.ctx();
        (c.win.scene, c.object())
    };
    let Some(ob) = ob else {
        return Ok(OpStatus::Finished);
    };
    let (main, log) = cx.main_mut();
    if main.scenes.get(scene).is_some_and(|s| s.camera != Some(ob))
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        s.camera = Some(ob);
    }
    Ok(OpStatus::Finished)
}
