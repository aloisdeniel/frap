// SPDX-License-Identifier: GPL-3.0-or-later
//! Operator-side helpers of the attribute operators: the context mesh
//! (`ed::object::context_object(C)->data`), the polls of
//! `editors/geometry/geometry_attributes.cc` (Blender 5.2.4, GPL-2.0-or-later), and
//! [`with_store`], which runs an edit on the mesh's attributes in Object Mode or on its edit
//! BMesh in Edit Mode and writes the mesh-level names back.
//!
//! frap's polls see `Main` and the window, not the edit BMeshes: a poll that depends on the
//! layers of an edit BMesh is approximated on the mesh in `Main` and repeated in `exec`, which
//! then raises the same `Operator bpy.ops.<idname>.poll() <message>` error
//! ([`exec_poll_error`]).

use frap_bmesh::BMesh;
use frap_mesh::Mesh;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId, PyExc, RnaError, RnaErrorKind};
use frap_scene::Context;
use smol_str::SmolStr;

use super::attrs::{Names, Store};

/// The active object and its mesh (`ED_mesh_context` / `context_object(C)->data`).
pub(crate) fn context_mesh(c: &Context) -> Option<(IdRef<ObjectId>, IdRef<MeshId>)> {
    let (ob, o) = poll::active(c)?;
    let me = o.data?.typed::<MeshId>()?;
    c.main.meshes.get(me)?;
    Some((ob, me))
}

/// Whether the mesh is in Edit Mode (has an edit BMesh).
pub(crate) fn in_edit(cx: &mut OpCtx, me: IdRef<MeshId>) -> bool {
    cx.split_edit().1.meshes.contains_key(&me.key())
}

/// `geometry_attributes_poll`: an editable active object whose data has attributes (frap: a
/// mesh).
pub(crate) fn attributes_poll(c: &Context) -> Result<(), PollFail> {
    poll::active_editable_mesh(c)
}

/// The requirements of [`attributes_poll`].
pub(crate) const ATTRIBUTES_REQUIRES: &[PollReq] = &[
    PollReq::ActiveObject,
    PollReq::ActiveEditable,
    PollReq::ActiveType("MESH"),
];

/// [`attributes_poll`] as a `Poll`.
pub(crate) const ATTRIBUTES_POLL: Poll = Poll {
    requires: ATTRIBUTES_REQUIRES,
    check: attributes_poll,
};

/// A poll failure with Blender's poll message (`CTX_wm_operator_poll_msg_set`):
/// `Operator bpy.ops.<idname>.poll() <msg>`.
pub(crate) fn poll_msg(idname: &str, msg: &str, hint: &str) -> PollFail {
    PollFail {
        message: Some(format!("Operator bpy.ops.{idname}.poll() {msg}")),
        unmet: smallvec::smallvec![],
        hint: Some(hint.into()),
    }
}

/// A poll failure without a message (`… poll() failed, context is incorrect`).
pub(crate) fn poll_fail(hint: &str) -> PollFail {
    PollFail {
        message: None,
        unmet: smallvec::smallvec![],
        hint: Some(hint.into()),
    }
}

/// Refuses Edit Mode (`Operation is not allowed in edit mode`).
pub(crate) fn not_in_edit(c: &Context, idname: &str) -> Result<(), PollFail> {
    if poll::edit_object(c).is_some() {
        return Err(poll_msg(
            idname,
            "Operation is not allowed in edit mode",
            "bpy.ops.object.mode_set(mode='OBJECT')",
        ));
    }
    Ok(())
}

/// The mesh in `Main` of the context object (for the polls).
pub(crate) fn context_mesh_data<'a>(c: &Context<'a>) -> Option<&'a Mesh> {
    let (_, me) = context_mesh(c)?;
    c.main.meshes.get(me)
}

/// Reads the names and the attributes of `me` (the edit BMesh in Edit Mode) without changing
/// anything.
pub(crate) fn read<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    f: impl FnOnce(&dyn Store, &Names) -> R,
) -> Option<R> {
    let (c, edit) = cx.split_edit();
    let m = c.main.meshes.get(me)?;
    let names = Names::of(m);
    match edit.meshes.get(&me.key()) {
        Some(em) => Some(f(&*em.bm as &BMesh, &names)),
        None => Some(f(m as &Mesh, &names)),
    }
}

/// The error a poll that needs the edit BMesh raises from `exec`:
/// `RuntimeError: Operator bpy.ops.<idname>.poll() <msg>`.
pub(crate) fn exec_poll_error(idname: &str, msg: &str) -> OpError {
    OpError::Rna(RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("Operator bpy.ops.{idname}.poll() {msg}"),
    ))
}

/// The error of a poll without a message, raised from `exec`.
pub(crate) fn exec_poll_failed(idname: &str) -> OpError {
    exec_poll_error(idname, "failed, context is incorrect")
}

/// Runs `f` on the attributes of `me` (Object Mode: the mesh; Edit Mode: its edit BMesh) with
/// the mesh-level names and the vertex-group names, then writes the names back (to the edit
/// BMesh's UV names too).
pub(crate) fn with_store<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    f: impl FnOnce(&mut dyn Store, &mut Names, &[SmolStr]) -> R,
) -> Result<R, OpError> {
    let edit = in_edit(cx, me);
    let (mut names, vgroups) = {
        let m = cx
            .main()
            .meshes
            .get(me)
            .ok_or_else(|| OpError::Runtime("mesh no longer exists".into()))?;
        (Names::of(m), m.vertex_group_names.clone())
    };
    if !edit {
        let m = cx.mesh_mut(me)?;
        let r = f(m, &mut names, &vgroups);
        names.write(m);
        return Ok(r);
    }
    let before = names.clone();
    let r = {
        let bm = cx.bmesh_mut(me)?;
        let r = f(bm, &mut names, &vgroups);
        names.write_bm(bm);
        r
    };
    if names != before {
        names.write(cx.mesh_mut(me)?);
    }
    Ok(r)
}
