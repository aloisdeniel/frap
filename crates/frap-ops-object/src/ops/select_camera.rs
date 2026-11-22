// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_camera(extend=False)`, tier T0
//! (`tests/golden/cases/object/select_m8/camera*.py`). Ported from
//! blender/scripts/startup/bl_operators/object.py (`SelectCamera`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//!
//! - no poll; the camera is `scene.camera` (frap's virtual 3D View has no local camera);
//!   none → WARNING `No camera found`, `CANCELLED`; not in the scene's objects → WARNING
//!   `Active camera is not in this scene`, `CANCELLED`;
//! - otherwise `extend=False` first runs `object.select_all(action='DESELECT')` (whose poll
//!   fails outside Object Mode: `RuntimeError`), then the camera becomes the active object
//!   (`RuntimeError: Error: ViewLayer 'ViewLayer' does not contain object 'Camera'` when it has
//!   no base, e.g. in an excluded collection) and is selected (`select_set(True)`: not when it
//!   is unselectable).

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::ObjectMode;
use frap_spec_ops_object::object;

use super::move_to_collection::common::{all_scene_collections, has_object};
use super::select_grouped::bases::Bases;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectCamera) -> OpResult {
    let (camera, in_scene, object_mode, vl_name) = {
        let c = cx.ctx();
        let scene = c.win.scene;
        let camera = c.scene().camera;
        let in_scene = camera.is_some_and(|cam| {
            all_scene_collections(c.main, scene)
                .into_iter()
                .any(|col| has_object(c.main, scene, col, cam))
        });
        let object_mode = poll::active(&c).is_none_or(|(_, o)| o.mode == ObjectMode::Object);
        (camera, in_scene, object_mode, c.view_layer().name.clone())
    };
    let Some(camera) = camera else {
        cx.report(ReportType::Warning, "No camera found");
        return Ok(OpStatus::Cancelled);
    };
    if !in_scene {
        cx.report(ReportType::Warning, "Active camera is not in this scene");
        return Ok(OpStatus::Cancelled);
    }
    let mut b = Bases::load(cx);
    if !p.extend.value {
        if !object_mode {
            return Err(OpError::Runtime(
                "Operator bpy.ops.object.select_all.poll() failed, context is incorrect".into(),
            ));
        }
        b.deselect_all();
    }
    let Some(i) = b.find(camera) else {
        let name = cx.main().objects.name_of(camera).unwrap_or_default().to_owned();
        return Err(OpError::Runtime(format!(
            "ViewLayer '{vl_name}' does not contain object '{name}'"
        )));
    };
    b.active = Some(camera);
    b.select(i, true);
    b.commit(cx);
    Ok(OpStatus::Finished)
}
