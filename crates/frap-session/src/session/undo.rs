// SPDX-License-Identifier: GPL-3.0-or-later
//! `ed.undo` / `ed.redo` / `ed.undo_redo` / `ed.undo_push` through a ring of committed
//! `State`s (ARCHITECTURE §12: "`ed.undo`/`ed.redo` (M8) keep the last N committed
//! `State`s").
//!
//! M8 W0 stub (decision log M8W0-5): the contract is wired and does nothing yet. The session
//! calls [`after_statement`] once per statement, after its outermost transaction (committed
//! or rolled back), and the `ed.*` operators (`frap-ops-io/src/ops/ed__*.rs`, WP 8.4) record
//! their [`frap_op::UndoRequest`] through `Services::undo` ([`request`]). WP 8.4 owns this
//! file (and `session/undo/**`): the ring (its length from the `edit.undo_steps`
//! preference, Blender's default 32), the steps a statement pushes (Blender pushes one per
//! operator with the `UNDO` option and per RNA write from Python), applying a request to the
//! session's state, and the response of an undo statement. A child module of `session`, so it
//! reaches `Session`'s private fields.

use frap_op::{OpError, UndoRequest};
use frap_scene::ChangeLog;

use super::Session;

/// The ring of committed states (empty until WP 8.4).
#[derive(Default)]
pub(crate) struct UndoRing {}

/// Records the running statement's undo request (`Services::undo`). The stub answers
/// `NotImplementedError` (WP 8.4).
pub(crate) fn request(pending: &mut Option<UndoRequest>, req: UndoRequest) -> Result<(), OpError> {
    let _ = pending;
    Err(OpError::NotImplemented(format!(
        "frap: {req:?}: the undo history is not implemented yet (M8, WP 8.4)"
    )))
}

/// Called once per statement after its outermost transaction: `committed` is false after a
/// rollback (error, CANCELLED, limit). The stub does nothing (WP 8.4).
pub(crate) fn after_statement(session: &mut Session, committed: bool, log: &ChangeLog) {
    let _ = (&session.undo, committed, log);
}
