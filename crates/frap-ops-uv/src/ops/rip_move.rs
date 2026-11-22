// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.rip_move(UV_OT_rip={…}, TRANSFORM_OT_translate={…})`, a macro, tier T1
//! (`tests/golden/cases/uv/edit_m8/rip*.py`). `UV_OT_rip_move` (`ED_operatormacros_uvedit`,
//! `editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later: `uv.rip` then
//! `transform.translate` of the UVs that stay selected (the macro forces
//! `use_proportional_edit` and `mirror` off).
//!
//! *Headless-extended*: in Blender the translation is the Image Editor's (it moves UVs; frap's
//! `transform.translate` is the virtual 3D View's and moves vertices), so this macro runs its
//! steps itself (`frap_op::pipeline::MACRO_OWN_STEPS`), on one UV selection: the rip leaves
//! `uv_select_sync_valid` set with sync selection and the translation reads that UV selection,
//! as Blender's steps share it. The translation is Blender's for an Image Editor without an
//! image (UV aspect 1): `value[0]`, `value[1]` are added to the selected UVs of visible faces
//! (`createTransUVs`, `flushTransUVs`); a given `value` is final, so `constraint_axis` and the
//! other `transform.translate` arguments change nothing (no snapping, no proportional
//! editing). A rip that changes nothing cancels the macro, as `wm_macro_exec`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::rip::rip_multi;
use super::unwrap::uvedit;
use super::weld::common::{self, IMAGE_EDITOR_NOTE};
use crate::select::{Uv, op};

/// The UV translation of the `TRANSFORM_OT_translate` step: its `value` (`T_INPUT_IS_VALUES_FINAL`:
/// given values bypass the constraint projection, so `constraint_axis` changes nothing).
fn translation(args: &OpArgs) -> Uv {
    let mut v = [0.0f32; 3];
    if let Some(Value::FloatArray { data, .. }) = args.values.get("value") {
        for (o, d) in v.iter_mut().zip(data.iter()) {
            *o = *d;
        }
    }
    [v[0], v[1]]
}

/// The `location` of the `UV_OT_rip` step.
fn location(args: &OpArgs) -> Uv {
    match args.values.get("location") {
        Some(Value::FloatArray { data, .. }) if data.len() >= 2 => [data[0], data[1]],
        _ => [0.0, 0.0],
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
    support = HeadlessExtended(IMAGE_EDITOR_NOTE)
)]
fn exec(cx: &mut OpCtx, p: uv::RipMove) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let Some(meshes) = rip_multi(cx, ts, location(&p.UV_OT_rip))? else {
        return Ok(OpStatus::Cancelled);
    };
    let offset = translation(&p.TRANSFORM_OT_translate);
    for (me, sync_valid) in meshes {
        common::edit(cx, me, ts, |s| {
            s.sync_valid = sync_valid;
            let mut changed = false;
            common::foreach_uv(s, true, true, |luv| {
                luv[0] += offset[0];
                luv[1] += offset[1];
                changed = true;
            });
            ((), changed)
        })?;
    }
    Ok(OpStatus::Finished)
}
