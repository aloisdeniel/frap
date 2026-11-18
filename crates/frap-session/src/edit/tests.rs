// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit Mode through the whole session (WP 2.2 acceptance): the `MODE_HANDLERS` entry for
//! multi-object edit, reads through the cached view (no `gen` bump, `is_dirty` and the
//! evaluation inputs unchanged, one conversion per BMesh generation), `RNA_EDITMODE_WRITE`,
//! primitive merge, the `edit_mesh` block, rollback and `.frap` re-entry. Blender parity of the
//! same behaviours is pinned by the goldens in `tests/golden/cases/edit/prereq/`.

use std::sync::Arc;

use frap_rna::{IdRef, MeshId};
use serde_json::Value as J;

use crate::{Session, SessionOptions};

#[allow(clippy::field_reassign_with_default)] // `SessionOptions` is `#[non_exhaustive]`
fn session() -> Session {
    let mut opts = SessionOptions::default();
    opts.cwd = std::env::temp_dir();
    Session::new(opts).expect("factory session")
}

fn one(s: &mut Session, src: &str) -> J {
    let mut v: Vec<J> = s.exec_source(src).iter().map(|r| r.to_json()).collect();
    assert_eq!(v.len(), 1, "{src}: {v:?}");
    v.remove(0)
}

fn ok(s: &mut Session, src: &str) -> J {
    let r = one(s, src);
    assert_eq!(r["status"], "FINISHED", "{src}: {r}");
    r
}

fn value(s: &mut Session, src: &str) -> J {
    ok(s, src)["value"].clone()
}

fn error(s: &mut Session, src: &str) -> J {
    let r = one(s, src);
    assert_eq!(r["status"], "ERROR", "{src}: {r}");
    assert_eq!(r["state"], "unchanged", "{src}: {r}");
    r["error"].clone()
}

fn floats(v: &J) -> Vec<f64> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_f64().expect("number"))
        .collect()
}

fn mesh(s: &Session, name: &str) -> IdRef<MeshId> {
    s.main().meshes.find(name).expect("mesh")
}

fn generation(s: &Session, name: &str) -> u64 {
    s.main()
        .meshes
        .get(mesh(s, name))
        .unwrap()
        .header
        .generation
}

fn bm_gen(s: &Session, name: &str) -> Option<u64> {
    s.host()
        .edit()
        .meshes
        .get(&mesh(s, name).key())
        .map(|em| em.bm_gen)
}

#[test]
fn mode_set_enters_every_selected_mesh_once() {
    let mut s = session();
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))",
    );
    ok(
        &mut s,
        "bpy.ops.object.duplicate_move_linked(OBJECT_OT_duplicate={'linked': True}, TRANSFORM_OT_translate={'value': (0, 3, 0)})",
    );
    ok(&mut s, "bpy.data.objects['Cube'].select_set(True)");
    ok(&mut s, "bpy.data.objects['Cube.001'].select_set(True)");
    let r = ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    // Cube.002 (active) shares Cube.001's mesh: only one of them enters.
    assert_eq!(value(&mut s, "bpy.data.objects['Cube.002'].mode"), "EDIT");
    assert_eq!(value(&mut s, "bpy.data.objects['Cube.001'].mode"), "OBJECT");
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].mode"), "EDIT");
    assert_eq!(s.host().edit().meshes.len(), 2);
    let block = r["edit_mesh"].as_object().expect("edit_mesh block");
    assert_eq!(
        block.keys().collect::<Vec<_>>(),
        ["Cube.002", "Cube"],
        "active first: {r}"
    );
    assert_eq!(r["context"]["mode"], "EDIT_MESH");
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert!(s.host().edit().meshes.is_empty());
    for n in ["Cube", "Cube.001", "Cube.002"] {
        assert_eq!(
            value(&mut s, &format!("bpy.data.objects['{n}'].mode")),
            "OBJECT"
        );
    }
}

#[test]
fn reads_use_the_cached_view_without_touching_main() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_plane_add(location=(0, 0, 3))",
    );
    let (generation0, bm, dirty) = (
        generation(&s, "Cube"),
        bm_gen(&s, "Cube").unwrap(),
        s.main().is_dirty,
    );
    let main_before = format!("{:?}", s.main());
    // Reads see the edit data (D-05): 8 + 4 vertices, the plane on top.
    assert_eq!(
        floats(&value(&mut s, "bpy.data.meshes['Cube'].vertices[11].co")),
        [1.0, 1.0, 3.0]
    );
    assert_eq!(
        value(&mut s, "bpy.data.meshes['Cube'].vertices[8].select"),
        true
    );
    assert_eq!(
        value(&mut s, "bpy.data.meshes['Cube'].vertices[0].select"),
        false
    );
    let me = mesh(&s, "Cube");
    let a = s.host().mesh_view(me);
    let b = s.host().mesh_view(me);
    assert!(Arc::ptr_eq(&a, &b), "one conversion per BMesh generation");
    assert_eq!(a.verts_num(), 12);
    assert_eq!(
        generation(&s, "Cube"),
        generation0,
        "reads bump no generation"
    );
    assert_eq!(bm_gen(&s, "Cube"), Some(bm));
    assert_eq!(s.main().is_dirty, dirty);
    assert_eq!(format!("{:?}", s.main()), main_before, "Main untouched");
    // The original mesh is still the pre-edit one until Edit Mode is left.
    assert_eq!(s.main().meshes.get(me).unwrap().verts_num(), 8);

    // An edit makes a new generation and a new view.
    ok(&mut s, "bpy.ops.mesh.select_all(action='SELECT')");
    let c = s.host().mesh_view(me);
    assert!(!Arc::ptr_eq(&a, &c));
    assert_ne!(bm_gen(&s, "Cube"), Some(bm));
    assert_eq!(
        generation(&s, "Cube"),
        generation0,
        "edits do not touch Main either"
    );

    // A non-geometry write shows through the view at once.
    ok(&mut s, "bpy.data.meshes['Cube'].name = 'Box'");
    assert_eq!(value(&mut s, "bpy.data.meshes['Box'].name"), "Box");
    assert_eq!(
        value(&mut s, "bpy.data.meshes['Box'].vertices[11].co[2]"),
        3.0
    );

    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(s.main().meshes.get(me).unwrap().verts_num(), 12);
    assert_ne!(
        generation(&s, "Box"),
        generation0,
        "leaving Edit Mode writes the mesh"
    );
}

#[test]
fn geometry_writes_in_edit_mode_are_refused() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    for src in [
        "bpy.data.meshes['Cube'].vertices[0].co = (1, 2, 3)",
        "bpy.data.meshes['Cube'].vertices.foreach_set('co', [0.0] * 24)",
        "bpy.data.meshes['Cube'].polygons[0].use_smooth = True",
        "bpy.data.meshes['Cube'].uv_layers['UVMap'].data[0].uv = (1, 1)",
        "bpy.data.meshes['Cube'].uv_layers['UVMap'].name = 'Other'",
        "bpy.data.meshes['Cube'].vertices.add(2)",
        "bpy.data.meshes['Cube'].uv_layers.new(name='B')",
        "bpy.data.meshes['Cube'].attributes.new('w', 'FLOAT', 'POINT')",
        "bpy.data.meshes['Cube'].clear_geometry()",
        "bpy.data.meshes['Cube'].from_pydata([(0, 0, 0)], [], [])",
        "bpy.data.meshes['Cube'].transform(((2, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1)))",
    ] {
        let e = error(&mut s, src);
        assert_eq!(e["code"], "RNA_EDITMODE_WRITE", "{src}: {e}");
        assert_eq!(e["exception"], "RuntimeError", "{src}: {e}");
        assert_eq!(
            e["hint"], "bpy.ops.object.mode_set(mode='OBJECT')",
            "{src}: {e}"
        );
    }
    // Non-geometry writes are allowed.
    ok(&mut s, "bpy.data.meshes['Cube'].materials.append(None)");
    ok(&mut s, "bpy.data.meshes['Cube'].uv_layers.active_index = 0");
    // Another mesh in Object Mode is writable.
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    ok(&mut s, "bpy.data.meshes['Cube'].vertices[0].co = (1, 2, 3)");
}

#[test]
fn edit_mesh_block_reports_counts_selection_and_active() {
    let mut s = session();
    let r = ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    assert_eq!(
        r["edit_mesh"]["Cube"],
        serde_json::json!({
            "verts": 8, "edges": 12, "faces": 6, "tris": 12, "select_mode": ["VERT"],
            "selected": {"verts": 8, "edges": 12, "faces": 6},
            "active": null, "select_history": [],
        })
    );
    let r = ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='FACE', indices=[2, 5])",
    );
    let c = &r["edit_mesh"]["Cube"];
    // Faces 2 and 5 share an edge.
    assert_eq!(
        c["selected"],
        serde_json::json!({"verts": 6, "edges": 7, "faces": 2})
    );
    assert_eq!(c["active"], serde_json::json!({"type": "FACE", "index": 5}));
    assert_eq!(c["select_history"].as_array().unwrap().len(), 2);
    let r = ok(&mut s, "bpy.ops.mesh.select_mode(type='FACE')");
    assert_eq!(
        r["edit_mesh"]["Cube"]["select_mode"],
        serde_json::json!(["FACE"])
    );
    assert_eq!(r["context"]["select_mode"], serde_json::json!(["FACE"]));
    let r = ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert!(r.get("edit_mesh").is_none(), "{r}");
}

#[test]
fn failed_statements_roll_the_edit_state_back() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let bm = bm_gen(&s, "Cube");
    let dump = s.state_dump();
    error(
        &mut s,
        "bpy.ops.frap.select_elements(type='VERT', indices=[0, 99])",
    );
    assert_eq!(bm_gen(&s, "Cube"), bm);
    assert_eq!(s.state_dump(), dump);
    // A cancelled select_mode (TOGGLE of the only mode) changes nothing either.
    let r = one(&mut s, "bpy.ops.mesh.select_mode(type='VERT')");
    assert_eq!(r["status"], "CANCELLED", "{r}");
    assert_eq!(bm_gen(&s, "Cube"), bm);
}

#[test]
fn primitives_merge_into_the_edit_mesh() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let objects = s.main().objects.iter().count();
    let r = ok(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 2))",
    );
    assert_eq!(
        r["result"]["object"],
        serde_json::json!({"$ref": "bpy.data.objects['Cube']"})
    );
    assert_eq!(s.main().objects.iter().count(), objects, "no object added");
    let c = &r["edit_mesh"]["Cube"];
    assert_eq!(c["verts"], 16);
    assert_eq!(
        c["selected"],
        serde_json::json!({"verts": 8, "edges": 12, "faces": 6})
    );
    assert_eq!(
        value(&mut s, "bpy.data.meshes['Cube'].vertices[8].co"),
        serde_json::json!([-0.5, -0.5, 1.5])
    );
    assert_eq!(
        value(&mut s, "bpy.data.meshes['Cube'].vertices[0].select"),
        false
    );
}

#[test]
fn editmode_toggle_round_trip() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.editmode_toggle()");
    assert_eq!(value(&mut s, "bpy.context.mode"), "EDIT_MESH");
    ok(&mut s, "bpy.ops.object.editmode_toggle()");
    assert_eq!(value(&mut s, "bpy.context.mode"), "OBJECT");
    ok(
        &mut s,
        "bpy.context.view_layer.objects.active = bpy.data.objects['Light']",
    );
    let e = error(&mut s, "bpy.ops.object.editmode_toggle()");
    assert_eq!(e["code"], "OP_POLL_FAILED", "{e}");
}

#[test]
fn frap_files_re_enter_edit_mode_on_load() {
    let dir = std::env::temp_dir().join(format!("frap-edit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("edit.frap");
    let path_s = path.to_string_lossy().replace('\\', "/");
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='VERT', indices=[1, 4])",
    );
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_plane_add(location=(0, 0, 2))",
    );
    ok(
        &mut s,
        &format!("bpy.ops.wm.save_as_mainfile(filepath='{path_s}')"),
    );
    let mut t = session();
    let r = ok(
        &mut t,
        &format!("bpy.ops.wm.open_mainfile(filepath='{path_s}')"),
    );
    assert_eq!(r["context"]["mode"], "EDIT_MESH", "{r}");
    assert_eq!(value(&mut t, "bpy.data.objects['Cube'].mode"), "EDIT");
    assert!(bm_gen(&t, "Cube").is_some(), "the edit BMesh is rebuilt");
    let r = ok(&mut t, "bpy.context.mode");
    let c = &r["edit_mesh"]["Cube"];
    assert_eq!(c["verts"], 12, "{r}");
    assert_eq!(c["selected"]["verts"], 4, "the plane is selected: {r}");
    ok(&mut t, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(
        value(&mut t, "bpy.data.meshes['Cube'].vertices[8].select"),
        true
    );
    let _ = std::fs::remove_dir_all(&dir);
}
