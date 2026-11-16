// SPDX-License-Identifier: GPL-3.0-or-later
//! Setters with side effects and RNA functions of WP 1.10, against values verified on Blender
//! 5.2.4 (`--factory-startup`; probe scripts summarised in each test).

mod support;

/// `(rotation_mode, euler, quaternion, axis-angle)` after a mode change.
type RotStep = (&'static str, [f32; 3], [f32; 4], [f32; 4]);

use frap_math::EulerOrder;
use frap_math::matrix::loc_eul_o_size_to_mat4;
use frap_rna::{IdRef, Input, MathValue, ObjectId, RnaPtr, Seg, Value};
use frap_scene::HostRead;
use frap_scene::id::layers::sync_all;
use frap_scene::id::{Collection, Object, Scene};
use glam::{Mat4, Vec3};
use support::{TestHost, call, id, names, read, write};

fn floats(v: &Value) -> Vec<f32> {
    match v {
        Value::FloatArray { data, .. } => data.to_vec(),
        other => panic!("not a float array: {other:?}"),
    }
}

#[track_caller]
fn close(got: &[f32], want: &[f32], tol: f32) {
    assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
    for (a, b) in got.iter().zip(want) {
        assert!((a - b).abs() <= tol, "{got:?} vs {want:?}");
    }
}

fn s(x: &str) -> Input {
    Input::Str(x.into())
}

fn vec_in(v: &[f64]) -> Input {
    Input::Tuple(v.iter().map(|x| Input::Float(*x)).collect())
}

fn mat_in(m: &Mat4) -> Input {
    // Row-major rows, as mathutils.Matrix.
    let r = m.transpose().to_cols_array_2d();
    Input::Math(MathValue::Matrix(
        r.iter()
            .map(|row| row.iter().map(|x| f64::from(*x)).collect())
            .collect(),
    ))
}

fn rot(h: &TestHost, ob: &RnaPtr) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    (
        floats(&read(h, ob, "rotation_euler").unwrap()),
        floats(&read(h, ob, "rotation_quaternion").unwrap()),
        floats(&read(h, ob, "rotation_axis_angle").unwrap()),
    )
}

/// `ob.rotation_euler = (0.3, -0.7, 1.9)` then a chain of `rotation_mode` changes
/// (`BKE_rotMode_change_values`).
#[test]
fn rotation_mode_conversion_matches_blender() {
    let mut h = TestHost::factory();
    let cube = h.ob_ptr("Cube");
    write(&mut h, &cube, "rotation_euler", vec_in(&[0.3, -0.7, 1.9])).unwrap();
    #[rustfmt::skip]
    let steps: [RotStep; 11] = [
        ("QUATERNION", [0.3, -0.7, 1.9], [0.4986005, 0.357442, -0.0830324, 0.7853269], [0.0, 0.0, 1.0, 0.0]),
        ("AXIS_ANGLE", [0.3, -0.7, 1.9], [0.4986005, 0.357442, -0.0830324, 0.7853269], [2.0976255, 0.4123545, -0.0957884, 0.9059737]),
        ("ZYX", [0.5877521, 0.4990796, 1.8562741], [0.4986005, 0.357442, -0.0830324, 0.7853269], [2.0976255, 0.4123545, -0.0957884, 0.9059737]),
        ("XYZ", [0.5877521, 0.4990796, 1.8562741], [0.4986005, 0.357442, -0.0830324, 0.7853269], [2.0976255, 0.4123545, -0.0957884, 0.9059737]),
        ("YXZ", [0.5877521, 0.4990796, 1.8562741], [0.4986005, 0.357442, -0.0830324, 0.7853269], [2.0976255, 0.4123545, -0.0957884, 0.9059737]),
        ("AXIS_ANGLE", [0.5877521, 0.4990796, 1.8562741], [0.4986005, 0.357442, -0.0830324, 0.7853269], [2.0976255, -0.024214, 0.4226409, 0.9059737]),
        ("XZY", [1.1963519, -0.6708036, 2.2662716], [0.4986005, 0.357442, -0.0830324, 0.7853269], [2.0976255, -0.024214, 0.4226409, 0.9059737]),
        ("QUATERNION", [1.1963519, -0.6708036, 2.2662716], [0.4986004, -0.0209896, 0.3663586, 0.785327], [2.0976255, -0.024214, 0.4226409, 0.9059737]),
        ("YZX", [-1.1709146, -0.5849014, 2.2167797], [0.4986004, -0.0209896, 0.3663586, 0.785327], [2.0976255, -0.024214, 0.4226409, 0.9059737]),
        ("ZXY", [-1.1709146, -0.5849014, 2.2167797], [0.4986004, -0.0209896, 0.3663586, 0.785327], [2.0976255, -0.024214, 0.4226409, 0.9059737]),
        ("XYZ", [-1.1709146, -0.5849014, 2.2167797], [0.4986004, -0.0209896, 0.3663586, 0.785327], [2.0976255, -0.024214, 0.4226409, 0.9059737]),
    ];
    for (mode, e, q, aa) in steps {
        write(&mut h, &cube, "rotation_mode", s(mode)).unwrap();
        let (ge, gq, gaa) = rot(&h, &cube);
        close(&ge, &e, 2e-6);
        close(&gq, &q, 2e-6);
        close(&gaa, &aa, 2e-6);
        assert_eq!(
            read(&h, &cube, "rotation_mode").unwrap(),
            Value::Enum(mode.into())
        );
    }
    // Leaving QUATERNION normalises the stored quaternion.
    write(&mut h, &cube, "rotation_mode", s("QUATERNION")).unwrap();
    write(
        &mut h,
        &cube,
        "rotation_quaternion",
        vec_in(&[2.0, 0.5, 0.1, -0.3]),
    )
    .unwrap();
    write(&mut h, &cube, "rotation_mode", s("XYZ")).unwrap();
    let (e, q, _) = rot(&h, &cube);
    close(&e, &[0.4688559, 0.1616223, -0.259102], 2e-6);
    close(&q, &[0.9589266, 0.2397316, 0.0479463, -0.143839], 2e-6);
    // A zero axis-angle rotation, and axis-angle to an Euler order.
    write(&mut h, &cube, "rotation_mode", s("AXIS_ANGLE")).unwrap();
    write(
        &mut h,
        &cube,
        "rotation_axis_angle",
        vec_in(&[1.0, 0.0, 0.0, 0.0]),
    )
    .unwrap();
    write(&mut h, &cube, "rotation_mode", s("QUATERNION")).unwrap();
    close(&rot(&h, &cube).1, &[1.0, 0.0, 0.0, 0.0], 1e-7);
    write(&mut h, &cube, "rotation_mode", s("AXIS_ANGLE")).unwrap();
    write(
        &mut h,
        &cube,
        "rotation_axis_angle",
        vec_in(&[4.0, 1.0, 2.0, 3.0]),
    )
    .unwrap();
    write(&mut h, &cube, "rotation_mode", s("ZXY")).unwrap();
    close(
        &rot(&h, &cube).0,
        &[-1.1456257, -0.1219473, -2.025543],
        2e-6,
    );
    // Same mode: nothing changes.
    let before = rot(&h, &cube);
    write(&mut h, &cube, "rotation_mode", s("ZXY")).unwrap();
    assert_eq!(rot(&h, &cube), before);
}

/// `ob.matrix_world = Matrix.LocRotScale((1, 2, 3), Euler((0.4, 0.5, -0.6)), (2, -1, 0.5))`
/// in each rotation mode, with a parent, with deltas, and with shear.
#[test]
fn matrix_world_write_decomposes_like_blender() {
    let mut h = TestHost::factory();
    let cube = h.ob_ptr("Cube");
    let m = loc_eul_o_size_to_mat4(
        Vec3::new(1.0, 2.0, 3.0),
        [0.4, 0.5, -0.6],
        EulerOrder::XYZ,
        Vec3::new(2.0, -1.0, 0.5),
    );
    let loc = |h: &TestHost| floats(&read(h, &cube, "location").unwrap());
    let scale = |h: &TestHost| floats(&read(h, &cube, "scale").unwrap());

    write(&mut h, &cube, "matrix_world", mat_in(&m)).unwrap();
    close(&loc(&h), &[1.0, 2.0, 3.0], 1e-6);
    close(&rot(&h, &cube).0, &[-0.4, -2.6415925, -0.6000001], 2e-6);
    close(&scale(&h), &[-2.0, -1.0, -0.5], 1e-6);

    write(&mut h, &cube, "rotation_mode", s("QUATERNION")).unwrap();
    write(&mut h, &cube, "matrix_world", mat_in(&m)).unwrap();
    close(
        &rot(&h, &cube).1,
        &[0.1747571, -0.3275819, -0.892661, -0.2555512],
        2e-6,
    );
    // Euler channel left as it was.
    close(&rot(&h, &cube).0, &[-0.4, -2.6415925, -0.6000001], 2e-6);

    write(&mut h, &cube, "rotation_mode", s("AXIS_ANGLE")).unwrap();
    write(&mut h, &cube, "matrix_world", mat_in(&m)).unwrap();
    close(
        &rot(&h, &cube).1,
        &[0.1747571, -0.3275819, -0.8926611, -0.2555513],
        2e-6,
    );
    close(
        &rot(&h, &cube).2,
        &[2.7902746, -0.3327017, -0.9066123, -0.2595452],
        2e-6,
    );

    write(&mut h, &cube, "rotation_mode", s("ZXY")).unwrap();
    write(&mut h, &cube, "matrix_world", mat_in(&m)).unwrap();
    close(
        &rot(&h, &cube).0,
        &[-0.6074016, -2.9646091, 0.6478317],
        2e-6,
    );
    close(&scale(&h), &[-2.0, -1.0, -0.5], 1e-6);

    // Parented to the factory Camera (an RNA parent assignment keeps the identity inverse).
    write(&mut h, &cube, "rotation_mode", s("XYZ")).unwrap();
    let cam = h.ob("Camera");
    write(
        &mut h,
        &cube,
        "parent",
        Input::Ptr(RnaPtr::id(cam.any(), &frap_spec_rna::OBJECT)),
    )
    .unwrap();
    write(&mut h, &cube, "matrix_world", mat_in(&m)).unwrap();
    close(&loc(&h), &[2.1333916, 3.0330274, -10.497_123], 2e-5);
    close(
        &rot(&h, &cube).0,
        &[-0.2310404, 2.5438437, -1.4052293],
        2e-5,
    );
    close(&scale(&h), &[-1.9999999, -0.9999999, -0.5], 1e-5);
    let back = floats(&read(&h, &cube, "matrix_world").unwrap());
    #[rustfmt::skip]
    close(&back, &[
        1.4486002, -0.6741579, 0.0722849, 1.0000006,
        -0.9910405, -0.6547673, -0.2853679, 1.9999995,
        -0.9588511, -0.3417467, 0.4041535, 3.0000002,
        0.0, 0.0, 0.0, 1.0,
    ], 1e-5);
    write(&mut h, &cube, "parent", Input::None).unwrap();

    // Delta transforms are removed from the decomposition.
    write(&mut h, &cube, "delta_location", vec_in(&[1.0, 0.0, 0.0])).unwrap();
    write(
        &mut h,
        &cube,
        "delta_rotation_euler",
        vec_in(&[0.1, 0.0, 0.0]),
    )
    .unwrap();
    write(&mut h, &cube, "delta_scale", vec_in(&[2.0, 2.0, 2.0])).unwrap();
    write(&mut h, &cube, "matrix_world", mat_in(&m)).unwrap();
    close(&loc(&h), &[0.0, 2.0, 3.0], 1e-6);
    close(
        &rot(&h, &cube).0,
        &[-0.3087258, -2.6997998, -0.6414501],
        2e-6,
    );
    close(&scale(&h), &[-1.0, -0.5, -0.25], 1e-6);
    write(&mut h, &cube, "delta_location", vec_in(&[0.0, 0.0, 0.0])).unwrap();
    write(
        &mut h,
        &cube,
        "delta_rotation_euler",
        vec_in(&[0.0, 0.0, 0.0]),
    )
    .unwrap();
    write(&mut h, &cube, "delta_scale", vec_in(&[1.0, 1.0, 1.0])).unwrap();

    // Shear is lost.
    let shear = Mat4::from_cols_array_2d(&[
        [1.0, 0.0, 0.0, 0.0],
        [0.5, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [1.0, 2.0, 3.0, 1.0],
    ]);
    write(&mut h, &cube, "matrix_world", mat_in(&shear)).unwrap();
    close(&loc(&h), &[1.0, 2.0, 3.0], 1e-6);
    close(&rot(&h, &cube).0, &[0.0, 0.0, -0.2286668], 2e-6);
    close(&scale(&h), &[1.0, 1.118034, 1.0], 1e-6);

    // Reads are fresh (D-04): no depsgraph update needed after a location change.
    write(&mut h, &cube, "matrix_world", mat_in(&Mat4::IDENTITY)).unwrap();
    write(&mut h, &cube, "location", vec_in(&[5.0, 5.0, 5.0])).unwrap();
    let mw = floats(&read(&h, &cube, "matrix_world").unwrap());
    assert_eq!(&mw[..4], &[1.0, 0.0, 0.0, 5.0]);
}

#[test]
fn matrix_local_and_basis_follow_blender() {
    let mut h = TestHost::factory();
    let cube = h.ob_ptr("Cube");
    let cam = h.ob("Camera");
    write(&mut h, &cube, "location", vec_in(&[1.0, 0.0, 0.0])).unwrap();
    let inv = Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0));
    write(&mut h, &cube, "matrix_parent_inverse", mat_in(&inv)).unwrap();
    // Unparented: matrix_local == matrix_basis.
    let basis = floats(&read(&h, &cube, "matrix_basis").unwrap());
    assert_eq!(basis, floats(&read(&h, &cube, "matrix_local").unwrap()));
    // Parented: matrix_local == matrix_parent_inverse @ matrix_basis (the assignment resets
    // the inverse, so it is written afterwards).
    write(
        &mut h,
        &cube,
        "parent",
        Input::Ptr(RnaPtr::id(cam.any(), &frap_spec_rna::OBJECT)),
    )
    .unwrap();
    write(&mut h, &cube, "matrix_parent_inverse", mat_in(&inv)).unwrap();
    let local = floats(&read(&h, &cube, "matrix_local").unwrap());
    close(&[local[3], local[7], local[11]], &[1.0, 0.0, 2.0], 1e-6);
    // Writing matrix_local undoes the inverse first.
    let target = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));
    write(&mut h, &cube, "matrix_local", mat_in(&target)).unwrap();
    close(
        &floats(&read(&h, &cube, "location").unwrap()),
        &[4.0, 5.0, 4.0],
        1e-6,
    );
    write(&mut h, &cube, "matrix_basis", mat_in(&target)).unwrap();
    close(
        &floats(&read(&h, &cube, "location").unwrap()),
        &[4.0, 5.0, 6.0],
        1e-6,
    );
}

/// `ob.matrix_parent_inverse = Translation((1, 2, 3)); ob.parent = Light`: the inverse is
/// reset to identity (verified on 5.2.4); a cycle clears the parent silently; the object
/// itself is a TypeError.
#[test]
fn parent_assignment_resets_the_inverse() {
    let mut h = TestHost::factory();
    let cube = h.ob_ptr("Cube");
    let light = h.ob("Light");
    let inv = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    write(&mut h, &cube, "matrix_parent_inverse", mat_in(&inv)).unwrap();
    let lp = RnaPtr::id(light.any(), &frap_spec_rna::OBJECT);
    write(&mut h, &cube, "parent", Input::Ptr(lp.clone())).unwrap();
    assert_eq!(id(&read(&h, &cube, "parent").unwrap()), Some(light.any()));
    let mpi = floats(&read(&h, &cube, "matrix_parent_inverse").unwrap());
    assert_eq!(&mpi[..4], &[1.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        read(&h, &cube, "parent_type").unwrap(),
        Value::Enum("OBJECT".into())
    );
    // Light -> Cube would close a cycle: the parent is cleared silently.
    let cam = h.ob_ptr("Camera");
    write(&mut h, &lp, "parent", Input::Ptr(cam)).unwrap();
    assert!(id(&read(&h, &lp, "parent").unwrap()).is_some());
    write(&mut h, &lp, "parent", Input::Ptr(cube.clone())).unwrap();
    assert_eq!(id(&read(&h, &lp, "parent").unwrap()), None);
    let e = write(&mut h, &cube, "parent", Input::Ptr(cube.clone())).unwrap_err();
    assert_eq!(
        e.message,
        "Object.parent ID type does not support assignment to itself"
    );
    assert_eq!(id(&read(&h, &cube, "parent").unwrap()), Some(light.any()));
    write(&mut h, &cube, "matrix_parent_inverse", mat_in(&inv)).unwrap();
    write(&mut h, &cube, "parent", Input::None).unwrap();
    assert_eq!(id(&read(&h, &cube, "parent").unwrap()), None);
    let mpi = floats(&read(&h, &cube, "matrix_parent_inverse").unwrap());
    assert_eq!(&mpi[..4], &[1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn frame_setters_follow_blender() {
    let mut h = TestHost::factory();
    let sc = RnaPtr::id(h.win.scene.any(), &frap_spec_rna::SCENE);
    for (v, want) in [
        (5, 5),
        (-5, 0),
        (0, 0),
        (1_048_574, 1_048_574),
        (-1_048_574, 0),
    ] {
        h.time_dirty = false;
        write(&mut h, &sc, "frame_current", Input::Int(v)).unwrap();
        assert_eq!(
            read(&h, &sc, "frame_current").unwrap(),
            Value::Int(want),
            "{v}"
        );
        assert!(h.time_dirty, "frame_current marks time dirty");
    }
    // The raw setter (callers skip the hard-range clamp, Scene::UNCLAMPED).
    assert_eq!(Scene::frame_from_input(1_000_000_000), 1_000_000_000);
    assert_eq!(Scene::frame_from_input(-1_000_000_000), 0);
    assert!(Scene::UNCLAMPED.contains(&("Scene", "frame_current")));
    // frame_end = 10; frame_start = 20 -> (20, 20); frame_end = 0 -> (0, 0).
    write(&mut h, &sc, "frame_end", Input::Int(10)).unwrap();
    write(&mut h, &sc, "frame_start", Input::Int(20)).unwrap();
    assert_eq!(read(&h, &sc, "frame_start").unwrap(), Value::Int(20));
    assert_eq!(read(&h, &sc, "frame_end").unwrap(), Value::Int(20));
    write(&mut h, &sc, "frame_start", Input::Int(1)).unwrap();
    write(&mut h, &sc, "frame_end", Input::Int(0)).unwrap();
    assert_eq!(read(&h, &sc, "frame_start").unwrap(), Value::Int(0));
    assert_eq!(read(&h, &sc, "frame_end").unwrap(), Value::Int(0));
    // Hard range of frame_start / frame_end (coerce): 2000000000 -> 1048574.
    write(&mut h, &sc, "frame_start", Input::Int(2_000_000_000)).unwrap();
    assert_eq!(read(&h, &sc, "frame_start").unwrap(), Value::Int(1_048_574));
    assert_eq!(read(&h, &sc, "frame_end").unwrap(), Value::Int(1_048_574));
    assert_eq!(
        read(&h, &sc, "frame_current_final").unwrap(),
        Value::Float(0.0)
    );
}

fn state(h: &TestHost) -> Vec<(String, bool, bool, bool)> {
    h.main
        .objects
        .iter()
        .map(|(r, o)| {
            let p = RnaPtr::id(r.any(), &frap_spec_rna::OBJECT);
            let mut h2 = TestHost::new((h.main.clone(), h.win.clone()));
            let b = |f: &str, h2: &mut TestHost| call(h2, &p, f, &[]).unwrap() == Value::Bool(true);
            (
                o.header.name.to_string(),
                b("select_get", &mut h2),
                b("hide_get", &mut h2),
                b("visible_get", &mut h2),
            )
        })
        .collect()
}

fn ctx_names(
    h: &TestHost,
    f: impl Fn(&frap_scene::Context<'_>) -> Vec<IdRef<ObjectId>>,
) -> Vec<String> {
    let c = h.ctx();
    f(&c)
        .into_iter()
        .map(|r| h.main.objects.name_of(r).unwrap().to_owned())
        .collect()
}

fn st(n: &str, sel: bool, hid: bool, vis: bool) -> (String, bool, bool, bool) {
    (n.to_owned(), sel, hid, vis)
}

/// `select_set`/`hide_set`/`visible_get` and the context lists (Blender 5.2.4 probe, factory).
#[test]
fn selection_functions_follow_blender() {
    let mut h = TestHost::factory();
    let cube = h.ob_ptr("Cube");
    let cam = h.ob_ptr("Camera");
    let light = h.ob_ptr("Light");
    let t = Value::Bool(true);
    let f = Value::Bool(false);
    assert_eq!(
        state(&h),
        [
            st("Camera", false, false, true),
            st("Cube", true, false, true),
            st("Light", false, false, true)
        ]
    );
    call(&mut h, &cam, "select_set", &[("state", t.clone())]).unwrap();
    assert!(h.take_log().context_changed);
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c.selected_objects()),
        ["Cube", "Camera"]
    );
    call(&mut h, &cube, "hide_set", &[("state", t.clone())]).unwrap();
    assert_eq!(
        state(&h),
        [
            st("Camera", true, false, true),
            st("Cube", false, true, false),
            st("Light", false, false, true)
        ]
    );
    assert_eq!(
        h.ctx().active_object(),
        Some(h.ob("Cube")),
        "a hidden object stays active"
    );
    call(&mut h, &cube, "select_set", &[("state", t.clone())]).unwrap();
    assert_eq!(
        call(&mut h, &cube, "select_get", &[]).unwrap(),
        f,
        "hidden: not selectable"
    );
    call(&mut h, &cube, "hide_set", &[("state", f.clone())]).unwrap();
    assert_eq!(
        state(&h),
        [
            st("Camera", true, false, true),
            st("Cube", false, false, true),
            st("Light", false, false, true)
        ]
    );
    // hide_viewport: invisible, unselectable.
    write(&mut h, &cube, "hide_viewport", Input::Bool(true)).unwrap();
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c.visible_objects()),
        ["Light", "Camera"]
    );
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c.selected_objects()),
        ["Camera"]
    );
    call(&mut h, &cube, "select_set", &[("state", t.clone())]).unwrap();
    assert_eq!(call(&mut h, &cube, "select_get", &[]).unwrap(), f);
    write(&mut h, &cube, "hide_viewport", Input::Bool(false)).unwrap();
    // hide_select: unselectable.
    write(&mut h, &cube, "hide_select", Input::Bool(true)).unwrap();
    call(&mut h, &cube, "select_set", &[("state", t.clone())]).unwrap();
    assert_eq!(call(&mut h, &cube, "select_get", &[]).unwrap(), f);
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c
            .selected_editable_objects()),
        ["Camera"]
    );
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c.selectable_objects()),
        ["Light", "Camera"]
    );
    write(&mut h, &cube, "hide_select", Input::Bool(false)).unwrap();
    // hide_select deselects at once (verified).
    call(&mut h, &cube, "select_set", &[("state", t.clone())]).unwrap();
    write(&mut h, &cube, "hide_select", Input::Bool(true)).unwrap();
    assert_eq!(call(&mut h, &cube, "select_get", &[]).unwrap(), f);
    write(&mut h, &cube, "hide_select", Input::Bool(false)).unwrap();

    // An object without a base.
    let mut log = frap_scene::ChangeLog::default();
    let orphan = h
        .main
        .objects
        .insert("Orphan", Object::new("", None), &mut log);
    let op = RnaPtr::id(orphan.any(), &frap_spec_rna::OBJECT);
    let e = call(&mut h, &op, "select_set", &[("state", t.clone())]).unwrap_err();
    assert_eq!(e.exception.as_str(), "RuntimeError");
    assert_eq!(
        e.message,
        "Error: Object 'Orphan' cannot be selected because it is not in View Layer 'ViewLayer'!\n"
    );
    let e = call(&mut h, &op, "hide_set", &[("state", t.clone())]).unwrap_err();
    assert_eq!(
        e.message,
        "Error: Object 'Orphan' cannot be hidden because it is not in View Layer 'ViewLayer'!\n"
    );
    // Deselecting or unhiding an object without a base is a silent no-op (verified on 5.2.4:
    // `for ob in bpy.data.objects: ob.select_set(False)` works with orphans).
    assert_eq!(
        call(&mut h, &op, "select_set", &[("state", f.clone())]).unwrap(),
        Value::None
    );
    assert_eq!(
        call(&mut h, &op, "hide_set", &[("state", f.clone())]).unwrap(),
        Value::None
    );
    for fun in ["select_get", "hide_get", "visible_get"] {
        assert_eq!(call(&mut h, &op, fun, &[]).unwrap(), f, "{fun}");
    }
    // view_layer.objects.active
    let objs = RnaPtr::id(h.win.scene.any(), &frap_spec_rna::SCENE)
        .child(Seg::Prop("view_layers"), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Key("ViewLayer".into()), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Prop("objects"), &frap_spec_rna::LAYER_OBJECTS);
    let e = write(&mut h, &objs, "active", Input::Ptr(op.clone())).unwrap_err();
    assert_eq!(
        e.message,
        "Error: ViewLayer 'ViewLayer' does not contain object 'Orphan'\n"
    );
    write(&mut h, &objs, "active", Input::None).unwrap();
    assert_eq!(id(&read(&h, &objs, "active").unwrap()), None);
    assert_eq!(h.ctx().object(), None);
    write(&mut h, &objs, "active", Input::Ptr(cam.clone())).unwrap();
    assert_eq!(
        id(&read(&h, &objs, "active").unwrap()),
        Some(h.ob("Camera").any())
    );
    // select_set with an explicit view_layer keyword; selection order is base order.
    let vl = Value::Ptr(Box::new(objs.clone()));
    let vl = match vl {
        Value::Ptr(mut p) => {
            p.path.pop();
            Value::Ptr(p)
        }
        v => v,
    };
    call(
        &mut h,
        &light,
        "select_set",
        &[("state", t.clone()), ("view_layer", vl)],
    )
    .unwrap();
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c.selected_objects()),
        ["Cube", "Light", "Camera"][1..]
    );
    assert_eq!(
        names(&h, &read(&h, &objs, "selected").unwrap()),
        ["Light", "Camera"]
    );
}

/// Layer collections: `hide_viewport` keeps selection flags but empties the context lists;
/// `exclude` removes the bases and the active object.
#[test]
fn layer_collection_visibility_and_exclusion() {
    let mut h = TestHost::factory();
    let cam = h.ob_ptr("Camera");
    call(&mut h, &cam, "select_set", &[("state", Value::Bool(true))]).unwrap();
    let light = h.ob_ptr("Light");
    call(
        &mut h,
        &light,
        "select_set",
        &[("state", Value::Bool(true))],
    )
    .unwrap();
    let cube = h.ob_ptr("Cube");
    call(
        &mut h,
        &cube,
        "select_set",
        &[("state", Value::Bool(false))],
    )
    .unwrap();
    let lc = RnaPtr::id(h.win.scene.any(), &frap_spec_rna::SCENE)
        .child(Seg::Prop("view_layers"), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Key("ViewLayer".into()), &frap_spec_rna::VIEW_LAYER)
        .child(
            Seg::Prop("layer_collection"),
            &frap_spec_rna::LAYER_COLLECTION,
        )
        .child(Seg::Prop("children"), &frap_spec_rna::LAYER_COLLECTION)
        .child(
            Seg::Key("Collection".into()),
            &frap_spec_rna::LAYER_COLLECTION,
        );
    assert_eq!(
        read(&h, &lc, "name").unwrap(),
        Value::Str("Collection".into())
    );
    assert_eq!(read(&h, &lc, "is_visible").unwrap(), Value::Bool(true));
    write(&mut h, &lc, "hide_viewport", Input::Bool(true)).unwrap();
    assert_eq!(
        state(&h),
        [
            st("Camera", true, false, false),
            st("Cube", false, false, false),
            st("Light", true, false, false)
        ]
    );
    assert!(ctx_names(&h, |c: &frap_scene::Context<'_>| c.visible_objects()).is_empty());
    assert!(ctx_names(&h, |c: &frap_scene::Context<'_>| c.selected_objects()).is_empty());
    assert_eq!(read(&h, &lc, "is_visible").unwrap(), Value::Bool(false));
    // Selecting under a hidden layer collection works (only BASE_SELECTABLE gates it), and
    // `view_layer.objects.selected` still lists the selection (verified on 5.2.4).
    call(&mut h, &cube, "select_set", &[("state", Value::Bool(true))]).unwrap();
    assert_eq!(
        call(&mut h, &cube, "select_get", &[]).unwrap(),
        Value::Bool(true)
    );
    let objs = RnaPtr::id(h.win.scene.any(), &frap_spec_rna::SCENE)
        .child(Seg::Prop("view_layers"), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Key("ViewLayer".into()), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Prop("objects"), &frap_spec_rna::LAYER_OBJECTS);
    assert_eq!(
        names(&h, &read(&h, &objs, "selected").unwrap()),
        ["Cube", "Light", "Camera"]
    );
    assert!(ctx_names(&h, |c: &frap_scene::Context<'_>| c.selectable_objects()).is_empty());
    call(
        &mut h,
        &cube,
        "select_set",
        &[("state", Value::Bool(false))],
    )
    .unwrap();
    write(&mut h, &lc, "hide_viewport", Input::Bool(false)).unwrap();
    write(&mut h, &lc, "exclude", Input::Bool(true)).unwrap();
    assert!(h.ctx().view_layer().bases.is_empty());
    assert_eq!(h.ctx().active_object(), None);
    write(&mut h, &lc, "exclude", Input::Bool(false)).unwrap();
    assert_eq!(h.ctx().view_layer().bases.len(), 3);
    assert_eq!(h.ctx().active_object(), None);
    // A hidden collection deselects and hides its objects.
    call(&mut h, &cam, "select_set", &[("state", Value::Bool(true))]).unwrap();
    let coll = RnaPtr::id(
        h.main.collections.find("Collection").unwrap().any(),
        &frap_spec_rna::COLLECTION,
    );
    write(&mut h, &coll, "hide_viewport", Input::Bool(true)).unwrap();
    assert_eq!(
        state(&h),
        [
            st("Camera", false, false, false),
            st("Cube", false, false, false),
            st("Light", false, false, false)
        ]
    );
    write(&mut h, &coll, "hide_viewport", Input::Bool(false)).unwrap();
    write(&mut h, &coll, "hide_select", Input::Bool(true)).unwrap();
    assert!(ctx_names(&h, |c: &frap_scene::Context<'_>| c.selectable_objects()).is_empty());
    // Read-only name.
    let e = write(&mut h, &lc, "name", Input::Str("x".into())).unwrap_err();
    assert_eq!(
        e.message,
        "bpy_struct: attribute \"name\" from \"LayerCollection\" is read-only"
    );
}

/// Base order after membership changes (verified on 5.2.4 after `view_layer.update()`).
#[test]
fn base_order_follows_layer_collection_sync() {
    let mut h = TestHost::factory();
    let mut log = frap_scene::ChangeLog::default();
    let main = &mut h.main;
    let scene = h.win.scene;
    let a = main
        .objects
        .insert("zz_master", Object::new("", None), &mut log);
    let b = main
        .objects
        .insert("aa_coll", Object::new("", None), &mut log);
    let c = main
        .objects
        .insert("mm_child", Object::new("", None), &mut log);
    let d = main
        .objects
        .insert("dd_top2", Object::new("", None), &mut log);
    let coll = main.collections.find("Collection").unwrap();
    let child = main
        .collections
        .insert("Child", Collection::new(""), &mut log);
    let top2 = main
        .collections
        .insert("Top2", Collection::new(""), &mut log);
    main.scenes
        .get_mut(scene, &mut log)
        .unwrap()
        .collection
        .objects
        .push(a);
    {
        let cc = main.collections.get_mut(coll, &mut log).unwrap();
        cc.objects.push(b);
        cc.children.push(child);
    }
    main.collections
        .get_mut(child, &mut log)
        .unwrap()
        .objects
        .push(c);
    main.collections
        .get_mut(top2, &mut log)
        .unwrap()
        .objects
        .push(d);
    main.scenes
        .get_mut(scene, &mut log)
        .unwrap()
        .collection
        .children
        .push(top2);
    sync_all(main, &mut log);
    let order = |h: &TestHost| -> Vec<String> {
        h.ctx()
            .view_layer()
            .bases
            .iter()
            .map(|b| h.main.objects.name_of(b.object).unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        order(&h),
        [
            "mm_child",
            "Cube",
            "Light",
            "Camera",
            "aa_coll",
            "dd_top2",
            "zz_master"
        ]
    );
    for o in [a, b, c, d] {
        let p = RnaPtr::id(o.any(), &frap_spec_rna::OBJECT);
        call(&mut h, &p, "select_set", &[("state", Value::Bool(true))]).unwrap();
    }
    assert_eq!(
        ctx_names(&h, |c: &frap_scene::Context<'_>| c.selected_objects()),
        ["mm_child", "Cube", "aa_coll", "dd_top2", "zz_master"]
    );
    let sc = RnaPtr::id(scene.any(), &frap_spec_rna::SCENE);
    assert_eq!(
        names(&h, &read(&h, &sc, "objects").unwrap()),
        [
            "zz_master",
            "Cube",
            "Light",
            "Camera",
            "aa_coll",
            "mm_child",
            "dd_top2"
        ]
    );
    // Unlink and relink Cube: it moves to the end of its collection and keeps its selection.
    let cube = h.ob("Cube");
    {
        let main = &mut h.main;
        let cc = main.collections.get_mut(coll, &mut log).unwrap();
        cc.objects.retain(|o| *o != cube);
        cc.objects.push(cube);
        sync_all(main, &mut log);
    }
    assert_eq!(
        order(&h),
        [
            "mm_child",
            "Light",
            "Camera",
            "aa_coll",
            "Cube",
            "dd_top2",
            "zz_master"
        ]
    );
    assert!(h.ctx().view_layer().base(cube).unwrap().selected());
    let cp = RnaPtr::id(coll.any(), &frap_spec_rna::COLLECTION);
    assert_eq!(
        names(&h, &read(&h, &cp, "all_objects").unwrap()),
        ["Light", "Camera", "aa_coll", "Cube", "mm_child"]
    );
    // An object in two collections takes its last place unless it is first or last.
    let light = h.ob("Light");
    {
        let main = &mut h.main;
        main.collections
            .get_mut(top2, &mut log)
            .unwrap()
            .objects
            .push(light);
        sync_all(main, &mut log);
    }
    assert_eq!(
        order(&h),
        [
            "mm_child",
            "Camera",
            "aa_coll",
            "Cube",
            "dd_top2",
            "Light",
            "zz_master"
        ]
    );
}

/// Python-only `children`, `children_recursive`, `users_collection`, `users_scene`.
#[test]
fn python_only_members_follow_blender() {
    let mut h = TestHost::factory();
    let mut log = frap_scene::ChangeLog::default();
    let scene = h.win.scene;
    let main = &mut h.main;
    let e1 = main
        .objects
        .insert("b_empty", Object::new("", None), &mut log);
    let e2 = main
        .objects
        .insert("A_empty", Object::new("", None), &mut log);
    let e3 = main
        .objects
        .insert("c_empty", Object::new("", None), &mut log);
    main.scenes
        .get_mut(scene, &mut log)
        .unwrap()
        .collection
        .objects
        .push(e1);
    let other = main
        .collections
        .insert("Other", Collection::new(""), &mut log);
    main.scenes
        .get_mut(scene, &mut log)
        .unwrap()
        .collection
        .children
        .push(other);
    let cube = main.objects.find("Cube").unwrap();
    let cam = main.objects.find("Camera").unwrap();
    let light = main.objects.find("Light").unwrap();
    main.collections
        .get_mut(other, &mut log)
        .unwrap()
        .objects
        .push(cube);
    for (o, p) in [(e1, cube), (e2, cube), (light, cube), (e3, e2), (cam, e1)] {
        main.objects.get_mut(o, &mut log).unwrap().parent = Some(p);
    }
    sync_all(main, &mut log);
    let cp = h.ob_ptr("Cube");
    assert_eq!(
        names(&h, &read(&h, &cp, "children").unwrap()),
        ["A_empty", "b_empty", "Light"]
    );
    assert_eq!(
        names(&h, &read(&h, &cp, "children_recursive").unwrap()),
        ["A_empty", "c_empty", "b_empty", "Camera", "Light"]
    );
    assert_eq!(
        names(&h, &read(&h, &cp, "users_collection").unwrap()),
        ["Collection", "Other"]
    );
    let e2p = RnaPtr::id(e2.any(), &frap_spec_rna::OBJECT);
    assert!(names(&h, &read(&h, &e2p, "users_collection").unwrap()).is_empty());
    assert!(names(&h, &read(&h, &e2p, "users_scene").unwrap()).is_empty());
    let e1p = RnaPtr::id(e1.any(), &frap_spec_rna::OBJECT);
    assert_eq!(
        names(&h, &read(&h, &e1p, "users_collection").unwrap()),
        ["bpy.data.scenes['Scene'].collection"]
    );
    assert_eq!(names(&h, &read(&h, &cp, "users_scene").unwrap()), ["Scene"]);
    // A second scene linking the cube: (S2, Scene) in bpy.data order.
    let mut s2 = Scene::new("", Vec::new());
    s2.collection.objects.push(cube);
    let vl =
        frap_scene::ViewLayer::new_synced("ViewLayer", &s2, &h.main.collections, &h.main.objects);
    s2.view_layers.push(vl);
    h.main.scenes.insert("S2", s2, &mut log);
    assert_eq!(
        names(&h, &read(&h, &cp, "users_scene").unwrap()),
        ["S2", "Scene"]
    );
    let u = read(&h, &cp, "users").unwrap();
    assert_eq!(
        u,
        Value::Int(3),
        "Collection, Other, S2's master collection"
    );
}

/// Derived material slots (§5.6; verified on 5.2.4).
#[test]
fn material_slots_are_derived() {
    let mut h = TestHost::factory();
    let cp = h.ob_ptr("Cube");
    let mut log = frap_scene::ChangeLog::default();
    let m2 = h
        .main
        .materials
        .insert("M2", frap_scene::id::Material::new(""), &mut log);
    let mat = h.main.materials.find("Material").unwrap();
    let m2p = RnaPtr::id(m2.any(), &frap_spec_rna::MATERIAL);
    assert_eq!(
        id(&read(&h, &cp, "active_material").unwrap()),
        Some(mat.any())
    );
    write(&mut h, &cp, "active_material", Input::Ptr(m2p.clone())).unwrap();
    let me = h.main.meshes.find("Cube").unwrap();
    assert_eq!(h.main.meshes.get(me).unwrap().materials, [Some(m2)]);
    h.main.meshes.get_mut(me, &mut log).unwrap().materials[0] = Some(mat);
    // An object without slots ignores active_material.
    let cam = h.ob_ptr("Camera");
    write(&mut h, &cam, "active_material", Input::Ptr(m2p.clone())).unwrap();
    assert_eq!(id(&read(&h, &cam, "active_material").unwrap()), None);
    let slot = cp
        .child(Seg::Prop("material_slots"), &frap_spec_rna::MATERIAL_SLOT)
        .child(Seg::Index(0), &frap_spec_rna::MATERIAL_SLOT);
    let slot_read = |h: &TestHost, p: &str| -> Value {
        support::with_struct(h, &cp, &mut |s| {
            let def = s.srna().find_prop("material_slots").unwrap();
            let hook = frap_scene::find_hook(s.srna(), "material_slots")
                .unwrap()
                .get
                .unwrap();
            let f = hook(h, &cp, def)?;
            let frap_rna::Field::Coll(c) = f else {
                panic!()
            };
            let frap_rna::Item::Struct(it) = c.item(0)? else {
                panic!()
            };
            Ok(support::field_value(
                it.get(frap_spec_rna::MATERIAL_SLOT.find_prop(p).unwrap())?,
            ))
        })
        .unwrap()
    };
    write(&mut h, &slot, "link", s("OBJECT")).unwrap();
    assert_eq!(slot_read(&h, "material"), Value::Id(None));
    write(&mut h, &slot, "material", Input::Ptr(m2p)).unwrap();
    assert_eq!(slot_read(&h, "material"), Value::Id(Some(m2.any())));
    assert_eq!(h.main.meshes.get(me).unwrap().materials, [Some(mat)]);
    write(&mut h, &slot, "link", s("DATA")).unwrap();
    assert_eq!(slot_read(&h, "material"), Value::Id(Some(mat.any())));
    assert_eq!(slot_read(&h, "name"), Value::Str("Material".into()));
    write(&mut h, &slot, "material", Input::None).unwrap();
    assert_eq!(slot_read(&h, "name"), Value::Str("".into()));
    assert_eq!(h.main.meshes.get(me).unwrap().materials, [None]);
    // The slot count follows the data: a second material on the mesh adds a slot.
    h.main
        .meshes
        .get_mut(me, &mut log)
        .unwrap()
        .materials
        .push(Some(mat));
    let n = support::field_value(
        frap_scene::find_hook(&frap_spec_rna::OBJECT, "material_slots")
            .unwrap()
            .get
            .unwrap()(&h, &cp, frap_spec_rna::object::MATERIAL_SLOTS)
        .unwrap(),
    );
    let Value::Tuple(items) = n else { panic!() };
    assert_eq!(items.len(), 2);
    assert_eq!(
        h.main
            .objects
            .get(h.ob("Cube"))
            .unwrap()
            .material_slot_overrides
            .len(),
        1
    );
    // active_material_index is clamped to the slots.
    write(&mut h, &cp, "active_material_index", Input::Int(5)).unwrap();
    assert_eq!(
        read(&h, &cp, "active_material_index").unwrap(),
        Value::Int(1)
    );
}

/// `ID.name` assignment and `Object.data`.
#[test]
fn name_and_data_hooks() {
    let mut h = TestHost::factory();
    let cp = h.ob_ptr("Cube");
    write(&mut h, &cp, "name", s("Camera")).unwrap();
    assert_eq!(
        read(&h, &cp, "name").unwrap(),
        Value::Str("Camera.001".into())
    );
    let log = h.take_log();
    assert_eq!(log.renamed.len(), 1);
    assert_eq!(
        h.main.names("objects").unwrap(),
        ["Camera", "Camera.001", "Light"]
    );
    write(&mut h, &cp, "name", s("")).unwrap();
    assert_eq!(read(&h, &cp, "name").unwrap(), Value::Str("Object".into()));
    write(&mut h, &cp, "name", s("Cube")).unwrap();
    // Object.data: wrong type refused, None ignored.
    let camdata = h.main.cameras.find("Camera").unwrap();
    let e = write(
        &mut h,
        &cp,
        "data",
        Input::Ptr(RnaPtr::id(camdata.any(), &frap_spec_rna::CAMERA)),
    )
    .unwrap_err();
    assert_eq!(e.message, "Object.data expected a Mesh type, not Camera");
    write(&mut h, &cp, "data", Input::None).unwrap();
    assert!(id(&read(&h, &cp, "data").unwrap()).is_some());
    // Embedded IDs keep their names; the master collection is read-only.
    let master = RnaPtr::new(
        frap_rna::Root::Embedded {
            owner: h.win.scene.any(),
            slot: frap_rna::EmbeddedSlot::MasterCollection,
        },
        &frap_spec_rna::COLLECTION,
    );
    assert_eq!(
        read(&h, &master, "name").unwrap(),
        Value::Str("Scene Collection".into())
    );
    let e = frap_scene::find_hook(&frap_spec_rna::COLLECTION, "name")
        .unwrap()
        .set
        .unwrap()(
        &mut h,
        &master,
        frap_spec_rna::id::NAME,
        Value::Str("X".into()),
    )
    .unwrap_err();
    assert_eq!(
        e.message,
        "bpy_struct: attribute \"name\" from \"Collection\" is read-only"
    );
}

/// `window.scene` switches the context scene; `window.view_layer` follows renames.
#[test]
fn window_stub() {
    let mut h = TestHost::factory();
    let mut log = frap_scene::ChangeLog::default();
    let mut s2 = Scene::new("", Vec::new());
    let vl =
        frap_scene::ViewLayer::new_synced("ViewLayer", &s2, &h.main.collections, &h.main.objects);
    s2.view_layers.push(vl);
    let s2 = h.main.scenes.insert("S2", s2, &mut log);
    let w = RnaPtr::new(frap_rna::Root::Context, &frap_spec_rna::CONTEXT)
        .child(Seg::Prop("window"), &frap_spec_rna::WINDOW);
    write(
        &mut h,
        &w,
        "scene",
        Input::Ptr(RnaPtr::id(s2.any(), &frap_spec_rna::SCENE)),
    )
    .unwrap();
    assert_eq!(h.ctx().scene().header.name, "S2");
    assert_eq!(h.ctx().object(), None);
    let Value::Ptr(p) = read(&h, &w, "view_layer").unwrap() else {
        panic!()
    };
    assert_eq!(
        p.repr(&h.main),
        "bpy.data.scenes['S2'].view_layers[\"ViewLayer\"]"
    );
    let back = h.main.scenes.find("Scene").unwrap();
    write(
        &mut h,
        &w,
        "scene",
        Input::Ptr(RnaPtr::id(back.any(), &frap_spec_rna::SCENE)),
    )
    .unwrap();
    // A view layer of another scene is ignored.
    write(&mut h, &w, "view_layer", Input::Ptr(*p)).unwrap();
    assert_eq!(h.ctx().scene().header.name, "Scene");
    // Renaming the view layer moves the window along; a taken name gets a suffix.
    let vlp = frap_scene::id::paths::view_layer_ptr(back, "ViewLayer");
    write(&mut h, &vlp, "name", s("VL")).unwrap();
    assert_eq!(h.win.view_layer, "VL");
    assert_eq!(read(&h, &w, "width").unwrap(), Value::Int(0));
}
