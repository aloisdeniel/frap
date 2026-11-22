// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.ed.undo_push` (`ED_OT_undo_push`, `editors/undo/ed_undo.cc`, Blender 5.2.4;
//! WP 8.4): adds an undo step named `message` at the current state (`ED_undo_push`: a Memfile
//! step in Object Mode, an Edit Mesh / Edit Armature step in Edit Mode; the oldest steps beyond
//! `edit.undo_steps` are dropped). The request ([`frap_op::UndoRequest::Push`]) is applied by
//! the session after the statement. Blender also tags the file modified
//! (`WM_file_tag_modified`): `bpy.data.is_dirty` becomes true. In `blender -b` this is the call
//! that creates the undo stack; frap has one from the start (deviations D-145).

use frap_op::prelude::*;
use frap_op::UndoRequest;
use frap_spec_ops_io::ed::{OPSPEC_ED_OT_UNDO_PUSH, UndoPush as P};

use super::ed__undo::HEADLESS;

fn exec(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = P::from_args(a);
    cx.services().undo(UndoRequest::Push {
        message: p.message.value.to_string(),
    })?;
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(OPERATORS)]
static DEF: OperatorDef = OperatorDef {
    spec: &OPSPEC_ED_OT_UNDO_PUSH,
    // `ED_operator_screenactive`: frap's virtual Layout screen is always active.
    poll: poll::ALWAYS,
    exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::HeadlessExtended(HEADLESS),
    notes: &["result.pushed names the step; bpy.data.is_dirty becomes True"],
};
