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
            "active": null, "select_history": [], "select_history_len": 0,
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

#[test]
fn object_data_cannot_be_reassigned_in_edit_mode() {
    let mut s = session();
    ok(&mut s, "bpy.data.meshes.new('Other')");
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    for src in [
        "bpy.data.objects['Cube'].data = bpy.data.meshes['Other']",
        "bpy.context.active_object.data = bpy.data.meshes['Other']",
    ] {
        let e = error(&mut s, src);
        assert_eq!(e["code"], "RNA_EDITMODE_WRITE", "{src}: {e}");
        assert_eq!(e["detail"]["object"], "Cube", "{e}");
        assert_eq!(e["hint"], "bpy.ops.object.mode_set(mode='OBJECT')", "{e}");
    }
    // The edit session is intact: operators still run on the edit mesh.
    let r = ok(&mut s, "bpy.ops.mesh.select_all(action='DESELECT')");
    assert_eq!(r["edit_mesh"]["Cube"]["selected"]["verts"], 0, "{r}");
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert!(bm_gen(&s, "Cube").is_none(), "the edit BMesh is gone");
    // In Object Mode the assignment works, and the old mesh is writable again.
    ok(
        &mut s,
        "bpy.data.objects['Cube'].data = bpy.data.meshes['Other']",
    );
    ok(&mut s, "bpy.data.meshes['Cube'].vertices[0].co = (1, 2, 3)");
    ok(
        &mut s,
        "bpy.data.objects['Cube'].data = bpy.data.meshes['Cube']",
    );
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].mode"), "EDIT");
    assert!(bm_gen(&s, "Cube").is_some());
}

#[test]
fn tool_settings_select_mode_converts_the_active_edit_mesh() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.ops.mesh.select_all(action='DESELECT')");
    ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])",
    );
    let g0 = bm_gen(&s, "Cube");
    let r = ok(
        &mut s,
        "bpy.context.tool_settings.mesh_select_mode = (False, False, True)",
    );
    let c = &r["edit_mesh"]["Cube"];
    assert_eq!(c["select_mode"], serde_json::json!(["FACE"]), "{r}");
    assert_eq!(
        c["selected"],
        serde_json::json!({"verts": 0, "edges": 0, "faces": 0}),
        "{r}"
    );
    assert_ne!(bm_gen(&s, "Cube"), g0, "the BMesh changed");
    assert_eq!(value(&mut s, "bpy.data.meshes['Cube'].total_vert_sel"), 0);
    // An empty mode is ignored.
    ok(
        &mut s,
        "bpy.context.tool_settings.mesh_select_mode = (False, False, False)",
    );
    assert_eq!(
        value(&mut s, "bpy.context.tool_settings.mesh_select_mode[:]"),
        serde_json::json!([false, false, true])
    );
    let r = ok(&mut s, "bpy.ops.mesh.select_all(action='INVERT')");
    assert_eq!(
        r["edit_mesh"]["Cube"]["selected"],
        serde_json::json!({"verts": 8, "edges": 12, "faces": 6}),
        "{r}"
    );
    // Setting the mode it already has keeps the BMesh.
    let g0 = bm_gen(&s, "Cube");
    ok(
        &mut s,
        "bpy.context.scene.tool_settings.mesh_select_mode = (False, False, True)",
    );
    assert_eq!(bm_gen(&s, "Cube"), g0);
}

#[test]
fn total_sel_counts_read_the_edit_bmesh() {
    let mut s = session();
    assert_eq!(value(&mut s, "bpy.data.meshes['Cube'].total_vert_sel"), 0);
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='FACE', indices=[0])",
    );
    assert_eq!(value(&mut s, "bpy.data.meshes['Cube'].total_vert_sel"), 4);
    assert_eq!(value(&mut s, "bpy.data.meshes['Cube'].total_edge_sel"), 4);
    assert_eq!(value(&mut s, "bpy.data.meshes['Cube'].total_face_sel"), 1);
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    // 0 outside Edit Mode (verified on 5.2.4).
    assert_eq!(value(&mut s, "bpy.data.meshes['Cube'].total_face_sel"), 0);
}

#[test]
fn responses_cap_the_selection_history() {
    let mut s = session();
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=9)",
    );
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.ops.mesh.select_all(action='DESELECT')");
    let mut last = J::Null;
    for i in 0..20 {
        last = ok(
            &mut s,
            &format!("bpy.ops.frap.select_elements(type='VERT', indices=[{i}], action='ADD')"),
        );
    }
    let c = &last["edit_mesh"]["Grid"];
    assert_eq!(c["select_history_len"], 20, "{last}");
    let h = c["select_history"].as_array().unwrap();
    assert_eq!(h.len(), super::HISTORY_CAP);
    assert_eq!(h[0], serde_json::json!({"type": "VERT", "index": 12}));
    assert_eq!(
        c["active"],
        serde_json::json!({"type": "VERT", "index": 19})
    );
    // The full block (golden dumper, `--verbosity full`) keeps every entry, in order.
    let full = s.edit_mesh_block(true).unwrap();
    let h = full["Grid"]["select_history"].as_array().unwrap().clone();
    assert_eq!(h.len(), 20);
    assert_eq!(h[0], serde_json::json!({"type": "VERT", "index": 0}));
    assert_eq!(h[19], serde_json::json!({"type": "VERT", "index": 19}));
}

#[test]
fn renaming_a_layer_in_edit_mode_says_frap_does_not_support_it() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let e = error(
        &mut s,
        "bpy.data.meshes['Cube'].uv_layers['UVMap'].name = 'Other'",
    );
    assert_eq!(e["code"], "RNA_EDITMODE_WRITE", "{e}");
    let msg = e["message"].as_str().unwrap();
    assert!(
        msg.contains("frap does not support it in Edit Mode yet"),
        "{msg}"
    );
    let e = error(&mut s, "bpy.data.meshes['Cube'].vertices[0].co = (1, 2, 3)");
    let msg = e["message"].as_str().unwrap();
    assert!(msg.contains("Blender would discard the write"), "{msg}");
}

#[test]
fn frap_files_reenter_edit_mode_only_where_blender_does() {
    let dir = std::env::temp_dir().join(format!("frap-edit-cond-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.frap").to_string_lossy().replace('\\', "/");
    let b = dir.join("b.frap").to_string_lossy().replace('\\', "/");
    // Saved with the camera active: both edited meshes load in Object Mode.
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(
        &mut s,
        "bpy.context.view_layer.objects.active = bpy.data.objects['Camera']",
    );
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].mode"), "EDIT");
    ok(
        &mut s,
        &format!("bpy.ops.wm.save_as_mainfile(filepath='{a}')"),
    );
    let mut t = session();
    let r = ok(&mut t, &format!("bpy.ops.wm.open_mainfile(filepath='{a}')"));
    assert_eq!(r["context"]["mode"], "OBJECT", "{r}");
    assert_eq!(value(&mut t, "bpy.data.objects['Cube'].mode"), "OBJECT");
    assert!(bm_gen(&t, "Cube").is_none());
    ok(&mut t, "bpy.data.meshes['Cube'].vertices[0].co = (1, 2, 3)");
    // A hidden edit object loads in Object Mode, the visible one re-enters.
    let mut s = session();
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))",
    );
    ok(&mut s, "bpy.data.objects['Cube'].select_set(True)");
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.data.objects['Cube'].hide_viewport = True");
    ok(
        &mut s,
        &format!("bpy.ops.wm.save_as_mainfile(filepath='{b}')"),
    );
    let mut t = session();
    ok(&mut t, &format!("bpy.ops.wm.open_mainfile(filepath='{b}')"));
    assert_eq!(value(&mut t, "bpy.data.objects['Plane'].mode"), "EDIT");
    assert_eq!(value(&mut t, "bpy.data.objects['Cube'].mode"), "OBJECT");
    assert!(bm_gen(&t, "Cube").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
