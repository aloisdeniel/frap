// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data` ordering, naming and copy-on-write (ARCHITECTURE §5.3–§5.4).

use frap_scene::id::Object;
use frap_scene::{ChangeLog, Main, startup};

#[test]
fn factory_matches_blender() {
    let (main, win) = startup::factory();
    assert_eq!(main.names("objects").unwrap(), ["Camera", "Cube", "Light"]);
    assert_eq!(main.names("meshes").unwrap(), ["Cube"]);
    assert_eq!(main.names("materials").unwrap(), ["Material"]);
    let prefs = frap_scene::Preferences::default();
    let ctx = frap_scene::Context {
        main: &main,
        win: &win,
        prefs: &prefs,
    };
    let names = |v: Vec<frap_rna::IdRef<frap_rna::ObjectId>>| -> Vec<String> {
        v.into_iter()
            .map(|r| main.objects.name_of(r).unwrap().to_owned())
            .collect()
    };
    assert_eq!(names(ctx.selected_objects()), ["Cube"]);
    assert_eq!(names(ctx.active_object().into_iter().collect()), ["Cube"]);
    assert_eq!(
        names(ctx.view_layer().bases.iter().map(|b| b.object).collect()),
        ["Cube", "Light", "Camera"]
    );
    let cube = main
        .objects
        .get(main.objects.find("Cube").unwrap())
        .unwrap();
    assert_eq!(cube.object_type(), "MESH");
    let cam = main
        .objects
        .get(main.objects.find("Camera").unwrap())
        .unwrap();
    assert_eq!(cam.location, [7.358_891_5_f32, -6.925_791, 4.958_309]);
    // The factory cube has its own vertex order (not primitive_cube_add's).
    let me = main.meshes.get(main.meshes.find("Cube").unwrap()).unwrap();
    assert_eq!(me.positions()[0], [1.0, 1.0, 1.0]);
    assert_eq!(me.materials.len(), 1);
}

#[test]
fn listbase_order_is_case_insensitive_non_natural_and_stable() {
    // [FID App. A Q3]: ['a10','a9','b','B','Cube','Cube.002','Cube.010','Cube.9']
    let mut main = Main::default();
    let mut log = ChangeLog::default();
    for n in [
        "Cube.9", "b", "Cube.010", "a9", "B", "Cube", "a10", "Cube.002",
    ] {
        main.objects.insert(n, Object::new("", None), &mut log);
    }
    assert_eq!(
        main.names("objects").unwrap(),
        [
            "a10", "a9", "b", "B", "Cube", "Cube.002", "Cube.010", "Cube.9"
        ]
    );
    let r = main.objects.insert("Cube", Object::new("", None), &mut log);
    assert_eq!(main.objects.name_of(r), Some("Cube.001"));
    assert_eq!(
        main.objects.at(-1).and_then(|r| main.objects.name_of(r)),
        Some("Cube.9")
    );
}

#[test]
fn snapshot_is_copy_on_write() {
    let (mut main, _) = startup::factory();
    let snapshot = main.clone();
    let mut log = ChangeLog::default();
    let cube = main.objects.find("Cube").unwrap();
    main.objects.get_mut(cube, &mut log).unwrap().location = [1.0, 2.0, 3.0];
    assert_eq!(snapshot.objects.get(cube).unwrap().location, [0.0; 3]);
    assert_eq!(main.objects.get(cube).unwrap().location, [1.0, 2.0, 3.0]);
    assert_eq!(log.modified.len(), 1);
}
