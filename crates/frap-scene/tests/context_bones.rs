// SPDX-License-Identifier: GPL-3.0-or-later
//! `Context` bone accessors (M5+M6 W0, decision log M56W0-12): `active_bone`,
//! `active_edit_bone`, `active_pose_bone`, `selected_pose_bones`, `pose_object`, with the
//! Blender 5.2 behaviour dumped from `object.armature_add` + `mode_set` (pose selection on the
//! pose bone, bone-collection visibility).

mod support;

use std::sync::Arc;

use frap_rna::ArmatureId;
use frap_scene::id::{Armature, Bone, BoneCollection, Pose, PoseBone};
use frap_scene::{BoneRef, ChangeLog, Context, EditArmature, ObjectMode};
use support::TestHost;

fn bone(name: &str, parent: Option<usize>) -> Bone {
    Bone {
        name: name.into(),
        parent,
        head_local: [0.0; 3],
        tail_local: [0.0, 0.0, 1.0],
        matrix_local: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        roll: 0.0,
        use_connect: false,
        use_deform: true,
        use_inherit_rotation: true,
        inherit_scale: "FULL".into(),
        use_local_location: true,
        use_relative_parent: false,
        envelope_distance: 0.25,
        envelope_weight: 1.0,
        use_envelope_multiply: false,
        head_radius: 0.1,
        tail_radius: 0.05,
        bbone_segments: 1,
        hide: false,
        hide_select: false,
        select: [false; 3],
        collections: Vec::new(),
    }
}

/// The factory Cube turned into an armature object with bones `Root` and `Arm`.
fn rig() -> (TestHost, frap_rna::IdRef<ArmatureId>) {
    let mut h = TestHost::factory();
    let mut arm = Armature::new("Armature");
    arm.bones = vec![bone("Root", None), bone("Arm", Some(0))];
    arm.collections = vec![BoneCollection {
        name: "Hidden".into(),
        parent: None,
        is_visible: false,
        is_solo: false,
    }];
    arm.active_bone = Some(1);
    let mut log = std::mem::take(&mut h.log);
    let r = h.main.armatures.insert("Armature", arm, &mut log);
    let cube = h.ob("Cube");
    let ob = h.main.objects.get_mut(cube, &mut log).expect("cube");
    ob.data = Some(r.any());
    let mut pose = Pose::default();
    pose.bones = vec![PoseBone::new("Root"), PoseBone::new("Arm")];
    ob.pose = Some(pose);
    (h, r)
}

fn cx(h: &TestHost) -> Context<'_> {
    Context {
        main: &h.main,
        win: &h.win,
        prefs: &h.prefs,
    }
}

fn set_mode(h: &mut TestHost, mode: ObjectMode) {
    let cube = h.ob("Cube");
    h.main
        .objects
        .get_mut(cube, &mut ChangeLog::default())
        .expect("cube")
        .mode = mode;
}

#[test]
fn active_bone_is_bones_active_outside_edit_mode() {
    let (h, _) = rig();
    let c = cx(&h);
    let want = BoneRef {
        object: h.ob("Cube"),
        bone: "Arm".into(),
    };
    assert_eq!(c.active_bone(), Some(want));
    // Object Mode: no pose object, so no active pose bone (Blender: None).
    assert_eq!(c.pose_object(), None);
    assert_eq!(c.active_pose_bone(), None);
    assert!(c.selected_pose_bones().is_empty());
}

#[test]
fn edit_mode_answers_the_active_edit_bone() {
    let (mut h, r) = rig();
    set_mode(&mut h, ObjectMode::Edit);
    let ea = EditArmature {
        edit_bones: Vec::new(),
        active: None,
    };
    h.edit.armatures.insert(r.key(), Arc::new(ea));
    let c = cx(&h);
    assert_eq!(c.active_bone(), None);
    assert_eq!(c.active_edit_bone(&h.edit), None);
}

#[test]
fn pose_mode_selection_follows_pose_bones_and_collections() {
    let (mut h, r) = rig();
    set_mode(&mut h, ObjectMode::Pose);
    let cube = h.ob("Cube");
    {
        let c = cx(&h);
        assert_eq!(c.pose_object(), Some(cube));
        // armature_add + POSE in Blender 5.2: active pose bone set, nothing selected.
        assert_eq!(c.active_pose_bone().map(|b| b.bone), Some("Arm".into()));
        assert!(c.selected_pose_bones().is_empty());
    }
    {
        let ob = h
            .main
            .objects
            .get_mut(cube, &mut ChangeLog::default())
            .expect("cube");
        let pose = ob.pose.as_mut().expect("pose");
        pose.bones[0].select = true;
        pose.bones[1].select = true;
    }
    let names = |h: &TestHost| -> Vec<String> {
        cx(h)
            .selected_pose_bones()
            .into_iter()
            .map(|b| b.bone.to_string())
            .collect()
    };
    assert_eq!(names(&h), ["Root", "Arm"]);
    // A hidden pose bone is not selected-visible.
    h.main
        .objects
        .get_mut(cube, &mut ChangeLog::default())
        .expect("cube")
        .pose
        .as_mut()
        .expect("pose")
        .bones[0]
        .hide = true;
    assert_eq!(names(&h), ["Arm"]);
    // A bone only in an invisible collection is neither selected-visible nor active.
    h.main
        .armatures
        .get_mut(r, &mut ChangeLog::default())
        .expect("arm")
        .bones[1]
        .collections = vec!["Hidden".into()];
    assert!(names(&h).is_empty());
    assert_eq!(cx(&h).active_pose_bone(), None);
}
