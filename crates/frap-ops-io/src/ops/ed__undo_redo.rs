// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.ed.undo_redo` (`ED_OT_undo_redo`, `editors/undo/ed_undo.cc`, Blender 5.2.4; WP 8.4):
//! undoes the last registered operator with the `UNDO` option and runs it again with the same
//! arguments (`ED_undo_operator_repeat`). The request ([`frap_op::UndoRequest::UndoRedo`]) is
//! applied by the session, which keeps the last such operator call (`WM_operator_last_redo`;
//! `ed.undo`/`ed.redo` and file loads forget it, as `WM_operator_stack_clear` does). The poll
//! is `ed_undo_redo_poll`: that operator exists and its undo step can be undone.
//! Headless-extended (deviations D-145, see `ed__undo.rs`): in `blender -b` Python operator
//! calls are never registered, so the poll always fails there.

use frap_op::prelude::*;
use frap_op::{Poll, PollFail, UndoRequest};
use frap_scene::Context;
use frap_spec_ops_io::ed::OPSPEC_ED_OT_UNDO_REDO;

use super::ed__undo::HEADLESS;

/// `ed_undo_redo_poll`.
fn check(_: &Context) -> Result<(), PollFail> {
    if frap_op::undo_status().can_undo_redo {
        Ok(())
    } else {
        Err(PollFail {
            message: None,
            unmet: Default::default(),
            hint: Some(
                "nothing to repeat: ed.undo_redo() re-runs the last operator with both the \
                 REGISTER and UNDO options (ed.undo, ed.redo and file loads forget it)"
                    .into(),
            ),
        })
    }
}

/// `ed_undo_redo_poll`.
const POLL: Poll = Poll {
    requires: &[],
    check,
};

fn exec(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    cx.services().undo(UndoRequest::UndoRedo)?;
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_ED_OT_UNDO_REDO,
    poll: POLL,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::HeadlessExtended(HEADLESS),
    notes: &[
        "CANCELLED (state unchanged) when the repeated operator does not finish; result.repeated names it",
    ],
};
