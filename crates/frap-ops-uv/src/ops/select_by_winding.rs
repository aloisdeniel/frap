// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_by_winding(winding='NEGATIVE', extend=False)`, tier T1
//! (`tests/golden/cases/uv/select/winding*.py`). Port of `uv_select_by_winding_exec`
//! (`editors/uvedit/uvedit_select.cc`, `BM_face_calc_area_uv_signed` from
//! `bmesh/intern/bmesh_polygon.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. Unless `extend`, every UV is deselected first; then on every
//! mesh in Edit Mode with a UV map the visible faces whose signed UV area is negative
//! (clockwise, `NEGATIVE`) or positive (`POSITIVE`) are selected with the sticky mode and the
//! selection flushed. Always finishes.

use frap_bmesh::{ElemFlag, FaceId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::select_by_winding::Winding;

use super::unwrap::uvedit;
use crate::select::{SelAction, UvSel, op};

/// `BM_face_calc_area_uv_signed`.
pub(crate) fn face_area_uv_signed(s: &UvSel, f: FaceId) -> f32 {
    let mut cross = 0.0f32;
    for l in s.face_loops(f) {
        let a = s.uv(l);
        let b = s.uv(s.next(l));
        cross += (b[0] - a[0]) * (b[1] + a[1]);
    }
    cross * 0.5
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SelectByWinding) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let objects = op::objects(cx);
    let sign = if p.winding.value == Winding::Positive {
        -1.0f32
    } else {
        1.0
    };
    if !p.extend.value {
        for &(_, me) in &objects {
            op::edit(cx, me, ts, |s| s.deselect_all(SelAction::Deselect))?;
        }
    }
    for (_, me) in objects {
        op::edit(cx, me, ts, |s| {
            s.face_tags_clear();
            let mut changed = false;
            for f in s.faces() {
                if !s.face_visible(f) {
                    continue;
                }
                if face_area_uv_signed(s, f) * sign > 0.0 {
                    s.set_fflag(f, ElemFlag::TAG, true);
                    changed = true;
                }
            }
            if changed {
                s.flush_from_tag_face(true);
                if s.ts.sync {
                    s.select_sync_flush(true);
                } else {
                    s.selectmode_flush();
                }
            }
        })?;
    }
    Ok(OpStatus::Finished)
}
