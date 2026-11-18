// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.vert_slide(value=…, use_even=…, flipped=…, use_clamp=…, direction=…,
//! correct_uv=…)`, tier T1 (`tests/golden/cases/edit/transform/vert_slide.py`). Port of
//! `TRANSFORM_OT_vert_slide`'s exec path (`initVertSlide_ex`, `applyVertSlide`,
//! `transform_mode_vert_slide.cc`; `transform_mesh_vert_slide_data_create`,
//! `transform_convert_mesh.cc`), Blender 5.2.4, GPL-2.0-or-later, through
//! [`crate::edit::slide`].
//!
//! Each selected vertex slides towards the neighbor whose edge points closest to the world
//! `direction` (default +X), by `value` (0..1 when clamped) of the edge, or by a distance
//! with `use_even` (the reference edge is the vertex projecting closest to the 3D View's
//! corner).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;
use glam::Vec3;

use crate::edit::{Mode, Request, set};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = Partial(
        "snapping options change nothing; an unset correct_uv always means True (Blender \
         remembers the last explicit value in the scene)"
    )
)]
fn exec(cx: &mut OpCtx, p: transform::VertSlide) -> OpResult {
    let mut r = Request::new(Mode::VertSlide);
    if let Some(v) = set(&p.value) {
        r.values[0] = v;
    }
    r.slide.use_even = p.use_even.value;
    r.slide.flipped = p.flipped.value;
    r.slide.use_clamp = p.use_clamp.value;
    r.slide.correct_uv = set(&p.correct_uv);
    r.slide.direction = set(&p.direction).map(Vec3::from_array);
    crate::edit::trans::run(cx, r)
}
