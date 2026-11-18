// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.tosphere(value=…, …)`, tier T1
//! (`tests/golden/cases/edit/transform/tosphere*.py`). Port of `TRANSFORM_OT_tosphere`'s exec
//! path, `to_sphere_radius_update` and `transdata_elem_to_sphere`
//! (`editors/transform/transform_mode_tosphere.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Vertices move towards the sphere about the pivot whose radius is their mean distance to it
//! (`value` 0..1). Edit Mode only in frap (Object Mode answers `NotImplementedError`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;
use glam::Vec3;

use crate::edit::{Mode, Request, proportional, set};

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = Partial("Edit Mode (meshes) only; snapping options change nothing"),
    notes = [
        "With proportional editing the radius is the factor-weighted mean, divided by the \
         factor sum truncated to an integer (as Blender accumulates it)."
    ]
)]
fn exec(cx: &mut OpCtx, p: transform::Tosphere) -> OpResult {
    crate::edit::require_edit_mesh(cx)?;
    let mut r = Request::new(Mode::ToSphere);
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
