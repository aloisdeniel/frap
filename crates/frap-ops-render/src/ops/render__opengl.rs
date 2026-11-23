// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.render.opengl(animation=False, render_keyed_only=False, sequencer=False,
//! write_still=False, view_context=True)` ("Viewport Render"), tier T4
//! (`tests/golden/cases/render/opengl/*.py` against Blender's `-b` Workbench render). Port of
//! `screen_opengl_render_init` / `screen_opengl_render_exec` (`editors/render/
//! render_opengl.cc`, Blender 5.2.4, GPL-2.0-or-later), **headless-extended** (D-160).
//!
//! In `blender -b` the operator always fails (`RuntimeError: Error: Cannot use OpenGL render
//! in background mode (no opengl context)`). frap has no viewport to draw, so it renders what
//! a viewport render shows through the scene camera: the Workbench engine (whatever
//! `scene.render.engine` says), the scene's display settings (`scene.display.shading`, as with
//! `view_context=False`), no overlays, from the scene camera. Everything else is
//! `render.render`'s (same frames, outputs, `Render Result` image, colour management and
//! `result` fields), and so is what an agent gets back.
//!
//! Checks, in Blender's order: `write_still` with a video format: `Cannot write a single file
//! with an animation format selected`; no scene camera: `Scene has no camera` (Blender only
//! checks the camera without a 3D View; frap always renders through it). `sequencer=True` and
//! `render_keyed_only=True` (with `animation`) answer `OP_NOT_IMPLEMENTED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::Prop;
use frap_spec_ops_render::render;

use super::render__render::{WORKBENCH, run};

fn prop<T>(value: T, is_set: bool) -> Prop<T> {
    Prop { value, is_set }
}

#[operator(
    poll = poll::ALWAYS,
    tier = T4,
    support = Partial(
        "headless-extended (D-160): renders the scene camera with Workbench and the scene \
         display settings; sequencer=True and render_keyed_only=True are not supported"
    ),
    notes = [
        "needs a GPU: GPU_UNAVAILABLE without an adapter or in a build without the gpu feature",
        "frames, output paths, the Render Result image and result fields are render.render's",
    ]
)]
fn exec(cx: &mut OpCtx, p: render::Opengl) -> OpResult {
    if p.sequencer.value {
        return Err(OpError::NotImplemented(
            "frap: render.opengl(sequencer=True) is not supported: frap has no sequencer".into(),
        ));
    }
    if p.animation.value && p.render_keyed_only.value {
        return Err(OpError::NotImplemented(
            "frap: render.opengl(animation=True, render_keyed_only=True) is not supported yet"
                .into(),
        ));
    }
    {
        let c = cx.ctx();
        let scene = c.scene();
        let write_still = p.write_still.value && !p.animation.value;
        if write_still && scene.render.image_settings.media_type == "VIDEO" {
            return Err(OpError::Runtime(
                "Cannot write a single file with an animation format selected".into(),
            ));
        }
        let marker_camera =
            frap_scene::id::timeline_marker::camera_switch_find(c.main, scene, scene.frame_current);
        if scene.camera.is_none() && marker_camera.is_none() {
            return Err(OpError::Runtime("Scene has no camera".into()));
        }
    }
    // `oglrender->write_still = is_write_still && !is_animation`.
    let rp = render::Render {
        animation: prop(p.animation.value, p.animation.is_set),
        write_still: prop(
            p.write_still.value && !p.animation.value,
            p.write_still.is_set,
        ),
        use_viewport: prop(false, false),
        use_sequencer_scene: prop(false, false),
        layer: prop(Default::default(), false),
        scene: prop(Default::default(), false),
        frame_start: prop(0, false),
        frame_end: prop(0, false),
    };
    run(cx, &rp, Some(WORKBENCH))
}
