// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_circle_add`, tier T1 (`tests/golden/cases/primitives/circle_*.py`).
//! Port of `add_primitive_circle_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `create_circle segments=… radius=… cap_ends=… cap_tris=…`, no scale in
//! the creation matrix.

use frap_mesh::primitives::circle::{Circle, circle};
use frap_mesh::primitives::cone::FillType;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, primitive_circle_add};

use super::primitive_cube_add::common;

fn fill(f: primitive_circle_add::FillType) -> FillType {
    match f {
        primitive_circle_add::FillType::Nothing => FillType::Nothing,
        primitive_circle_add::FillType::Ngon => FillType::Ngon,
        primitive_circle_add::FillType::Trifan => FillType::Trifan,
    }
}

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial(common::PARTIAL),
    notes = common::NOTES
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveCircleAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        false,
    )?;
    let mesh = circle(&Circle {
        vertices: p.vertices.value,
        radius: p.radius.value,
        fill_type: fill(p.fill_type.value),
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    });
    common::finish(cx, "Circle", mesh, prim.at, p.enter_editmode.value)
}
