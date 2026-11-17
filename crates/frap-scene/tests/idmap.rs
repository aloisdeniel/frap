// SPDX-License-Identifier: GPL-3.0-or-later
//! `IdMap` / `Main` behaviour (§5.2–§5.5): copy-on-write, generations, change logs, renames,
//! `(name, library)` keys and the tuple-key rule, embedded IDs, users and orphans. Expected
//! values are Blender 5.2.4's (probe results quoted per test).

use frap_nodes::{NodeTree, NodeTreeType};
use frap_rna::{EmbeddedSlot, IdCode, IdHeader, IdType, PyExc, Value};
use frap_scene::id::{Library, Material, Object};
use frap_scene::{ChangeLog, Main, RenameMode, RenameResult, startup};

#[test]
fn get_mut_is_copy_on_write_bumps_the_generation_and_logs() {
    let (mut main, _) = startup::factory();
    let snapshot = main.clone();
    let cube = main.objects.find("Cube").unwrap();
    let g0 = main.objects.get(cube).unwrap().header.generation;
    let mut log = ChangeLog::default();
    main.objects.get_mut(cube, &mut log).unwrap().location = [1.0, 2.0, 3.0];
    let g1 = main.objects.get(cube).unwrap().header.generation;
    assert!(g1 > g0, "generation bumps on every mutable access");
    assert_eq!(snapshot.objects.get(cube).unwrap().location, [0.0; 3]);
    assert_eq!(snapshot.objects.get(cube).unwrap().header.generation, g0);
    assert!(log.modified.contains(&cube.any()));
    // Unwritten IDs are shared with the snapshot (no clone).
    let cam = main.objects.find("Camera").unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &main.objects.get_arc(cam).unwrap(),
        &snapshot.objects.get_arc(cam).unwrap()
    ));
    // Generations never repeat, even across clones.
    let mut other = snapshot.clone();
    other.objects.get_mut(cube, &mut log).unwrap();
    assert!(other.objects.get(cube).unwrap().header.generation > g1);
    // insert / remove are logged with names.
    let mut log = ChangeLog::default();
    let e = main
        .objects
        .insert("Empty", Object::new("", None), &mut log);
    assert_eq!(log.added, [e.any()]);
    main.objects.remove(e, &mut log);
    assert_eq!(log.removed, [(e.any(), "Empty".into())]);
}

/// Embedded IDs are mutated through their owner: the owner's generation bumps and the owner is
/// logged (§5.1).
#[test]
fn embedded_ids_mutate_through_the_owner() {
    let (mut main, win) = startup::factory();
    let mat = main.materials.find("Material").unwrap();
    let g0 = main.materials.get(mat).unwrap().header.generation;
    let mut log = ChangeLog::default();
    let tree = main
        .embedded_mut(mat.any(), EmbeddedSlot::NodeTree, &mut log)
        .unwrap();
    assert_eq!(tree.srna().identifier, "ShaderNodeTree");
    assert!(main.materials.get(mat).unwrap().header.generation > g0);
    assert_eq!(
        log.modified.iter().copied().collect::<Vec<_>>(),
        [mat.any()]
    );
    let h = main
        .embedded_header(mat.any(), EmbeddedSlot::NodeTree)
        .unwrap();
    assert!(h.is_embedded_data);
    assert_eq!(h.name, "Shader Nodetree");
    let master = main
        .embedded_header(win.scene.any(), EmbeddedSlot::MasterCollection)
        .unwrap();
    assert_eq!(master.name, "Scene Collection");
    assert!(master.is_embedded_data);
    let mut log = ChangeLog::default();
    main.embedded_mut(win.scene.any(), EmbeddedSlot::MasterCollection, &mut log)
        .unwrap();
    assert!(log.modified.contains(&win.scene.any()));
    assert!(
        main.embedded(win.scene.any(), EmbeddedSlot::NodeTree)
            .is_none()
    );
}

/// `materials.new('Foo')` twice, `m2.name = 'Foo'`, `m3.rename('Foo', mode=…)` (5.2.4).
#[test]
fn rename_modes_follow_blender() {
    let (mut main, _) = startup::factory();
    let mut log = ChangeLog::default();
    let m1 = main.materials.insert("Foo", Material::new(""), &mut log);
    let m2 = main.materials.insert("Foo", Material::new(""), &mut log);
    let name = |m: &Main, r| m.materials.name_of(r).unwrap().to_owned();
    assert_eq!(
        (name(&main, m1), name(&main, m2)),
        ("Foo".into(), "Foo.001".into())
    );
    // Assigning a taken name: the assigned ID gets the lowest free number (its own here).
    let r = main
        .materials
        .rename(m2, "Foo", RenameMode::Never, &mut log);
    assert_eq!(r, RenameResult::UnchangedCollision);
    assert_eq!(name(&main, m2), "Foo.001");
    let m3 = main.materials.insert("Bar", Material::new(""), &mut log);
    let mut log = ChangeLog::default();
    assert_eq!(
        main.materials
            .rename(m3, "Foo", RenameMode::Never, &mut log),
        RenameResult::RenamedCollisionAdjusted
    );
    assert_eq!(name(&main, m3), "Foo.002");
    assert_eq!(log.renamed, [(m3.any(), "Bar".into())]);
    assert_eq!(
        main.materials
            .rename(m3, "Foo", RenameMode::Always, &mut log),
        RenameResult::RenamedCollisionForced
    );
    assert_eq!(name(&main, m3), "Foo");
    assert_eq!(
        name(&main, m1),
        "Foo.002",
        "the other ID takes the lowest free name"
    );
    assert_eq!(
        main.materials
            .rename(m3, "Foo", RenameMode::SameRoot, &mut log),
        RenameResult::Unchanged
    );
    assert_eq!(
        main.materials
            .rename(m1, "Foo.002", RenameMode::Never, &mut log),
        RenameResult::Unchanged
    );
    // SAME_ROOT forces when the current name shares the root.
    assert_eq!(
        main.materials
            .rename(m1, "Foo", RenameMode::SameRoot, &mut log),
        RenameResult::RenamedCollisionForced
    );
    assert_eq!(name(&main, m1), "Foo");
    assert_eq!(
        main.materials
            .rename(m1, "Unique1", RenameMode::Never, &mut log),
        RenameResult::RenamedNoCollision
    );
    assert_eq!(
        main.materials
            .rename(m1, "Unique2", RenameMode::SameRoot, &mut log),
        RenameResult::RenamedNoCollision
    );
    // Empty → the type's default name, uniquified.
    assert_eq!(
        main.materials.rename(m1, "", RenameMode::Never, &mut log),
        RenameResult::RenamedCollisionAdjusted
    );
    assert_eq!(name(&main, m1), "Material.001");
    assert_eq!(
        main.materials.rename(m1, "", RenameMode::Never, &mut log),
        RenameResult::UnchangedCollision
    );
    main.materials
        .rename(m1, &"x".repeat(300), RenameMode::Never, &mut log);
    assert_eq!(name(&main, m1).len(), 255);
    // Renames re-sort (case-insensitive): ['a', 'B', 'Foo', 'Foo.001', 'Material'].
    main.materials.rename(m1, "B", RenameMode::Never, &mut log);
    main.materials.rename(m3, "a", RenameMode::Never, &mut log);
    assert_eq!(
        main.names("materials").unwrap(),
        ["a", "B", "Foo.001", "Material"]
    );
    assert_eq!(main.materials.find("B"), Some(m1));
    assert_eq!(main.materials.find("Foo"), None);
    assert_eq!(main.materials.index_of(m3), Some(0));
    // Main::rename on an IdAny.
    assert_eq!(
        main.rename(m3.any(), "b", RenameMode::Never, &mut log),
        RenameResult::RenamedNoCollision
    );
}

fn linked_group(main: &mut Main, name: &str, lib: frap_rna::IdRef<frap_rna::LibraryId>) {
    let mut log = ChangeLog::default();
    let mut ng = NodeTree::new("", NodeTreeType::Geometry, false);
    ng.header.library = Some(lib);
    ng.header.is_linked_packed = true;
    main.node_groups.insert(name, ng, &mut log);
}

/// `object.shade_auto_smooth` links "Smooth by Angle" from the packed essentials library
/// (`geometry_nodes_essentials.blend.001`); both libraries share one filepath (5.2.4).
fn essentials(main: &mut Main) -> (frap_rna::IdRef<frap_rna::LibraryId>, &'static str) {
    const PATH: &str = "/blender/datafiles/assets/nodes/geometry_nodes_essentials.blend";
    let mut log = ChangeLog::default();
    main.libraries.insert(
        "geometry_nodes_essentials.blend",
        Library::new("", PATH),
        &mut log,
    );
    let mut packed = Library::new("", PATH);
    packed.is_archive = true;
    let lib = main
        .libraries
        .insert("geometry_nodes_essentials.blend.001", packed, &mut log);
    linked_group(main, "Smooth by Angle", lib);
    (lib, PATH)
}

/// Tuple keys, pinned on 5.2.4: `coll[name, None]` finds local IDs only; a string names a
/// library by filepath and the **first** library with that filepath is used, so the group
/// linked from the second (`.blend.001`) library is not found by filepath; a string that is no
/// library filepath is a different KeyError.
#[test]
fn tuple_key_rule() {
    let (mut main, _) = startup::factory();
    let (lib, path) = essentials(&mut main);
    let not_found = "bpy_prop_collection[key, lib]: not found";
    let err = |r: Result<Option<frap_rna::IdAny>, frap_rna::RnaError>| {
        let e = r.unwrap_err();
        assert_eq!(e.exception, PyExc::KeyError);
        e.message
    };
    assert_eq!(
        err(main.find_key_lib("node_groups", "Smooth by Angle", None)),
        not_found
    );
    assert_eq!(
        err(main.find_key_lib("node_groups", "Smooth by Angle", Some(path))),
        not_found
    );
    assert_eq!(
        err(main.find_key_lib(
            "node_groups",
            "Smooth by Angle",
            Some("geometry_nodes_essentials.blend.001")
        )),
        "bpy_prop_collection[id, lib]: lib filepath 'geometry_nodes_essentials.blend.001' does not reference a valid library"
    );
    let cube = main.objects.find("Cube").unwrap();
    assert_eq!(
        main.find_key_lib("objects", "Cube", None).unwrap(),
        Some(cube.any())
    );
    assert_eq!(
        err(main.find_key_lib("objects", "Cube", Some(path))),
        not_found
    );
    assert_eq!(err(main.find_key_lib("objects", "Nope", None)), not_found);
    assert_eq!(
        main.library_by_filepath(path),
        main.libraries.find("geometry_nodes_essentials.blend")
    );
    // The exact (name, library) key reaches the linked group.
    let linked = main
        .node_groups
        .find_in("Smooth by Angle", Some(lib))
        .unwrap();
    assert_eq!(main.node_groups.find("Smooth by Angle"), Some(linked));
    // Unshadowed, the linked group's repr is Blender's plain key (5.2.4:
    // `bpy.data.node_groups['Smooth by Angle']`), which resolves back to it (D-15).
    let p = frap_rna::RnaPtr::id(linked.any(), &frap_spec_rna::NODE_TREE);
    assert_eq!(p.repr(&main), "bpy.data.node_groups['Smooth by Angle']");
    // Shadowed by a local group, the repr names the library; for this archive library the
    // tuple key does not resolve back (the first library with the filepath is used).
    let mut log = ChangeLog::default();
    let local = main.node_groups.insert(
        "Smooth by Angle",
        NodeTree::new("", NodeTreeType::Geometry, false),
        &mut log,
    );
    assert_eq!(
        p.repr(&main),
        format!("bpy.data.node_groups['Smooth by Angle', '{path}']")
    );
    let lp = frap_rna::RnaPtr::id(local.any(), &frap_spec_rna::NODE_TREE);
    assert_eq!(lp.repr(&main), "bpy.data.node_groups['Smooth by Angle']");
}

/// A shadowed group linked from the first library with its filepath gets a tuple-key repr
/// that resolves back to it.
#[test]
fn shadowed_linked_repr_resolves() {
    let (mut main, _) = startup::factory();
    const PATH: &str = "/lib/other.blend";
    let mut log = ChangeLog::default();
    let lib = main
        .libraries
        .insert("other.blend", Library::new("", PATH), &mut log);
    linked_group(&mut main, "G", lib);
    let linked = main.node_groups.find_in("G", Some(lib)).unwrap();
    main.node_groups.insert(
        "G",
        NodeTree::new("", NodeTreeType::Geometry, false),
        &mut log,
    );
    let p = frap_rna::RnaPtr::id(linked.any(), &frap_spec_rna::NODE_TREE);
    assert_eq!(
        p.repr(&main),
        format!("bpy.data.node_groups['G', '{PATH}']")
    );
    assert_eq!(
        main.find_key_lib("node_groups", "G", Some(PATH)).unwrap(),
        Some(linked.any())
    );
}

/// A local ID may share its name with a linked one; `coll['Name']` is the local one; local IDs
/// sort before linked ones (5.2.4: `['Aaa', 'Smooth by Angle', 'Smooth by Angle.001', 'Zzz',
/// 'Smooth by Angle' (linked)]`).
#[test]
fn local_before_linked() {
    let (mut main, _) = startup::factory();
    let (lib, _) = essentials(&mut main);
    let linked = main.node_groups.find("Smooth by Angle").unwrap();
    let mut log = ChangeLog::default();
    let local = main.node_groups.insert(
        "Smooth by Angle",
        NodeTree::new("", NodeTreeType::Geometry, false),
        &mut log,
    );
    assert_eq!(
        main.node_groups.name_of(local),
        Some("Smooth by Angle"),
        "unique per library"
    );
    assert_eq!(main.node_groups.find("Smooth by Angle"), Some(local));
    assert_eq!(
        main.find_key_lib("node_groups", "Smooth by Angle", None)
            .unwrap(),
        Some(local.any())
    );
    assert_eq!(
        main.node_groups.find_in("Smooth by Angle", Some(lib)),
        Some(linked)
    );
    for n in ["Aaa", "Zzz", "Smooth by Angle.001"] {
        main.node_groups.insert(
            n,
            NodeTree::new("", NodeTreeType::Geometry, false),
            &mut log,
        );
    }
    let order: Vec<(String, bool)> = main
        .node_groups
        .iter()
        .map(|(_, t)| (t.header.name.to_string(), t.header.library.is_some()))
        .collect();
    assert_eq!(
        order,
        [
            ("Aaa".to_owned(), false),
            ("Smooth by Angle".to_owned(), false),
            ("Smooth by Angle.001".to_owned(), false),
            ("Zzz".to_owned(), false),
            ("Smooth by Angle".to_owned(), true),
        ]
    );
    assert_eq!(main.node_groups.index_of(linked), Some(4));
    // name_full of a linked ID names its library.
    let h = frap_rna::RnaPtr::id(linked.any(), &frap_spec_rna::NODE_TREE);
    let full = frap_scene::find_hook(&frap_spec_rna::NODE_TREE, "name_full")
        .unwrap()
        .get
        .unwrap();
    let host = HostOnly(main.clone(), startup::factory().1);
    let frap_rna::Field::Value(Value::Str(s)) =
        full(&host, &h, frap_spec_rna::id::NAME_FULL).unwrap()
    else {
        panic!()
    };
    assert_eq!(s, "Smooth by Angle [geometry_nodes_essentials.blend.001]");
    assert_eq!(
        main.users(
            main.libraries
                .find("geometry_nodes_essentials.blend")
                .unwrap()
                .any()
        ),
        1
    );
}

/// A read-only host for get hooks.
struct HostOnly(Main, frap_scene::WindowState);
impl frap_scene::HostRead for HostOnly {
    fn main(&self) -> &Main {
        &self.0
    }
    fn win(&self) -> &frap_scene::WindowState {
        &self.1
    }
    fn edit(&self) -> &frap_scene::EditState {
        unimplemented!()
    }
    fn ctx(&self) -> frap_scene::Context<'_> {
        unimplemented!()
    }
    fn mesh_view(&self, _: frap_rna::IdRef<frap_rna::MeshId>) -> std::sync::Arc<frap_mesh::Mesh> {
        unimplemented!()
    }
}

/// Users (5.2.4 factory: 1 each; a fresh mesh 0, 1 with an object, 2 with a fake user; an
/// unlinked object 0; parenting is not a user), remove with unlink, orphans.
#[test]
fn users_remove_and_orphans() {
    let (mut main, win) = startup::factory();
    let users = |m: &Main, coll: &str, n: &str| m.users(m.find(coll, n).unwrap());
    for (c, n) in [
        ("objects", "Camera"),
        ("objects", "Cube"),
        ("meshes", "Cube"),
        ("materials", "Material"),
        ("cameras", "Camera"),
        ("lights", "Light"),
        ("worlds", "World"),
        ("collections", "Collection"),
    ] {
        assert_eq!(users(&main, c, n), 1, "{c}['{n}']");
    }
    // The scene's user is the window (added by the ID.users hook).
    assert_eq!(main.users(win.scene.any()), 0);
    let mut log = ChangeLog::default();
    let me = main.meshes.insert("M", frap_mesh::Mesh::new(""), &mut log);
    assert_eq!(main.users(me.any()), 0);
    let ob = main
        .objects
        .insert("O", Object::new("", Some(me.any())), &mut log);
    assert_eq!(main.users(me.any()), 1);
    assert_eq!(main.users(ob.any()), 0);
    main.meshes
        .get_mut(me, &mut log)
        .unwrap()
        .header
        .use_fake_user = true;
    assert_eq!(main.users(me.any()), 2);
    main.meshes
        .get_mut(me, &mut log)
        .unwrap()
        .header
        .use_fake_user = false;
    let cube = main.objects.find("Cube").unwrap();
    let cam = main.objects.find("Camera").unwrap();
    main.objects.get_mut(cube, &mut log).unwrap().parent = Some(cam);
    assert_eq!(main.users(cam.any()), 1, "a parent pointer is not a user");
    assert_eq!(
        main.orphans(),
        [ob.any()],
        "the unlinked object (its mesh has a user)"
    );
    // Removing the Cube object unlinks it everywhere and leaves its mesh orphaned.
    let mut log = ChangeLog::default();
    assert!(main.remove(cube.any(), &mut log));
    assert!(main.objects.find("Cube").is_none());
    let coll = main
        .collections
        .get(main.collections.find("Collection").unwrap())
        .unwrap();
    assert!(!coll.objects.contains(&cube));
    let sc = main.scenes.get(win.scene).unwrap();
    assert!(sc.view_layers[0].bases.iter().all(|b| b.object != cube));
    assert_eq!(sc.view_layers[0].active, None);
    let mesh = main.meshes.find("Cube").unwrap();
    assert_eq!(main.users(mesh.any()), 0);
    assert!(main.orphans().contains(&mesh.any()));
    assert!(
        log.removed
            .iter()
            .any(|(a, n)| *a == cube.any() && n == "Cube")
    );
    // Removing the camera object clears scene.camera.
    let mut log = ChangeLog::default();
    main.remove(cam.any(), &mut log);
    assert_eq!(main.scenes.get(win.scene).unwrap().camera, None);
    assert!(!main.remove(cam.any(), &mut log), "already removed");
    // Removing the material empties the mesh slot (do_unlink).
    let mat = main.materials.find("Material").unwrap();
    main.remove(mat.any(), &mut log);
    assert_eq!(main.meshes.get(mesh).unwrap().materials, [None]);
    let _ = (IdCode::OB, IdHeader::new(""));
    assert!(main.header(mat.any()).is_none());
    assert_eq!(main.objects.get(ob).unwrap().header().name, "O");
}
