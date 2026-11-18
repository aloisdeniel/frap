// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_cone_add`, tier T1 (`tests/golden/cases/primitives/cone_*.py`).
//! Port of `add_primitive_cone_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `create_cone radius1=… radius2=… depth=…` with the operator scale in the
//! creation matrix.

use frap_mesh::primitives::cone::{Cone, FillType, cone};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, primitive_cone_add};

use super::primitive_cube_add::common;

fn fill(f: primitive_cone_add::EndFillType) -> FillType {
    match f {
        primitive_cone_add::EndFillType::Nothing => FillType::Nothing,
        primitive_cone_add::EndFillType::Ngon => FillType::Ngon,
        primitive_cone_add::EndFillType::Trifan => FillType::Trifan,
    }
}

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial(common::PARTIAL),
    notes = common::NOTES
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveConeAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        true,
    )?;
    let params = Cone {
        vertices: p.vertices.value,
        radius1: p.radius1.value,
        radius2: p.radius2.value,
        depth: p.depth.value,
        end_fill_type: fill(p.end_fill_type.value),
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    };
    common::reserve(params.estimate())?;
    let mesh = cone(&params);
    common::finish(cx, "Cone", mesh, prim.at, p.enter_editmode.value)
}
