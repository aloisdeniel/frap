// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.move_on_axis(type='UDIM', axis='X', distance=1)`, tier T1
//! (`tests/golden/cases/uv/edit_m8/move_on_axis.py`). Port of `uv_move_on_axis_exec`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended* (frap's virtual Image Editor,
//! `weld/common.rs`). Moves the selected UVs of visible faces of every mesh in Edit Mode with
//! a UV map and a selected vertex along `axis` by `distance` UDIM tiles (`UDIM`), pixels of
//! the 256 × 256 image size (`PIXEL`) or steps of the (10, 10) custom grid (`DYNAMIC`). Always
//! finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use super::weld::common::{self, CUSTOM_GRID_SUBDIV, IMAGE_EDITOR_NOTE, IMAGE_SIZE};
use crate::select::op;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = HeadlessExtended(IMAGE_EDITOR_NOTE)
)]
fn exec(cx: &mut OpCtx, p: uv::MoveOnAxis) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let axis = usize::from(p.axis.value.identifier() == "Y");
    let distance = p.distance.value;
    let distance_final = match p.r#type.value.identifier() {
        "DYNAMIC" => distance as f32 / CUSTOM_GRID_SUBDIV[axis] as f32,
        "PIXEL" => distance as f32 / IMAGE_SIZE[axis] as f32,
        _ => distance as f32,
    };
    let ts = op::ts(cx);
    for (_, me) in common::objects(cx) {
        if common::no_vert_selected(cx, me) {
            continue;
        }
        common::edit(cx, me, ts, |s| {
            let mut changed = false;
            common::foreach_uv(s, true, true, |luv| {
                luv[axis] += distance_final;
                changed = true;
            });
            ((), changed)
        })?;
    }
    Ok(OpStatus::Finished)
}
