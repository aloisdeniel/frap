// SPDX-License-Identifier: GPL-3.0-or-later
//! `wm.save_as_mainfile`, `wm.save_mainfile`, `wm.open_mainfile`, `wm.revert_mainfile`,
//! `wm.read_homefile`, `wm.read_factory_settings` through a real session (WP 1.23, §15).

mod support;

use frap_nodes::{NodeTree, NodeTreeType};
use frap_scene::ChangeLog;
use frap_scene::id::Library;
use serde_json::json;
use support::*;

#[test]
fn save_then_open_restores_the_state() {
    let d = Scratch::new("save-open");
    let mut s = session_in(d.path(), false);
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_cone_add(vertices=7, location=(2, 0, 0))\n\
         bpy.data.objects['Cube'].location = (0.1, 0.2, 0.3)\n\
         bpy.data.objects['Cube'].parent = bpy.data.objects['Cone']\n\
         bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.5\n\
         bpy.context.scene.cursor.location = (1, 2, 3)\n\
         bpy.data.objects['Light'].select_set(True)\n\
         bpy.ops.wm.save_as_mainfile(filepath='a.frap')",
    );
    let before = saved_state(&s);
    ok(
        &mut s,
        "bpy.data.objects['Cube'].location = (9, 9, 9)\n\
         bpy.ops.object.select_all(action='DESELECT')\n\
         bpy.data.objects.remove(bpy.data.objects['Camera'])",
    );
    let r = ok(&mut s, "bpy.ops.wm.open_mainfile(filepath='a.frap')");
    assert_eq!(
        r[0].result.as_ref().unwrap()["filepath"],
        json!(d.path().join("a.frap").to_string_lossy())
    );
    assert_same_state(&state(&s), &before, "open after save");
    assert_eq!(
        get(&mut s, "bpy.data.objects['Cube'].location"),
        json!([0.1, 0.2, 0.3])
    );
    assert_eq!(get(&mut s, "bpy.context.active_object.name"), json!("Cone"));
    assert_eq!(get(&mut s, "bpy.data.is_dirty"), json!(false));
}

#[test]
fn saves_are_byte_identical() {
    let d = Scratch::new("bytes");
    let mut s = session_in(d.path(), false);
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_monkey_add(location=(1, 2, 3))\n\
         bpy.data.objects['Suzanne'].data.shade_smooth()\n\
         bpy.ops.wm.save_as_mainfile(filepath='one.frap', copy=True)\n\
         bpy.ops.wm.save_as_mainfile(filepath='two.frap', copy=True)",
    );
    let one = std::fs::read(d.path().join("one.frap")).unwrap();
    let two = std::fs::read(d.path().join("two.frap")).unwrap();
    assert_eq!(one, two);
    // Opening and saving again gives the same bytes.
    ok(
        &mut s,
        "bpy.ops.wm.open_mainfile(filepath='one.frap')\nbpy.ops.wm.save_as_mainfile(filepath='three.frap')",
    );
    assert_eq!(std::fs::read(d.path().join("three.frap")).unwrap(), one);
}

#[test]
fn filepath_and_reports_follow_blender() {
    let d = Scratch::new("filepath");
    let mut s = session_in(d.path(), false);
    assert_eq!(get(&mut s, "bpy.data.filepath"), json!(""));
    let r = ok(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='first.frap')");
    assert_eq!(r[0].reports[0].message, "Saved as \"first.frap\"");
    let first = d.path().join("first.frap").to_string_lossy().into_owned();
    assert_eq!(get(&mut s, "bpy.data.filepath"), json!(first));
    let r = ok(
        &mut s,
        "bpy.ops.wm.save_as_mainfile(filepath='copy.frap', copy=True)",
    );
    assert_eq!(r[0].reports[0].message, "Saved copy as \"copy.frap\"");
    assert_eq!(get(&mut s, "bpy.data.filepath"), json!(first));
    let r = ok(&mut s, "bpy.ops.wm.save_mainfile()");
    assert_eq!(r[0].reports[0].message, "Saved \"first.frap\"");
    let r = ok(&mut s, "bpy.ops.wm.save_as_mainfile()");
    assert_eq!(r[0].reports[0].message, "Saved as \"first.frap\"");
    let r = ok(&mut s, "bpy.ops.wm.save_mainfile(filepath='second.frap')");
    assert_eq!(r[0].reports[0].message, "Saved \"second.frap\"");
    assert!(
        get(&mut s, "bpy.data.filepath")
            .as_str()
            .unwrap()
            .ends_with("second.frap")
    );
    // Reading the home file forgets the path.
    ok(&mut s, "bpy.ops.wm.read_homefile()");
    assert_eq!(get(&mut s, "bpy.data.filepath"), json!(""));
    // Props that do not apply are accepted with an INFO report.
    let r = ok(
        &mut s,
        "bpy.ops.wm.save_as_mainfile(filepath='c.frap', compress=True, relative_remap=False)",
    );
    let infos: Vec<&str> = r[0].reports.iter().map(|r| r.message.as_str()).collect();
    assert!(
        infos.contains(
            &"frap: wm.save_as_mainfile: 'compress' does not apply to frap and was ignored"
        ),
        "{infos:?}"
    );
    assert!(
        infos.iter().any(|m| m.contains("'relative_remap'")),
        "{infos:?}"
    );
}

#[test]
fn errors_use_blenders_messages() {
    let d = Scratch::new("errors");
    let mut s = session_in(d.path(), false);
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.save_mainfile()");
    assert_eq!(code, "OP_EXEC_ERROR");
    assert_eq!(
        msg,
        "Error: Unable to save an unsaved file with an empty or unset \"filepath\" property"
    );
    let (code, msg) = err_code(
        &mut s,
        "bpy.ops.wm.save_as_mainfile(filepath='missing/dir/a.frap')",
    );
    assert_eq!(code, "IO_ERROR");
    assert_eq!(
        msg,
        format!(
            "Error: Cannot open file {}@ for writing: No such file or directory",
            d.path()
                .join("missing")
                .join("dir")
                .join("a.frap")
                .display()
        )
    );
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.open_mainfile(filepath='nope.frap')");
    assert_eq!(code, "IO_ERROR");
    assert_eq!(
        msg,
        format!(
            "Error: Cannot read file \"{}\": No such file or directory",
            d.path().join("nope.frap").display()
        )
    );
    let (code, _) = err_code(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='x.blend')");
    assert_eq!(code, "IO_UNSUPPORTED_FORMAT");
    assert!(!d.path().join("x.blend").exists());
    std::fs::write(d.path().join("y.blend"), b"BLENDER-v502").unwrap();
    let (code, _) = err_code(&mut s, "bpy.ops.wm.open_mainfile(filepath='y.blend')");
    assert_eq!(code, "IO_UNSUPPORTED_FORMAT");
    std::fs::write(d.path().join("junk.frap"), b"not a zip").unwrap();
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.open_mainfile(filepath='junk.frap')");
    assert_eq!(code, "IO_UNSUPPORTED_FORMAT");
    assert!(
        msg.starts_with("Error: File format is not supported in file"),
        "{msg}"
    );
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.revert_mainfile()");
    assert_eq!(code, "OP_POLL_FAILED");
    assert_eq!(
        msg,
        "Operator bpy.ops.wm.revert_mainfile.poll() failed, context is incorrect"
    );
    assert_eq!(
        get(&mut s, "bpy.ops.wm.revert_mainfile.poll()"),
        json!(false)
    );
    // Nothing was left behind by the failed saves (no temporary files).
    let mut names: Vec<String> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["junk.frap", "y.blend"]);
}

#[test]
fn revert_discards_edits() {
    let d = Scratch::new("revert");
    let mut s = session_in(d.path(), false);
    ok(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='r.frap')");
    let before = state(&s);
    ok(
        &mut s,
        "bpy.data.objects['Cube'].location = (5, 5, 5)\nbpy.ops.mesh.primitive_plane_add()",
    );
    assert_eq!(
        get(&mut s, "bpy.ops.wm.revert_mainfile.poll()"),
        json!(true)
    );
    ok(&mut s, "bpy.ops.wm.revert_mainfile()");
    assert_same_state(&state(&s), &before, "revert");
}

#[test]
fn orphans_are_dropped() {
    let d = Scratch::new("orphans");
    let mut s = session_in(d.path(), false);
    ok(
        &mut s,
        "bpy.ops.object.select_all(action='DESELECT')\n\
         bpy.data.objects['Cube'].select_set(True)\n\
         bpy.ops.object.delete()\n\
         bpy.data.materials.new('Loose')\n\
         bpy.data.materials.new('Kept')\n\
         bpy.data.materials['Kept'].use_fake_user = True",
    );
    let r = ok(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='o.frap')");
    let result = r[0].result.as_ref().unwrap();
    // The Cube's mesh, its material (only used by that mesh: Blender writes users at save
    // time, so the Material is kept until the next save) and `Loose`.
    assert_eq!(result["orphans_dropped"], json!(2), "{}", r[0].to_json());
    assert!(
        r[0].reports
            .iter()
            .any(|r| r.message.contains("meshes[\"Cube\"]"))
    );
    // The in-memory state is unchanged by the save.
    assert_eq!(s.main().meshes.len(), 1);
    ok(&mut s, "bpy.ops.wm.open_mainfile(filepath='o.frap')");
    assert_eq!(s.main().meshes.len(), 0);
    assert_eq!(get(&mut s, "bpy.data.materials.find('Loose')"), json!(-1));
    assert_eq!(
        get(&mut s, "bpy.data.materials['Kept'].use_fake_user"),
        json!(true)
    );
}

#[test]
fn read_homefile_and_factory_settings() {
    let d = Scratch::new("home");
    let mut s = session_in(d.path(), false);
    ok(&mut s, "bpy.ops.wm.read_homefile(use_empty=True)");
    assert_eq!(s.main().objects.len(), 0);
    assert_eq!(get(&mut s, "bpy.context.scene.name"), json!("Scene"));
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_plane_add()\nbpy.ops.wm.read_factory_settings()",
    );
    let factory = state(&session_in(d.path(), false));
    assert_same_state(&state(&s), &factory, "read_factory_settings");
    ok(&mut s, "bpy.ops.wm.read_factory_settings(use_empty=True)");
    let empty = state(&session_in(d.path(), true));
    assert_same_state(&state(&s), &empty, "read_factory_settings(use_empty=True)");
    ok(&mut s, "bpy.ops.wm.read_homefile()");
    assert_same_state(&state(&s), &factory, "read_homefile");
    // A .frap as the startup file: loaded, but bpy.data.filepath stays empty.
    ok(
        &mut s,
        "bpy.data.objects['Cube'].name = 'Box'\nbpy.ops.wm.save_as_mainfile(filepath='home.frap')",
    );
    ok(&mut s, "bpy.ops.wm.read_homefile(filepath='home.frap')");
    assert_eq!(get(&mut s, "bpy.data.objects[0].name"), json!("Box"));
    assert_eq!(get(&mut s, "bpy.data.filepath"), json!(""));
}

#[test]
fn double_slash_paths_resolve_against_the_saved_file() {
    let d = Scratch::new("slashes");
    std::fs::create_dir_all(d.path().join("proj")).unwrap();
    let mut s = session_in(d.path(), false);
    // Unsaved: `//` is the cwd.
    ok(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='//unsaved.glb')",
    );
    assert!(d.path().join("unsaved.glb").is_file());
    ok(
        &mut s,
        "bpy.ops.wm.save_as_mainfile(filepath='proj/p.frap')",
    );
    ok(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='//out/rel.glb')",
    );
    assert!(d.path().join("proj/out/rel.glb").is_file());
    ok(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='//p2.frap')");
    assert!(d.path().join("proj/p2.frap").is_file());
    ok(
        &mut s,
        "bpy.ops.wm.read_homefile(use_empty=True)\nbpy.ops.wm.open_mainfile(filepath='proj/p2.frap')",
    );
    ok(
        &mut s,
        "bpy.ops.import_scene.gltf(filepath='//out/rel.glb')",
    );
    assert_eq!(get(&mut s, "bpy.data.objects.find('Cube.001')"), json!(2));
}

/// A `.frap` written from Rust with a library, a linked group shadowed by a local one, and a
/// second scene that is the window scene: the operators keep all of it.
#[test]
fn linked_groups_and_window_state_round_trip() {
    let d = Scratch::new("linked");
    let (mut main, mut win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let lib = main.libraries.insert(
        "Essentials",
        Library::new("Essentials", "//Essentials.blend"),
        &mut log,
    );
    let mut linked = NodeTree::new("Smooth by Angle", NodeTreeType::Geometry, false);
    linked.header.library = Some(lib);
    linked.header.use_fake_user = true;
    main.node_groups.insert("Smooth by Angle", linked, &mut log);
    let mut local = NodeTree::new("Smooth by Angle", NodeTreeType::Geometry, false);
    local.header.use_fake_user = true;
    main.node_groups.insert("Smooth by Angle", local, &mut log);
    let layers = main.scenes.get(win.scene).unwrap().view_layers.clone();
    let other = main.scenes.insert(
        "Other",
        frap_scene::id::Scene::new("Other", layers),
        &mut log,
    );
    win.scene = other;
    win.view3d.view_matrix = view_matrix();
    let saved = frap_project::scene_map::save(&main, &win, &Default::default(), None).unwrap();
    std::fs::write(d.path().join("linked.frap"), &saved.bytes).unwrap();

    let mut s = session_in(d.path(), false);
    ok(&mut s, "bpy.ops.wm.open_mainfile(filepath='linked.frap')");
    assert_eq!(get(&mut s, "bpy.context.scene.name"), json!("Other"));
    assert_eq!(s.main().node_groups.len(), 2);
    assert_eq!(
        get(&mut s, "bpy.data.node_groups[1].library.filepath"),
        json!("//Essentials.blend")
    );
    assert_eq!(get(&mut s, "bpy.data.node_groups[0].library"), json!(null));
    assert_eq!(s.host().win().view3d.view_matrix, win.view3d.view_matrix);
    ok(&mut s, "bpy.ops.wm.save_mainfile()");
    assert_eq!(
        std::fs::read(d.path().join("linked.frap")).unwrap(),
        saved.bytes
    );
}

fn view_matrix() -> glam::Mat4 {
    glam::Mat4::from_cols_array(&[
        0.6, 0.0, -0.8, 0.0, 0.0, 1.0, 0.0, 0.0, 0.8, 0.0, 0.6, 0.0, 1.0, -2.0, -10.0, 1.0,
    ])
}

/// §15: the glTF add-on preferences are saved state: `open_mainfile` restores them, a save
/// writes them back, `read_homefile` keeps them and `read_factory_settings` resets them.
#[test]
fn addon_preferences_round_trip_and_reset() {
    let d = Scratch::new("prefs");
    let (main, win) = frap_scene::startup::factory();
    let mut prefs = frap_scene::Preferences::default();
    prefs.addons.insert(
        "io_scene_gltf2".into(),
        [("allow_embedded_format".into(), frap_rna::Value::Bool(true))]
            .into_iter()
            .collect(),
    );
    let saved = frap_project::scene_map::save(&main, &win, &prefs, None).unwrap();
    std::fs::write(d.path().join("p.frap"), &saved.bytes).unwrap();

    let mut s = session_in(d.path(), false);
    assert!(s.host().ctx().prefs.addons.is_empty());
    ok(&mut s, "bpy.ops.wm.open_mainfile(filepath='p.frap')");
    assert_eq!(s.host().ctx().prefs.addons, prefs.addons);
    ok(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='q.frap')");
    assert_eq!(
        std::fs::read(d.path().join("q.frap")).unwrap(),
        saved.bytes,
        "the restored preferences are saved again"
    );
    ok(&mut s, "bpy.ops.wm.read_homefile()");
    assert_eq!(s.host().ctx().prefs.addons, prefs.addons);
    ok(&mut s, "bpy.ops.wm.read_factory_settings()");
    assert!(s.host().ctx().prefs.addons.is_empty());
    // A failing open leaves the preferences untouched (statement rollback, §12).
    ok(&mut s, "bpy.ops.wm.open_mainfile(filepath='p.frap')");
    let _ = err_code(&mut s, "bpy.ops.wm.open_mainfile(filepath='nope.frap')");
    assert_eq!(s.host().ctx().prefs.addons, prefs.addons);
}

/// `wm.read_homefile(filepath=…)` errors, as Blender 5.2.4 words them (the path as given).
#[test]
fn read_homefile_errors_use_blenders_messages() {
    let d = Scratch::new("home-errors");
    let mut s = session_in(d.path(), false);
    let before = state(&s);
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.read_homefile(filepath='nope.frap')");
    assert_eq!(code, "IO_ERROR");
    assert_eq!(
        msg,
        "Error: Cannot read alternative start-up file: \"nope.frap\""
    );
    std::fs::write(d.path().join("junk.frap"), b"not a zip").unwrap();
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.read_homefile(filepath='junk.frap')");
    assert_eq!(code, "IO_UNSUPPORTED_FORMAT");
    assert_eq!(msg, "Error: Could not read \"junk.frap\"");
    std::fs::write(d.path().join("y.blend"), b"BLENDER-v502").unwrap();
    let (code, msg) = err_code(&mut s, "bpy.ops.wm.read_homefile(filepath='y.blend')");
    assert_eq!(code, "IO_UNSUPPORTED_FORMAT");
    assert_eq!(msg, "Error: Could not read \"y.blend\"");
    assert_same_state(&state(&s), &before, "failed read_homefile");
}
