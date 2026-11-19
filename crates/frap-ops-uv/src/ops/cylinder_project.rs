// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.cylinder_project(direction='VIEW_ON_EQUATOR', align='POLAR_ZX', pole='PINCH',
//! seam=False, radius=1.0, correct_aspect=True, clip_to_bounds=False, scale_to_bounds=False)`,
//! tier T1 (`tests/golden/cases/uv/project/cylinder_*.py`). Port of `cylinder_project_exec`
//! (`editors/uvedit/uvedit_unwrap_ops.cc`), Blender 5.2.4, GPL-2.0-or-later; see
//! `sphere_project.rs` for the shared steps.
//!
//! Poll `ED_operator_uvmap`. As `uv.sphere_project` with `map_to_tube`: the selected faces are
//! wrapped around a cylinder (`radius` scales the frame for the view directions) through the
//! transform pivot, oriented by the virtual 3D View (D-10) and the object, or by the object
//! alone (`ALIGN_TO_OBJECT`). Always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_uv::project::CurvedProjection;

use super::sphere_project::{Curved, curved_exec};
use super::unwrap::uvedit;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::CylinderProject) -> OpResult {
    curved_exec(
        cx,
        &Curved {
            kind: CurvedProjection::Cylinder,
            direction: p.direction.value.identifier(),
            align: p.align.value.identifier(),
            fan: p.pole.value.identifier() == "FAN",
            seam: p.seam.value,
            radius: p.radius.value,
            clip: uvedit::clip_props(
                p.correct_aspect.value,
                p.clip_to_bounds.value,
                p.scale_to_bounds.value,
            ),
        },
    )
}
