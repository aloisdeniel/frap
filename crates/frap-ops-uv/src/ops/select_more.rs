// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_more()`, tier T1 (`tests/golden/cases/uv/select/more_less*.py`). Port of
//! `uv_select_more_less` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended* (frap runs it as in an Image
//! Editor in UV mode). In face select mode the faces next to the selected UV faces are
//! selected, otherwise the UVs one edge away from the selected ones (sticky modes apply);
//! with sync selection and `SHARED_VERTEX` it is `mesh.select_more(use_face_step=True)`.
//! Always finishes.

pub(crate) mod more_less;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: uv::SelectMore) -> OpResult {
    more_less::exec(cx, true)
}
