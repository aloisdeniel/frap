// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.shrink_fatten(value=…, use_even_offset=…, …)`, tier T1
//! (`tests/golden/cases/edit/transform/shrink_fatten*.py`). Port of `TRANSFORM_OT_shrink_fatten`'s
//! exec path and `transdata_elem_shrink_fatten` (`editors/transform/transform_ops.cc`,
//! `transform_mode_shrink_fatten.cc`, Blender 5.2.4, GPL-2.0-or-later) through the edit-mode
//! engine ([`crate::edit::trans`]).
//!
//! Each selected vertex moves along its normal (in face select mode the normal of its selected
//! faces) by `value` in object space; `use_even_offset` scales by the shell factor.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;

use crate::edit::{Mode, Request, proportional, set};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = Partial(
        "snapping options change nothing (as in a non-interactive Blender call); Auto Merge is \
         not implemented"
    ),
    notes = [
        "value is an object-space distance along the vertex normals (the object scale is not \
         applied), as in Blender."
    ]
)]
fn exec(cx: &mut OpCtx, p: transform::ShrinkFatten) -> OpResult {
    let mut r = Request::new(Mode::ShrinkFatten {
        even: p.use_even_offset.is_set && p.use_even_offset.value,
    });
    if let Some(v) = set(&p.value) {
        r.values[0] = v;
    }
    r.proportional = proportional(
        &p.use_proportional_edit,
        p.proportional_edit_falloff.value.identifier(),
        p.proportional_size.value,
        p.use_proportional_connected.value,
        p.use_proportional_projected.value,
    );
    crate::edit::trans::run(cx, r)
}
