// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays the WP 1.10 golden cases (`tests/golden/cases/scene/*.py`) through the bindings,
//! hooks and RNA functions directly and compares with the Blender-blessed fixtures. The golden
//! harness runs the same cases through the session (WP 1.14); this test pins the scene-side
//! behaviour independently of the resolver. Matrix literals are columns (Blender fills a
//! matrix property from nested sequences in memory order).

mod support;

use frap_rna::{Input, MathValue, RnaPtr, Seg, Value};
use frap_scene::HostRead;
use glam::Mat4;
use serde_json::Value as J;
use support::{TestHost, call, read, write};

fn fixture(name: &str) -> J {
    let p = format!(
        "{}/../../tests/golden/fixtures/scene/{name}.blender.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(p).expect("fixture exists")).unwrap()
}

/// A matrix literal whose inner tuples are columns.
fn cols(c: [[f32; 4]; 4]) -> Input {
    let m = Mat4::from_cols_array_2d(&c).transpose().to_cols_array_2d();
    Input::Math(MathValue::Matrix(
        m.iter()
            .map(|r| r.iter().map(|x| f64::from(*x)).collect())
            .collect(),
    ))
}

fn s(x: &str) -> Input {
    Input::Str(x.into())
}

fn ob(h: &TestHost, n: &str) -> RnaPtr {
    h.ob_ptr(n)
}

fn floats(v: &Value) -> Vec<f64> {
    match v {
        Value::FloatArray { data, .. } => data.iter().map(|x| f64::from(*x)).collect(),
        other => panic!("{other:?}"),
    }
}

/// Compares the `objects` section of a fixture (parent, location, rotation of the current
/// mode, scale, matrix_world) with frap's state.
fn compare_objects(h: &TestHost, fx: &J, tol: f64) {
    for (name, o) in fx["objects"].as_object().unwrap() {
        let p = ob(h, name);
        let parent = match read(h, &p, "parent").unwrap() {
            Value::Id(Some(a)) => Some(h.main.header(a).unwrap().name.to_string()),
            _ => None,
        };
        assert_eq!(parent.as_deref(), o["parent"].as_str(), "{name}.parent");
        let mode = o["rotation_mode"].as_str().unwrap();
        assert_eq!(
            read(h, &p, "rotation_mode").unwrap(),
            Value::Enum(mode.into()),
            "{name}"
        );
        let rot = match mode {
            "QUATERNION" => "rotation_quaternion",
            "AXIS_ANGLE" => "rotation_axis_angle",
            _ => "rotation_euler",
        };
        for (prop, key) in [
            ("location", "location"),
            (rot, "rotation"),
            ("scale", "scale"),
        ] {
            let got = floats(&read(h, &p, prop).unwrap());
            let want: Vec<f64> = o[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            for (a, b) in got.iter().zip(&want) {
                assert!(
                    (a - b).abs() <= tol * b.abs().max(1.0),
                    "{name}.{prop}: {got:?} vs {want:?}"
                );
            }
        }
        let mw = floats(&read(h, &p, "matrix_world").unwrap());
        let want: Vec<f64> = o["matrix_world"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()))
            .collect();
        for (a, b) in mw.iter().zip(&want) {
            assert!(
                (a - b).abs() <= tol * b.abs().max(1.0),
                "{name}.matrix_world: {mw:?} vs {want:?}"
            );
        }
    }
}

fn compare_context(h: &TestHost, fx: &J) {
    let c = &fx["context"];
    let ctx = h.ctx();
    let name = |r| h.main.objects.name_of(r).unwrap().to_owned();
    assert_eq!(
        ctx.active_object().map(name).as_deref(),
        c["active"].as_str()
    );
    let sel: Vec<String> = ctx.selected_objects().into_iter().map(name).collect();
    let want: Vec<&str> = c["selected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(sel, want);
}

/// The fixture's outcome of source line `line` (`None` when ok).
fn outcome(fx: &J, line: u64) -> Option<String> {
    let l = fx["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["line"] == line)
        .unwrap();
    (l["ok"] != true).then(|| l["message"].as_str().unwrap().to_owned())
}

#[test]
fn rotation_mode_case() {
    let fx = fixture("rotation_mode");
    let mut h = TestHost::factory();
    let (cube, cam, light) = (ob(&h, "Cube"), ob(&h, "Camera"), ob(&h, "Light"));
    let tup = |v: &[f64]| Input::Tuple(v.iter().map(|x| Input::Float(*x)).collect());
    write(&mut h, &cube, "rotation_euler", tup(&[0.3, -0.7, 1.9])).unwrap();
    write(&mut h, &cube, "rotation_mode", s("QUATERNION")).unwrap();
    write(&mut h, &cam, "rotation_mode", s("AXIS_ANGLE")).unwrap();
    write(&mut h, &light, "rotation_mode", s("QUATERNION")).unwrap();
    write(
        &mut h,
        &light,
        "rotation_quaternion",
        tup(&[2.0, 0.5, 0.1, -0.3]),
    )
    .unwrap();
    write(&mut h, &light, "rotation_mode", s("ZXY")).unwrap();
    compare_objects(&h, &fx, 1e-5);
}

#[test]
fn matrix_world_case() {
    let fx = fixture("matrix_world");
    let mut h = TestHost::factory();
    let (cube, cam, light) = (ob(&h, "Cube"), ob(&h, "Camera"), ob(&h, "Light"));
    write(&mut h, &cube, "parent", Input::Ptr(cam.clone())).unwrap();
    #[rustfmt::skip]
    write(&mut h, &cube, "matrix_world", cols([
        [1.4486002, -0.9910405, -0.9588511, 0.0], [-0.6741579, -0.6547673, -0.3417467, 0.0],
        [0.0722849, -0.2853679, 0.4041535, 0.0], [1.0, 2.0, 3.0, 1.0],
    ])).unwrap();
    write(&mut h, &light, "rotation_mode", s("QUATERNION")).unwrap();
    #[rustfmt::skip]
    write(&mut h, &light, "matrix_world", cols([
        [1.0, 0.0, 0.0, 0.0], [0.5, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 2.0, 3.0, 1.0],
    ])).unwrap();
    write(&mut h, &cam, "rotation_mode", s("AXIS_ANGLE")).unwrap();
    write(
        &mut h,
        &cam,
        "delta_location",
        Input::Tuple(vec![
            Input::Float(0.5),
            Input::Float(0.0),
            Input::Float(0.0),
        ]),
    )
    .unwrap();
    #[rustfmt::skip]
    write(&mut h, &cam, "matrix_world", cols([
        [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, 4.0, 1.0],
    ])).unwrap();
    compare_objects(&h, &fx, 1e-5);
}

#[test]
fn parent_case() {
    let fx = fixture("parent");
    let mut h = TestHost::factory();
    let (cube, cam, light) = (ob(&h, "Cube"), ob(&h, "Camera"), ob(&h, "Light"));
    write(
        &mut h,
        &cube,
        "matrix_parent_inverse",
        cols([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 2.0, 3.0, 1.0],
        ]),
    )
    .unwrap();
    write(&mut h, &cube, "parent", Input::Ptr(light.clone())).unwrap();
    write(&mut h, &light, "parent", Input::Ptr(cube.clone())).unwrap();
    let e = write(&mut h, &cube, "parent", Input::Ptr(cube.clone())).unwrap_err();
    assert_eq!(
        format!("bpy_struct: item.attr = val: {}", e.message),
        outcome(&fx, 9).unwrap()
    );
    write(&mut h, &cam, "parent", Input::Ptr(cube.clone())).unwrap();
    compare_objects(&h, &fx, 1e-5);
}

#[test]
fn selection_case() {
    let fx = fixture("selection");
    let mut h = TestHost::factory();
    let (cube, cam, light) = (ob(&h, "Cube"), ob(&h, "Camera"), ob(&h, "Light"));
    let t = || vec![("state", Value::Bool(true))];
    let objs = RnaPtr::id(h.win.scene.any(), &frap_spec_rna::SCENE)
        .child(Seg::Prop("view_layers"), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Key("ViewLayer".into()), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Prop("objects"), &frap_spec_rna::LAYER_OBJECTS);
    call(&mut h, &cam, "select_set", &t()).unwrap();
    call(&mut h, &cube, "hide_set", &t()).unwrap();
    call(&mut h, &cube, "select_set", &t()).unwrap();
    call(&mut h, &light, "select_set", &t()).unwrap();
    write(&mut h, &objs, "active", Input::Ptr(light.clone())).unwrap();
    write(&mut h, &cam, "hide_select", Input::Bool(true)).unwrap();
    call(&mut h, &cube, "hide_set", &[("state", Value::Bool(false))]).unwrap();
    call(&mut h, &cube, "select_set", &t()).unwrap();
    write(&mut h, &objs, "active", Input::None).unwrap();
    compare_context(&h, &fx);
    compare_objects(&h, &fx, 1e-5);
}

#[test]
fn layer_collections_case() {
    let fx = fixture("layer_collections");
    let mut h = TestHost::factory();
    let light = ob(&h, "Light");
    call(
        &mut h,
        &light,
        "select_set",
        &[("state", Value::Bool(true))],
    )
    .unwrap();
    let lc = frap_scene::id::paths::view_layer_ptr(h.win.scene, "ViewLayer")
        .child(
            Seg::Prop("layer_collection"),
            &frap_spec_rna::LAYER_COLLECTION,
        )
        .child(Seg::Prop("children"), &frap_spec_rna::LAYER_COLLECTION)
        .child(
            Seg::Key("Collection".into()),
            &frap_spec_rna::LAYER_COLLECTION,
        );
    write(&mut h, &lc, "hide_viewport", Input::Bool(true)).unwrap();
    write(&mut h, &lc, "hide_viewport", Input::Bool(false)).unwrap();
    write(&mut h, &lc, "exclude", Input::Bool(true)).unwrap();
    let e = write(&mut h, &lc, "name", s("X")).unwrap_err();
    assert_eq!(e.message, outcome(&fx, 9).unwrap());
    compare_context(&h, &fx);
}

#[test]
fn names_case() {
    let fx = fixture("names");
    let mut h = TestHost::factory();
    let (cube, light) = (ob(&h, "Cube"), ob(&h, "Light"));
    write(&mut h, &cube, "name", s("Camera")).unwrap();
    write(&mut h, &light, "name", s("")).unwrap();
    let mat = RnaPtr::id(
        h.main.materials.find("Material").unwrap().any(),
        &frap_spec_rna::MATERIAL,
    );
    write(&mut h, &mat, "name", s("Camera")).unwrap();
    let c1 = ob(&h, "Camera.001");
    write(&mut h, &c1, "name", s("Camera.001")).unwrap();
    let want: Vec<&str> = fx["objects"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(h.main.names("objects").unwrap(), want);
    let want: Vec<&str> = fx["materials"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(h.main.names("materials").unwrap(), want);
    compare_context(&h, &fx);
    compare_objects(&h, &fx, 1e-5);
}

#[test]
fn tuple_keys_case() {
    let fx = fixture("tuple_keys");
    let h = TestHost::factory();
    assert!(outcome(&fx, 6).is_none());
    assert!(
        h.main
            .find_key_lib("objects", "Cube", None)
            .unwrap()
            .is_some()
    );
    let e = h.main.find_key_lib("objects", "Nope", None).unwrap_err();
    assert_eq!(e.message, outcome(&fx, 7).unwrap());
    let e = h
        .main
        .find_key_lib("objects", "Cube", Some("/nonexistent/library.blend"))
        .unwrap_err();
    assert_eq!(e.message, outcome(&fx, 8).unwrap());
}

#[test]
fn select_hidden_case() {
    let fx = fixture("select_hidden");
    let mut h = TestHost::factory();
    let (cube, light) = (ob(&h, "Cube"), ob(&h, "Light"));
    let lc = frap_scene::id::paths::view_layer_ptr(h.win.scene, "ViewLayer")
        .child(
            Seg::Prop("layer_collection"),
            &frap_spec_rna::LAYER_COLLECTION,
        )
        .child(Seg::Prop("children"), &frap_spec_rna::LAYER_COLLECTION)
        .child(
            Seg::Key("Collection".into()),
            &frap_spec_rna::LAYER_COLLECTION,
        );
    let t = || vec![("state", Value::Bool(true))];
    let f = || vec![("state", Value::Bool(false))];
    write(&mut h, &lc, "hide_viewport", Input::Bool(true)).unwrap();
    call(&mut h, &cube, "select_set", &f()).unwrap();
    call(&mut h, &light, "select_set", &t()).unwrap();
    write(&mut h, &lc, "hide_viewport", Input::Bool(false)).unwrap();
    // Excluding the collection removes the bases: the Cube has none.
    write(&mut h, &lc, "exclude", Input::Bool(true)).unwrap();
    for line in [12, 13] {
        assert!(outcome(&fx, line).is_none());
    }
    call(&mut h, &cube, "select_set", &f()).unwrap();
    call(&mut h, &cube, "hide_set", &f()).unwrap();
    let e = call(&mut h, &cube, "select_set", &t()).unwrap_err();
    assert_eq!(e.message, outcome(&fx, 14).unwrap());
    let e = call(&mut h, &cube, "hide_set", &t()).unwrap_err();
    assert_eq!(e.message, outcome(&fx, 15).unwrap());
    compare_context(&h, &fx);
}
