// SPDX-License-Identifier: GPL-3.0-or-later
//! M2 W0 wiring: a `.frap` saved with an object in Edit Mode loads that object as `EDIT`, and
//! `wm.open_mainfile` hands it to `frap_op::helpers::edit_mode::reenter_after_load`. Until WP 2.2
//! re-enters Edit Mode there, the object comes back in Object Mode with the M1 WARNING.

mod support;

use frap_scene::ObjectMode;
use support::*;

#[test]
fn objects_saved_in_edit_mode_reach_the_reentry_helper() {
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
    let reports = j["reports"].to_string();
    assert!(
        reports.contains("\\\"Cube\\\" was saved in Edit Mode"),
        "{reports}"
    );
    assert_eq!(get(&mut s, "bpy.data.objects['Cube'].mode"), "OBJECT");
}
