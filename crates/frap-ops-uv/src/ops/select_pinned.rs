// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_pinned()`, tier T1 (`tests/golden/cases/uv/select/pinned*.py`). Port of
//! `uv_select_pinned_exec` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. Only in vertex select mode (the UV select mode, or with sync
//! selection the mesh one; else the error "Pinned vertices can be selected in Vertex Mode
//! only"): on every mesh in Edit Mode whose active UV map has a pin layer (`.pn.<uv map>`),
//! the pinned UVs of visible faces are selected and the selection flushed. Always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{UV_SELECT_VERT, op};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: uv::SelectPinned) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    if ts.select_mode_get() != UV_SELECT_VERT {
        return Err(OpError::Runtime(
            "Pinned vertices can be selected in Vertex Mode only".into(),
        ));
    }
    for (_, me) in op::objects(cx) {
        let pins = uvedit::edit_bm(cx, me).and_then(|bm| {
            let layer = uvedit::active_uv(&bm)?;
            let name = uvedit::pin_name(&uvedit::uv_name(&bm, layer));
            bm.ldata.find(&name)?;
            Some(uvedit::read_pins(&bm, layer))
        });
        let Some(pins) = pins else {
            continue;
        };
        op::edit(cx, me, ts, |s| {
            s.prepare();
            for f in s.faces() {
                if !s.face_visible(f) {
                    continue;
                }
                for l in s.face_loops(f) {
                    if pins.get(l.slot()).copied().unwrap_or(false) {
                        s.uv_select_enable(l);
                    }
                }
            }
            if s.ts.sync {
                s.select_sync_flush(true);
            } else {
                s.selectmode_flush();
            }
        })?;
    }
    Ok(OpStatus::Finished)
}
