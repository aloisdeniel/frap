// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_cylinder_add`, tier T1
//! (`tests/golden/cases/primitives/cylinder_*.py`). Port of `add_primitive_cylinder_exec`
//! (`editors/mesh/editmesh_add.cc`, Blender 5.2.4, GPL-2.0-or-later): `create_cone
//! radius1=radius radius2=radius depth=…` with the operator scale in the creation matrix.

use frap_mesh::primitives::cone::FillType;
use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, primitive_cylinder_add};

use super::primitive_cube_add::common;

fn fill(f: primitive_cylinder_add::EndFillType) -> FillType {
    match f {
        primitive_cylinder_add::EndFillType::Nothing => FillType::Nothing,
        primitive_cylinder_add::EndFillType::Ngon => FillType::Ngon,
        primitive_cylinder_add::EndFillType::Trifan => FillType::Trifan,
    }
}

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial(common::PARTIAL),
    notes = common::NOTES
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveCylinderAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        true,
    )?;
    let mesh = cylinder(&Cylinder {
        vertices: p.vertices.value,
        radius: p.radius.value,
        depth: p.depth.value,
        end_fill_type: fill(p.end_fill_type.value),
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    });
    common::finish(cx, "Cylinder", mesh, prim.at, p.enter_editmode.value)
}
