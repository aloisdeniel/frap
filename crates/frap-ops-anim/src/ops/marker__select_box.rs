// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.select_box(xmin=0, xmax=0, ymin=0, ymax=0, wait_for_input=True, mode='SET',
//! tweak=False)`, tier T0 (WP 7.6). Port of `ed_marker_box_select_exec`
//! (blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: `xmin`/`xmax` are region pixels of the Timeline, turned into frames by its
//! `View2D` (`UI_view2d_region_to_view_rctf`; frap's Timeline is the factory one,
//! `marker__add::common`: pixel 0 is frame -9, pixel 1573 frame 260); `SET` deselects every
//! marker first, then the markers whose frame lies in `[xmin, xmax]` (inclusive; nothing when
//! `xmin > xmax`) are selected, or deselected with `SUB`. The `y` bounds are not used.
//! Verified on 5.2.4 (markers at -9, 10, 260: `xmin=0, xmax=111` selects the one at -9 only,
//! `xmin=111, xmax=0` none, the defaults select the one at -9). Poll
//! `ed_markers_poll_markers_exist`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::marker::{self, select_box::Mode};

use super::marker__add::common;

#[operator(
    poll = common::EXIST,
    tier = T0,
    spec = marker::OPSPEC_MARKER_OT_SELECT_BOX,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::MarkerSelectBox) -> OpResult {
    let xmin = common::region_to_view_x(p.xmin.value as f32);
    let xmax = common::region_to_view_x(p.xmax.value as f32);
    let mode = p.mode.value;
    common::with_markers(cx, |ms| {
        // `SEL_OP_USE_PRE_DESELECT`
        if mode == Mode::Set {
            common::deselect_all(ms);
        }
        let select = mode != Mode::Sub;
        for m in ms.iter_mut() {
            // `BLI_rctf_isect_x`
            let f = m.frame as f32;
            if f >= xmin && f <= xmax {
                m.select = select;
            }
        }
    });
    Ok(OpStatus::Finished)
}
