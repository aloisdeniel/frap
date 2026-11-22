// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_linked(delimit=set())`, tier T1
//! (`tests/golden/cases/uv/select/linked*.py`). Port of `uv_select_linked_internal` (without
//! picking) and `uv_select_linked_multi` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. On every mesh in Edit Mode with a UV map, the UV islands
//! (faces connected through corners at the same UV, `STD_UV_CONNECT_LIMIT`) of the selected
//! UVs are selected; `delimit` stops at seams, sharp edges or material changes. With sync
//! selection and `SHARED_VERTEX` in face select mode, mesh faces are selected directly. Always
//! finishes.

use frap_bmesh::SelectMode;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::linked::{Delimit, select_linked};
use crate::select::{Sticky, op};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SelectLinked) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let delimit = Delimit {
        seam: p.delimit.value.contains("SEAM"),
        sharp: p.delimit.value.contains("SHARP"),
        material: p.delimit.value.contains("MATERIAL"),
    };
    let select_faces =
        ts.sync && ts.selectmode.contains(SelectMode::FACE) && ts.sticky == Sticky::Vert;
    for (_, me) in op::objects(cx) {
        op::edit(cx, me, ts, |s| {
            select_linked(s, true, false, false, select_faces, delimit);
        })?;
    }
    Ok(OpStatus::Finished)
}
