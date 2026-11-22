// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.rip_move(MESH_OT_rip={…}, TRANSFORM_OT_translate={…})`, a macro
//! (`MESH_OT_rip_move`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `mesh.rip` then `transform.translate` with their nested arguments, tier T1,
//! *headless-extended* like `mesh.rip` (D-150: the rip measures against `frap_location`, or
//! the 3D cursor, as the mouse, in the factory view or along `frap_direction`, passed as
//! `MESH_OT_rip={'frap_location': …, 'frap_direction': …}`;
//! `tests/golden/cases/edit/tools_m8/rip_{edges,verts}.py`). The framework runs the
//! steps (§10.2); this exec is the macro's post-step hook and changes nothing. A cancelled or
//! failed rip ends the macro, so nothing moves.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH_VIEW3D,
    tier = T1,
    kind = Macro,
    support = HeadlessExtended("headless-extended (deviations D-150): blender -b cannot run mesh.rip (invoke only, no window event); frap runs Blender's algorithm with the mouse over MESH_OT_rip's frap_location (unset: the 3D cursor), seen in the factory Layout 3D View (1574x954 WINDOW region) or, with a non-zero frap_direction, in an orthographic view looking along frap_direction: put the point on the side to rip towards")
)]
fn exec(_cx: &mut OpCtx, _p: mesh::RipMove) -> OpResult {
    Ok(OpStatus::Finished)
}
