// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_convert(modifier='')`, tier T0
//! (`tests/golden/cases/object/data_m8/modifier_convert.py`). Port of `modifier_convert_exec`
//! and its poll `edit_modifier_poll` (`edit_modifier_poll_generic(C, RNA_Modifier, 0, true,
//! false)`, `editors/object/object_modifier.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4: the poll needs an active local object (`Context missing active object`
//! otherwise); Edit Mode is allowed. The exec converts the particles of a particle system
//! modifier into a mesh object (`convert_psys_to_mesh`); any other modifier, or a name that is
//! not on the active object, is `CANCELLED` without a report. Particle systems are out of
//! scope for frap, so a `PARTICLE_SYSTEM` modifier answers `OP_NOT_IMPLEMENTED`; the other
//! cases cancel as in Blender, with a frap WARNING saying why.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::modifier_add::shared;

fn convert_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, "modifier_convert", true)
}

/// `edit_modifier_poll`.
const CONVERT_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: convert_poll,
};

#[operator(
    poll = CONVERT_POLL,
    tier = T0,
    support = Partial(
        "converts particle systems only, which frap does not have: a PARTICLE_SYSTEM modifier \
         answers OP_NOT_IMPLEMENTED, every other modifier is CANCELLED as in Blender"
    )
)]
fn exec(cx: &mut OpCtx, p: object::ModifierConvert) -> OpResult {
    let Some(act) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let found = cx.main().objects.get(act).and_then(|o| {
        shared::find(o, &p.modifier.value).map(|i| {
            let m = &o.modifiers[i];
            (m.name.clone(), m.settings.type_identifier().to_owned())
        })
    });
    let Some((name, ty)) = found else {
        shared::report_missing(cx, &[act], &p.modifier.value);
        return Ok(OpStatus::Cancelled);
    };
    if ty == "PARTICLE_SYSTEM" {
        return Err(OpError::NotImplemented(format!(
            "frap: object.modifier_convert cannot convert the particle system modifier \
             '{name}': particle systems are out of scope"
        )));
    }
    cx.report(
        ReportType::Warning,
        format!(
            "frap: modifier '{name}' ({ty}) is not a particle system; modifier_convert only \
             converts particle systems (use object.modifier_apply or object.convert)"
        ),
    );
    Ok(OpStatus::Cancelled)
}
