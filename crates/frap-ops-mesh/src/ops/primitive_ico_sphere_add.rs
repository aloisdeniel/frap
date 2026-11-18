// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_ico_sphere_add`, tier T1
//! (`tests/golden/cases/primitives/ico_sphere_*.py`). Port of `add_primitive_icosphere_exec`
//! (`editors/mesh/editmesh_add.cc`, Blender 5.2.4, GPL-2.0-or-later): `create_icosphere
//! subdivisions=… radius=…` with the operator scale in the creation matrix.

use frap_mesh::primitives::ico_sphere::{IcoSphere, ico_sphere};
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
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveIcoSphereAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        true,
    )?;
    let params = IcoSphere {
        subdivisions: p.subdivisions.value,
        radius: p.radius.value,
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    };
    common::reserve(params.estimate())?;
    let mesh = ico_sphere(&params);
    common::finish(cx, "Icosphere", mesh, prim.at, p.enter_editmode.value)
}
