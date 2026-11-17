// SPDX-License-Identifier: GPL-3.0-or-later
//! M1 stabilization round 1: the end-to-end responses the exit verifier found missing. The
//! values are Blender 5.2.4's (goldens: `scene/python_only_members`, `scene/dimensions`,
//! `rna/frame_unclamped`, `rna/sockets_unbounded`, `errors/layer_objects_active`,
//! `context/visible_members`, `data/mesh_api`); these tests pin the frap side of the protocol
//! (hints, value encoding, `--file` startup) that goldens do not see.

mod support;

use frap_session::{Session, SessionOptions, Startup};
use serde_json::json;
use support::*;

#[test]
fn operator_angle_arguments_get_the_degrees_hint() {
    let mut s = session();
    for (src, shown) in [
        (
            "bpy.ops.mesh.primitive_cube_add(rotation=(90, 0, 0))",
            "rotation=90,0,0",
        ),
        ("mesh.primitive_cube_add rotation=0,0,90", "rotation=0,0,90"),
        ("bpy.ops.transform.rotate(value=90)", "value=90"),
    ] {
        let r = one(&mut s, src);
        assert_eq!(r["status"], "FINISHED", "{src}: {r}");
        let h = &r["hints"][0];
        assert_eq!(h["code"], "ANGLE_LIKELY_DEGREES", "{src}: {r}");
        assert_eq!(
            h["message"],
            format!("{shown} is in radians (90 rad); did you mean radians(90)?")
        );
    }
    let r = one(&mut s, "bpy.ops.object.camera_add(rotation=(1.5, 0, 0.5))");
    assert_eq!(r["hints"], json!([]));
}

#[test]
fn python_only_collection_members_read_as_lists() {
    let mut s = session();
    run(
        &mut s,
        "bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']\n\
         bpy.data.objects['Light'].parent = bpy.data.objects['Cube']",
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Camera'].children"),
        json!([{"$ref": "bpy.data.objects['Cube']"}])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Camera'].children_recursive"),
        json!([{"$ref": "bpy.data.objects['Cube']"}, {"$ref": "bpy.data.objects['Light']"}])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].users_collection"),
        json!([{"$ref": "bpy.data.collections['Collection']"}])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].users_scene"),
        json!([{"$ref": "bpy.data.scenes['Scene']"}])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].users_collection[0].name"),
        json!("Collection")
    );
    let r = one(&mut s, "print(bpy.data.objects['Light'].children)");
    assert_eq!(r["status"], "FINISHED", "{r}");
}

#[test]
fn dimensions_and_bound_box_are_evaluated_bounds() {
    let mut s = session();
    run(&mut s, "bpy.data.objects['Cube'].scale = (2, -1, 3)");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].dimensions"),
        json!([4, 2, 6])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Camera'].dimensions"),
        json!([0, 0, 0])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].bound_box[6]"),
        json!([1, 1, 1])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].bound_box")[0],
        json!([-1, -1, -1])
    );
    run(&mut s, "bpy.data.objects['Cube'].dimensions = (4, 5, 0)");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].scale"),
        json!([2, -2.5, 0])
    );
}

#[test]
fn frame_current_keeps_large_frames() {
    let mut s = session();
    let r = one(&mut s, "bpy.context.scene.frame_current = 1000000000");
    assert_eq!(r["reports"], json!([]), "{r}");
    assert_eq!(
        val(&mut s, "bpy.context.scene.frame_current"),
        json!(1000000000)
    );
    run(&mut s, "bpy.context.scene.frame_current = -5");
    assert_eq!(val(&mut s, "bpy.context.scene.frame_current"), json!(0));
}

#[test]
fn context_object_lists_and_api_struct_assignment_prefix() {
    let mut s = session();
    run(&mut s, "bpy.data.objects['Cube'].hide_set(True)");
    assert_eq!(
        val(&mut s, "bpy.context.visible_objects"),
        json!([{"$ref": "bpy.data.objects['Light']"}, {"$ref": "bpy.data.objects['Camera']"}])
    );
    let (exc, _, msg) = err(&mut s, "bpy.context.view_layer.objects.active = 'Cube'");
    assert_eq!(exc, "TypeError");
    assert_eq!(
        msg,
        "BPy_PropertyRNA - Attribute (setattr): LayerObjects.active expected a Object type, not str"
    );
}

#[test]
fn mesh_update_after_from_pydata() {
    let mut s = session();
    run(
        &mut s,
        "bpy.data.meshes.new('M')\n\
         bpy.data.meshes['M'].from_pydata([(0, 0, 0), (1, 0, 0), (0, 1, 0)], [], [(0, 1, 2)])",
    );
    let r = one(&mut s, "bpy.data.meshes['M'].update()");
    assert_eq!(r["status"], "FINISHED", "{r}");
    assert_eq!(val(&mut s, "bpy.data.meshes['M'].validate()"), json!(false));
    assert_eq!(
        val(
            &mut s,
            "bpy.data.meshes['M'].attributes.new('w', 'FLOAT', 'POINT')"
        ),
        json!({"$ptr": "bpy.data.meshes['M'].attributes[\"w\"]", "type": "FloatAttribute"})
    );
}

#[test]
fn file_startup_opens_the_saved_frap() {
    let dir = std::env::temp_dir().join(format!("frap-stab-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.frap");
    let mut s = session_with(|o| o.cwd = dir.clone());
    run(
        &mut s,
        "bpy.data.objects['Cube'].location.x = 7\n\
         bpy.ops.wm.save_as_mainfile(filepath='scene.frap')",
    );
    let mut opts = SessionOptions::default();
    opts.startup = Startup::File(path.clone());
    opts.cwd = dir.clone();
    let mut s2 = Session::new(opts).expect("--file session");
    assert_eq!(
        val(&mut s2, "bpy.data.objects['Cube'].location.x"),
        json!(7)
    );
    assert_eq!(
        val(&mut s2, "bpy.data.filepath"),
        json!(path.to_string_lossy())
    );
    let mut bad = SessionOptions::default();
    bad.startup = Startup::File(dir.clone());
    bad.cwd = dir.clone();
    assert!(Session::new(bad).is_err());
    std::fs::remove_dir_all(&dir).ok();
}
