// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.duplicate_move(OBJECT_OT_duplicate={…}, TRANSFORM_OT_translate={…})`, a
//! macro (`OBJECT_OT_duplicate_move`, `editors/object/object_ops.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `object.duplicate` then `transform.translate` with their nested
//! arguments (bpy form `TRANSFORM_OT_translate={'value': (2, 0, 0)}`, short form
//! `TRANSFORM_OT_translate.value=2,0,0`). The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//! Verified on 5.2.4: `OBJECT_OT_duplicate={'linked': True}` shares the data; the copies are
//! translated, the originals stay (`tests/cli/m1/object_ops/duplicate_move.toml`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    kind = Macro,
)]
fn exec(_cx: &mut OpCtx, _p: object::DuplicateMove) -> OpResult {
    Ok(OpStatus::Finished)
}
