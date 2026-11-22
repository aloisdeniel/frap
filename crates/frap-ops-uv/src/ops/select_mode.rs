// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_mode(type='VERTEX')`, tier T1 (`tests/golden/cases/uv/select/mode*.py`).
//! Port of `uv_select_mode_exec` and `ED_uvedit_selectmode_clean_multi`
//! (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: Blender needs an Image Editor in UV mode; frap runs
//! it *headless-extended* as in one (deviation D-111; goldens call Blender's operator in a
//! `#@blender` `temp_override` of an Image Editor). Sets `tool_settings.uv_select_mode` and
//! cleans every UV selection for the new mode (vertex: shared UVs follow; edge: edges and their
//! shared copies; face: faces); `CANCELLED` when the mode already is `type`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{op, uv_selectmode_of};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SelectMode) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let id = p.r#type.value.identifier();
    let new = uv_selectmode_of(id);
    if new == op::ts(cx).uv_selectmode {
        return Ok(OpStatus::Cancelled);
    }
    op::set_tool_settings(cx, |ts| ts.uv_select_mode = id.into());
    let ts = op::ts(cx);
    for (_, me) in op::objects(cx) {
        op::edit(cx, me, ts, |s| s.selectmode_clean())?;
    }
    Ok(OpStatus::Finished)
}
