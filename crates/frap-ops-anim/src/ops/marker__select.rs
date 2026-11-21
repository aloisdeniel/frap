// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.select(wait_to_deselect_others=False, use_select_on_click=False, mouse_x=0,
//! mouse_y=0, extend=False, camera=False)`, tier T0 (WP 7.6). Port of `ed_marker_select_exec`,
//! `ed_marker_select` and `select_marker_camera_switch`
//! (blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! `mouse_x` is a region pixel of the Timeline (frap's is the factory one,
//! `marker__add::common`): the click hits the nearest marker only when it is within
//! `UI_ICON_SIZE` pixels, which is 0 headless, so only a marker exactly on the clicked frame
//! (pixel 0 is frame -9, pixel 1573 frame 260) is hit (verified on 5.2.4). A click that hits
//! selects that marker alone (the next marker on the same frame when one there is selected
//! already, which `wait_to_deselect_others` keeps); with `extend` it toggles it; a miss
//! deselects every marker. With `camera`, in Object Mode, the cameras bound to markers on the
//! hit frame become selected (deselecting every other object without `extend`) and active
//! without `extend`; outside Object Mode a WARNING says the camera is only selected in Object
//! Mode; a `camera` click that misses does nothing. Blender answers `PASS_THROUGH` (with
//! `FINISHED` or `RUNNING_MODAL`), which frap answers as `FINISHED`. Poll
//! `ed_markers_poll_markers_exist_visible`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::{BaseFlag, ContextMode};
use frap_spec_ops_anim::marker;

use super::marker__add::common;

/// `select_marker_camera_switch(C, true, extend, markers, cfra)` in Object Mode.
fn select_cameras(cx: &mut OpCtx, extend: bool, cfra: i32) {
    let (scene, layer, sel, cameras, selectable) = {
        let c = cx.ctx();
        let ms = common::markers(&c);
        let sel = ms
            .iter()
            .find(|m| m.frame == cfra && m.camera.is_some())
            .is_some_and(|m| m.select);
        let cameras: Vec<_> = ms
            .iter()
            .filter(|m| m.frame == cfra)
            .filter_map(|m| m.camera)
            .collect();
        let vis = c.base_visibility();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let selectable: Vec<_> = cameras
            .iter()
            .map(|ob| vis.get(ob).is_some_and(|v| v.selectable))
            .collect();
        (c.win.scene, layer, sel, cameras, selectable)
    };
    let (main, log) = cx.main_mut();
    let Some(s) = main.scenes.get_mut(scene, log) else {
        return;
    };
    let Some(vl) = s.view_layers.get_mut(layer) else {
        return;
    };
    if !extend {
        // `BKE_view_layer_base_deselect_all`
        for b in &mut vl.bases {
            b.flag.remove(BaseFlag::SELECTED);
        }
    }
    for (ob, selectable) in cameras.iter().zip(selectable) {
        let Some(b) = vl.base_mut(*ob) else { continue };
        // `base_select(base, BA_SELECT / BA_DESELECT)`: only a selectable base is selected.
        if !sel {
            b.flag.remove(BaseFlag::SELECTED);
        } else if selectable {
            b.flag.insert(BaseFlag::SELECTED);
        }
        if !extend {
            vl.active = Some(*ob);
        }
    }
    log.context_changed = true;
}

#[operator(
    poll = common::EXIST,
    tier = T0,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::Select) -> OpResult {
    let mx = p.mouse_x.value as f32;
    let (extend, camera, wait) = (
        p.extend.value,
        p.camera.value,
        p.wait_to_deselect_others.value,
    );
    let (marker_camera, over, nearest_frame, cfra, is_selected, object_mode) = {
        let c = cx.ctx();
        let ms = common::markers(&c);
        let frame = common::region_to_view_x(mx);
        let marker_camera = common::nearest(ms, frame).is_some_and(|i| ms[i].camera.is_some());
        let over = common::over_marker(ms, mx);
        (
            marker_camera,
            over.is_some(),
            over.map(|i| ms[i].frame),
            common::nearest_time(ms, frame),
            over.is_some_and(|i| ms[i].select),
            c.mode() == ContextMode::Object,
        )
    };
    if camera && marker_camera && !object_mode {
        cx.report(
            ReportType::Warning,
            "Automatic selection of the camera bound to this marker is only supported in object \
             mode",
        );
    }
    if camera && !over {
        // `PASS_THROUGH`
        return Ok(OpStatus::Finished);
    }
    // `ed_marker_select(C, mval, extend, deselect_all = true, camera, wait_to_deselect_others)`
    // `sel_op`: `SET` without `extend`, else `SUB` on a selected marker and `ADD` otherwise; a
    // `SET` that hits, and any miss, become `ADD` after deselecting every marker (unless
    // `wait_to_deselect_others` delays that: Blender's `RUNNING_MODAL`).
    let found = over;
    let mut sub = extend && is_selected;
    if (!extend && found) || !found {
        sub = false;
        if !(wait && (!found || is_selected)) {
            common::with_markers(cx, |ms| common::deselect_all(ms));
        }
    }
    if found {
        common::with_markers(cx, |ms| {
            // Selection cycling: start after the first selected marker on the frame.
            let start = ms
                .iter()
                .position(|m| m.frame == cfra && m.select)
                .map_or(0, |i| if i + 1 < ms.len() { i + 1 } else { 0 });
            let n = ms.len();
            if let Some(i) = (0..n)
                .map(|k| (start + k) % n)
                .find(|&i| ms[i].frame == cfra)
            {
                ms[i].select = !sub;
            }
        });
    }
    if found
        && camera
        && object_mode
        && let Some(f) = nearest_frame
    {
        select_cameras(cx, extend, f);
    }
    Ok(OpStatus::Finished)
}
