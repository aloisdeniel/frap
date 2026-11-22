// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.unlink_data()`, tier T0
//! (`tests/golden/cases/object/data_m8/unlink_data.py`). Port of `object_unlink_data_exec`
//! (`editors/object/object_relations.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! The operator is the unlink button of a `template_ID` (it unlinks an image from an image
//! empty) and reads the button under the mouse (`context_active_but_prop_get_templateID`).
//! Verified on 5.2.4: a script has no such button, so every call fails with `RuntimeError:
//! Error: Incorrect context for running object data unlink` (no poll, nothing changes). frap
//! has no UI either and answers the same; `object.data` is not writable, so a script unlinks
//! an image empty's image with `bpy.data.objects.new` or by removing the image.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    notes = ["always fails as in Blender scripts: it needs the template_ID button it was pressed from"]
)]
fn exec(_cx: &mut OpCtx, _p: object::UnlinkData) -> OpResult {
    Err(OpError::Runtime(
        "Incorrect context for running object data unlink".into(),
    ))
}
