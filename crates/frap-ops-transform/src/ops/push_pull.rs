// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.push_pull(value=…, …)`, tier T1
//! (`tests/golden/cases/edit/transform/push_pull*.py`). Port of `TRANSFORM_OT_push_pull`'s exec
//! path and `transdata_elem_push_pull` (`editors/transform/transform_mode_push_pull.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! Each vertex moves towards the pivot by `value` (object space; negative values push away).
//! Edit Mode only in frap (Object Mode answers `NotImplementedError`, decision log M2W0-9).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;
use glam::Vec3;

use crate::edit::{Mode, Request, proportional, set};

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = Partial("Edit Mode (meshes) only; snapping options change nothing")
)]
fn exec(cx: &mut OpCtx, p: transform::PushPull) -> OpResult {
    crate::edit::require_edit_mesh(cx)?;
    let mut r = Request::new(Mode::PushPull);
    if let Some(v) = set(&p.value) {
        r.values[0] = v;
    }
    r.center_override = set(&p.center_override).map(Vec3::from_array);
    r.proportional = proportional(
        &p.use_proportional_edit,
        p.proportional_edit_falloff.value.identifier(),
        p.proportional_size.value,
        p.use_proportional_connected.value,
        p.use_proportional_projected.value,
    );
    crate::edit::trans::run(cx, r)
}
