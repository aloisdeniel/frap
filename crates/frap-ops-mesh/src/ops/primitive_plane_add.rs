// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_plane_add`, tier T1 (`tests/golden/cases/primitives/plane_*.py`).
//! Port of `add_primitive_plane_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `create_grid x_segments=0 y_segments=0 size=size/2`, no scale in the
//! creation matrix (the operator's `scale` is ignored, verified on 5.2.4).

use frap_mesh::primitives::plane::{Plane, plane};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::primitive_cube_add::common;

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial(common::PARTIAL),
    notes = common::NOTES
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitivePlaneAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        false,
    )?;
    let mesh = plane(&Plane {
        size: p.size.value,
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    });
    common::finish(cx, "Plane", mesh, prim.at, p.enter_editmode.value)
}
