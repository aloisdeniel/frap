// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared declarative poll library with Blender's `ED_operator_*` names (ARCHITECTURE §8).
//!
//! Each entry is a [`Poll`]: its `requires` are data (published by `--describe` and in
//! `error.detail.requires`), its check is a port of the Blender function named in its doc
//! (`source/blender/editors/screen/screen_ops.cc`, Blender 5.2.4, GPL-2.0-or-later) that reports
//! the failing requirements (`unmet`) and a `hint` naming the exact fix, e.g.
//! `bpy.ops.object.mode_set(mode='EDIT')`.
//!
//! Operators with their own poll compose the public check functions (`objectmode`,
//! `active_object`, …) in a `const` `Poll` of their file; `#[operator(poll = …)]` names it.

use frap_rna::{IdRef, ObjectId};
use frap_scene::Context;
use frap_scene::ObjectMode;
use frap_scene::id::Object;
use smallvec::smallvec;

use crate::def::{Poll, PollFail, PollReq};

// ------------------------------------------------------------------------------- helpers

/// Python literal of a name (`'Cube'`, or `"it's"` when it holds a quote).
pub fn py_name(name: &str) -> String {
    if name.contains('\'') && !name.contains('"') {
        format!("\"{name}\"")
    } else {
        format!("'{}'", name.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

/// `bpy.data.objects['Cube']`
fn object_path(name: &str) -> String {
    format!("bpy.data.objects[{}]", py_name(name))
}

/// A failure with one unmet requirement and a hint.
pub fn fail(req: PollReq, hint: impl Into<String>) -> PollFail {
    PollFail {
        message: None,
        unmet: smallvec![req],
        hint: Some(hint.into()),
    }
}

/// The active object of the context view layer (`CTX_data_active_object`).
pub fn active<'a>(c: &Context<'a>) -> Option<(IdRef<ObjectId>, &'a Object)> {
    c.active_object()
        .and_then(|r| c.main.objects.get(r).map(|o| (r, o)))
}

/// The object in Edit Mode (`CTX_data_edit_object`: the active object, when in Edit Mode).
pub fn edit_object<'a>(c: &Context<'a>) -> Option<(IdRef<ObjectId>, &'a Object)> {
    active(c).filter(|(_, o)| o.mode == ObjectMode::Edit)
}

/// `ed_object_hidden`: hidden in the viewport and not in Edit Mode.
fn hidden(ob: &Object) -> bool {
    ob.hide_viewport && ob.mode != ObjectMode::Edit
}

/// `ID_IS_EDITABLE` of an object and of its data (local, not linked from a library).
fn editable(c: &Context, ob: &Object) -> bool {
    ob.header.library.is_none()
        && ob
            .data
            .and_then(|d| c.main.header(d))
            .is_none_or(|h| h.library.is_none())
}

/// The object of this type ("" = any) to suggest: the first selected one, else the first in
/// base order of the context view layer.
fn first_of_type<'a>(c: &Context<'a>, ty: &str) -> Option<&'a Object> {
    let bases = &c.view_layer().bases;
    let of_type = |sel: bool| {
        bases
            .iter()
            .filter(move |b| !sel || b.selected())
            .filter_map(|b| c.main.objects.get(b.object))
            .find(|o| ty.is_empty() || o.object_type() == ty)
    };
    of_type(true).or_else(|| of_type(false))
}

/// `bpy.context.view_layer.objects.active = bpy.data.objects['X']` for the first object of `ty`
/// (or how to add one).
fn make_active_hint(c: &Context, ty: &str) -> String {
    match first_of_type(c, ty) {
        Some(o) => format!(
            "bpy.context.view_layer.objects.active = {}",
            object_path(&o.header.name)
        ),
        None => match ty {
            "MESH" | "" => "add one first, e.g. bpy.ops.mesh.primitive_cube_add()".into(),
            "ARMATURE" => "add one first, e.g. bpy.ops.object.armature_add()".into(),
            "CAMERA" => "add one first, e.g. bpy.ops.object.camera_add()".into(),
            other => format!("add an object of type '{other}' first"),
        },
    }
}

/// Hint that puts the context into `mode` (`'EDIT'`, `'POSE'`) on an object of type `ty`.
fn enter_mode_hint(c: &Context, ty: &str, mode: &str) -> String {
    let set = format!("bpy.ops.object.mode_set(mode='{mode}')");
    match active(c) {
        Some((_, ob)) if ob.object_type() == ty => {
            if ob.mode != ObjectMode::Object && ob.mode.as_str() != mode {
                format!("bpy.ops.object.mode_set(mode='OBJECT'); {set}")
            } else {
                set
            }
        }
        Some((_, ob)) if ob.mode != ObjectMode::Object => format!(
            "bpy.ops.object.mode_set(mode='OBJECT'); {}; {set}",
            make_active_hint(c, ty)
        ),
        _ => {
            let make = make_active_hint(c, ty);
            if make.starts_with("add ") {
                make
            } else {
                format!("{make}; {set}")
            }
        }
    }
}

// ------------------------------------------------------------------------------- checks

fn ok(_: &Context) -> Result<(), PollFail> {
    Ok(())
}
/// Always passes (primitive add operators: poll matrix all Y).
pub const ALWAYS: Poll = Poll {
    requires: &[],
    check: ok,
};

/// `ED_operator_scene_editable`: the context scene is local.
pub fn scene_editable(c: &Context) -> Result<(), PollFail> {
    let scene = c.scene();
    if scene.header.library.is_none() {
        Ok(())
    } else {
        Err(fail(
            PollReq::SceneEditable,
            format!(
                "bpy.data.scenes[{}].make_local()",
                py_name(&scene.header.name)
            ),
        ))
    }
}
/// `ED_operator_scene_editable`
pub const SCENE_EDITABLE: Poll = Poll {
    requires: &[PollReq::SceneEditable],
    check: scene_editable,
};

/// `ED_operator_objectmode`: editable scene, no object in Edit Mode, the active object (if any)
/// in Object Mode.
pub fn objectmode(c: &Context) -> Result<(), PollFail> {
    scene_editable(c)?;
    match active(c) {
        Some((_, ob)) if ob.mode != ObjectMode::Object => Err(fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        )),
        _ => Ok(()),
    }
}
/// `ED_operator_objectmode`
pub const OBJECTMODE: Poll = Poll {
    requires: &[PollReq::SceneEditable, PollReq::ModeObject],
    check: objectmode,
};

/// `ED_operator_object_active`: an active object that is not hidden (`ed_object_hidden`).
pub fn active_object(c: &Context) -> Result<(), PollFail> {
    match active(c) {
        None => Err(fail(PollReq::ActiveObject, make_active_hint(c, ""))),
        Some((_, ob)) if hidden(ob) => Err(fail(
            PollReq::ActiveObject,
            format!("{}.hide_viewport = False", object_path(&ob.header.name)),
        )),
        Some(_) => Ok(()),
    }
}
/// `ED_operator_object_active`
pub const ACTIVE_OBJECT: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: active_object,
};

/// `ED_operator_object_active_editable`: [`active_object`] and the object is local.
pub fn active_editable(c: &Context) -> Result<(), PollFail> {
    active_object(c)?;
    let (_, ob) = active(c).expect("invariant: checked by active_object");
    if ob.header.library.is_none() {
        Ok(())
    } else {
        Err(fail(
            PollReq::ActiveEditable,
            format!("{}.make_local()", object_path(&ob.header.name)),
        ))
    }
}
/// `ED_operator_object_active_editable`
pub const ACTIVE_EDITABLE: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: active_editable,
};
/// `ED_operator_object_active_local_editable` (same check in frap: no library overrides).
pub const ACTIVE_LOCAL_EDITABLE: Poll = ACTIVE_EDITABLE;

/// `ED_operator_object_active_editable_mesh`: [`active_object`], object and data local, type
/// MESH.
pub fn active_editable_mesh(c: &Context) -> Result<(), PollFail> {
    active_object(c)?;
    let (_, ob) = active(c).expect("invariant: checked by active_object");
    if ob.object_type() != "MESH" {
        return Err(fail(
            PollReq::ActiveType("MESH"),
            make_active_hint(c, "MESH"),
        ));
    }
    if !editable(c, ob) {
        return Err(fail(
            PollReq::ActiveEditable,
            format!("{}.make_local()", object_path(&ob.header.name)),
        ));
    }
    Ok(())
}
/// `ED_operator_object_active_editable_mesh`
pub const ACTIVE_EDITABLE_MESH: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: active_editable_mesh,
};

/// `ED_operator_editmesh`: the edit object is a mesh (with an edit BMesh).
pub fn editmesh(c: &Context) -> Result<(), PollFail> {
    match edit_object(c) {
        Some((_, ob)) if ob.object_type() == "MESH" => Ok(()),
        _ => Err(fail(
            PollReq::ModeEditMesh,
            enter_mode_hint(c, "MESH", "EDIT"),
        )),
    }
}
/// `ED_operator_editmesh`
pub const EDITMESH: Poll = Poll {
    requires: &[PollReq::ModeEditMesh],
    check: editmesh,
};
/// `ED_operator_editmesh_view3d` / `ED_operator_editmesh_region_view3d`: frap always has its
/// virtual 3D View (deviation D-10), so this equals [`EDITMESH`].
pub const EDITMESH_VIEW3D: Poll = EDITMESH;

/// `ED_operator_posemode`: the active object is an armature in Pose Mode (not in Edit Mode).
pub fn posemode(c: &Context) -> Result<(), PollFail> {
    match active(c) {
        Some((_, ob)) if ob.object_type() == "ARMATURE" && ob.mode == ObjectMode::Pose => Ok(()),
        _ => Err(fail(
            PollReq::ModePose,
            enter_mode_hint(c, "ARMATURE", "POSE"),
        )),
    }
}
/// `ED_operator_posemode`
pub const POSEMODE: Poll = Poll {
    requires: &[PollReq::ModePose],
    check: posemode,
};

/// `ED_operator_editarmature`: the edit object is an armature.
pub fn editarmature(c: &Context) -> Result<(), PollFail> {
    match edit_object(c) {
        Some((_, ob)) if ob.object_type() == "ARMATURE" => Ok(()),
        _ => Err(fail(
            PollReq::ModeEditArmature,
            enter_mode_hint(c, "ARMATURE", "EDIT"),
        )),
    }
}
/// `ED_operator_editarmature`
pub const EDITARMATURE: Poll = Poll {
    requires: &[PollReq::ModeEditArmature],
    check: editarmature,
};

/// At least one selected object (`CTX_data_selected_objects` not empty; used by operators whose
/// Blender poll is inline, e.g. `object.join`'s).
pub fn has_selected_objects(c: &Context) -> Result<(), PollFail> {
    if !c.selected_objects().is_empty() {
        return Ok(());
    }
    let hint = match active(c).map(|(_, o)| o).or_else(|| first_of_type(c, "")) {
        Some(o) => format!("{}.select_set(True)", object_path(&o.header.name)),
        None => "add one first, e.g. bpy.ops.mesh.primitive_cube_add()".into(),
    };
    Err(fail(PollReq::HasSelectedObjects, hint))
}
/// At least one selected object (frap composition; no `ED_operator_*` of its own).
pub const SELECTED_OBJECTS: Poll = Poll {
    requires: &[PollReq::HasSelectedObjects],
    check: has_selected_objects,
};

/// `scene.camera` is set.
pub fn scene_has_camera(c: &Context) -> Result<(), PollFail> {
    if c.scene().camera.is_some() {
        return Ok(());
    }
    let hint = match first_of_type(c, "CAMERA") {
        Some(o) => format!("bpy.context.scene.camera = {}", object_path(&o.header.name)),
        None => "bpy.ops.object.camera_add(); bpy.context.scene.camera = bpy.context.object".into(),
    };
    Err(fail(PollReq::SceneHasCamera, hint))
}
/// `scene.camera` is set (frap composition; used by operators that need the scene camera).
pub const SCENE_HAS_CAMERA: Poll = Poll {
    requires: &[PollReq::SceneHasCamera],
    check: scene_has_camera,
};

/// Every named poll of the library (`--describe`, tests).
pub const LIBRARY: &[(&str, Poll)] = &[
    ("ALWAYS", ALWAYS),
    ("SCENE_EDITABLE", SCENE_EDITABLE),
    ("OBJECTMODE", OBJECTMODE),
    ("ACTIVE_OBJECT", ACTIVE_OBJECT),
    ("ACTIVE_EDITABLE", ACTIVE_EDITABLE),
    ("ACTIVE_LOCAL_EDITABLE", ACTIVE_LOCAL_EDITABLE),
    ("ACTIVE_EDITABLE_MESH", ACTIVE_EDITABLE_MESH),
    ("EDITMESH", EDITMESH),
    ("EDITMESH_VIEW3D", EDITMESH_VIEW3D),
    ("POSEMODE", POSEMODE),
    ("EDITARMATURE", EDITARMATURE),
    ("SELECTED_OBJECTS", SELECTED_OBJECTS),
    ("SCENE_HAS_CAMERA", SCENE_HAS_CAMERA),
];
