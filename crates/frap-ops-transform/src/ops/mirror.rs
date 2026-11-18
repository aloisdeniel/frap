// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.mirror(constraint_axis=…, orient_type=…, …)`, tier T1
//! (`tests/golden/cases/edit/transform/mirror*.py`). Port of `TRANSFORM_OT_mirror`'s exec path,
//! `applyMirror` and `ElementMirror` (`editors/transform/transform_mode_mirror.cc`, Blender
//! 5.2.4, GPL-2.0-or-later).
//!
//! The selection is reflected about the pivot along the constrained axes of the orientation
//! (one axis: a reflection; two: both; three: a point reflection). Without `constraint_axis`
//! nothing moves. Edit Mode only in frap (Object Mode answers `NotImplementedError`, decision
//! log M2W0-9).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;
use glam::Vec3;

use crate::edit::{Mode, Request, matrix, set};

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = Partial("Edit Mode (meshes) only")
)]
fn exec(cx: &mut OpCtx, p: transform::Mirror) -> OpResult {
    crate::edit::require_edit_mesh(cx)?;
    let mut r = Request::new(Mode::Mirror);
    r.orient_type = set(&p.orient_type).map(|s| s.to_string());
    r.orient_matrix = matrix(&p.orient_matrix);
    r.orient_matrix_type = set(&p.orient_matrix_type).map(|s| s.to_string());
    r.constraint_axis = set(&p.constraint_axis);
    r.has_constraint_prop = true;
    r.center_override = set(&p.center_override).map(Vec3::from_array);
    crate::edit::trans::run(cx, r)
}
