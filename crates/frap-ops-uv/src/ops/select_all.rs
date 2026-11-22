// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_all(action='TOGGLE')`, tier T1 (`tests/golden/cases/uv/select/all*.py`).
//! Port of `uv_select_all_exec`, `uv_select_all_perform_multi_ex` and `ED_uvedit_deselect_all`
//! (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. On every mesh in Edit Mode with a UV map: `SELECT` / `DESELECT`
//! (`TOGGLE` picks one across the meshes: deselect when any UV is selected) select or
//! deselect every visible UV; with UV sync selection that is the whole mesh selection.
//! `INVERT` inverts the UV selection in the UV select mode (sync: the mesh selection of the
//! mesh select mode). Always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{SelAction, op};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SelectAll) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let objects = op::objects(cx);
    let mut action = SelAction::of(p.action.value.identifier());
    if action == SelAction::Toggle {
        let any = objects
            .iter()
            .any(|&(_, me)| op::read(cx, me, ts, |s| s.is_any_selected()).unwrap_or(false));
        action = if any {
            SelAction::Deselect
        } else {
            SelAction::Select
        };
    }
    for (_, me) in objects {
        op::edit(cx, me, ts, |s| s.deselect_all(action))?;
    }
    Ok(OpStatus::Finished)
}
