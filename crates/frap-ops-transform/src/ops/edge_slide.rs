// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.edge_slide(value=…, use_even=…, flipped=…, use_clamp=…, correct_uv=…)`,
//! tier T1 (`tests/golden/cases/edit/transform/edge_slide*.py`). Port of
//! `TRANSFORM_OT_edge_slide`'s exec path (`initEdgeSlide`, `applyEdgeSlide`,
//! `transform_mode_edge_slide.cc`; `transform_mesh_edge_slide_data_create`,
//! `transform_convert_mesh.cc`), Blender 5.2.4, GPL-2.0-or-later, through
//! [`crate::edit::slide`].
//!
//! The selected edges must form loops or chains (each selected vertex on one or two selected
//! edges, each selected edge with one or two faces), otherwise the operator is cancelled. Which
//! side `value > 0` slides to depends on the 3D View as in Blender (frap uses the factory
//! viewport). `correct_uv` (default True) re-interpolates the corner data of the slid vertices.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;

use crate::edit::{Mode, Request, set};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = Partial(
        "snapping options change nothing; an unset correct_uv always means True (Blender \
         remembers the last explicit value in the scene)"
    ),
    notes = [
        "The slide side of value > 0 follows the factory 3D View (as a background Blender): \
         every edge loop is oriented like the one whose slide segment projects closest to the \
         region corner."
    ]
)]
fn exec(cx: &mut OpCtx, p: transform::EdgeSlide) -> OpResult {
    let mut r = Request::new(Mode::EdgeSlide);
    if let Some(v) = set(&p.value) {
        r.values[0] = v;
    }
    r.slide.use_even = p.use_even.value;
    r.slide.flipped = p.flipped.value;
    r.slide.use_clamp = p.use_clamp.value;
    r.slide.single_side = p.single_side.value;
    r.slide.correct_uv = set(&p.correct_uv);
    crate::edit::trans::run(cx, r)
}
