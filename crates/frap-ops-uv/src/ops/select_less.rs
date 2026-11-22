// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_less()`, tier T1 (`tests/golden/cases/uv/select/more_less*.py`). Port of
//! `uv_select_more_less` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended*. Deselects the UVs (face
//! select mode: the faces) at the boundary of each selected region; with sync selection and
//! `SHARED_VERTEX` it is `mesh.select_less(use_face_step=True)`. Always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::select_more::more_less;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: uv::SelectLess) -> OpResult {
    more_less::exec(cx, false)
}
