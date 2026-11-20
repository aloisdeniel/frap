// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.mode_set(mode=…, toggle=False)`, tier T0: the mode-switching dispatcher
//! (ARCHITECTURE §8, owned once). Port of `object_mode_set_exec`, `object_mode_set_poll`,
//! `object_mode_set_itemf` and `ED_object_mode_compat_test` (`editors/object/object_modes.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! The switch itself is delegated to the `MODE_HANDLERS` slice (`frap_op::ModeHandler`): the
//! handler of the current mode leaves it, the handler of the requested mode enters it.
//! `modes/object.rs` (M1) is the only handler so far; `modes/edit_mesh.rs` (M2) and
//! `modes/{edit_armature,pose}.rs` (M6) add files, never edit this one. A mode without a
//! handler answers `NotImplementedError` (`OP_NOT_IMPLEMENTED`) naming the mode, before
//! anything changes. Verified on 5.2.4:
//! - poll: an active, local, not hidden (`hide_viewport`) object, with Blender's messages
//!   `Context missing active object`, `Cannot edit library linked object`, `Cannot edit hidden
//!   object`;
//! - `mode` items follow the active object's type (mesh: `OBJECT EDIT SCULPT VERTEX_PAINT
//!   WEIGHT_PAINT TEXTURE_PAINT`; armature: `OBJECT EDIT POSE`; camera, light, empty: `OBJECT`
//!   only), so `mode='POSE'` on a mesh is a `TypeError` listing them;
//! - the current mode → `FINISHED`, nothing changes; `toggle=True` in the requested mode
//!   goes back to Object Mode (frap keeps no `restore_mode` other than `OBJECT`, the only one
//!   reachable before M2).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, ModeHandler, Poll, PollFail, PollReq};
use frap_rna::{IdAny, IdRef, ObjectId, PropKind, PyExc, RnaError, RnaErrorKind};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;

/// `object_mode_set_poll` → `ED_operator_object_active_editable_ex`, with its poll messages.
fn active_editable(c: &Context) -> Result<(), PollFail> {
    poll::active_editable(c).map_err(|mut f| {
        let why = match poll::active(c) {
            None => "Context missing active object",
            Some((_, ob)) if ob.header.library.is_some() => "Cannot edit library linked object",
            Some(_) => "Cannot edit hidden object",
        };
        f.message = Some(format!("Operator bpy.ops.object.mode_set.poll() {why}"));
        f
    })
}

/// `object_mode_set_poll` (the legacy grease-pencil exception does not apply: frap has no
/// legacy grease pencil data).
const ACTIVE_EDITABLE: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: active_editable,
};

/// `ED_object_mode_compat_test`: can an object of `object_type` be in `mode`? (Particle edit
/// needs particle systems, which frap does not have.)
fn compatible(object_type: &str, mode: &str) -> bool {
    if mode == "OBJECT" {
        return true;
    }
    let modes: &[&str] = match object_type {
        "MESH" => &[
            "EDIT",
            "SCULPT",
            "VERTEX_PAINT",
            "WEIGHT_PAINT",
            "TEXTURE_PAINT",
        ],
        "CURVE" | "SURFACE" | "FONT" | "META" | "LATTICE" | "POINTCLOUD" => &["EDIT"],
        "ARMATURE" => &["EDIT", "POSE"],
        "CURVES" => &["EDIT", "SCULPT_CURVES"],
        "GREASEPENCIL" => &[
            "EDIT",
            "SCULPT_GREASE_PENCIL",
            "PAINT_GREASE_PENCIL",
            "WEIGHT_GREASE_PENCIL",
            "VERTEX_GREASE_PENCIL",
        ],
        _ => &[],
    };
    modes.contains(&mode)
}

/// `object_mode_set_itemf`: the modes of the active object's type, in RNA order (`OBJECT` only
/// without an active object).
fn mode_items(c: &Context) -> Vec<EnumItemOwned> {
    let PropKind::Enum { items, .. } = &object::MODE_SET_PROPS[0].kind else {
        return Vec::new();
    };
    let ty = poll::active(c).map(|(_, o)| o.object_type());
    items
        .iter()
        .filter(|i| match ty {
            Some(t) => compatible(t, i.identifier),
            None => i.identifier == "OBJECT",
        })
        .map(|i| EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

/// The objects in `mode` with the active object's type: the active one first, then the others
/// in base order (`editmode_toggle`'s exit loop runs over every object of the view layer).
fn objects_in_mode(
    c: &Context,
    active: IdRef<ObjectId>,
    ty: &str,
    mode: ObjectMode,
) -> Vec<IdRef<ObjectId>> {
    let mut out = vec![active];
    for b in &c.view_layer().bases {
        if b.object != active
            && c.main
                .objects
                .get(b.object)
                .is_some_and(|o| o.mode == mode && o.object_type() == ty)
        {
            out.push(b.object);
        }
    }
    out
}

/// The objects entering a mode: the active one, then every selected (visible) local object of
/// its type, each object data once (multi-object editing, `editmode_toggle`).
fn objects_to_enter(c: &Context, active: IdRef<ObjectId>, ty: &str) -> Vec<IdRef<ObjectId>> {
    let data_of = |r: IdRef<ObjectId>| c.main.objects.get(r).and_then(|o| o.data);
    let mut seen: Vec<Option<IdAny>> = vec![data_of(active)];
    let mut out = vec![active];
    for r in c.selected_objects() {
        let Some(o) = c.main.objects.get(r) else {
            continue;
        };
        if r == active || o.object_type() != ty || o.header.library.is_some() {
            continue;
        }
        if o.data.is_some() && seen.contains(&o.data) {
            continue;
        }
        seen.push(o.data);
        out.push(r);
    }
    out
}

/// Runs one handler step; a cancelled step cancels the operator.
fn step(
    cx: &mut OpCtx,
    f: fn(&mut OpCtx, &[IdRef<ObjectId>]) -> OpResult,
    objects: &[IdRef<ObjectId>],
) -> Result<bool, OpError> {
    Ok(matches!(f(cx, objects)?, OpStatus::Finished))
}

#[operator(
    poll = ACTIVE_EDITABLE,
    tier = T0,
    support = Partial("mode handlers exist for OBJECT, EDIT (meshes and armatures) and POSE (armatures); other modes answer OP_NOT_IMPLEMENTED"),
    notes = [
        "mode items follow the active object's type (OBJECT only for cameras, lights and empties)",
        "toggle=True in the requested mode goes back to OBJECT",
    ],
    dyn_enums = [("mode", mode_items)]
)]
fn exec(cx: &mut OpCtx, p: object::ModeSet) -> OpResult {
    let (active, ty, current) = {
        let c = cx.ctx();
        let (r, ob) = poll::active(&c)
            .ok_or_else(|| OpError::Runtime("Context missing active object".into()))?;
        (r, ob.object_type(), ob.mode)
    };
    let requested = p.mode.value.as_str();
    if !compatible(ty, requested) {
        // Blender validates `mode` against `object_mode_set_itemf` while converting the
        // arguments (the session does too, with the context). A caller that converts without
        // a context (static superset) gets the same check here, with Blender's message.
        let items = mode_items(&cx.ctx());
        let list = items
            .iter()
            .map(|i| format!("'{}'", i.identifier))
            .collect::<Vec<_>>()
            .join(", ");
        let mut e = RnaError::new(
            RnaErrorKind::Enum,
            PyExc::TypeError,
            format!(
                "Converting py args to operator properties: enum \"{requested}\" not found in ({list})"
            ),
        );
        e.detail.insert(
            "valid".into(),
            items
                .iter()
                .map(|i| i.identifier.as_str())
                .collect::<Vec<_>>()
                .into(),
        );
        return Err(OpError::Rna(e));
    }
    let target = match requested {
        "OBJECT" => ObjectMode::Object,
        "EDIT" => ObjectMode::Edit,
        "POSE" => ObjectMode::Pose,
        other => {
            return Err(OpError::NotImplemented(format!(
                "frap: mode '{other}' is not implemented for {ty} objects yet (no mode handler)"
            )));
        }
    };
    let target = if p.toggle.value && current == target {
        ObjectMode::Object
    } else {
        target
    };
    if current == target {
        return Ok(OpStatus::Finished);
    }
    // Both handlers first: a missing one fails before anything changes.
    let leave: &ModeHandler = helpers::mode_handler(current, ty)?;
    let enter: &ModeHandler = helpers::mode_handler(target, ty)?;
    let (leaving, entering) = {
        let c = cx.ctx();
        let in_current = objects_in_mode(&c, active, ty, current);
        if target == ObjectMode::Object {
            (in_current.clone(), in_current)
        } else if current == ObjectMode::Object {
            let e = objects_to_enter(&c, active, ty);
            (e.clone(), e)
        } else {
            (in_current, objects_to_enter(&c, active, ty))
        }
    };
    if !step(cx, leave.exit, &leaving)? || !step(cx, enter.enter, &entering)? {
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_modes_match_blender() {
        assert!(compatible("CAMERA", "OBJECT"));
        assert!(!compatible("CAMERA", "EDIT"));
        assert!(compatible("MESH", "SCULPT"));
        assert!(!compatible("MESH", "POSE"));
        assert!(!compatible("MESH", "PARTICLE_EDIT"));
        assert!(compatible("ARMATURE", "POSE"));
        assert!(!compatible("LATTICE", "WEIGHT_PAINT"));
        assert!(compatible("CURVES", "SCULPT_CURVES"));
        assert!(!compatible("EMPTY", "EDIT"));
    }
}
