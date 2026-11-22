// SPDX-License-Identifier: GPL-3.0-or-later
//! `ed.undo` / `ed.redo` / `ed.undo_redo` / `ed.undo_push` through a ring of committed
//! `State`s (ARCHITECTURE §12: "`ed.undo`/`ed.redo` (M8) keep the last N committed
//! `State`s"; WP 8.4).
//!
//! **What Blender does, verified on 5.2.4.** `blender -b` starts without an undo stack
//! (`ed.undo` fails its poll with "Undo disabled at startup in background-mode (call
//! `ed.undo_push()` to explicitly initialize the undo-system)"); `ed.undo_push()` creates it,
//! and afterwards undo and redo work headless. Python never pushes steps by itself: operators
//! called from Python run with `op_undo_depth > 0` (unless called with the positional
//! `undo=True`) and RNA writes from Python have no undo push at all; the interactive Blender
//! pushes one step per operator with the `UNDO` option (`wm_operator_finished`) and per
//! property edited in the UI (`ui_apply_but_undo`).
//!
//! **What frap does (headless-extended, deviations D-149).** The stack of an interactive
//! Blender: created with the session (lazily, before its first statement) and on every file
//! load (`wm.open_mainfile`, `wm.read_homefile`, `wm.read_factory_settings`,
//! `wm.revert_mainfile`: `wm_file_read_post` clears it and pushes "Original", plus "Original
//! Mode" in Edit Mode). After every statement that committed ([`finish`]):
//!
//! - an operator call pushes a step named after the operator (`ED_undo_push_op`, its label)
//!   when it has the `UNDO` option, a grouped one (`ED_undo_grouped_push_op`) with
//!   `UNDO_GROUPED`; one with `REGISTER` and `UNDO` becomes the operator `ed.undo_redo`
//!   repeats (`WM_operator_last_redo`);
//! - an RNA assignment pushes a step named after the property (as a UI button edit does)
//!   when Blender would (`ED_undo_is_legacy_compatible_for_property`): the written struct
//!   belongs to a datablock (not to the window, the 3D View or the preferences) and, in Edit
//!   Mode, to one of the edit data's type;
//! - an RNA function call (or an expression with one) that changed data pushes a step named
//!   after the function, with the same Edit Mode rule;
//! - an `ed.*` operator applies its [`UndoRequest`] (recorded through `Services::undo`,
//!   [`request`]): `undo_push` pushes a named step (and tags the file modified, as
//!   `ED_undo_push` does in `blender -b` too, before it returns for a missing stack), `undo` and
//!   `redo` load the previous / next step (`ed_undo_step_direction`, forgetting the operator
//!   to repeat as `WM_operator_stack_clear` does), `undo_redo` loads the step before the
//!   repeated operator's (`ED_undo_pop_op`) and runs it again (`ED_undo_operator_repeat`).
//!   Their response's `changes` name what came back (`undo/diff.rs`).
//!
//! The implicit steps above leave `bpy.data.is_dirty` alone (D-149), although the interactive
//! `ED_undo_push` they emulate calls `WM_file_tag_modified`: `is_dirty` keeps the `blender -b`
//! behaviour, where Python's operator calls and RNA writes never push and the file stays
//! unmodified, so the other goldens' `is_dirty` still compare. Only `ed.undo_push` tags it.
//!
//! Steps are pushed through `ED_undo_push` (`edit.undo_steps`, 32, limits the stack) and
//! `BKE_undosys_step_push` (`undo/stack.rs`, typed Memfile / Edit Mesh / Edit Armature steps
//! in `undo/steps.rs`), including `WITH_GLOBAL_UNDO_ENSURE_UPDATED`: an edit-mode step pushed
//! while datablocks were added, removed or renamed since the last memfile step is preceded by
//! a skipped "MemFile Internal (pre)" memfile step. Every step shares its data with the state
//! it was made from (`Arc`s): a step costs O(#IDs), not O(data).
//!
//! The polls of `ed.undo`, `ed.redo` and `ed.undo_redo` read [`frap_op::undo_status`], which
//! [`enter`] and [`finish`] publish around every statement.

mod diff;
mod stack;
mod steps;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use frap_lang::{Command, Stmt};
use frap_op::{OpError, UndoRequest, UndoStatus};
use frap_rna::{IdCode, IdKey, IdRef, Input, ObjectId, OpOptions, Report, ReportType, Root, Span};
use frap_scene::{HostRead, ObjectMode};
use indexmap::IndexMap;
use serde_json::Value as J;
use smol_str::SmolStr;

use self::stack::{Stack, StepData, step_name};
use self::steps::{Data, Kind};
use super::{Fail, Session, eval};
use crate::response::{Response, Status};

/// `U.undosteps` of the factory preferences (`preferences.edit.undo_steps`).
pub(crate) const UNDO_STEPS: usize = 32;

/// The `UNDO_GROUPED` operators' `undo_group` (every one frap has is a frame-change operator,
/// `ot->undo_group = "Frame Change"`).
const FRAME_CHANGE: &str = "Frame Change";

/// The operators whose file read resets the stack (`wm_file_read_post`).
const FILE_READS: [&str; 4] = [
    "wm.open_mainfile",
    "wm.read_homefile",
    "wm.read_factory_settings",
    "wm.revert_mainfile",
];

/// How to run the last registered operator again (`wmOperator` kept in `wm->operators`).
#[derive(Clone, Debug)]
enum Rerun {
    /// The statement that called it (its arguments are evaluated again on the restored
    /// state, which is the state they were first evaluated on).
    Stmt(Box<Stmt>),
    /// `Session::invoke` with keyword inputs.
    Invoke {
        idname: SmolStr,
        kwargs: IndexMap<String, Input>,
    },
}

/// `WM_operator_last_redo`: the last operator with `REGISTER` and `UNDO`.
#[derive(Clone, Debug)]
struct LastRedo {
    /// `op->type->idname`.
    idname: SmolStr,
    /// `op->type->name`, the name of its undo step.
    label: SmolStr,
    rerun: Rerun,
}

/// The ring of committed states (`wm->runtime->undo_stack`). Cloning it is cheap: the steps
/// share their data (`Arc`s).
#[derive(Clone)]
pub(crate) struct UndoRing {
    stack: Stack<Data>,
    /// The stack was created (lazily, before the session's first statement).
    initialized: bool,
    /// `Main.is_memfile_undo_written`: no datablock was added, removed or renamed (and no
    /// armature entered Edit Mode) since the last memfile step was written.
    memfile_written: bool,
    /// `WM_operator_last_redo`.
    last_redo: Option<LastRedo>,
    /// The armatures in Edit Mode after the last statement (entering armature Edit Mode
    /// clears `is_memfile_undo_written`, `object_edit.cc`).
    edit_armatures: BTreeSet<IdKey>,
    /// `U.undosteps`.
    steps: usize,
    /// The active object and its mode before the running statement (`object.mode_set` pushes
    /// through the toggle operator it calls).
    mode_before: Option<(IdRef<ObjectId>, ObjectMode)>,
}

impl Default for UndoRing {
    fn default() -> Self {
        UndoRing {
            stack: Stack::default(),
            initialized: false,
            memfile_written: false,
            last_redo: None,
            edit_armatures: BTreeSet::new(),
            steps: UNDO_STEPS,
            mode_before: None,
        }
    }
}

/// Where a statement came from, for `ed.undo_redo` to run an operator call again.
pub(crate) enum Origin<'a> {
    /// A parsed statement (`Session::exec`).
    Stmt(&'a Stmt),
    /// `Session::invoke(idname, kwargs)`.
    Invoke(&'a str, &'a IndexMap<String, Input>),
    /// `Session::get` / `set` / `call`.
    Other,
}

/// Records the running statement's undo request (`Services::undo`); the session applies it
/// in [`finish`] once the statement has committed.
pub(crate) fn request(pending: &mut Option<UndoRequest>, req: UndoRequest) -> Result<(), OpError> {
    *pending = Some(req);
    Ok(())
}

/// Called once per statement after its outermost transaction: a rolled-back statement
/// (error, CANCELLED, limit) drops its undo request.
pub(crate) fn after_statement(
    session: &mut Session,
    committed: bool,
    _log: &frap_scene::ChangeLog,
) {
    if !committed {
        session.services.undo_request = None;
    }
}

/// Before a statement: creates the stack on first use (the session's start state is the
/// "Original" step) and publishes the poll status of this session.
pub(crate) fn enter(session: &mut Session) {
    if !session.undo.initialized {
        let mut ring = std::mem::take(&mut session.undo);
        reset(session, &mut ring);
        session.undo = ring;
    }
    session.undo.mode_before = active_mode(session);
    publish(&session.undo);
}

/// The active object and its mode.
fn active_mode(session: &Session) -> Option<(IdRef<ObjectId>, ObjectMode)> {
    let ob = session.host.state.ctx().active_object()?;
    Some((ob, session.host.state.main.objects.get(ob)?.mode))
}

/// `object_mode_op_string`: the toggle operator `mode_set_ex` calls for a mode.
fn mode_toggle_op(mode: ObjectMode) -> Option<&'static str> {
    match mode {
        ObjectMode::Edit => Some("object.editmode_toggle"),
        ObjectMode::Pose => Some("object.posemode_toggle"),
        _ => None,
    }
}

/// After a statement's response is built: pushes the step the statement makes, applies an
/// `ed.*` request (rewriting the response of an undo statement), and publishes the poll status.
pub(crate) fn finish(session: &mut Session, origin: Origin<'_>, r: &mut Response) {
    let req = session.services.undo_request.take();
    if !session.undo.initialized {
        enter(session);
    }
    let mut ring = std::mem::take(&mut session.undo);
    let decodes = req.as_ref().is_some_and(|q| {
        matches!(
            q,
            UndoRequest::Undo | UndoRequest::Redo | UndoRequest::UndoRedo
        )
    });
    // What an unwind while a step is decoded goes back to: the statement's result and the
    // history it was made with (the steps are `Arc`-shared, so both copies are cheap).
    let saved = decodes.then(|| (session.host.state.clone(), ring.clone()));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        apply(session, &mut ring, origin, req, r);
    }));
    if let Err(payload) = outcome {
        // `--stmt-timeout` or `--max-elements` while a step was decoded (an Edit Mesh step
        // goes through `BMesh::to_mesh`/`from_mesh`, which have cancellation points): the
        // statement is rolled back, history included.
        let budget = frap_math::budget::interrupted(&*payload).is_some()
            || frap_math::budget::elements_exceeded(&*payload).is_some();
        let err = session.unwind_error(payload);
        let mut reports = vec![];
        match saved {
            Some((state, saved_ring)) if budget => {
                session.host.state = state;
                ring = saved_ring;
            }
            saved => {
                // A bug while loading or encoding a step: back to the statement's result,
                // with a new history that starts there.
                if let Some((state, _)) = saved {
                    session.host.state = state;
                }
                ring = UndoRing {
                    steps: ring.steps,
                    ..UndoRing::default()
                };
                reset(session, &mut ring);
                reports.push(Report::new(
                    ReportType::Warning,
                    "frap: the undo history was cleared; it starts again at the current state",
                ));
            }
        }
        if decodes {
            let op = r.op.clone();
            *r = session.error_response(
                Fail {
                    kind: "op",
                    op,
                    err,
                    reports,
                },
                r.line,
            );
        } else {
            r.reports.push(Report::new(
                ReportType::Warning,
                format!(
                    "frap: the undo step could not be recorded ({})",
                    err.message
                ),
            ));
            r.reports.extend(reports);
        }
    }
    ring.edit_armatures = session.host.state.edit.armatures.keys().copied().collect();
    session.undo = ring;
    publish(&session.undo);
}

/// Publishes the poll status ([`frap_op::undo_status`]).
fn publish(ring: &UndoRing) {
    frap_op::set_undo_status(UndoStatus {
        can_undo: ring.stack.can_undo(),
        can_redo: ring.stack.can_redo(),
        can_undo_redo: ring
            .last_redo
            .as_ref()
            .is_some_and(|l| ring.stack.has_undo_named(&l.label)),
    });
}

/// Loads steps into the session; a memfile step read gives a new `Main`, whose
/// `is_memfile_undo_written` is false.
fn load(
    session: &mut Session,
    ring: &mut UndoRing,
    f: impl FnOnce(&mut Stack<Data>, &mut dyn FnMut(&Data, stack::Dir, bool)) -> bool,
) -> bool {
    let mut memfile = false;
    let ok = f(&mut ring.stack, &mut |d, _, _| {
        memfile |= d.is_memfile();
        steps::decode(session, d);
    });
    if memfile {
        ring.memfile_written = false;
    }
    ok
}

/// `wm_file_read_post`: `BKE_undosys_stack_clear`, `BKE_undosys_stack_init_from_main`
/// ("Original", a memfile step) and `BKE_undosys_stack_init_from_context` ("Original Mode" in
/// Edit Mode); the operator list goes too.
fn reset(session: &Session, ring: &mut UndoRing) {
    ring.stack.clear();
    ring.memfile_written = false;
    ring.last_redo = None;
    push_with_kind(session, ring, "Original", Kind::Memfile);
    if let Some(kind) = steps::kind_from_context(&session.host.state, true)
        && kind != Kind::Memfile
    {
        push_with_kind(session, ring, "Original Mode", kind);
    }
    ring.edit_armatures = session.host.state.edit.armatures.keys().copied().collect();
    ring.initialized = true;
}

/// `BKE_undosys_step_push_with_type`.
fn push_with_kind(session: &Session, ring: &mut UndoRing, name: &str, kind: Kind) {
    let state = &session.host.state;
    ring.stack.truncate_after_active();
    if kind != Kind::Memfile && !ring.memfile_written {
        // `WITH_GLOBAL_UNDO_ENSURE_UPDATED`.
        ring.stack.append(
            SmolStr::new_static("MemFile Internal (pre)"),
            steps::encode(state, Kind::Memfile),
            true,
        );
        ring.memfile_written = true;
    }
    ring.stack
        .append(step_name(name), steps::encode(state, kind), false);
    if kind == Kind::Memfile {
        ring.memfile_written = true;
    }
}

/// `ED_undo_push`: the limit (`edit.undo_steps`) when the active step is the last one, then a
/// step of the context's type. Nothing when no type applies.
fn push(session: &Session, ring: &mut UndoRing, name: &str) {
    let Some(kind) = steps::kind_from_context(&session.host.state, ring.stack.active.is_some())
    else {
        return;
    };
    if ring.steps == 0 {
        return;
    }
    if ring.stack.active.is_some() && ring.stack.active_is_last() {
        ring.stack.limit_steps(ring.steps - 1);
    }
    push_with_kind(session, ring, name, kind);
}

/// `ED_undo_grouped_push`: replaces the active step when it has the same name.
fn grouped_push(session: &Session, ring: &mut UndoRing, name: &str) {
    if ring
        .stack
        .active
        .is_some_and(|a| ring.stack.steps[a].name == name)
    {
        ring.stack.clear_active();
    }
    push(session, ring, name);
}

/// The step an operator call pushes (`wm_operator_finished`): the operator's label with
/// `UNDO`, its undo group with `UNDO_GROUPED`. Returns the operator's options.
fn push_for_operator(
    session: &Session,
    ring: &mut UndoRing,
    idname: &str,
) -> Option<(OpOptions, SmolStr)> {
    let def = frap_op::lookup(idname).ok()?;
    let options = def.spec.options;
    let label = SmolStr::new(def.spec.label);
    if options.contains(OpOptions::UNDO) {
        push(session, ring, &label);
    } else if options.contains(OpOptions::UNDO_GROUPED) {
        grouped_push(session, ring, FRAME_CHANGE);
    }
    Some((options, label))
}

/// The arrays of a response's `changes`.
fn changed(r: &Response, keys: &[&str]) -> bool {
    r.changes.as_ref().is_some_and(|c| {
        keys.iter().any(|k| {
            c.get(*k)
                .and_then(J::as_array)
                .is_some_and(|a| !a.is_empty())
        })
    })
}

/// The ID code of a `bpy.data.<collection>[…]` repr.
fn code_of_repr(repr: &str) -> Option<IdCode> {
    let rest = repr.strip_prefix("bpy.data.")?;
    let coll = &rest[..rest.find(['[', '.']).unwrap_or(rest.len())];
    frap_scene::Main::code_of(coll)
}

/// In Edit Mode, the code of the edit data (`ED_undo_is_legacy_compatible_for_property`: no
/// push for a datablock of another type).
fn edit_data_code(session: &Session) -> Option<Option<IdCode>> {
    let ctx = session.host.state.ctx();
    let ob = ctx.active_object()?;
    let o = session.host.state.main.objects.get(ob)?;
    (o.mode == ObjectMode::Edit).then(|| o.data.map(|d| d.code))
}

/// `ED_undo_is_legacy_compatible_for_property` for a datablock of `code` (`None`: a struct of
/// the window, the 3D View or the preferences, which Blender never undoes).
fn undo_compatible(session: &Session, code: Option<IdCode>) -> bool {
    let Some(code) = code else {
        return false;
    };
    match edit_data_code(session) {
        Some(edit) => edit == Some(code),
        None => true,
    }
}

/// The step an RNA assignment pushes: the property's UI name, if Blender would push one.
fn assignment_step(session: &Session, path: &str) -> Option<SmolStr> {
    let resolved = super::exec::parse_path(path)
        .ok()
        .and_then(|p| crate::resolve::resolve(&session.host, &p).ok());
    let (code, name) = match resolved {
        Some(crate::resolve::Target::Prop(place)) => {
            let code = match place.ptr.root {
                Root::Id(a) | Root::Embedded { owner: a, .. } => Some(a.code),
                _ => None,
            };
            (code, SmolStr::new(place.prop.name))
        }
        _ => {
            let tail = path.rsplit(['.', '[']).next().unwrap_or(path);
            (code_of_repr(path), SmolStr::new(tail.trim_end_matches(']')))
        }
    };
    undo_compatible(session, code).then_some(name)
}

/// The step an RNA function call pushes: named after the function, when the call changed a
/// datablock Blender's undo would record.
fn call_step(session: &Session, r: &Response) -> Option<SmolStr> {
    if !changed(r, &["added", "removed", "modified", "renamed"]) {
        return None;
    }
    let reprs: Vec<&str> = r
        .changes
        .as_ref()
        .into_iter()
        .flat_map(|c| ["added", "modified", "removed"].map(|k| c.get(k)))
        .flatten()
        .filter_map(J::as_array)
        .flatten()
        .filter_map(J::as_str)
        .collect();
    let compatible = match edit_data_code(session) {
        Some(edit) => reprs
            .iter()
            .any(|s| code_of_repr(s) == edit && edit.is_some()),
        None => true,
    };
    if !compatible {
        return None;
    }
    Some(match r.func.as_deref() {
        Some(f) => SmolStr::new(f.rsplit('.').next().unwrap_or(f)),
        None => SmolStr::new_static("Python"),
    })
}

/// The pushes and requests of a committed statement.
fn apply(
    session: &mut Session,
    ring: &mut UndoRing,
    origin: Origin<'_>,
    req: Option<UndoRequest>,
    r: &mut Response,
) {
    if r.status != Status::Finished {
        return;
    }
    if changed(r, &["added", "removed", "renamed"])
        || session
            .host
            .state
            .edit
            .armatures
            .keys()
            .any(|k| !ring.edit_armatures.contains(k))
    {
        ring.memfile_written = false;
    }
    if let Some(req) = req
        && r.kind == "op"
    {
        apply_request(session, ring, req, r);
        return;
    }
    match r.kind {
        "op" => {
            let Some(idname) = r.op.clone() else {
                return;
            };
            if FILE_READS.contains(&idname.as_str()) {
                reset(session, ring);
                return;
            }
            if idname == "object.mode_set" {
                // `object_mode_set_exec` has no flags; `mode_set_ex` calls the toggle operator
                // of the mode entered (of the mode left, for Object Mode) with undo, which
                // pushes its step and becomes the operator to repeat.
                let after = active_mode(session);
                let toggle = match (ring.mode_before, after) {
                    (Some((ob0, m0)), Some((ob1, m1))) if ob0 == ob1 && m0 != m1 => {
                        mode_toggle_op(if m1 == ObjectMode::Object { m0 } else { m1 })
                    }
                    _ => None,
                };
                if let Some(toggle) = toggle
                    && let Some((options, label)) = push_for_operator(session, ring, toggle)
                    && options.contains(OpOptions::REGISTER | OpOptions::UNDO)
                {
                    ring.last_redo = Some(LastRedo {
                        idname: SmolStr::new_static(toggle),
                        label,
                        rerun: Rerun::Invoke {
                            idname: SmolStr::new_static(toggle),
                            kwargs: IndexMap::new(),
                        },
                    });
                }
                return;
            }
            let Some((options, label)) = push_for_operator(session, ring, &idname) else {
                return;
            };
            if options.contains(OpOptions::REGISTER | OpOptions::UNDO) {
                ring.last_redo = match origin {
                    Origin::Stmt(s) if matches!(s.cmd, Command::Invoke { .. }) => Some(LastRedo {
                        idname,
                        label,
                        rerun: Rerun::Stmt(Box::new(s.clone())),
                    }),
                    Origin::Invoke(id, kw) => Some(LastRedo {
                        idname,
                        label,
                        rerun: Rerun::Invoke {
                            idname: SmolStr::new(id),
                            kwargs: kw.clone(),
                        },
                    }),
                    _ => None,
                };
            }
        }
        "set" => {
            if let Some(path) = r.path.clone()
                && let Some(name) = assignment_step(session, &path)
            {
                push(session, ring, &name);
            }
        }
        "call" | "get" | "print" => {
            if let Some(name) = call_step(session, r) {
                push(session, ring, &name);
            }
        }
        _ => {}
    }
}

/// Turns the response of an `ed.*` call whose poll no longer holds (the status was published
/// by another session on this thread) into the poll error.
fn poll_failed(session: &Session, ring: &UndoRing, r: &mut Response) {
    publish(ring);
    let idname = r.op.clone().unwrap_or_default();
    let err = frap_op::lookup(&idname)
        .and_then(|def| frap_op::check_poll(def, &session.host.state.ctx()).map(|()| def))
        .err();
    if let Some(err) = err {
        *r = session.error_response(
            Fail {
                kind: "op",
                op: r.op.clone(),
                err,
                reports: vec![],
            },
            r.line,
        );
    }
}

/// The response of an undo statement after its step was loaded: what came back, and the
/// context and edit-mesh blocks of the restored state.
fn restored(
    session: &Session,
    before: &super::State,
    r: &mut Response,
    key: &str,
    step: Option<SmolStr>,
) {
    let log = diff::changes(before, &session.host.state);
    let fresh = session.base(r.kind, r.line, r.status);
    r.context = fresh.context;
    r.edit_mesh = fresh.edit_mesh;
    r.changes = Some(session.changes_json(&log));
    if let Some(step) = step {
        r.result
            .get_or_insert_with(Default::default)
            .insert(key.into(), J::from(step.as_str()));
    }
}

/// `ED_undo_push` / `ed_undo_step_direction` / `ED_undo_operator_repeat` for a request.
fn apply_request(session: &mut Session, ring: &mut UndoRing, req: UndoRequest, r: &mut Response) {
    match req {
        UndoRequest::Push { message } => {
            // `WM_file_tag_modified`.
            session.host.state.main.is_dirty = true;
            push(session, ring, &message);
            r.result
                .get_or_insert_with(Default::default)
                .insert("pushed".into(), J::from(step_name(&message).as_str()));
        }
        UndoRequest::Undo => {
            if !ring.stack.can_undo() {
                return poll_failed(session, ring, r);
            }
            // `WM_operator_stack_clear`.
            ring.last_redo = None;
            let before = session.host.state.clone();
            let undone = ring.stack.active.map(|a| ring.stack.steps[a].name.clone());
            load(session, ring, |st, dec| st.undo(dec));
            restored(session, &before, r, "undone", undone);
        }
        UndoRequest::Redo => {
            if !ring.stack.can_redo() {
                return poll_failed(session, ring, r);
            }
            ring.last_redo = None;
            let before = session.host.state.clone();
            load(session, ring, |st, dec| st.redo(dec));
            let redone = ring.stack.active.map(|a| ring.stack.steps[a].name.clone());
            restored(session, &before, r, "redone", redone);
        }
        UndoRequest::UndoRedo => undo_redo(session, ring, r),
        _ => {}
    }
}

/// `ED_undo_operator_repeat`: the repeat check and poll on the current state, `ED_undo_pop_op`,
/// the operator again (`WM_operator_repeat`, which pushes its step like the first run), and
/// `ED_undo_redo` when it does not finish (CANCELLED).
fn undo_redo(session: &mut Session, ring: &mut UndoRing, r: &mut Response) {
    let Some(last) = ring
        .last_redo
        .clone()
        .filter(|l| ring.stack.has_undo_named(&l.label))
    else {
        return poll_failed(session, ring, r);
    };
    let before = session.host.state.clone();
    let polls = frap_op::lookup(&last.idname)
        .and_then(|def| frap_op::check_poll(def, &session.host.state.ctx()))
        .is_ok();
    let mut success = false;
    if polls {
        load(session, ring, |st, dec| st.undo_by_name(&last.label, dec));
        let rerun = match &last.rerun {
            Rerun::Stmt(s) => session.dispatch(s),
            Rerun::Invoke { idname, kwargs } => {
                let kw = kwargs
                    .iter()
                    .map(|(k, v)| (k.clone(), eval::Py::from_input(v.clone()), Span::default()))
                    .collect();
                session.invoke_values(r.line, idname, vec![], kw)
            }
        };
        match rerun {
            Ok(rr) if rr.status == Status::Finished => {
                success = true;
                r.reports.extend(rr.reports);
                push_for_operator(session, ring, &last.idname);
            }
            Ok(rr) => {
                r.reports.extend(rr.reports);
                load(session, ring, |st, dec| st.redo(dec));
            }
            Err(f) => {
                r.reports.extend(f.reports);
                r.reports.push(Report::new(
                    ReportType::Warning,
                    format!(
                        "frap: {} failed when repeated: {}",
                        last.idname, f.err.message
                    ),
                ));
                load(session, ring, |st, dec| st.redo(dec));
            }
        }
    }
    if !success {
        r.status = Status::Cancelled;
        r.result = None;
    }
    restored(
        session,
        &before,
        r,
        "repeated",
        success.then(|| last.idname.clone()),
    );
    if !success {
        r.changes = None;
    }
}
