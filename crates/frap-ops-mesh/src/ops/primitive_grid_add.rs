// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_grid_add`, tier T1 (`tests/golden/cases/primitives/grid_*.py`).
//! Port of `add_primitive_grid_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `create_grid x_segments=… y_segments=… size=size/2`, no scale in the
//! creation matrix.

use frap_mesh::primitives::grid::{Grid, grid};
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
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveGridAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        false,
    )?;
    let params = Grid {
        x_subdivisions: p.x_subdivisions.value,
        y_subdivisions: p.y_subdivisions.value,
        size: p.size.value,
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    };
    common::reserve(params.estimate())?;
    let mesh = grid(&params);
    common::finish(cx, "Grid", mesh, prim.at, p.enter_editmode.value)
}
