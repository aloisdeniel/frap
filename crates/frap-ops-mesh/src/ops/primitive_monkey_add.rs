// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_monkey_add` (Suzanne), tier T1
//! (`tests/golden/cases/primitives/monkey_*.py`; licence decision D1). Port of
//! `add_primitive_monkey_exec` (`editors/mesh/editmesh_add.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): no scale in the creation matrix; the generator multiplies its 3×3 part
//! by `size / 2`.

use frap_mesh::primitives::monkey::{Monkey, monkey};
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
fn exec(cx: &mut OpCtx, p: mesh::PrimitiveMonkeyAdd) -> OpResult {
    // `ED_object_add_generic_get_opts(C, op, 'Y', …)`: Suzanne faces the view.
    let prim = common::prim_aligned(
        cx,
        &p.location,
        &p.rotation,
        p.align.value.identifier(),
        &p.scale,
        false,
        common::ViewAxis::Y,
    )?;
    let params = Monkey {
        size: p.size.value,
        calc_uvs: p.calc_uvs.value,
        matrix: prim.matrix,
    };
    common::reserve(params.estimate())?;
    let mesh = monkey(&params);
    common::finish(cx, "Suzanne", mesh, prim.at, p.enter_editmode.value)
}
