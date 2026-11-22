// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_copy_to_selected(modifier='')`, tier T0
//! (`tests/golden/cases/object/data_m8/modifiers_copy.py`). Port of
//! `modifier_copy_to_selected_exec`, `modifier_copy_to_selected_poll` and
//! `ed::object::modifier_copy_to_object` (`editors/object/object_modifier.cc`),
//! `BKE_object_copy_modifier`, `BKE_object_supports_modifiers` and
//! `BKE_object_support_modifier_type_check` (`blenkernel/intern/object.cc`),
//! `BKE_modifier_copydata_ex` and `BKE_modifiers_add_at_end_if_possible`
//! (`blenkernel/intern/modifier.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (no `modifier` context pointer, as from a script):
//! - poll: an active object (`No selected object is active`) of a type with modifiers (`Object
//!   type of source object is not supported`; meshes, point clouds, curves, empties) and
//!   another selected object of such a type (`No supported objects were selected`);
//! - the active object's modifier `modifier` (none: `CANCELLED`) is copied to every other
//!   selected local object: settings, show flags, pin and active flag, named uniquely
//!   (`Subdivision.001` next to a `Subdivision`), at the end of the stack (before the pinned
//!   run; after the leading deform-only run for types that need original data and Hook); the
//!   copy becomes the target's active modifier;
//! - a target that does not take the type: WARNING `Object 'X' does not support Subdivision
//!   modifiers`; a second single-instance modifier: WARNING `Modifier can only be added once to
//!   object 'X'`; Hook and Collision are never copied: ERROR `Copying modifier 'Hook' to object
//!   'X' failed`;
//! - nothing copied: ERROR `Modifier 'X' was not copied to any objects` and `CANCELLED`, which
//!   `bpy.ops` raises as one `RuntimeError` with every ERROR line.
//!
//! The copy's `persistent_uid` is one more than the largest in use (D-47). Copying a particle
//! system (`object_copy_particle_system`) is not supported: particle systems are out of scope.
//!
//! This file also holds [`copy_to_object`], shared with `object.modifiers_copy_to_selected`.

use frap_modifiers::stack::edit;
use frap_modifiers::{ModTypeFlags, ModifierType, registry};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, ObjectId};
use frap_scene::Context;
use frap_scene::id::Modifier;
use frap_spec_ops_object::object;

use super::modifier_add::shared;

/// `BKE_object_supports_modifiers` for frap's object types (Blender 5.2.4 also lists
/// `OB_EMPTY`, which takes geometry-nodes modifiers).
pub(crate) fn supports_modifiers(object_type: &str) -> bool {
    matches!(object_type, "MESH" | "POINTCLOUD" | "CURVES" | "EMPTY")
}

/// Why [`copy_to_object`] copied nothing: Blender's report, WARNING or ERROR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CopyRefused {
    /// A WARNING report.
    Warning(String),
    /// An ERROR report (`Copying modifier '…' to object '…' failed`).
    Error(String),
}

/// `ed::object::modifier_copy_to_object` + `BKE_object_copy_modifier`: a copy of the source
/// object's modifier `md` added to `dst`. Returns the copy's index; the target's active
/// modifier is left to the caller (`BKE_object_modifier_set_active`). A particle system answers
/// `OP_NOT_IMPLEMENTED`.
pub(crate) fn copy_to_object(
    md: &Modifier,
    dst: &mut frap_scene::id::Object,
) -> Result<Result<usize, CopyRefused>, OpError> {
    let id = md.settings.type_identifier();
    let Some(ty) = ModifierType::from_identifier(id) else {
        return Err(OpError::NotImplemented(format!(
            "frap: modifier type '{id}' cannot be copied"
        )));
    };
    let meta = registry::meta(ty);
    let dst_name = dst.header.name.clone();
    if !registry::object_supports(dst.object_type(), ty) {
        return Ok(Err(CopyRefused::Warning(format!(
            "Object '{dst_name}' does not support {} modifiers",
            meta.name
        ))));
    }
    let has_type = |o: &frap_scene::id::Object| {
        o.modifiers
            .iter()
            .any(|m| m.settings.type_identifier() == id)
    };
    if meta.flags.contains(ModTypeFlags::SINGLE) && has_type(dst) {
        return Ok(Err(CopyRefused::Warning(format!(
            "Modifier can only be added once to object '{dst_name}'"
        ))));
    }
    // `object_modifier_type_copy_check`.
    if matches!(ty, ModifierType::Hook | ModifierType::Collision) {
        return Ok(Err(CopyRefused::Error(format!(
            "Copying modifier '{}' to object '{dst_name}' failed",
            md.name
        ))));
    }
    if ty == ModifierType::ParticleSystem {
        return Err(OpError::NotImplemented(format!(
            "frap: copying the particle system modifier '{}' is not supported (particle \
             systems are out of scope)",
            md.name
        )));
    }
    // `BKE_modifier_new` + `BKE_modifier_copydata` (mode, flag incl. the active and pin
    // bits, UI expansion, settings, ID users), placed by `BKE_modifiers_add_at_end_if_possible`
    // and named by `BKE_modifier_unique_name`. `edit::add` places and names a fresh modifier
    // of the type the same way, then the copied data replaces it; the other modifiers keep
    // their active flags (`edit::add` makes the new one active).
    let was_active: Vec<bool> = dst.modifiers.iter().map(|m| m.is_active).collect();
    let at = edit::add(dst, Some(md.name.as_str()), ty)
        .map_err(|e| OpError::Runtime(format!("Copying modifier '{}' failed: {e}", md.name)))?;
    let fresh = &dst.modifiers[at];
    let (name, uid) = (fresh.name.clone(), fresh.persistent_uid);
    let mut copy = md.clone();
    copy.name = name;
    copy.persistent_uid = uid;
    dst.modifiers[at] = copy;
    let mut flags = was_active;
    flags.insert(at, md.is_active);
    for (m, a) in dst.modifiers.iter_mut().zip(flags) {
        m.is_active = a;
    }
    Ok(Ok(at))
}

/// Reports a refused copy (WARNINGs now; ERRORs are collected by the caller).
pub(crate) fn report_refused(cx: &mut OpCtx, r: CopyRefused, errors: &mut Vec<String>) {
    match r {
        CopyRefused::Warning(m) => cx.report(ReportType::Warning, m),
        CopyRefused::Error(m) => errors.push(m),
    }
}

fn poll_fail(why: &str, req: PollReq, hint: &str) -> PollFail {
    let mut f = poll::fail(req, hint);
    f.message = Some(shared::poll_message("modifier_copy_to_selected", why));
    f
}

/// `modifier_copy_to_selected_poll` (no `modifier` context pointer: the modifier type is not
/// checked).
fn copy_to_selected_poll(c: &Context) -> Result<(), PollFail> {
    let Some((act, ob)) = poll::active(c) else {
        return Err(poll_fail(
            "No selected object is active",
            PollReq::ActiveObject,
            "bpy.context.view_layer.objects.active = bpy.data.objects[…]",
        ));
    };
    if !supports_modifiers(ob.object_type()) {
        return Err(poll_fail(
            "Object type of source object is not supported",
            PollReq::ActiveType("MESH"),
            "make an object with modifiers (a mesh) active",
        ));
    }
    let found = c.selected_objects().into_iter().any(|o| {
        o != act
            && c.main
                .objects
                .get(o)
                .is_some_and(|x| supports_modifiers(x.object_type()))
    });
    if !found {
        return Err(poll_fail(
            "No supported objects were selected",
            PollReq::HasSelectedObjects,
            "select the objects to copy to (bpy.data.objects['Name'].select_set(True))",
        ));
    }
    Ok(())
}

/// `modifier_copy_to_selected_poll`.
const COPY_TO_SELECTED_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::HasSelectedObjects],
    check: copy_to_selected_poll,
};

/// The selected objects a copy goes to: every selected object but `act`, local
/// (`ID_IS_EDITABLE`), in base order.
pub(crate) fn targets(
    c: &Context,
    act: IdRef<ObjectId>,
    editable_only: bool,
) -> Vec<IdRef<ObjectId>> {
    c.selected_objects()
        .into_iter()
        .filter(|&o| o != act)
        .filter(|&o| {
            !editable_only
                || c.main
                    .objects
                    .get(o)
                    .is_some_and(|x| x.header.library.is_none())
        })
        .collect()
}

#[operator(
    poll = COPY_TO_SELECTED_POLL,
    tier = T0,
    notes = [
        "persistent_uid is one more than the largest in use (D-47)",
        "a particle system modifier is not copied (OP_NOT_IMPLEMENTED: particle systems are out of scope)",
    ]
)]
fn exec(cx: &mut OpCtx, p: object::ModifierCopyToSelected) -> OpResult {
    let Some(act) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(md) = cx
        .main()
        .objects
        .get(act)
        .and_then(|o| shared::find(o, &p.modifier.value).map(|i| o.modifiers[i].clone()))
    else {
        shared::report_missing(cx, &[act], &p.modifier.value);
        return Ok(OpStatus::Cancelled);
    };
    let targets = targets(&cx.ctx(), act, true);
    let mut errors: Vec<String> = Vec::new();
    let mut copied = 0usize;
    for ob in targets {
        let dst = cx.object_mut(ob)?;
        match copy_to_object(&md, dst)? {
            Ok(i) => {
                edit::set_active(&mut dst.modifiers, Some(i));
                copied += 1;
            }
            Err(r) => report_refused(cx, r, &mut errors),
        }
    }
    if copied == 0 {
        errors.push(format!(
            "Modifier '{}' was not copied to any objects",
            md.name
        ));
        return Err(OpError::Runtime(errors.join("\nError: ")));
    }
    if !errors.is_empty() {
        // A target refused a Hook or Collision copy that another took: impossible (both are
        // refused everywhere), kept for the general rule (D-158).
        return Err(OpError::Runtime(errors.join("\nError: ")));
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::{IdKey, IdRef, MeshId};
    use frap_scene::id::Object;

    fn mesh_object(name: &str) -> Object {
        let me = IdRef::<MeshId>::from_key(IdKey::next());
        Object::new(name, Some(me.into()))
    }

    #[test]
    fn copies_keep_settings_and_get_unique_names() {
        let mut src = mesh_object("Cube");
        edit::add(&mut src, Some("Sub"), ModifierType::Subsurf).unwrap();
        src.modifiers[0].show_viewport = false;
        let mut dst = mesh_object("Sphere");
        edit::add(&mut dst, Some("Sub"), ModifierType::Subsurf).unwrap();
        edit::add(&mut dst, Some("Mirror"), ModifierType::Mirror).unwrap();
        edit::set_active(&mut dst.modifiers, Some(0));
        let i = copy_to_object(&src.modifiers[0], &mut dst)
            .unwrap()
            .unwrap();
        assert_eq!(i, 2);
        let names: Vec<&str> = dst.modifiers.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Sub", "Mirror", "Sub.001"]);
        assert!(!dst.modifiers[2].show_viewport);
        // The source was active: the flag is copied; the target's own active one stays.
        let active: Vec<bool> = dst.modifiers.iter().map(|m| m.is_active).collect();
        assert_eq!(active, [true, false, true]);
        assert_ne!(
            dst.modifiers[2].persistent_uid,
            dst.modifiers[0].persistent_uid
        );
    }

    #[test]
    fn refusals_follow_blender() {
        let mut src = mesh_object("Cube");
        edit::add(&mut src, Some("Hook"), ModifierType::Hook).unwrap();
        edit::add(&mut src, Some("Cloth"), ModifierType::Cloth).unwrap();
        edit::add(&mut src, Some("Sub"), ModifierType::Subsurf).unwrap();
        let mut dst = mesh_object("Sphere");
        assert_eq!(
            copy_to_object(&src.modifiers[0], &mut dst).unwrap(),
            Err(CopyRefused::Error(
                "Copying modifier 'Hook' to object 'Sphere' failed".into()
            ))
        );
        assert!(copy_to_object(&src.modifiers[1], &mut dst).unwrap().is_ok());
        assert_eq!(
            copy_to_object(&src.modifiers[1], &mut dst).unwrap(),
            Err(CopyRefused::Warning(
                "Modifier can only be added once to object 'Sphere'".into()
            ))
        );
        let mut empty = Object::new("Empty", None);
        assert_eq!(
            copy_to_object(&src.modifiers[2], &mut empty).unwrap(),
            Err(CopyRefused::Warning(
                "Object 'Empty' does not support Subdivision modifiers".into()
            ))
        );
    }
}
