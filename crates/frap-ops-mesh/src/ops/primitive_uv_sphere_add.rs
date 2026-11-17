// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_uv_sphere_add`, tier T1 for vertices and positions; edges, faces,
//! UVs and normals compared canonically (deviation D-19: Blender's own edge and face order is
//! not stable between runs), `tests/golden/cases/primitives/uv_sphere_*.py`. Port of
//! `add_primitive_uvsphere_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `create_uvsphere u_segments=… v_segments=… radius=…` with the operator
//! scale in the creation matrix.

use frap_mesh::primitives::uv_sphere::{UvSphere, uv_sphere};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::primitive_cube_add::common;

#[operator(
    poll = poll::always,
    tier = T1,
    support = Partial(common::PARTIAL),
    notes = [
        common::NOTES[0],
        common::NOTES[1],
        common::NOTES[2],
        common::NOTES[3],
        "edge and face order differ from Blender's, which is unstable between runs (deviation D-19); vertex order and positions are Blender's",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveUvSphereAdd) -> OpResult {
    let prim = common::prim(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        true,
    )?;
    let mesh = uv_sphere(&UvSphere {
        segments: p.segments.value,
        ring_count: p.ring_count.value,
        radius: p.radius.value,
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    });
    common::finish(cx, "Sphere", mesh, prim.at, p.enter_editmode.value)
}
