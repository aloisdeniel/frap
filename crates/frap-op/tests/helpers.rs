// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers (§10.3) against values verified on Blender 5.2.4: `placement` (cursor,
//! `align='VIEW'` with the factory Layout viewport, `align='CURSOR'` in every rotation mode,
//! `scale` is-set), adding objects with independent object/data names, mode-handler dispatch,
//! `frame_range_or_scene` and `iter_edit_meshes`.

mod common;

use approx::assert_abs_diff_eq;
use common::dummy::StepCancel as Aliased;
use common::{TestHost, TestServices, dummy};
use frap_math::glam::{Mat4, Vec3};
use frap_math::quat::quat_to_mat4;
use frap_op::helpers::{self, Align};
use frap_op::prelude::*;
use frap_op::{MODE_HANDLERS, ModeHandler, Registry, Support, operator};
use frap_rna::{IdRef, MeshId, ObjectId, Prop};
use frap_scene::id::{Camera, Object};
use frap_scene::{HostRead, ObjectMode};

// A mesh Edit Mode handler (the M2 one lives in frap-ops-object): enter/exit only flip the mode.
fn enter(cx: &mut OpCtx, obs: &[IdRef<ObjectId>]) -> OpResult {
    for o in obs {
        cx.object_mut(*o)?.mode = ObjectMode::Edit;
    }
    Ok(OpStatus::Finished)
}
fn exit(cx: &mut OpCtx, obs: &[IdRef<ObjectId>]) -> OpResult {
    for o in obs {
        cx.object_mut(*o)?.mode = ObjectMode::Object;
    }
    cx.result("exited", obs.len());
    Ok(OpStatus::Finished)
}
#[allow(unsafe_code)]
#[linkme::distributed_slice(MODE_HANDLERS)]
static EDIT_MESH: ModeHandler = ModeHandler {
    mode: ObjectMode::Edit,
    object_type: "MESH",
    enter,
    exit,
};

// `#[operator]` with an aliased props type needs `spec = …`; a lower-case poll path names the
// upper-case constant. The generated test checks that `spec` is `Aliased::spec()`.
#[operator(poll = poll::objectmode, tier = T2, spec = dummy::OPSPEC_DUMMY_OT_STEP_CANCEL,
           support = HeadlessExtended("test"), notes = ["a", "b"])]
fn aliased(_cx: &mut OpCtx, _p: Aliased) -> OpResult {
    Ok(OpStatus::Finished)
}

#[test]
fn operator_macro_arguments() {
    let d = Registry::global().get("dummy.step_cancel").unwrap();
    assert_eq!(d.poll.requires, poll::OBJECTMODE.requires);
    assert_eq!(d.tier, Tier::T2);
    assert_eq!(d.support, Support::HeadlessExtended("test"));
    assert_eq!(d.notes, ["a", "b"]);
    assert_eq!(d.kind, OpKind::Normal);
    assert!(d.dyn_enums.is_empty());
}

fn set<T>(value: T) -> Prop<T> {
    Prop {
        value,
        is_set: true,
    }
}
fn unset<T>(value: T) -> Prop<T> {
    Prop {
        value,
        is_set: false,
    }
}

fn with_cx<R>(h: &mut TestHost, f: impl FnOnce(&mut OpCtx) -> R) -> R {
    let mut s = TestServices::default();
    let mut cx = OpCtx::new(h, &mut s, &dummy::OPSPEC_DUMMY_OT_ADD);
    f(&mut cx)
}

fn place(h: &mut TestHost, rot: Prop<[f32; 3]>, align: Align) -> [f32; 3] {
    with_cx(h, |cx| {
        helpers::placement(cx, &unset([0.0; 3]), &rot, align, &unset([0.0; 3]))
            .unwrap()
            .rotation
    })
}

fn close(a: [f32; 3], b: [f64; 3]) {
    for i in 0..3 {
        assert_abs_diff_eq!(a[i] as f64, b[i], epsilon = 2e-6);
    }
}

/// The factory Layout viewport as Blender's `view3d_viewmatrix_set` builds it from `viewquat`,
/// `ofs` and `dist`. RNA's `region_3d.view_rotation` (spec/blender-5.2/startup.json) is the
/// **inverse** of `viewquat` (`rna_RegionView3D_view_rotation_get`), so `viewquat` is its
/// conjugate. (The dumped `view_matrix` is stale in `-b`: it is never redrawn.)
fn factory_view_matrix() -> Mat4 {
    let view_rotation = [0.712_375_9_f32, 0.441_062_1, 0.287_358_25, 0.464_122_9];
    let viewquat = [
        view_rotation[0],
        -view_rotation[1],
        -view_rotation[2],
        -view_rotation[3],
    ];
    let rot = quat_to_mat4(viewquat);
    let ofs = Vec3::new(0.020_693_248, 0.046_025_47, -0.000_669_430_8);
    Mat4::from_translation(Vec3::new(0.0, 0.0, -18.388_048)) * rot * Mat4::from_translation(ofs)
}

#[test]
fn placement_location_and_scale_follow_is_set() {
    let mut h = TestHost::factory();
    let scene = h.state.win.scene;
    h.state
        .main
        .scenes
        .get_mut(scene, &mut h.log)
        .unwrap()
        .cursor
        .location = [1.0, 2.0, 3.0];
    with_cx(&mut h, |cx| {
        let p = helpers::placement(
            cx,
            &unset([0.0; 3]),
            &unset([0.0; 3]),
            Align::World,
            &unset([0.0; 3]),
        )
        .unwrap();
        assert_eq!(p.location, [1.0, 2.0, 3.0], "unset location = 3D cursor");
        assert_eq!(p.rotation, [0.0; 3]);
        assert_eq!(p.scale, [1.0; 3], "unset scale = 1");
        // Explicit (0,0,0) differs from omitting it (verified).
        let p = helpers::placement(
            cx,
            &set([0.0; 3]),
            &unset([0.0; 3]),
            Align::World,
            &set([0.0; 3]),
        )
        .unwrap();
        assert_eq!(p.location, [0.0; 3]);
        assert_eq!(p.scale, [0.0; 3], "a set scale is used as is, even (0,0,0)");
        let p = helpers::placement(
            cx,
            &set([0.0; 3]),
            &unset([0.0; 3]),
            Align::World,
            &set([2.0, 0.0, 1.0]),
        )
        .unwrap();
        assert_eq!(p.scale, [2.0, 0.0, 1.0]);
    });
}

#[test]
fn placement_align_view_matches_blender() {
    let mut h = TestHost::factory();
    h.state.win.view3d.view_matrix = factory_view_matrix();
    // Verified: primitive_cube_add(align='VIEW') under the VIEW_3D override of the factory
    // Layout screen gives rotation_euler (1.1087520122528076, 1.2411510397214442e-07,
    // 1.154877781867981).
    close(
        place(&mut h, unset([0.0; 3]), Align::View),
        [
            1.108_752_012_252_807_6,
            1.241_151_039_721_444_2e-7,
            1.154_877_781_867_981,
        ],
    );
    // A set rotation always wins.
    assert_eq!(
        place(&mut h, set([0.5, 0.0, 0.0]), Align::View),
        [0.5, 0.0, 0.0]
    );
    // Identity view → identity rotation.
    h.state.win.view3d.view_matrix = Mat4::IDENTITY;
    close(place(&mut h, unset([0.0; 3]), Align::View), [0.0, 0.0, 0.0]);
}

#[test]
fn placement_align_cursor_matches_blender() {
    let mut h = TestHost::factory();
    let scene = h.state.win.scene;
    let cursor = |h: &mut TestHost, f: &dyn Fn(&mut frap_scene::generated::plain::View3DCursor)| {
        f(&mut h
            .state
            .main
            .scenes
            .get_mut(scene, &mut h.log)
            .unwrap()
            .cursor)
    };
    // Values verified on 5.2.4 (cursor rotation (0.1, 0.2, 0.3) etc., see the WP 1.12 probe).
    cursor(&mut h, &|c| c.rotation_euler = [0.1, 0.2, 0.3]);
    close(
        place(&mut h, unset([0.0; 3]), Align::Cursor),
        [0.1, 0.2, 0.3],
    );
    cursor(&mut h, &|c| c.rotation_mode = "ZYX".into());
    close(
        place(&mut h, unset([0.0; 3]), Align::Cursor),
        [
            0.156_419_530_510_902_4,
            0.160_027_220_845_222_47,
            0.322_609_692_811_965_94,
        ],
    );
    cursor(&mut h, &|c| {
        c.rotation_mode = "QUATERNION".into();
        c.rotation_quaternion = [2.0, 0.2, 0.4, 0.6];
    });
    close(
        place(&mut h, unset([0.0; 3]), Align::Cursor),
        [
            0.298_498_958_349_227_9,
            0.302_854_090_929_031_37,
            0.628_796_339_035_034_2,
        ],
    );
    cursor(&mut h, &|c| {
        c.rotation_mode = "AXIS_ANGLE".into();
        c.rotation_axis_angle = [0.7, 1.0, 1.0, 0.0];
    });
    close(
        place(&mut h, unset([0.0; 3]), Align::Cursor),
        [
            0.537_168_920_040_130_6,
            0.472_968_250_513_076_8,
            0.132_465_511_560_440_06,
        ],
    );
    // A set rotation wins over the cursor (verified: (0.5, 0, 0)).
    assert_eq!(
        place(&mut h, set([0.5, 0.0, 0.0]), Align::Cursor),
        [0.5, 0.0, 0.0]
    );
}

fn cube_mesh() -> frap_mesh::Mesh {
    frap_mesh::primitives::cube(2.0, true, [1.0; 3])
}

fn at() -> Placement {
    Placement {
        location: [0.0, 0.0, 1.0],
        rotation: [0.0; 3],
        scale: [1.0; 3],
    }
}

#[test]
fn add_primitive_uniquifies_object_and_data_independently() {
    let mut h = TestHost::factory();
    // Verified: after deleting the factory Cube object, cube_add gives object `Cube` with mesh
    // `Cube.001` (the orphan mesh `Cube` keeps its name).
    h.delete_object("Cube");
    h.log = Default::default();
    let ob = with_cx(&mut h, |cx| {
        helpers::add_primitive_or_merge(cx, "Cube", cube_mesh(), at(), false)
    })
    .unwrap();
    let o = h.state.main.objects.get(ob).unwrap();
    assert_eq!(o.header.name, "Cube");
    assert_eq!(o.location, [0.0, 0.0, 1.0]);
    assert_eq!(o.scale, [1.0; 3]);
    let me = o.data.unwrap().typed::<MeshId>().unwrap();
    assert_eq!(h.state.main.meshes.get(me).unwrap().header.name, "Cube.001");
    assert_eq!(h.selected_names(), ["Cube"]);
    assert_eq!(h.active_name().as_deref(), Some("Cube"));
    assert_eq!(
        h.log.added,
        vec![ob.any(), me.any()],
        "object listed before its data"
    );
    // Again: object `Cube.001`, mesh `Cube.002`; only the new one is selected.
    let ob2 = with_cx(&mut h, |cx| {
        helpers::add_primitive_or_merge(cx, "Cube", cube_mesh(), at(), false)
    })
    .unwrap();
    assert_eq!(
        h.state.main.objects.get(ob2).unwrap().header.name,
        "Cube.001"
    );
    assert_eq!(h.mesh_names(), ["Cube", "Cube.001", "Cube.002"]);
    assert_eq!(h.selected_names(), ["Cube.001"]);
    assert_eq!(h.object_names(), ["Camera", "Cube", "Cube.001", "Light"]);
}

#[test]
fn add_object_with_data_uniquifies_independently() {
    let mut h = TestHost::factory();
    h.delete_object("Camera");
    // The caller inserts the data under its own requested name: `Camera` exists (orphan) →
    // `Camera.001`; the object takes the free name `Camera`.
    let ob = with_cx(&mut h, |cx| {
        let (main, log) = cx.main_mut();
        let cam = main.cameras.insert("Camera", Camera::new(""), log);
        helpers::add_object_with_data(cx, "Camera", Some(cam.any()), at(), false)
    })
    .unwrap();
    let o = h.state.main.objects.get(ob).unwrap();
    assert_eq!(o.header.name, "Camera");
    assert_eq!(o.object_type(), "CAMERA");
    let cam = o.data.unwrap().typed::<frap_rna::CameraId>().unwrap();
    assert_eq!(
        h.state.main.cameras.get(cam).unwrap().header.name,
        "Camera.001"
    );
    let added = &h.log.added;
    assert!(added.iter().position(|a| *a == ob.any()) < added.iter().position(|a| *a == cam.any()));
    // An Empty: no data.
    let empty = with_cx(&mut h, |cx| {
        helpers::add_object_with_data(cx, "Empty", None, at(), false)
    })
    .unwrap();
    assert_eq!(
        h.state.main.objects.get(empty).unwrap().object_type(),
        "EMPTY"
    );
    assert_eq!(h.selected_names(), ["Empty"]);
    assert_eq!(h.active_name().as_deref(), Some("Empty"));
}

#[test]
fn enter_editmode_dispatches_to_the_mode_handler() {
    let mut h = TestHost::factory();
    let ob = with_cx(&mut h, |cx| {
        helpers::add_primitive_or_merge(cx, "Cube", cube_mesh(), at(), true)
    })
    .unwrap();
    assert_eq!(h.state.main.objects.get(ob).unwrap().mode, ObjectMode::Edit);
    // Adding a primitive while a mesh is in Edit Mode merges into it in Blender (M2).
    let e = with_cx(&mut h, |cx| {
        helpers::add_primitive_or_merge(cx, "Cube", cube_mesh(), at(), false)
    })
    .unwrap_err();
    assert!(
        matches!(&e, OpError::NotImplemented(m) if m.contains("Edit Mode")),
        "{e:?}"
    );
    // Adding another type leaves Edit Mode first (through the handler's exit).
    let empty = with_cx(&mut h, |cx| {
        helpers::add_object_with_data(cx, "Empty", None, at(), false)
    })
    .unwrap();
    assert_eq!(
        h.state.main.objects.get(ob).unwrap().mode,
        ObjectMode::Object
    );
    assert_eq!(h.active_name().as_deref(), Some("Empty"));
    let _ = empty;
    // No handler for cameras: OP_NOT_IMPLEMENTED naming the mode.
    let e = with_cx(&mut h, |cx| {
        let (main, log) = cx.main_mut();
        let cam = main.cameras.insert("Camera", Camera::new(""), log);
        helpers::add_object_with_data(cx, "Camera", Some(cam.any()), at(), true)
    })
    .unwrap_err();
    let f = e.into_frap();
    assert_eq!(f.code, frap_op::errors::OP_NOT_IMPLEMENTED);
    assert_eq!(
        f.message,
        "frap: mode 'EDIT' is not implemented for CAMERA objects yet (no mode handler)"
    );
    assert!(helpers::mode_handler(ObjectMode::Edit, "MESH").is_ok());
    assert!(helpers::mode_handler(ObjectMode::Pose, "ARMATURE").is_err());
}

#[test]
fn frame_range_or_scene_follows_is_set() {
    let mut h = TestHost::factory();
    assert_eq!(
        with_cx(&mut h, |cx| helpers::frame_range_or_scene(
            cx,
            &unset(1),
            &unset(250)
        )),
        (1, 250)
    );
    let scene = h.state.win.scene;
    {
        let s = h.state.main.scenes.get_mut(scene, &mut h.log).unwrap();
        s.frame_start = 10;
        s.frame_end = 20;
    }
    assert_eq!(
        with_cx(&mut h, |cx| helpers::frame_range_or_scene(
            cx,
            &unset(1),
            &unset(250)
        )),
        (10, 20)
    );
    assert_eq!(
        with_cx(&mut h, |cx| helpers::frame_range_or_scene(
            cx,
            &set(1),
            &unset(250)
        )),
        (1, 20)
    );
    assert_eq!(
        with_cx(&mut h, |cx| helpers::frame_range_or_scene(
            cx,
            &unset(1),
            &set(250)
        )),
        (10, 250)
    );
}

/// Adds a mesh object `name` (sharing `share`'s data if given) to the scene, unselected.
fn add_mesh_object(h: &mut TestHost, name: &str, share: Option<&str>) -> IdRef<ObjectId> {
    let data = match share {
        Some(other) => h.state.main.objects.get(h.object(other)).unwrap().data,
        None => {
            let me = h.state.main.meshes.insert(name, cube_mesh(), &mut h.log);
            Some(me.any())
        }
    };
    let ob = h
        .state
        .main
        .objects
        .insert(name, Object::new(name, data), &mut h.log);
    let scene = h.state.win.scene;
    let s = h.state.main.scenes.get_mut(scene, &mut h.log).unwrap();
    s.collection.objects.push(ob);
    for vl in &mut s.view_layers {
        vl.bases.push(frap_scene::Base {
            object: ob,
            flag: frap_scene::BaseFlag::empty(),
        });
    }
    ob
}

#[test]
fn iter_edit_meshes_order_and_unique_data() {
    let mut h = TestHost::factory();
    let edit_meshes = |h: &mut TestHost| -> Vec<String> {
        with_cx(h, |cx| {
            helpers::iter_edit_meshes(cx)
                .into_iter()
                .map(|(o, m)| {
                    format!(
                        "{}:{}",
                        cx.main().objects.get(o).unwrap().header.name,
                        cx.main().meshes.get(m).unwrap().header.name
                    )
                })
                .collect()
        })
    };
    assert!(edit_meshes(&mut h).is_empty(), "Object Mode");
    let a = add_mesh_object(&mut h, "A", None); // before the active one in base order
    let _b = add_mesh_object(&mut h, "B", Some("Cube")); // shares Cube's mesh
    let c = add_mesh_object(&mut h, "C", None);
    let d = add_mesh_object(&mut h, "D", None); // hidden
    for n in ["Cube", "A", "B", "C", "D"] {
        h.object_mut(n).mode = ObjectMode::Edit;
    }
    h.object_mut("D").hide_viewport = true;
    h.object_mut("Camera").mode = ObjectMode::Edit; // other type: excluded
    h.set_active(Some("C"));
    // Active first, then base order (Camera, Cube, Light, A, B, C, D), unique data, visible.
    assert_eq!(edit_meshes(&mut h), ["C:C", "Cube:Cube", "A:A"]);
    h.set_active(Some("Cube"));
    assert_eq!(edit_meshes(&mut h), ["Cube:Cube", "A:A", "C:C"]);
    // The active object not in Edit Mode: the others of its type still count; B now stands for
    // the shared mesh (Blender's iterator skips the active base when it is not in the mode).
    h.object_mut("Cube").mode = ObjectMode::Object;
    assert_eq!(edit_meshes(&mut h), ["A:A", "B:Cube", "C:C"]);
    // No active object: nothing.
    h.set_active(None);
    assert!(edit_meshes(&mut h).is_empty());
    let _ = (a, c, d, h.state.ctx());
}
