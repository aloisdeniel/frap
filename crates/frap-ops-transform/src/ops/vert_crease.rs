// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.vert_crease(value=…)`, tier T1
//! (`tests/golden/cases/edit/transform/crease_weight.py`). Port of `TRANSFORM_OT_vert_crease`'s exec
//! path and `transdata_elem_value` (`editors/transform/transform_mode_customdata.cc`,
//! `transform_convert_mesh_vert_cdata.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Adds `value` (clamped to at most 1) to the crease of the selected vertices, each result
//! clamped to 0..1; the `crease_vert` attribute is created when missing.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;

use crate::edit::{Mode, Request, set};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: transform::VertCrease) -> OpResult {
    let mut r = Request::new(Mode::VertCrease);
    if let Some(v) = set(&p.value) {
        r.values[0] = v;
    }
    crate::edit::trans::run(cx, r)
}
