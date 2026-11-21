// SPDX-License-Identifier: GPL-3.0-or-later
//! Round trips of the `scene.json` mapping (WP 1.23).

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use frap_nodes::{NodeTree, NodeTreeType};
use frap_rna::{IdProp, IdRef, ObjectId, Value};
use frap_scene::generated::modifier_settings::{ModifierSettings, ModifierType};
use frap_scene::id::{
    Camera, EncodedImage, Image, Library, Light, Material, Modifier, Object, PointCloud, SlotLink,
    SlotOverride,
};
use frap_scene::{Base, BaseFlag, ChangeLog, Main, Preferences, WindowState};
use serde_json::{Value as J, json};
use smol_str::SmolStr;

use super::*;

/// `Debug` of the state with `IdKey`s renumbered by first appearance and generations zeroed:
/// two states that differ only in keys and generations (a save → open) print the same.
fn assert_same(a: &str, b: &str) {
    if a != b {
        let (a, b) = (a.replace(", ", ",\n"), b.replace(", ", ",\n"));
        let (la, lb): (Vec<_>, Vec<_>) = (a.lines().collect(), b.lines().collect());
        let i = la
            .iter()
            .zip(&lb)
            .position(|(x, y)| x != y)
            .unwrap_or(la.len().min(lb.len()));
        let lo = i.saturating_sub(12);
        panic!(
            "states differ at line {i}:\n--- loaded\n{}\n--- expected\n{}",
            la[lo..(i + 5).min(la.len())].join("\n"),
            lb[lo..(i + 5).min(lb.len())].join("\n")
        );
    }
}

pub(crate) fn normalized(main: &Main, win: &WindowState) -> String {
    let text = format!("{main:?}\n{win:?}");
    let mut out = String::with_capacity(text.len());
    let mut ids: Vec<String> = Vec::new();
    let mut rest = text.as_str();
    let pats = ["IdKey(", ">(", "session_uid: ", "generation: "];
    while let Some((at, pat)) = pats
        .iter()
        .filter_map(|p| rest.find(p).map(|i| (i, *p)))
        .min_by_key(|(i, _)| *i)
    {
        let end = at + pat.len();
        out.push_str(&rest[..end]);
        rest = &rest[end..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            continue;
        }
        let n = &rest[..digits];
        if pat == "generation: " {
            out.push('0');
        } else {
            let i = match ids.iter().position(|x| x == n) {
                Some(i) => i,
                None => {
                    ids.push(n.to_owned());
                    ids.len() - 1
                }
            };
            out.push_str(&format!("#{i}"));
        }
        rest = &rest[digits..];
    }
    out.push_str(rest);
    out
}

fn attr(name: &str, domain: Domain, data: AttrData) -> Attribute {
    Attribute {
        name: name.into(),
        domain,
        data,
    }
}

/// The factory scene plus one datablock of every kind the mapping handles, with non-default
/// values everywhere, a linked library group, ID properties and an orphan.
fn rich() -> (Main, WindowState, Preferences) {
    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let cube = main.objects.find("Cube").unwrap();

    // A mesh with every attribute type and more than 64 elements (buffers).
    let mut me = Mesh::new("Big");
    let n = 100usize;
    let pos: Vec<[f32; 3]> = (0..n)
        .map(|i| [i as f32 * 0.1, -(i as f32), 1.0 / 3.0])
        .collect();
    let faces: Vec<Vec<i32>> = (0..n as i32 - 2)
        .step_by(3)
        .map(|i| vec![i, i + 1, i + 2])
        .collect();
    me.from_pydata(&pos, &[], &faces, true);
    let nf = me.faces_num();
    let nc = me.corners_num();
    {
        let a = me.attributes_mut();
        a.insert(attr(
            "f",
            Domain::Point,
            AttrData::Float(Arc::new((0..n).map(|i| i as f32 / 7.0).collect())),
        ));
        a.insert(attr(
            "i8",
            Domain::Point,
            AttrData::Int8(Arc::new((0..n).map(|i| i as i8).collect())),
        ));
        a.insert(attr(
            "uv",
            Domain::Corner,
            AttrData::Float2(Arc::new(vec![[0.25, 0.5]; nc])),
        ));
        a.insert(attr(
            "q",
            Domain::Face,
            AttrData::Quaternion(Arc::new(vec![[1.0, 0.0, 0.0, 0.0]; nf])),
        ));
        a.insert(attr(
            "col",
            Domain::Corner,
            AttrData::ByteColor(Arc::new(vec![[1, 2, 3, 255]; nc])),
        ));
        a.insert(attr(
            "fc",
            Domain::Point,
            AttrData::FloatColor(Arc::new(vec![[0.1, 0.2, f32::INFINITY, 1.0]; n])),
        ));
        a.insert(attr(
            "i16",
            Domain::Corner,
            AttrData::Int16x2(Arc::new(vec![[-3, 4]; nc])),
        ));
        a.insert(attr(
            "m",
            Domain::Face,
            AttrData::Float4x4(Arc::new(vec![[[1.0, 2.0, 3.0, 4.0]; 4]; nf])),
        ));
        a.insert(attr(
            "s",
            Domain::Face,
            AttrData::String(Arc::new(vec!["a\"b".into(); nf])),
        ));
        a.insert(attr("f4", Domain::Edge, AttrData::Float4(Arc::new(vec![]))));
    }
    me.vertex_group_names = vec!["Group".into(), "Other".into()];
    {
        let dv = me.deform_verts_mut();
        dv[3] = DeformVert {
            weights: [(0u32, 0.5f32), (1, 0.25)].into_iter().collect(),
        };
    }
    me.uv_active = Some("uv".into());
    me.texspace.auto = false;
    me.texspace.size = [2.0, 3.0, 4.0];

    // An image packed into the file and a material that samples it.
    let mut img = Image::new("tex.png", 2, 2);
    img.source = "FILE".into();
    img.filepath = "//tex.png".into();
    img.packed_filepath = "//orig/tex.png".into();
    img.generated_depth = 32;
    img.packed_file = Some(Arc::new(EncodedImage {
        bytes: vec![0x89, b'P', b'N', b'G', 1, 2, 3],
        mime: "image/png".into(),
    }));
    let img = main.images.insert("tex.png", img, &mut log);
    let mut mat = Material::new("Steel");
    mat.node_tree = NodeTree::default_material();
    let i = mat.node_tree.new_node("ShaderNodeTexImage").unwrap();
    mat.node_tree.nodes[i].set_prop("image", Value::Id(Some(img.any())));
    mat.node_tree.nodes[i].location = [-500.0, 0.1];
    mat.diffuse_color = [0.6, 0.6, 0.7, 1.0];
    let mat = main.materials.insert("Steel", mat, &mut log);
    me.materials = vec![Some(mat), None];
    let me = main.meshes.insert("Big", me, &mut log);

    let mut ob = Object::new("Thing", Some(me.any()));
    ob.parent = Some(cube);
    ob.location = [0.1, 0.2, 0.3];
    ob.rotation_mode = "QUATERNION".into();
    ob.rotation_quaternion = [0.5, 0.5, 0.5, 0.5];
    ob.matrix_parent_inverse = glam::Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
    ob.material_slot_overrides = vec![
        SlotOverride::default(),
        SlotOverride {
            link: SlotLink::Object,
            material: Some(mat),
        },
    ];
    let mut md = Modifier {
        name: "Bevel".into(),
        show_viewport: true,
        show_render: false,
        show_in_editmode: true,
        show_on_cage: false,
        show_expanded: true,
        is_active: true,
        use_pin_to_last: false,
        persistent_uid: 7,
        settings: ModifierSettings::new(ModifierType::Bevel),
    };
    if let ModifierSettings::Bevel(b) = &mut md.settings {
        b.width = 0.25;
    }
    ob.modifiers.push(md);
    ob.header.custom.insert("int".into(), IdProp::Int(3));
    ob.header.custom.insert("float".into(), IdProp::Float(1.0));
    ob.header
        .custom
        .insert("ints".into(), IdProp::IntArray(vec![]));
    ob.header
        .custom
        .insert("floats".into(), IdProp::FloatArray(vec![0.5, 2.0]));
    ob.header.custom.insert(
        "group".into(),
        IdProp::Group(
            [("s".into(), IdProp::Str("x".into()))]
                .into_iter()
                .collect(),
        ),
    );
    ob.header
        .custom
        .insert("list".into(), IdProp::List(vec![IdProp::Bool(true)]));
    let thing = main.objects.insert("Thing", ob, &mut log);

    // Camera DOF focus object, light type.
    let cam = main.cameras.find("Camera").unwrap();
    {
        let c: &mut Camera = main.cameras.get_mut(cam, &mut log).unwrap();
        c.fields.dof.focus_object = Some(thing.any());
        c.fields.lens = 35.5;
    }
    let li = main.lights.find("Light").unwrap();
    {
        let l: &mut Light = main.lights.get_mut(li, &mut log).unwrap();
        l.r#type = "SPOT".into();
    }

    // Link the new object into the scene and select it.
    let scene = win.scene;
    {
        let s = main.scenes.get_mut(scene, &mut log).unwrap();
        s.collection.objects.push(thing);
        s.frame_current = 42;
        s.cursor.location = [1.0, 2.0, 3.0];
        let vl = &mut s.view_layers[0];
        vl.bases.push(Base {
            object: thing,
            flag: BaseFlag::SELECTED | BaseFlag::HIDDEN,
        });
        vl.active = Some(thing);
    }

    // A library with a linked group that shares its name with a local one.
    let lib = main.libraries.insert(
        "Essentials",
        Library::new("Essentials", "//lib/Essentials.blend"),
        &mut log,
    );
    let mut linked = NodeTree::new("Smooth by Angle", NodeTreeType::Geometry, false);
    linked.header.library = Some(lib);
    linked.header.use_fake_user = true;
    linked.header.is_linked_packed = true;
    main.node_groups.insert("Smooth by Angle", linked, &mut log);
    let mut local = NodeTree::new("Smooth by Angle", NodeTreeType::Geometry, false);
    local.header.use_fake_user = true;
    // An interface emptied by `clear()`: the identifier counter survives (WP 3.1).
    local.next_interface_uid = 3;
    main.node_groups.insert("Smooth by Angle", local, &mut log);

    // An orphan: dropped on save.
    main.meshes.insert("Orphan", Mesh::new("Orphan"), &mut log);

    let mut prefs = Preferences::default();
    prefs.addons.insert(
        "io_scene_gltf2".into(),
        [(SmolStr::new("KHR_materials_variants_ui"), Value::Bool(true))]
            .into_iter()
            .collect(),
    );
    (main, win, prefs)
}

/// `main` without its orphans, removed from their maps only (no relinking or layer sync).
fn without_orphans(mut main: Main) -> Main {
    let mut log = ChangeLog::default();
    for a in main.orphans() {
        assert_eq!(
            a.code,
            frap_rna::IdCode::ME,
            "the rich state has one orphan mesh"
        );
        main.meshes.remove(IdRef::from_any(a).unwrap(), &mut log);
    }
    main
}

#[test]
fn factory_round_trips() {
    let (main, win) = frap_scene::startup::factory();
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    assert!(saved.dropped.is_empty(), "{:?}", saved.dropped);
    let o = open(&saved.bytes, false).unwrap();
    assert_same(&normalized(&o.main, &o.win), &normalized(&main, &win));
    assert!(o.warnings.is_empty());
}

#[test]
fn empty_round_trips() {
    let (main, win) = frap_scene::startup::empty();
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    let o = open(&saved.bytes, false).unwrap();
    assert_same(&normalized(&o.main, &o.win), &normalized(&main, &win));
}

#[test]
fn rich_state_round_trips_and_drops_orphans() {
    let (main, win, prefs) = rich();
    let saved = save(&main, &win, &prefs, None).unwrap();
    assert_eq!(saved.dropped, vec!["meshes[\"Orphan\"]".to_owned()]);
    let o = open(&saved.bytes, false).unwrap();
    assert!(o.main.meshes.find("Orphan").is_none());
    let expected = without_orphans(main.clone());
    assert_same(&normalized(&o.main, &o.win), &normalized(&expected, &win));
    assert_eq!(o.prefs.addons, prefs.addons);
    // Linked and local groups of the same name both survive, in Blender order (local first).
    let names: Vec<(String, bool)> = o
        .main
        .node_groups
        .iter()
        .map(|(_, t)| (t.header.name.to_string(), t.header.library.is_some()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Smooth by Angle".to_owned(), false),
            ("Smooth by Angle".to_owned(), true)
        ]
    );
    // Saving the loaded state gives the same bytes.
    let again = save(&o.main, &o.win, &o.prefs, None).unwrap();
    assert_eq!(again.bytes, saved.bytes);
}

#[test]
fn saves_are_byte_identical() {
    let (main, win, prefs) = rich();
    let a = save(&main, &win, &prefs, None).unwrap();
    let b = save(&main.clone(), &win, &prefs, None).unwrap();
    assert_eq!(a.bytes, b.bytes);
}

#[test]
fn scene_json_shape() {
    let (main, win, prefs) = rich();
    let (tree, buffers, images, _) = to_json(&main, &win, &prefs, None).unwrap();
    assert_eq!(tree["window"]["scene"], json!("Scene"));
    assert_eq!(
        tree["objects"]["Thing"]["data"],
        json!({"$id": "ME", "name": "Big"})
    );
    assert_eq!(tree["objects"]["Thing"]["location"], json!([0.1, 0.2, 0.3]));
    assert_eq!(
        tree["objects"]["Thing"]["matrix_parent_inverse"][0],
        json!([1.0, 0.0, 0.0, 1.0])
    );
    assert_eq!(tree["objects"]["Cube"]["type"], json!("MESH"));
    let pos = &tree["meshes"]["Big"]["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "position")
        .unwrap()["data"];
    assert_eq!(pos["dtype"], json!("f32x3"));
    assert_eq!(pos["len"], json!(100));
    assert!(buffers.len() > 3);
    assert_eq!(images.len(), 1);
    // WP 3.2: the packed file's own path and the generated colour depth.
    let im = &tree["images"]["tex.png"];
    assert_eq!(im["packed_file"]["filepath"], json!("//orig/tex.png"));
    assert_eq!(im["packed_file"]["mime"], json!("image/png"));
    assert_eq!(im["generated_depth"], json!(32));
    let linked = &tree["node_groups"][LINKED]["Essentials"]["Smooth by Angle"];
    assert_eq!(linked["is_linked_packed"], json!(true));
    assert_eq!(
        tree["libraries"]["Essentials"]["filepath"],
        json!("//lib/Essentials.blend")
    );
    // Non-finite floats inside a buffer survive; the Cube's mesh is small enough to be inline.
    assert!(tree["meshes"]["Cube"]["attributes"][0]["data"].is_array());
    assert_eq!(tree["scenes"]["Scene"]["frame_current"], json!(42));
    assert_eq!(
        tree["cameras"]["Camera"]["dof"]["focus_object"],
        json!({"$id": "OB", "name": "Thing"})
    );
    assert_eq!(
        tree["objects"]["Thing"]["modifiers"][0]["width"],
        json!(0.25)
    );
}

#[test]
fn unknown_fields_fail_unless_lenient() {
    let (main, win) = frap_scene::startup::factory();
    let (mut tree, buffers, images, _) =
        to_json(&main, &win, &Preferences::default(), None).unwrap();
    tree["objects"]["Cube"]["sparkle"] = json!(1);
    let e = from_json(&tree, &buffers, &images, false).unwrap_err();
    assert!(
        matches!(&e, MapError::Unknown(p) if p.contains("sparkle")),
        "{e}"
    );
    assert!(from_json(&tree, &buffers, &images, true).is_ok());
    // Unknown fields inside generated structs too.
    let mut t2 = to_json(&main, &win, &Preferences::default(), None)
        .unwrap()
        .0;
    t2["scenes"]["Scene"]["render"]["sparkle"] = json!(true);
    assert!(matches!(
        from_json(&t2, &buffers, &images, false),
        Err(MapError::Unknown(_))
    ));
    let mut t3 = to_json(&main, &win, &Preferences::default(), None)
        .unwrap()
        .0;
    t3["frobs"] = json!({});
    assert!(matches!(
        from_json(&t3, &buffers, &images, false),
        Err(MapError::Unknown(_))
    ));
}

#[test]
fn dangling_references_are_errors() {
    let (main, win) = frap_scene::startup::factory();
    let (mut tree, buffers, images, _) =
        to_json(&main, &win, &Preferences::default(), None).unwrap();
    tree["objects"]["Cube"]["data"] = json!({"$id": "ME", "name": "Nope"});
    let e = from_json(&tree, &buffers, &images, true)
        .unwrap_err()
        .to_string();
    assert!(e.contains("missing ID meshes[\"Nope\"]"), "{e}");
}

#[test]
fn references_to_orphans_become_null() {
    // A camera object unlinked from every collection is an orphan; the scene camera
    // (a non-user pointer) then reads None after the round trip.
    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let cam: IdRef<ObjectId> = main.objects.find("Camera").unwrap();
    let colls: Vec<_> = main.collections.iter().map(|(r, _)| r).collect();
    for c in colls {
        main.collections
            .get_mut(c, &mut log)
            .unwrap()
            .objects
            .retain(|o| *o != cam);
    }
    let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
    s.collection.objects.retain(|o| *o != cam);
    for vl in &mut s.view_layers {
        vl.bases.retain(|b| b.object != cam);
    }
    assert_eq!(main.scenes.get(win.scene).unwrap().camera, Some(cam));
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    assert!(
        saved.dropped.contains(&"objects[\"Camera\"]".to_owned()),
        "{:?}",
        saved.dropped
    );
    let o = open(&saved.bytes, false).unwrap();
    assert_eq!(o.main.scenes.get(o.win.scene).unwrap().camera, None);
}

#[test]
fn the_window_state_round_trips() {
    let (mut main, mut win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let s2 = frap_scene::id::Scene::new(
        "Other",
        main.scenes.get(win.scene).unwrap().view_layers.clone(),
    );
    let s2 = main.scenes.insert("Other", s2, &mut log);
    win.scene = s2;
    win.view3d.view_matrix = glam::Mat4::from_rotation_z(0.5);
    win.view3d.shading.r#type = "WIREFRAME".into();
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    let o = open(&saved.bytes, false).unwrap();
    assert_eq!(o.main.scenes.name_of(o.win.scene), Some("Other"));
    assert_eq!(o.win.view3d.view_matrix, win.view3d.view_matrix);
    assert_eq!(o.win.view3d.shading.r#type, "WIREFRAME");
}

#[test]
fn edit_mode_objects_load_in_edit_mode() {
    // M2 W0: the loader keeps `mode: EDIT`; `wm.open_mainfile` re-enters Edit Mode
    // (`frap_op::helpers::edit_mode::reenter_after_load`, WP 2.2).
    let (main, win) = frap_scene::startup::factory();
    let (mut tree, buffers, images, _) =
        to_json(&main, &win, &Preferences::default(), None).unwrap();
    tree["objects"]["Cube"]["mode"] = json!("EDIT");
    let o = from_json(&tree, &buffers, &images, false).unwrap();
    assert!(o.warnings.is_empty(), "{:?}", o.warnings);
    let cube = o.main.objects.find("Cube").unwrap();
    assert_eq!(
        o.main.objects.get(cube).unwrap().mode,
        frap_scene::ObjectMode::Edit
    );
}

#[test]
fn mesh_view_overrides_the_saved_mesh() {
    let (main, win) = frap_scene::startup::factory();
    let cube_mesh = main.meshes.find("Cube").unwrap();
    let mut edited = main.meshes.get(cube_mesh).unwrap().clone();
    edited.positions_mut()[0] = [9.0, 9.0, 9.0];
    let edited = Arc::new(edited);
    let view = |m| (m == cube_mesh).then(|| edited.clone());
    let saved = save(&main, &win, &Preferences::default(), Some(&view)).unwrap();
    let o = open(&saved.bytes, false).unwrap();
    let m = o
        .main
        .meshes
        .get(o.main.meshes.find("Cube").unwrap())
        .unwrap();
    assert_eq!(m.positions()[0], [9.0, 9.0, 9.0]);
}

#[test]
fn shortest_float_spelling() {
    assert_eq!(json::f32j(0.1).to_string(), "0.1");
    assert_eq!(json::f32j(1.0).to_string(), "1.0");
    assert_eq!(json::f32j(f32::NAN), json!({"$float": "nan"}));
    let v: J = json::f32j(-f32::INFINITY);
    assert_eq!(json::num(&v, "x").unwrap(), f32::NEG_INFINITY);
}

#[test]
fn header_only_ids_are_reported() {
    // M1 creates none of these IDs: the factory and the rich state report nothing.
    let (main, win) = frap_scene::startup::factory();
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    assert!(saved.header_only.is_empty(), "{:?}", saved.header_only);
    let (main, win, prefs) = rich();
    assert!(
        save(&main, &win, &prefs, None)
            .unwrap()
            .header_only
            .is_empty()
    );
    // An action kept by a fake user is written in full since WP 6.4 (not named in
    // `header_only`); an orphan action is dropped.
    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let mut kept = frap_anim::Action::new("Kept");
    kept.header.use_fake_user = true;
    main.actions.insert("Kept", kept, &mut log);
    main.actions
        .insert("Loose", frap_anim::Action::new("Loose"), &mut log);
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    assert!(saved.header_only.is_empty(), "{:?}", saved.header_only);
    assert_eq!(saved.dropped, vec!["actions[\"Loose\"]".to_owned()]);
}

/// WP 3.2: packed images keep their original bytes and their packed path; a pre-M3 file
/// without the packed path reads the image's own path; modified pixels are not saved (as
/// Blender, which does not write unpacked pixel edits into `.blend` files).
#[test]
fn images_packed_bytes_and_buffers() {
    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let mut img = Image::new("Painted", 2, 1);
    img.generated_depth = 32;
    let mut buf = img.generated_buffer().unwrap();
    buf.set_pixels(&[1.0; 8]);
    img.buffer = Some(Arc::new(buf));
    // Unused images are dropped on save: give both a fake user.
    img.header.use_fake_user = true;
    main.images.insert("Painted", img, &mut log);
    let mut packed = Image::new("p.png", 0, 0);
    packed.header.use_fake_user = true;
    packed.source = "FILE".into();
    packed.filepath = "//p.png".into();
    packed.packed_filepath = "//p.png".into();
    let bytes = vec![0x89, b'P', b'N', b'G', 9, 8, 7];
    packed.packed_file = Some(Arc::new(EncodedImage {
        bytes: bytes.clone(),
        mime: "image/png".into(),
    }));
    main.images.insert("p.png", packed, &mut log);
    let prefs = Preferences::default();
    let (mut tree, buffers, images, _) = to_json(&main, &win, &prefs, None).unwrap();
    assert!(tree["images"]["Painted"].get("buffer").is_none());
    let o = from_json(&tree, &buffers, &images, false).unwrap();
    let find = |m: &Main, n: &str| {
        m.images
            .iter()
            .find(|(_, i)| i.header.name == n)
            .unwrap()
            .1
            .clone()
    };
    let painted = find(&o.main, "Painted");
    assert!(painted.buffer.is_none() && !painted.is_dirty());
    assert_eq!(painted.generated_depth, 32);
    let p = find(&o.main, "p.png");
    assert_eq!(
        p.packed_file.as_ref().unwrap().bytes,
        bytes,
        "original bytes"
    );
    assert_eq!(p.packed_filepath, "//p.png");
    // A file written before the packed path existed.
    tree["images"]["p.png"]["packed_file"]
        .as_object_mut()
        .unwrap()
        .remove("filepath");
    tree["images"]["p.png"]["filepath"] = json!("//q.png");
    let o = from_json(&tree, &buffers, &images, false).unwrap();
    assert_eq!(find(&o.main, "p.png").packed_filepath, "//q.png");
}

/// WP 3.7: point clouds keep their point count, attributes (in order), material slots and
/// custom properties; they are no longer header-only.
#[test]
fn pointclouds_round_trip() {
    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    let mat = main.materials.find("Material");
    let mut pc = PointCloud::new("PC");
    pc.resize(3);
    pc.set_position(1, [1.0, 2.0, 3.0]);
    pc.set_radius(2, 0.25);
    pc.set_attribute("_w", AttrData::Float(Arc::new(vec![1.0, 2.0, 3.0])))
        .unwrap();
    pc.set_attribute(
        "Col",
        AttrData::ByteColor(Arc::new(vec![[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]])),
    )
    .unwrap();
    pc.materials = vec![mat, None];
    pc.header
        .custom
        .insert(SmolStr::new("k"), IdProp::Str("v".into()));
    let r = main.pointclouds.insert("PC", pc, &mut log);
    let ob = main.objects.insert(
        "PCObj",
        Object::new("", Some(frap_rna::IdAny::from(r))),
        &mut log,
    );
    {
        let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
        s.collection.objects.push(ob);
        s.view_layers[0].bases.push(Base {
            object: ob,
            flag: BaseFlag::empty(),
        });
    }
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    assert!(saved.header_only.is_empty(), "{:?}", saved.header_only);
    let o = open(&saved.bytes, false).unwrap();
    assert_same(&normalized(&o.main, &o.win), &normalized(&main, &win));
    let pc = o
        .main
        .pointclouds
        .get(o.main.pointclouds.find("PC").unwrap())
        .unwrap();
    let names: Vec<_> = pc.attributes().iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["position", "radius", "_w", "Col"]);
    assert_eq!(pc.radii(), [0.01, 0.01, 0.25]);
}

/// WP 6.4: the M6 data round-trips: actions (slots, layers, strips, channelbags, groups,
/// F-curves with every key setting), the full `AnimData` with NLA tracks and strips (whose
/// actions are users, so they are not dropped as orphans), armatures with bones and bone
/// collections, poses, shape keys with their key blocks, vertex groups and their locks.
#[test]
fn m6_data_round_trips() {
    use frap_anim::{
        Action, ActionGroup, ActionLayer, ActionSlot, ActionStrip, AnimData, Channelbag, Easing,
        Extrapolation, FCurve, HandleType, Interpolation, Keyframe, KeyframeStrip, KeyframeType,
        NlaStrip, NlaTrack,
    };
    use frap_scene::id::{Armature, Bone, BoneCollection, Key, Pose, PoseBone, ShapeKey};

    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();

    // An action with a typed slot, one layer, one strip, a group and two F-curves.
    let mut act = Action::new("CubeAction");
    act.last_slot_handle += 1;
    let handle = act.last_slot_handle;
    act.slots.push(ActionSlot::new(handle, "OBCube", "OBJECT"));
    let mut fc = FCurve::new("location", 2);
    fc.group = Some("Object Transforms".into());
    fc.extrapolation = Extrapolation::Linear;
    fc.color_mode = "AUTO_RGB".into();
    for i in 0..70 {
        // More than 64 keys: the arrays go to buffers.
        let mut k = Keyframe::new(i as f32, (i * i) as f32 * 0.5);
        k.handle_left = [i as f32 - 0.3, 1.0];
        k.handle_right = [i as f32 + 0.3, 2.0];
        k.interpolation = if i % 2 == 0 {
            Interpolation::Bezier
        } else {
            Interpolation::Elastic
        };
        k.easing = Easing::EaseOut;
        k.handle_left_type = HandleType::Vector;
        k.r#type = KeyframeType::Breakdown;
        k.select_left_handle = i % 3 == 0;
        k.back = 2.5;
        fc.keyframe_points.push(k);
    }
    let mut hide = FCurve::new("hide_render", 0);
    hide.discrete_values = true;
    hide.int_values = true;
    hide.keyframe_points.push(Keyframe::new(1.0, 1.0));
    let mut group = ActionGroup::new("Object Transforms");
    group.lock = true;
    let mut layer = ActionLayer::new("Layer");
    layer.influence = 0.75;
    layer.strips.push(ActionStrip::Keyframe(KeyframeStrip {
        channelbags: vec![Channelbag {
            slot_handle: handle,
            groups: vec![group],
            fcurves: vec![fc, hide],
        }],
    }));
    act.layers.push(layer);
    act.use_frame_range = true;
    act.frame_range = [1.0, 40.0];
    let a = main.actions.insert("CubeAction", act, &mut log);
    let pushed = main
        .actions
        .insert("Pushed", Action::new("Pushed"), &mut log);

    // The Cube animates with the action and an NLA track whose strip uses the other action.
    let cube = main.objects.find("Cube").unwrap();
    {
        let ob = main.objects.get_mut(cube, &mut log).unwrap();
        let mut adt = AnimData {
            action: Some(a),
            action_slot_handle: handle,
            last_slot_identifier: "OBCube".into(),
            action_blend_type: "ADD".into(),
            action_extrapolation: "HOLD_FORWARD".into(),
            action_influence: 0.5,
            ..AnimData::default()
        };
        let mut track = NlaTrack::new("NlaTrack");
        let mut strip = NlaStrip::new_clip("Pushed", Some(pushed), 3.0, [1.0, 9.0]);
        strip.blend_type = "MULTIPLY".into();
        strip.repeat = 2.0;
        strip.use_reverse = true;
        track.strips.push(strip);
        adt.nla_tracks.push(track);
        ob.animation_data = Some(adt);
    }

    // An armature with two bones and a collection, and its posed object.
    let mut arm = Armature::new("Armature");
    let mut b0 = Bone::new("Bone");
    b0.collections = vec!["Bones".into()];
    b0.select = [true, false, true];
    let mut b1 = Bone::new("Bone.001");
    b1.parent = Some(0);
    b1.use_connect = true;
    b1.head_local = [0.0, 0.0, 1.0];
    b1.tail_local = [0.0, 0.5, 2.0];
    b1.matrix_local[3] = [0.0, 0.0, 1.0, 1.0];
    b1.roll = 0.25;
    arm.bones = vec![b0, b1];
    arm.active_bone = Some(1);
    arm.collections.push(BoneCollection {
        name: "Bones".into(),
        parent: None,
        is_visible: false,
        is_solo: true,
    });
    arm.active_collection = Some(0);
    arm.display_type = "STICK".into();
    let ar = main.armatures.insert("Armature", arm, &mut log);
    let rig = main.objects.insert(
        "Rig",
        Object::new("", Some(frap_rna::IdAny::from(ar))),
        &mut log,
    );
    {
        let ob = main.objects.get_mut(rig, &mut log).unwrap();
        let mut pose = Pose::default();
        let mut pb = PoseBone::new("Bone");
        pb.location = [1.0, 2.0, 3.0];
        pb.rotation_mode = "XZY".into();
        pb.lock_rotation = [true, false, true];
        pb.select = true;
        pose.bones = vec![pb, PoseBone::new("Bone.001")];
        pose.use_mirror_x = true;
        ob.pose = Some(pose);
        ob.mode = frap_scene::ObjectMode::Pose;
    }

    // Shape keys and vertex groups on the Cube's mesh.
    let me = main.meshes.find("Cube").unwrap();
    let verts = main.meshes.get(me).unwrap().positions().to_vec();
    let mut key = Key::new("Key");
    key.user = Some(me);
    key.uidgen = 3;
    let basis = ShapeKey::new("Basis", verts.clone());
    let mut k1 = ShapeKey::new(
        "Key 1",
        verts.iter().map(|p| [p[0], p[1], p[2] + 1.0]).collect(),
    );
    k1.value = 0.4;
    k1.vertex_group = "Group".into();
    k1.uid = 2;
    key.key_blocks = vec![basis, k1];
    key.animation_data = Some(AnimData::default());
    let k = main.shape_keys.insert("Key", key, &mut log);
    {
        let m = main.meshes.get_mut(me, &mut log).unwrap();
        m.shape_keys = Some(k);
        m.vertex_group_names = vec!["Group".into(), "Locked".into()];
        m.vertex_group_locks = vec!["Locked".into()];
    }
    {
        let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
        s.collection.objects.push(rig);
        s.view_layers[0].bases.push(Base {
            object: rig,
            flag: BaseFlag::empty(),
        });
    }

    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    assert!(saved.dropped.is_empty(), "{:?}", saved.dropped);
    assert!(saved.header_only.is_empty(), "{:?}", saved.header_only);
    let o = open(&saved.bytes, false).unwrap();
    assert_same(&normalized(&o.main, &o.win), &normalized(&main, &win));
    // Saving the loaded state again is byte-identical.
    let again = save(&o.main, &o.win, &Preferences::default(), None).unwrap();
    assert_eq!(again.bytes, saved.bytes);
}

/// WP 7.6: timeline markers (name, frame, selection, camera) and the glTF add-on's variant data
/// (M7 W0: the scene's variant list and active variant, a mesh's per-slot mappings, default
/// materials and UI pointer) round-trip; files without them read as before and get no keys.
#[test]
fn m7_markers_and_gltf_variants_round_trip() {
    use frap_mesh::gltf_variants::{
        GltfVariantDefaultMaterial, GltfVariantPointer, GltfVariantPrimitive,
    };
    use frap_scene::id::{GltfVariant, TimelineMarker};

    let (mut main, win) = frap_scene::startup::factory();
    let text = to_json(&main, &win, &Preferences::default(), None)
        .unwrap()
        .0
        .to_string();
    let mut log = ChangeLog::default();
    let camera = main.objects.find("Camera");
    let mat = main.materials.find("Material");
    {
        let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
        s.timeline_markers = vec![
            TimelineMarker {
                name: "Intro".into(),
                frame: -3,
                select: false,
                camera,
            },
            TimelineMarker::new("B", 40),
            TimelineMarker::new("B", 41),
        ];
        s.gltf2_variants.variants = vec![
            GltfVariant {
                name: "Red".into(),
                variant_idx: 0,
            },
            GltfVariant {
                name: "Blue".into(),
                variant_idx: 1,
            },
        ];
        s.gltf2_variants.active_variant = 1;
    }
    {
        let me = main.meshes.find("Cube").unwrap();
        let m = main.meshes.get_mut(me, &mut log).unwrap();
        m.gltf2_variants.mesh_data = vec![GltfVariantPrimitive {
            material_slot_index: 0,
            material: mat,
            variants: vec![
                GltfVariantPointer { variant_idx: 0 },
                GltfVariantPointer { variant_idx: 1 },
            ],
            active_variant_idx: 1,
        }];
        m.gltf2_variants.default_materials = vec![GltfVariantDefaultMaterial {
            material_slot_index: 0,
            default_material: mat,
        }];
        m.gltf2_variants.pointer = "Red".into();
    }
    let full = to_json(&main, &win, &Preferences::default(), None)
        .unwrap()
        .0
        .to_string();
    for key in [
        "timeline_markers",
        "gltf2_KHR_materials_variants_variants",
        "gltf2_variant_mesh_data",
    ] {
        assert!(full.contains(key), "{key} not written");
    }
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    let o = open(&saved.bytes, false).unwrap();
    assert_same(&normalized(&o.main, &o.win), &normalized(&main, &win));
    let s = o.main.scenes.get(o.win.scene).unwrap();
    assert_eq!(s.timeline_markers.len(), 3);
    assert_eq!(s.timeline_markers[0].camera, o.main.objects.find("Camera"));
    assert!(s.timeline_markers[1].select);
    // A file without markers or variants has none of the keys (older readers keep reading it).
    for key in [
        "timeline_markers",
        "gltf2_KHR_materials_variants_variants",
        "gltf2_variant_mesh_data",
    ] {
        assert!(
            !text.contains(key),
            "{key} written for a plain factory file"
        );
    }
}

/// WP 7.6: loading runs `IMB_colormanagement_check_file_config`: an output override whose
/// display lacks its view falls back as Blender's file read does, with Blender's log line.
#[test]
fn opening_checks_the_colour_management_of_the_file() {
    let (mut main, win) = frap_scene::startup::factory();
    let mut log = ChangeLog::default();
    {
        let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
        let ims = &mut s.render.image_settings;
        ims.color_management = "OVERRIDE".into();
        ims.view_settings.view_transform = "Filmic".into();
        ims.view_settings.look = "Medium High Contrast".into();
        ims.display_settings.display_device = "Display P3".into();
    }
    let saved = save(&main, &win, &Preferences::default(), None).unwrap();
    let o = open(&saved.bytes, false).unwrap();
    assert_eq!(
        o.warnings,
        ["scene output view \"Filmic\" not found, setting to \"Standard\"."]
    );
    let s = o.main.scenes.get(o.win.scene).unwrap();
    let vs = &s.render.image_settings.view_settings;
    assert_eq!(
        (vs.view_transform.as_str(), vs.look.as_str()),
        ("Standard", "Medium High Contrast")
    );
    assert_eq!(s.view_settings.view_transform, "AgX");
}
