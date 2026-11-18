// SPDX-License-Identifier: GPL-3.0-or-later
//! A `.frap` saved with an object in Edit Mode loads that object as `EDIT`, and
//! `wm.open_mainfile` hands it to `frap_op::helpers::edit_mode::reenter_after_load`, which
//! re-enters Edit Mode (WP 2.2): the object keeps its mode and its mesh gets an edit BMesh
//! again, as Blender does for a `.blend` saved in Edit Mode.

mod support;

use frap_scene::ObjectMode;
use support::*;

#[test]
fn objects_saved_in_edit_mode_re_enter_it_on_load() {
    let d = Scratch::new("editload");
    let (mut main, win) = frap_scene::startup::factory();
    let cube = main.objects.find("Cube").unwrap();
    let mut log = frap_scene::ChangeLog::default();
    main.objects.get_mut(cube, &mut log).unwrap().mode = ObjectMode::Edit;
    let saved = frap_project::scene_map::save(&main, &win, &Default::default(), None).unwrap();
    let path = d.path().join("edit.frap");
    std::fs::write(&path, &saved.bytes).unwrap();

    let mut s = session_in(d.path(), false);
    let r = run(&mut s, "bpy.ops.wm.open_mainfile(filepath='edit.frap')");
    assert!(r.ok(), "{}", r.to_json());
    let j = r.to_json();
    assert_eq!(j["context"]["mode"], "EDIT_MESH", "{j}");
    assert_eq!(j["edit_mesh"]["Cube"]["verts"], 8, "{j}");
    assert_eq!(get(&mut s, "bpy.data.objects['Cube'].mode"), "EDIT");
    let me = s.main().meshes.find("Cube").unwrap();
    assert!(s.host().edit().meshes.contains_key(&me.key()));
}
