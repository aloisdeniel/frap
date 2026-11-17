// SPDX-License-Identifier: GPL-3.0-or-later
//! The declarative poll library (§8): for every poll, `requires` is data, a failing context
//! reports the failing requirements in `unmet` (a subset of `requires`) and a `hint` with the
//! exact fix; a passing context passes. `check_poll` turns failures into Blender's error.

mod common;

use common::TestHost;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq, check_poll, errors};
use frap_rna::{IdKey, IdRef, PyExc};
use frap_scene::id::{Armature, Object};
use frap_scene::{HostRead, ObjectMode};

fn check(h: &TestHost, p: Poll) -> Result<(), PollFail> {
    (p.check)(&h.state.ctx())
}

fn fails(h: &TestHost, p: Poll, unmet: &[PollReq], hint: &str) {
    let f = check(h, p).expect_err("poll must fail");
    assert_eq!(f.unmet.as_slice(), unmet);
    assert!(
        f.unmet.iter().all(|u| p.requires.contains(u)),
        "unmet ⊆ requires"
    );
    assert_eq!(f.hint.as_deref(), Some(hint));
    assert_eq!(f.message, None);
}

fn linked() -> Option<IdRef<frap_rna::LibraryId>> {
    Some(IdRef::from_key(IdKey::next()))
}

/// Adds an armature object `Armature` in `mode` and makes it active.
fn add_armature(h: &mut TestHost, mode: ObjectMode) {
    let log = &mut h.log;
    let main = &mut h.state.main;
    let arm = main.armatures.insert("Armature", Armature::new(""), log);
    let mut ob = Object::new("Armature", Some(arm.any()));
    ob.mode = mode;
    let ob = main.objects.insert("Armature", ob, log);
    let scene = h.state.win.scene;
    let s = main.scenes.get_mut(scene, log).unwrap();
    s.collection.objects.push(ob);
    for vl in &mut s.view_layers {
        vl.bases.push(frap_scene::Base {
            object: ob,
            flag: frap_scene::BaseFlag::SELECTED,
        });
        vl.active = Some(ob);
    }
}

#[test]
fn requires_are_published_as_codes() {
    assert!(poll::ALWAYS.requires.is_empty());
    let codes = |p: Poll| p.requires.iter().map(|r| r.code()).collect::<Vec<_>>();
    assert_eq!(codes(poll::EDITMESH), ["MODE_EDIT_MESH"]);
    assert_eq!(codes(poll::OBJECTMODE), ["SCENE_EDITABLE", "MODE_OBJECT"]);
    assert_eq!(
        codes(poll::ACTIVE_EDITABLE_MESH),
        ["ACTIVE_OBJECT", "ACTIVE_EDITABLE", "ACTIVE_TYPE_MESH"]
    );
    assert_eq!(codes(poll::POSEMODE), ["MODE_POSE"]);
    assert_eq!(codes(poll::EDITARMATURE), ["MODE_EDIT_ARMATURE"]);
    assert_eq!(PollReq::ActiveType("MESH").to_string(), "ACTIVE_TYPE_MESH");
    assert_eq!(
        PollReq::ModeEditMesh.text(),
        "bpy.context.mode == 'EDIT_MESH'"
    );
    // Every library entry is named once.
    let mut names: Vec<_> = poll::LIBRARY.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), poll::LIBRARY.len());
}

#[test]
fn always() {
    let h = TestHost::factory();
    assert!(check(&h, poll::ALWAYS).is_ok());
}

#[test]
fn scene_editable() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::SCENE_EDITABLE).is_ok());
    let scene = h.state.win.scene;
    h.state
        .main
        .scenes
        .get_mut(scene, &mut h.log)
        .unwrap()
        .header
        .library = linked();
    fails(
        &h,
        poll::SCENE_EDITABLE,
        &[PollReq::SceneEditable],
        "bpy.data.scenes['Scene'].make_local()",
    );
    fails(
        &h,
        poll::OBJECTMODE,
        &[PollReq::SceneEditable],
        "bpy.data.scenes['Scene'].make_local()",
    );
}

#[test]
fn objectmode() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::OBJECTMODE).is_ok());
    h.set_active(None);
    assert!(
        check(&h, poll::OBJECTMODE).is_ok(),
        "no active object is fine"
    );
    h.set_active(Some("Cube"));
    h.object_mut("Cube").mode = ObjectMode::Edit;
    fails(
        &h,
        poll::OBJECTMODE,
        &[PollReq::ModeObject],
        "bpy.ops.object.mode_set(mode='OBJECT')",
    );
}

#[test]
fn active_object() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::ACTIVE_OBJECT).is_ok());
    h.set_active(None);
    fails(
        &h,
        poll::ACTIVE_OBJECT,
        &[PollReq::ActiveObject],
        "bpy.context.view_layer.objects.active = bpy.data.objects['Cube']",
    );
    h.set_active(Some("Cube"));
    h.object_mut("Cube").hide_viewport = true;
    fails(
        &h,
        poll::ACTIVE_OBJECT,
        &[PollReq::ActiveObject],
        "bpy.data.objects['Cube'].hide_viewport = False",
    );
    // Hidden but in Edit Mode still counts (`ed_object_hidden`).
    h.object_mut("Cube").mode = ObjectMode::Edit;
    assert!(check(&h, poll::ACTIVE_OBJECT).is_ok());
}

#[test]
fn active_editable() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::ACTIVE_EDITABLE).is_ok());
    assert!(check(&h, poll::ACTIVE_LOCAL_EDITABLE).is_ok());
    h.set_active(None);
    fails(
        &h,
        poll::ACTIVE_EDITABLE,
        &[PollReq::ActiveObject],
        "bpy.context.view_layer.objects.active = bpy.data.objects['Cube']",
    );
    h.set_active(Some("Cube"));
    h.object_mut("Cube").header.library = linked();
    fails(
        &h,
        poll::ACTIVE_EDITABLE,
        &[PollReq::ActiveEditable],
        "bpy.data.objects['Cube'].make_local()",
    );
}

#[test]
fn active_editable_mesh() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::ACTIVE_EDITABLE_MESH).is_ok());
    h.set_active(Some("Camera"));
    fails(
        &h,
        poll::ACTIVE_EDITABLE_MESH,
        &[PollReq::ActiveType("MESH")],
        "bpy.context.view_layer.objects.active = bpy.data.objects['Cube']",
    );
    h.set_active(Some("Cube"));
    let me = h
        .state
        .main
        .objects
        .get(h.object("Cube"))
        .unwrap()
        .data
        .unwrap();
    let me = me.typed::<frap_rna::MeshId>().unwrap();
    h.state
        .main
        .meshes
        .get_mut(me, &mut h.log)
        .unwrap()
        .header
        .library = linked();
    fails(
        &h,
        poll::ACTIVE_EDITABLE_MESH,
        &[PollReq::ActiveEditable],
        "bpy.data.objects['Cube'].make_local()",
    );
}

#[test]
fn editmesh() {
    let mut h = TestHost::factory();
    fails(
        &h,
        poll::EDITMESH,
        &[PollReq::ModeEditMesh],
        "bpy.ops.object.mode_set(mode='EDIT')",
    );
    fails(
        &h,
        poll::EDITMESH_VIEW3D,
        &[PollReq::ModeEditMesh],
        "bpy.ops.object.mode_set(mode='EDIT')",
    );
    h.set_active(Some("Camera"));
    fails(
        &h,
        poll::EDITMESH,
        &[PollReq::ModeEditMesh],
        "bpy.context.view_layer.objects.active = bpy.data.objects['Cube']; bpy.ops.object.mode_set(mode='EDIT')",
    );
    h.set_active(Some("Cube"));
    h.object_mut("Cube").mode = ObjectMode::Edit;
    assert!(check(&h, poll::EDITMESH).is_ok());
    // A mesh in Edit Mode is not Object Mode, nor Pose Mode.
    assert!(check(&h, poll::OBJECTMODE).is_err());
    h.delete_object("Cube");
    h.delete_object("Camera");
    h.delete_object("Light");
    fails(
        &h,
        poll::EDITMESH,
        &[PollReq::ModeEditMesh],
        "add one first, e.g. bpy.ops.mesh.primitive_cube_add()",
    );
}

#[test]
fn posemode() {
    let mut h = TestHost::factory();
    fails(
        &h,
        poll::POSEMODE,
        &[PollReq::ModePose],
        "add one first, e.g. bpy.ops.object.armature_add()",
    );
    add_armature(&mut h, ObjectMode::Object);
    fails(
        &h,
        poll::POSEMODE,
        &[PollReq::ModePose],
        "bpy.ops.object.mode_set(mode='POSE')",
    );
    h.object_mut("Armature").mode = ObjectMode::Edit;
    fails(
        &h,
        poll::POSEMODE,
        &[PollReq::ModePose],
        "bpy.ops.object.mode_set(mode='OBJECT'); bpy.ops.object.mode_set(mode='POSE')",
    );
    h.object_mut("Armature").mode = ObjectMode::Pose;
    assert!(check(&h, poll::POSEMODE).is_ok());
    h.set_active(Some("Cube"));
    fails(
        &h,
        poll::POSEMODE,
        &[PollReq::ModePose],
        "bpy.context.view_layer.objects.active = bpy.data.objects['Armature']; bpy.ops.object.mode_set(mode='POSE')",
    );
}

#[test]
fn editarmature() {
    let mut h = TestHost::factory();
    add_armature(&mut h, ObjectMode::Object);
    fails(
        &h,
        poll::EDITARMATURE,
        &[PollReq::ModeEditArmature],
        "bpy.ops.object.mode_set(mode='EDIT')",
    );
    h.object_mut("Armature").mode = ObjectMode::Edit;
    assert!(check(&h, poll::EDITARMATURE).is_ok());
    assert!(
        check(&h, poll::EDITMESH).is_err(),
        "an edit armature is not an edit mesh"
    );
    h.set_active(Some("Cube"));
    h.object_mut("Cube").mode = ObjectMode::Edit;
    fails(
        &h,
        poll::EDITARMATURE,
        &[PollReq::ModeEditArmature],
        "bpy.ops.object.mode_set(mode='OBJECT'); bpy.context.view_layer.objects.active = bpy.data.objects['Armature']; bpy.ops.object.mode_set(mode='EDIT')",
    );
}

#[test]
fn selected_objects() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::SELECTED_OBJECTS).is_ok());
    h.select_only(&[]);
    fails(
        &h,
        poll::SELECTED_OBJECTS,
        &[PollReq::HasSelectedObjects],
        "bpy.data.objects['Cube'].select_set(True)",
    );
}

#[test]
fn scene_has_camera() {
    let mut h = TestHost::factory();
    assert!(check(&h, poll::SCENE_HAS_CAMERA).is_ok());
    let scene = h.state.win.scene;
    h.state
        .main
        .scenes
        .get_mut(scene, &mut h.log)
        .unwrap()
        .camera = None;
    fails(
        &h,
        poll::SCENE_HAS_CAMERA,
        &[PollReq::SceneHasCamera],
        "bpy.context.scene.camera = bpy.data.objects['Camera']",
    );
    h.delete_object("Camera");
    fails(
        &h,
        poll::SCENE_HAS_CAMERA,
        &[PollReq::SceneHasCamera],
        "bpy.ops.object.camera_add(); bpy.context.scene.camera = bpy.context.object",
    );
}

#[test]
fn names_with_quotes_are_python_literals() {
    let mut h = TestHost::factory();
    h.object_mut("Cube").hide_viewport = true;
    let r = h.object("Cube");
    h.state
        .main
        .objects
        .get_mut(r, &mut h.log)
        .unwrap()
        .header
        .name = "it's".into();
    let f = check(&h, poll::ACTIVE_OBJECT).unwrap_err();
    assert_eq!(
        f.hint.as_deref(),
        Some("bpy.data.objects[\"it's\"].hide_viewport = False")
    );
}

#[test]
fn every_library_poll_fails_somewhere_with_a_hint() {
    // A context where every requirement but SceneEditable fails: an armature in Edit Mode is
    // active, nothing selected, no scene camera; plus a linked scene for the scene polls.
    let mut h = TestHost::factory();
    add_armature(&mut h, ObjectMode::Edit);
    h.select_only(&[]);
    let scene = h.state.win.scene;
    h.state
        .main
        .scenes
        .get_mut(scene, &mut h.log)
        .unwrap()
        .camera = None;
    h.object_mut("Armature").header.library = linked();
    let mut linked_scene = TestHost::factory();
    let s2 = linked_scene.state.win.scene;
    linked_scene
        .state
        .main
        .scenes
        .get_mut(s2, &mut linked_scene.log)
        .unwrap()
        .header
        .library = linked();
    let mut no_active = TestHost::factory();
    no_active.set_active(None);
    let contexts = [&h, &linked_scene, &no_active];
    for (name, p) in poll::LIBRARY {
        if p.requires.is_empty() {
            continue;
        }
        let f = contexts
            .iter()
            .find_map(|c| check(c, *p).err())
            .unwrap_or_else(|| panic!("{name} never fails"));
        assert!(!f.unmet.is_empty(), "{name}");
        assert!(f.unmet.iter().all(|u| p.requires.contains(u)), "{name}");
        assert!(f.hint.as_deref().is_some_and(|s| !s.is_empty()), "{name}");
    }
}

#[test]
fn check_poll_error_shape() {
    use frap_op::OperatorDef;
    fn exec(_: &mut OpCtx, _: &OpArgs) -> OpResult {
        Ok(OpStatus::Finished)
    }
    static DEF: OperatorDef = OperatorDef {
        spec: &frap_spec_ops_object::object::OPSPEC_OBJECT_OT_DELETE,
        poll: poll::OBJECTMODE,
        exec,
        kind: OpKind::Normal,
        dyn_enums: &[],
        tier: Tier::T0,
        support: Support::Full,
        notes: &[],
    };
    let mut h = TestHost::factory();
    assert!(check_poll(&DEF, &h.state.ctx()).is_ok());
    h.object_mut("Cube").mode = ObjectMode::Edit;
    let e = check_poll(&DEF, &h.state.ctx()).unwrap_err();
    assert_eq!(e.exception, PyExc::RuntimeError);
    assert_eq!(e.code, errors::OP_POLL_FAILED);
    assert_eq!(
        e.message,
        "Operator bpy.ops.object.delete.poll() failed, context is incorrect"
    );
    assert_eq!(
        e.detail["requires"],
        serde_json::json!(["SCENE_EDITABLE", "MODE_OBJECT"])
    );
    assert_eq!(e.detail["unmet"], serde_json::json!(["MODE_OBJECT"]));
    assert_eq!(
        e.hint.as_deref(),
        Some("bpy.ops.object.mode_set(mode='OBJECT')")
    );
}

#[test]
fn custom_message_is_used_verbatim() {
    fn needs_groups(_: &frap_scene::Context) -> Result<(), PollFail> {
        Err(PollFail {
            message: Some("Object has no vertex groups".into()),
            unmet: Default::default(),
            hint: None,
        })
    }
    fn exec(_: &mut OpCtx, _: &OpArgs) -> OpResult {
        Ok(OpStatus::Finished)
    }
    static DEF: frap_op::OperatorDef = frap_op::OperatorDef {
        spec: &frap_spec_ops_object::object::OPSPEC_OBJECT_OT_DELETE,
        poll: Poll {
            requires: &[],
            check: needs_groups,
        },
        exec,
        kind: OpKind::Normal,
        dyn_enums: &[],
        tier: Tier::T0,
        support: Support::Full,
        notes: &[],
    };
    let h = TestHost::factory();
    let e = check_poll(&DEF, &h.state.ctx()).unwrap_err();
    assert_eq!(e.message, "Object has no vertex groups");
}
