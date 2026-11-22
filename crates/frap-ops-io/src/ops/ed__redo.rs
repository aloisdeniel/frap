// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.ed.redo` (`ED_OT_redo`, `editors/undo/ed_undo.cc`, Blender 5.2.4; WP 8.4): forward
//! to the next undo step. Like `ed.undo` it only records the request
//! ([`frap_op::UndoRequest::Redo`]) for the session's ring; the poll is `ed_redo_poll` (the
//! active step has a next one). Headless-extended (deviations D-145, see `ed__undo.rs`).

use frap_op::prelude::*;
use frap_op::{Poll, PollFail, UndoRequest};
use frap_scene::Context;
use frap_spec_ops_io::ed::OPSPEC_ED_OT_REDO;

use super::ed__undo::HEADLESS;

/// `ed_redo_poll`.
fn check(_: &Context) -> Result<(), PollFail> {
    if frap_op::undo_status().can_redo {
        Ok(())
    } else {
        Err(PollFail {
            message: None,
            unmet: Default::default(),
            hint: Some(
                "nothing to redo: redo follows an ed.undo(), and any new undo step (an \
                 operator, an RNA write, ed.undo_push()) drops the steps that could be redone"
                    .into(),
            ),
        })
    }
}

/// `ed_redo_poll`.
const POLL: Poll = Poll {
    requires: &[],
    check,
};

fn exec(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    cx.services().undo(UndoRequest::Redo)?;
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_ED_OT_REDO,
    poll: POLL,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::HeadlessExtended(HEADLESS),
    notes: &[
        "the response's changes name the datablocks that came back, went away or changed; result.redone names the step redone",
    ],
};
