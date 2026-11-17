// SPDX-License-Identifier: GPL-3.0-or-later
//! Session behaviour of the M0 skeleton: envelope, both syntaxes, Blender error wording,
//! rollback. Expected strings are Blender 5.2.4's (ARCHITECTURE Appendix A).

use frap_session::{Session, SessionOptions};
use serde_json::{Value as J, json};

fn session(dir: &std::path::Path) -> Session {
    let mut opts = SessionOptions::default();
    opts.cwd = dir.to_path_buf();
    Session::new(opts).unwrap()
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("frap-m0-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(s: &mut Session, src: &str) -> Vec<J> {
    s.exec_source(src).iter().map(|r| r.to_json()).collect()
}

#[test]
fn both_syntaxes_give_the_same_response() {
    let d = tmpdir("syntax");
    let a = run(
        &mut session(&d),
        "bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))",
    );
    let b = run(
        &mut session(&d),
        "mesh.primitive_cube_add size=2 location=0,0,1",
    );
    assert_eq!(a, b);
    assert_eq!(
        a[0]["result"]["object"],
        json!({"$ref": "bpy.data.objects['Cube.001']"})
    );
}

#[test]
fn second_cube_is_cube_001_and_located() {
    let d = tmpdir("second");
    let mut s = session(&d);
    let r = run(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))\nbpy.data.objects['Cube.001'].location\nbpy.data.objects\nbpy.data.meshes['Cube.001']",
    );
    assert_eq!(r[1]["value"], json!([0, 0, 1]));
    assert_eq!(
        r[2]["value"]["items"],
        json!(["Camera", "Cube", "Cube.001", "Light"])
    );
    assert_eq!(
        r[3]["value"],
        json!({"$ref": "bpy.data.meshes['Cube.001']"})
    );
    assert_eq!(r[0]["context"]["selected_objects"], json!(["Cube.001"]));
}

#[test]
fn errors_use_blender_wording_and_leave_state_unchanged() {
    let d = tmpdir("errors");
    let mut s = session(&d);
    let cases = [
        (
            "bpy.ops.mesh.primitive_cube_add(sizee=2)",
            "TypeError",
            "OP_ARG_UNKNOWN",
            "Converting py args to operator properties:: keyword \"sizee\" unrecognized",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(size=\"x\")",
            "TypeError",
            "OP_ARG_TYPE",
            "Converting py args to operator properties: MESH_OT_primitive_cube_add.size expected a float type, not str",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(align='FOO')",
            "TypeError",
            "OP_ARG_ENUM",
            "Converting py args to operator properties: enum \"FOO\" not found in ('WORLD', 'VIEW', 'CURSOR')",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(location=(1, 2))",
            "ValueError",
            "OP_ARG_LENGTH",
            "Converting py args to operator properties: sequences of dimension 0 should contain 3 items, not 2",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(calc_uvs=2)",
            "TypeError",
            "OP_ARG_TYPE",
            "Converting py args to operator properties: MESH_OT_primitive_cube_add.calc_uvs expected True/False or 0/1, not int",
        ),
        (
            "bpy.ops.mesh.foo()",
            "AttributeError",
            "OP_UNKNOWN",
            "Calling operator \"bpy.ops.mesh.foo\" error, could not be found",
        ),
        (
            "bpy.data.objects['Nope']",
            "KeyError",
            "RNA_KEY_NOT_FOUND",
            "'bpy_prop_collection[key]: key \"Nope\" not found'",
        ),
        (
            "bpy.data.objects[9]",
            "IndexError",
            "RNA_INDEX_RANGE",
            "bpy_prop_collection[index]: index 9 out of range, size 3",
        ),
        (
            "bpy.data.objects['Cube'].location = (1, 2)",
            "ValueError",
            "RNA_LENGTH",
            "bpy_struct: item.attr = val: sequences of dimension 0 should contain 3 items, not 2",
        ),
        (
            "bpy.data.objects['Cube'].location = (1, 2, 'a')",
            "TypeError",
            "RNA_TYPE",
            "bpy_struct: item.attr = val: expected sequence items of type float, not str",
        ),
        (
            // Blender 5.2.4: no trailing space after the prefix when the body is empty.
            "bpy.data.objects['Cube'].location = 5",
            "TypeError",
            "RNA_TYPE",
            "bpy_struct: item.attr = val:",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(location=5)",
            "TypeError",
            "OP_ARG_TYPE",
            "Converting py args to operator properties:",
        ),
        (
            "bpy.data.objects['Cube'].foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'Object' object has no attribute 'foo'",
        ),
        (
            "bpy.data.foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'BlendData' object has no attribute 'foo'",
        ),
        (
            "bpy.context.object = None",
            "AttributeError",
            "CONTEXT_READONLY",
            "bpy_struct: Context property \"object\" is read-only",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(size=2",
            "SyntaxError",
            "SYNTAX",
            "'(' was never closed",
        ),
    ];
    for (src, exc, code, msg) in cases {
        let r = &run(&mut s, src)[0];
        assert_eq!(r["ok"], false, "{src}");
        assert_eq!(r["status"], "ERROR", "{src}");
        assert_eq!(r["state"], "unchanged", "{src}");
        assert_eq!(r["error"]["exception"], exc, "{src}");
        assert_eq!(r["error"]["code"], code, "{src}");
        assert_eq!(r["error"]["message"], msg, "{src}");
    }
    // The stream continued and nothing changed.
    let r = run(&mut s, "bpy.data.objects");
    assert_eq!(r[0]["value"]["items"], json!(["Camera", "Cube", "Light"]));
    // Error detail and argument span (§13.5).
    let r = &run(&mut s, "bpy.ops.mesh.primitive_cube_add(sizee=2)")[0];
    assert_eq!(r["error"]["detail"]["did_you_mean"], json!(["size"]));
    assert_eq!(
        r["error"]["detail"]["valid"],
        json!([
            "size",
            "calc_uvs",
            "enter_editmode",
            "align",
            "location",
            "rotation",
            "scale"
        ])
    );
    assert_eq!(
        r["error"]["span"],
        json!({"line": 1, "col": 33, "col_chars": 33, "len": 5})
    );
}

#[test]
fn defaults_cursor_and_bare_enum() {
    let d = tmpdir("defaults");
    let mut s = session(&d);
    let r = run(
        &mut s,
        "mesh.primitive_cube_add align=WORLD\nbpy.data.objects['Cube.001'].location\nmesh.primitive_cube_add size=-1",
    );
    assert_eq!(r[0]["ok"], true);
    assert_eq!(
        r[1]["value"],
        json!([0, 0, 0]),
        "unset location = 3D cursor"
    );
    assert_eq!(
        r[2]["reports"][0]["type"], "INFO",
        "clamped values are reported"
    );
    // D-03 / §9.2 wording, shared by operator arguments and assignments.
    assert_eq!(
        r[2]["reports"][0]["message"],
        "frap: size=-1 clamped to hard min 0.0"
    );
    let r = run(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(size=1e13, location=(1e13, 0, 0))",
    );
    let msgs: Vec<&str> = r[0]["reports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        msgs,
        [
            "frap: size=10000000000000.0 clamped to hard max 999999995904.0",
            "frap: location=(10000000000000.0, 0, 0) clamped to hard range [-999999995904.0, 999999995904.0]",
        ]
    );
}

#[test]
fn export_writes_a_glb_and_creates_directories() {
    let d = tmpdir("export");
    let mut s = session(&d);
    let r = run(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='nested/dir/x.glb')",
    );
    assert_eq!(r[0]["ok"], true, "{}", r[0]);
    let bytes = std::fs::read(d.join("nested/dir/x.glb")).unwrap();
    assert_eq!(&bytes[0..4], b"glTF");
    assert_eq!(r[0]["result"]["bytes"], json!(bytes.len()));
    let r = run(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='y.gltf', export_format='GLTF_SEPARATE')",
    );
    // WP 1.23: GLTF_SEPARATE writes the .gltf and its .bin (it was NotImplementedError in M0).
    assert_eq!(r[0]["ok"], true, "{}", r[0]);
    assert_eq!(r[0]["result"]["filepath"], "y.gltf");
    assert!(d.join("y.gltf").is_file());
    assert!(d.join("y.bin").is_file());
}

#[test]
fn minimal_verbosity_drops_context() {
    let d = tmpdir("verbosity");
    let mut opts = SessionOptions::default();
    opts.cwd = d.clone();
    opts.verbosity = frap_session::Verbosity::Minimal;
    let mut s = Session::new(opts).unwrap();
    let r = run(&mut s, "bpy.data.objects['Cube'].location");
    assert_eq!(r[0]["ok"], true, "{}", r[0]);
    assert!(r[0].get("context").is_none(), "{}", r[0]);
    let mut s = session(&d);
    let r = run(&mut s, "bpy.data.objects['Cube'].location");
    assert_eq!(r[0]["context"]["active_object"], "Cube");
}
