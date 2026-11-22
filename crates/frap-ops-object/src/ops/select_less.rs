// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_less()`, tier T0
//! (`tests/golden/cases/object/select_m8/more_less*.py`). Ported from
//! blender/source/blender/editors/object/object_select.cc (`object_select_more_less`,
//! `object_select_less_exec`), Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4: poll
//! `ED_operator_objectmode`; both ends of every parent–child pair of which exactly one is
//! selected (and visible) are tagged, and the tagged visible, selectable bases are deselected:
//! the selection shrinks at its hierarchy boundaries; nothing changed → `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use super::select_more::more_less::more_less;

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::SelectLess) -> OpResult {
    if more_less(cx, false) {
        return Ok(OpStatus::Finished);
    }
    cx.report(
        ReportType::Warning,
        "frap: nothing to deselect: no selected object has an unselected parent or child",
    );
    Ok(OpStatus::Cancelled)
}
