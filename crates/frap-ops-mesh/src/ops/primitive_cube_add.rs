// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_cube_add` (ARCHITECTURE §10.4), tier T1: geometry index by index
//! against Blender 5.2.4 (`crates/frap-mesh/tests/primitives_vs_blender.rs`) and the operator
//! with its placement variants in `tests/golden/cases/primitives/cube_*.py`. Port of
//! `add_primitive_cube_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `create_cube size=… matrix=… calc_uvs=…` with the operator scale in the
//! creation matrix.

pub(crate) mod common;

use frap_mesh::primitives::cube::{Cube, cube_mesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial(common::PARTIAL),
    notes = common::NOTES
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveCubeAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        true,
    )?;
    let params = Cube {
        size: p.size.value,
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    };
    common::reserve(params.estimate())?;
    let mesh = cube_mesh(&params);
    common::finish(cx, "Cube", mesh, prim.at, p.enter_editmode.value)
}
