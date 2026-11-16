// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.primitive_cube_add` (ARCHITECTURE §10.4). Tier T1: verified index by index
//! against Blender 5.2.4 (crates/frap-mesh/tests/cube_vs_blender.rs). Moved from
//! `frap-session/src/ops/` by M1 W0; WP 1.19 owns it from W3 (and adopts `#[operator]`).

use frap_op::helpers::{self, Align};
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{PrimitiveCubeAdd as P, primitive_cube_add};

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    let align = match p.align.value {
        primitive_cube_add::Align::World => Align::World,
        primitive_cube_add::Align::View => Align::View,
        primitive_cube_add::Align::Cursor => Align::Cursor,
    };
    let at = helpers::placement(cx, &p.location, &p.rotation, align, &p.scale)?;
    let mesh = frap_mesh::primitives::cube(p.size.value, p.calc_uvs.value, at.scale);
    let ob = helpers::add_primitive_or_merge(cx, "Cube", mesh, at, p.enter_editmode.value)?;
    cx.result("object", ob);
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1); #[operator] emits this
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &frap_spec_ops_mesh::mesh::OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD,
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Partial("align='VIEW'/'CURSOR' and enter_editmode=True arrive in M1/M2"),
    notes: &["unset location => 3D cursor; unset rotation => from align; scale (0,0,0) => 1"],
};
