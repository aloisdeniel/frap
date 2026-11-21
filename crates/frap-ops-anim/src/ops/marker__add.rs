// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.add()`, tier T0 (WP 7.6). Port of `ed_marker_add_exec`
//! (blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: a marker `F_<frame>` (two digits at least: `F_05`) at the scene's current
//! frame, selected, appended, every other marker deselected; a marker already on that frame
//! cancels (no report). The camera does not switch.
//!
//! [`common`] holds what the 11 `marker.*` operators share. Blender runs them in an animation
//! editor that shows markers (Timeline, Dope Sheet, Graph Editor, NLA, Sequencer:
//! `operator_markers_region_active`), so `blender -b` without one fails their poll; frap has no
//! editors and runs them as Blender does in the factory Layout screen's Timeline
//! (headless-extended, deviations §1): the context scene's markers
//! (`ED_context_get_markers` of a Timeline), and for the operators that take region
//! coordinates (`marker.select`'s `mouse_x`, `marker.select_box`'s `xmin`/`xmax`) that
//! Timeline's `View2D` in background mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::TimelineMarker;
use frap_spec_ops_anim::marker;

/// Shared by the `marker.*` operators.
pub(crate) mod common {
    use frap_op::prelude::*;
    use frap_op::{Poll, PollFail};
    use frap_rna::{IdRef, SceneId};
    use frap_scene::Context;
    use frap_scene::id::TimelineMarker;

    // The factory Layout screen's Timeline in `blender -b --factory-startup` (verified on
    // 5.2.4): its WINDOW region is 1574 x 48 pixels, `view2d.region_to_view(0, 0)` reads frame
    // -9.0 and `(1573, 0)` frame 260.0 (`View2D.cur` x -9..260 over `mask` x 0..1573). Without a
    // window `UI_SCALE_FAC` is 0, so `UI_ICON_SIZE` (16 * `UI_SCALE_FAC`) is 0: a click selects a
    // marker only on its exact frame (verified: `mouse_x=0` selects a marker at -9, `mouse_x=1`
    // none), and `UI_MARKERS_MINY` (2 * `HEADERY` * `UI_SCALE_FAC`) never hides the markers.
    const CUR_XMIN: f32 = -9.0;
    const CUR_XMAX: f32 = 260.0;
    const MASK_XMIN: i32 = 0;
    const MASK_XMAX: i32 = 1573;
    const UI_ICON_SIZE: f32 = 0.0;

    /// `UI_view2d_region_to_view_x` of the Timeline (f32, as Blender computes it).
    pub(crate) fn region_to_view_x(x: f32) -> f32 {
        CUR_XMIN + (CUR_XMAX - CUR_XMIN) * (x - MASK_XMIN as f32) / (MASK_XMAX - MASK_XMIN) as f32
    }

    /// `UI_view2d_scale_get_x`: pixels per frame.
    fn scale_x() -> f32 {
        (MASK_XMAX - MASK_XMIN) as f32 / (CUR_XMAX - CUR_XMIN)
    }

    /// `ED_markers_find_nearest_marker`: the first marker at the smallest distance to `frame`.
    pub(crate) fn nearest(markers: &[TimelineMarker], frame: f32) -> Option<usize> {
        let first = markers.first()?;
        let mut best = 0;
        let mut min_dist = (first.frame as f32 - frame).abs();
        for (i, m) in markers.iter().enumerate().skip(1) {
            let d = (m.frame as f32 - frame).abs();
            if d < min_dist {
                min_dist = d;
                best = i;
            }
        }
        Some(best)
    }

    /// `ED_markers_find_nearest_marker_time`
    pub(crate) fn nearest_time(markers: &[TimelineMarker], x: f32) -> i32 {
        match nearest(markers, x) {
            Some(i) => markers[i].frame,
            None => x.round() as i32,
        }
    }

    /// `region_position_is_over_marker`: the nearest marker when it is within `UI_ICON_SIZE`
    /// pixels of `region_x`.
    pub(crate) fn over_marker(markers: &[TimelineMarker], region_x: f32) -> Option<usize> {
        let frame = region_to_view_x(region_x);
        let i = nearest(markers, frame)?;
        let dist = scale_x() * (markers[i].frame as f32 - frame).abs();
        (dist <= UI_ICON_SIZE).then_some(i)
    }

    /// The context scene (`CTX_data_scene`), whose markers a Timeline shows.
    pub(crate) fn scene(cx: &OpCtx) -> IdRef<SceneId> {
        cx.ctx().win.scene
    }

    /// The context scene's markers.
    pub(crate) fn markers<'a>(c: &Context<'a>) -> &'a [TimelineMarker] {
        &c.scene().timeline_markers
    }

    /// Runs `f` on the context scene's markers, mutably.
    pub(crate) fn with_markers<R>(
        cx: &mut OpCtx,
        f: impl FnOnce(&mut Vec<TimelineMarker>) -> R,
    ) -> Option<R> {
        let scene = scene(cx);
        let (main, log) = cx.main_mut();
        main.scenes
            .get_mut(scene, log)
            .map(|s| f(&mut s.timeline_markers))
    }

    /// `BKE_scene_camera_switch_update` of the context scene.
    pub(crate) fn camera_switch(cx: &mut OpCtx) -> bool {
        let scene = scene(cx);
        let (main, log) = cx.main_mut();
        frap_scene::id::timeline_marker::camera_switch_update(main, log, scene)
    }

    /// `ED_markers_deselect_all` (`SEL_TOGGLE` already resolved by the caller when needed).
    pub(crate) fn deselect_all(markers: &mut [TimelineMarker]) {
        for m in markers {
            m.select = false;
        }
    }

    /// The hint of a failed marker poll.
    const HINT_ADD: &str = "add a marker first: bpy.context.scene.timeline_markers.new('M', \
                            frame=bpy.context.scene.frame_current), or bpy.ops.marker.add()";
    const HINT_SELECT: &str = "select a marker first: \
                               bpy.context.scene.timeline_markers[0].select = True, or \
                               bpy.ops.marker.select_all(action='SELECT')";
    const HINT_UNLOCK: &str = "bpy.context.scene.tool_settings.lock_markers = False";

    fn poll_fail(message: Option<String>, hint: &str) -> PollFail {
        PollFail {
            message,
            unmet: Default::default(),
            hint: Some(hint.into()),
        }
    }

    /// `ed_markers_poll_selected_markers`: a selected marker (`No markers are selected`).
    fn selected(c: &Context, idname: &str) -> Result<(), PollFail> {
        if markers(c).iter().any(|m| m.select) {
            Ok(())
        } else {
            Err(poll_fail(
                Some(format!(
                    "Operator bpy.ops.{idname}.poll() No markers are selected"
                )),
                HINT_SELECT,
            ))
        }
    }

    /// `ed_markers_poll_selected_no_locked_markers`: `Markers are locked`, then a selected
    /// marker.
    fn selected_unlocked(c: &Context, idname: &str) -> Result<(), PollFail> {
        if c.tool_settings().lock_markers {
            return Err(poll_fail(
                Some(format!(
                    "Operator bpy.ops.{idname}.poll() Markers are locked"
                )),
                HINT_UNLOCK,
            ));
        }
        selected(c, idname)
    }

    /// `ed_markers_poll_markers_exist` (and `_visible`: the markers row of the Timeline is
    /// never too small headless): markers not locked and at least one marker, with the generic
    /// message.
    pub(crate) fn exist(c: &Context) -> Result<(), PollFail> {
        if c.tool_settings().lock_markers {
            return Err(poll_fail(None, HINT_UNLOCK));
        }
        if markers(c).is_empty() {
            return Err(poll_fail(None, HINT_ADD));
        }
        Ok(())
    }

    /// `ed_markers_poll_markers_exist`
    pub(crate) const EXIST: Poll = Poll {
        requires: &[],
        check: exist,
    };

    macro_rules! polls {
        ($($name:ident, $f:ident, $idname:literal;)*) => {$(
            /// A marker poll with the operator's idname in its message.
            pub(crate) const $name: Poll = Poll {
                requires: &[],
                check: |c| $f(c, $idname),
            };
        )*};
    }

    polls! {
        SELECTED_UNLOCKED_DELETE, selected_unlocked, "marker.delete";
        SELECTED_UNLOCKED_DUPLICATE, selected_unlocked, "marker.duplicate";
        SELECTED_UNLOCKED_MOVE, selected_unlocked, "marker.move";
        SELECTED_UNLOCKED_RENAME, selected_unlocked, "marker.rename";
        SELECTED_MAKE_LINKS, selected, "marker.make_links_scene";
    }

    /// The poll notes `--describe` shows for the headless-extended context.
    pub(crate) const CONTEXT_NOTE: &str = "headless-extended: runs as in the factory Layout screen's Timeline (Blender needs an animation editor showing markers): on the context scene's timeline_markers, with Blender's marker polls (markers exist or are selected, scene.tool_settings.lock_markers off)";
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, _p: marker::Add) -> OpResult {
    let frame = cx.ctx().scene().frame_current;
    if common::markers(&cx.ctx()).iter().any(|m| m.frame == frame) {
        return Ok(OpStatus::Cancelled);
    }
    common::with_markers(cx, |ms| {
        common::deselect_all(ms);
        ms.push(TimelineMarker::new(
            &TimelineMarker::frame_name(frame),
            frame,
        ));
    });
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::common::*;
    use frap_scene::id::TimelineMarker;

    fn at(frames: &[i32]) -> Vec<TimelineMarker> {
        frames
            .iter()
            .map(|f| TimelineMarker::new("M", *f))
            .collect()
    }

    /// The factory Timeline's View2D (values verified on 5.2.4).
    #[test]
    fn timeline_view2d_matches_blender() {
        assert_eq!(region_to_view_x(0.0), -9.0);
        assert_eq!(region_to_view_x(1573.0), 260.0);
        assert_eq!(region_to_view_x(1.0), -8.828_989);
        assert_eq!(region_to_view_x(100.0), 8.101_08);
    }

    /// Clicks hit a marker on its exact frame only (`UI_ICON_SIZE` is 0 headless).
    #[test]
    fn clicks_hit_exact_frames_only() {
        let ms = at(&[-9, 10, 260]);
        assert_eq!(over_marker(&ms, 0.0), Some(0));
        assert_eq!(over_marker(&ms, 1.0), None);
        assert_eq!(over_marker(&ms, 111.0), None);
        assert_eq!(over_marker(&ms, 1573.0), Some(2));
        assert_eq!(nearest(&ms, 9.0), Some(1));
        // Ties keep the first.
        assert_eq!(nearest(&at(&[0, 2]), 1.0), Some(0));
        assert_eq!(nearest_time(&[], 2.6), 3);
    }

    #[test]
    fn frame_names_have_two_digits() {
        assert_eq!(TimelineMarker::frame_name(5), "F_05");
        assert_eq!(TimelineMarker::frame_name(120), "F_120");
        assert_eq!(TimelineMarker::frame_name(-3), "F_-3");
    }
}
