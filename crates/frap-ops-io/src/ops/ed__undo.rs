// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.ed.undo` (`ED_OT_undo`, `editors/undo/ed_undo.cc`, Blender 5.2.4; WP 8.4): back
//! to the state before the last undo step.
//!
//! The operator itself changes nothing: it asks the session for the undo
//! ([`frap_op::UndoRequest::Undo`]), which the session applies to its ring of committed
//! states once the statement has committed (`frap-session/src/session/undo.rs`). The poll is
//! `ed_undo_poll`: the active step has a previous one. **Headless-extended** (deviations
//! D-149): `blender -b` keeps no undo stack until a script calls `ed.undo_push()`, and its
//! Python operator calls and RNA writes never push; frap keeps the stack an interactive
//! Blender keeps, from the session start (or the last file load) on.

use frap_op::prelude::*;
use frap_op::{Poll, PollFail, UndoRequest};
use frap_scene::Context;
use frap_spec_ops_io::ed::OPSPEC_ED_OT_UNDO;

/// The `--describe` note of the four `ed.*` operators.
pub(crate) const HEADLESS: &str = "headless-extended (deviations D-149): the undo history an interactive Blender keeps, from the session start or the last file load (blender -b keeps none until ed.undo_push()); every operator with the UNDO option, every RNA assignment and every RNA function call that changed data pushes a step (in Edit Mode only those on the edit data), ed.undo_push() pushes a named one; edit.undo_steps (32) steps are kept";

/// `ed_undo_poll`.
fn check(_: &Context) -> Result<(), PollFail> {
    if frap_op::undo_status().can_undo {
        Ok(())
    } else {
        Err(PollFail {
            message: None,
            unmet: Default::default(),
            hint: Some(
                "nothing to undo: the history starts at the session start or the last file \
                 load, and only committed steps are kept (operators with the UNDO option, RNA \
                 writes, ed.undo_push())"
                    .into(),
            ),
        })
    }
}

/// `ed_undo_poll`.
const POLL: Poll = Poll {
    requires: &[],
    check,
};

fn exec(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    cx.services().undo(UndoRequest::Undo)?;
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_ED_OT_UNDO,
    poll: POLL,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::HeadlessExtended(HEADLESS),
    notes: &[
        "the response's changes name the datablocks that came back (added), went away (removed) or changed (modified); result.undone names the step undone",
        "in Edit Mode an Edit Mesh / Edit Armature step restores the edit data only (as Blender's edit-mode undo types)",
    ],
};
