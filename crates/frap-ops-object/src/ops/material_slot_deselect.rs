// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_deselect()`, tier T1 (`tests/golden/cases/edit/select/
//! material_slot.py`). Port of `material_slot_deselect_exec` (`editors/render/
//! render_shading.cc`, Blender 5.2.4, GPL-2.0-or-later); the shared body is
//! `material_slot_select::common::de_select`.
//!
//! In Edit Mode, deselects the visible faces that use the material of the active object's
//! active slot, on every mesh in Edit Mode that has that material (`BM_face_select_set(False)`:
//! in vertex mode their vertices and edges too). The selection is not flushed. Without such
//! faces, or outside Edit Mode, the operator is `CANCELLED` (it has no poll, as in Blender).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use super::material_slot_select::common;

#[operator(poll = poll::ALWAYS, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotDeselect) -> OpResult {
    common::de_select(cx, false)
}
