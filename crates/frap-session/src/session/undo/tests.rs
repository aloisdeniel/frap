// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the undo ring (WP 8.4): the stack algorithm on its own (`stack.rs` against
//! Blender's `undo_system.cc` rules), then sessions driving the `ed.*` operators. The Blender
//! comparisons are the goldens `tests/golden/cases/undo/*` (Blender pushes the same steps with
//! `#@blender bpy.ops.ed.undo_push`); these tests cover what Blender -b cannot show (the
//! automatic steps' names and types, grouped pushes, `ed.undo_redo`, file loads, the 1000-step
//! memory bound, the response of an undo statement).

use std::sync::Arc;

use serde_json::{Value as J, json};

use super::stack::{Dir, Stack, StepData};
use super::steps::Kind;
use crate::session::{Session, SessionOptions};

// ------------------------------------------------------------------ the stack on its own

/// A step of the mock stack: memfile or not, and a tag recording decodes.
#[derive(Clone, Debug, PartialEq)]
struct Mock {
    memfile: bool,
    tag: u32,
}

impl StepData for Mock {
    fn is_memfile(&self) -> bool {
        self.memfile
    }
}

fn mock(steps: &[(bool, bool)]) -> Stack<Mock> {
    let mut s = Stack::default();
    for (i, &(memfile, skip)) in steps.iter().enumerate() {
        s.append(
            smol_str::SmolStr::new(format!("s{i}")),
            Mock {
                memfile,
                tag: i as u32,
            },
            skip,
        );
    }
    s
}

fn names(s: &Stack<Mock>) -> Vec<String> {
    s.steps.iter().map(|x| x.name.to_string()).collect()
}

#[test]
fn limit_keeps_the_newest_steps_below_the_last() {
    // 41 memfile steps, limit 31 before a push (`ED_undo_push` with 32 steps): 32 stay.
    let mut s = mock(&[(true, false); 41]);
    s.limit_steps(31);
    assert_eq!(s.steps.len(), 32);
    assert_eq!(s.steps[0].name, "s9");
    assert_eq!(s.active, Some(31));
    assert_eq!(s.active_memfile, Some(31));
    // Fewer steps than the limit: nothing goes.
    let mut s = mock(&[(true, false); 5]);
    s.limit_steps(31);
    assert_eq!(s.steps.len(), 5);
}

#[test]
fn limit_keeps_one_memfile_below_edit_steps() {
    // memfile, then 5 edit steps; limit 2 keeps s4, s5 (and s3 where the count stops) and the
    // memfile s0 as a skipped step (`WITH_GLOBAL_UNDO_KEEP_ONE`).
    let mut s = mock(&[
        (true, false),
        (false, false),
        (false, false),
        (false, false),
        (false, false),
        (false, false),
    ]);
    s.limit_steps(2);
    assert_eq!(names(&s), ["s0", "s3", "s4", "s5"]);
    assert!(s.steps[0].skip);
    assert_eq!(s.active, Some(3));
}

#[test]
fn skipped_steps_are_not_stops_and_memfiles_load_first() {
    // s0 memfile, s1 edit, s2 memfile (skip, "MemFile Internal (pre)"), s3 edit.
    let mut s = mock(&[(true, false), (false, false), (true, true), (false, false)]);
    s.active_memfile = Some(2);
    let mut seen = Vec::new();
    assert!(s.undo(&mut |d: &Mock, dir, last| seen.push((d.tag, dir, last))));
    // s2 is decoded on the way, then s1 after the memfile it was made on (s0).
    assert_eq!(
        seen,
        [
            (2, Dir::Undo, false),
            (0, Dir::Undo, false),
            (1, Dir::Undo, true)
        ]
    );
    assert_eq!(s.active, Some(1));
    assert_eq!(s.active_memfile, Some(0));
    seen.clear();
    assert!(s.redo(&mut |d: &Mock, dir, last| seen.push((d.tag, dir, last))));
    assert_eq!(seen, [(2, Dir::Redo, false), (3, Dir::Redo, true)]);
    assert_eq!(s.active, Some(3));
    // Nothing before the first step.
    let mut s = mock(&[(true, false)]);
    assert!(!s.can_undo() && !s.can_redo());
    assert!(!s.undo(&mut |_, _, _| panic!("no decode")));
    // A skipped first step below the active one: the limit is reached, nothing loads.
    let mut s = mock(&[(true, true), (false, false)]);
    assert!(s.can_undo());
    assert!(!s.undo(&mut |_, _, _| panic!("no decode")));
    assert_eq!(s.active, Some(1));
}

#[test]
fn truncation_grouping_and_names() {
    let mut s = mock(&[(true, false), (true, false), (true, false)]);
    s.undo(&mut |_, _, _| {});
    s.truncate_after_active();
    assert_eq!(names(&s), ["s0", "s1"]);
    s.clear_active();
    assert_eq!(names(&s), ["s0"]);
    assert_eq!(s.active, Some(0));
    assert_eq!(s.find_by_name("s0"), Some(0));
    assert!(!s.has_undo_named("s0"));
    assert_eq!(super::stack::step_name(&"é".repeat(40)).len(), 62);
    assert_eq!(super::stack::step_name("Move"), "Move");
}

// ------------------------------------------------------------------ sessions

fn session() -> Session {
    Session::new(SessionOptions::default()).expect("factory session")
}

/// Runs statements, returning the JSON responses.
fn run(s: &mut Session, src: &str) -> Vec<J> {
    s.exec_source(src).iter().map(|r| r.to_json()).collect()
}

/// Runs one statement that must succeed.
fn ok(s: &mut Session, src: &str) -> J {
    let r = run(s, src).pop().expect("one response");
    assert_eq!(r["status"], "FINISHED", "{src}: {r:#}");
    r
}

fn step_names(s: &Session) -> Vec<String> {
    s.undo
        .stack
        .steps
        .iter()
        .map(|x| {
            let k = match x.data.kind() {
                Kind::Memfile => "M",
                Kind::EditMesh => "E",
                Kind::EditArmature => "A",
            };
            format!("{}{}:{}", k, if x.skip { "~" } else { "" }, x.name)
        })
        .collect()
}

fn active_name(s: &Session) -> String {
    let st = &s.undo.stack;
    st.steps[st.active.expect("active")].name.to_string()
}

fn value(s: &mut Session, path: &str) -> J {
    let r = s.get(path).to_json();
    r["value"].clone()
}

#[test]
fn fresh_session_has_only_the_original_step() {
    let mut s = session();
    let r = run(
        &mut s,
        "bpy.ops.ed.undo.poll()\nbpy.ops.ed.undo()\nbpy.ops.ed.redo()",
    );
    assert_eq!(r[0]["value"], false);
    assert_eq!(r[1]["error"]["code"], "OP_POLL_FAILED");
    assert_eq!(
        r[1]["error"]["message"],
        "Operator bpy.ops.ed.undo.poll() failed, context is incorrect"
    );
    assert_eq!(
        r[2]["error"]["message"],
        "Operator bpy.ops.ed.redo.poll() failed, context is incorrect"
    );
    assert_eq!(step_names(&s), ["M:Original"]);
    let r = ok(&mut s, "bpy.ops.ed.undo_push.poll()");
    assert_eq!(r["value"], true);
}

#[test]
fn steps_follow_operators_writes_and_calls() {
    let mut s = session();
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))",
    );
    ok(&mut s, "bpy.data.objects['Cube'].location.x = 2.0");
    // Context-only structs (the 3D View, the window) are not undone.
    ok(&mut s, "bpy.context.space_data.shading.type = 'WIREFRAME'");
    // An RNA call that changed data pushes, one that did not does not.
    ok(&mut s, "bpy.data.materials.new('M')");
    ok(&mut s, "bpy.data.objects['Cube'].select_get()");
    // Operators without UNDO (saving, exporting) push nothing.
    ok(&mut s, "bpy.ops.ed.undo_push(message='mark')");
    // A failed statement pushes nothing.
    assert_eq!(
        run(&mut s, "bpy.ops.mesh.primitive_cube_add(size=-1, sizee=2)")[0]["status"],
        "ERROR"
    );
    assert_eq!(
        step_names(&s),
        ["M:Original", "M:Add Cube", "M:Location", "M:new", "M:mark"]
    );
    assert_eq!(value(&mut s, "bpy.data.is_dirty"), true);
    // Undo statements name what came back.
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(r["result"]["undone"], "mark");
    assert_eq!(r["changes"]["added"], json!([]));
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(r["result"]["undone"], "new");
    assert_eq!(r["changes"]["removed"], json!(["bpy.data.materials['M']"]));
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(
        r["changes"]["modified"],
        json!(["bpy.data.objects['Cube']"])
    );
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].location.x"), 0.0);
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(
        r["changes"]["removed"],
        json!([
            "bpy.data.meshes['Cube.001']",
            "bpy.data.objects['Cube.001']"
        ])
    );
    assert_eq!(r["context"]["active_object"], "Cube");
    // The 3D View was not undone; the file stays modified.
    assert_eq!(
        value(&mut s, "bpy.context.space_data.shading.type"),
        "WIREFRAME"
    );
    let r = ok(&mut s, "bpy.ops.ed.redo()");
    assert_eq!(r["result"]["redone"], "Add Cube");
    assert_eq!(
        r["changes"]["added"],
        json!([
            "bpy.data.objects['Cube.001']",
            "bpy.data.meshes['Cube.001']"
        ])
    );
    // A new step drops what could be redone.
    ok(&mut s, "bpy.data.objects['Cube'].scale.z = 2.0");
    assert_eq!(step_names(&s), ["M:Original", "M:Add Cube", "M:Scale"]);
    assert!(!s.undo.stack.can_redo());
}

#[test]
fn grouped_frame_changes_are_one_step() {
    let mut s = session();
    ok(&mut s, "bpy.ops.screen.frame_offset(delta=1)");
    ok(&mut s, "bpy.ops.screen.frame_offset(delta=1)");
    ok(&mut s, "bpy.ops.screen.frame_jump(end=True)");
    assert_eq!(step_names(&s), ["M:Original", "M:Frame Change"]);
    ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(value(&mut s, "bpy.context.scene.frame_current"), 1);
}

#[test]
fn edit_mode_steps_and_compatibility() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.ops.mesh.select_all(action='DESELECT')");
    // An Object write in mesh Edit Mode is not undone (no step), the edit data is.
    ok(&mut s, "bpy.data.objects['Cube'].location.x = 4.0");
    ok(&mut s, "bpy.data.meshes['Cube'].name = 'Box'");
    ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='VERT', indices=[0])",
    );
    ok(&mut s, "bpy.ops.transform.translate(value=(0, 0, 1))");
    assert_eq!(
        step_names(&s),
        [
            "M:Original",
            "E:Edit Mode",
            "E:(De)select All",
            "M~:MemFile Internal (pre)",
            "E:Name",
            "E:Select Elements",
            "E:Move"
        ]
    );
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(r["result"]["undone"], "Move");
    assert_eq!(r["changes"]["modified"], json!(["bpy.data.meshes['Box']"]));
    assert_eq!(r["edit_mesh"]["Cube"]["selected"]["verts"], 1);
    ok(&mut s, "bpy.ops.ed.undo()");
    // Undoing the rename steps over the skipped memfile step and loads the memfile below the
    // "(De)select All" step: the rename and the location go.
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(r["result"]["undone"], "Name");
    assert_eq!(active_name(&s), "(De)select All");
    assert_eq!(
        r["changes"]["renamed"],
        json!([{"from": "bpy.data.meshes['Box']", "to": "bpy.data.meshes['Cube']"}])
    );
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].location.x"), 0.0);
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(r["edit_mesh"]["Cube"]["selected"]["verts"], 8);
    let r = ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(r["context"]["mode"], "OBJECT");
    assert!(s.host.state.edit.meshes.is_empty());
    // Redo back into Edit Mode, then through the skipped step.
    let r = ok(&mut s, "bpy.ops.ed.redo()");
    assert_eq!(r["context"]["mode"], "EDIT_MESH");
    ok(&mut s, "bpy.ops.ed.redo()");
    ok(&mut s, "bpy.ops.ed.redo()");
    assert_eq!(active_name(&s), "Name");
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].location.x"), 4.0);
    ok(&mut s, "bpy.ops.ed.redo()");
    let r = ok(&mut s, "bpy.ops.ed.redo()");
    assert_eq!(r["result"]["redone"], "Move");
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(active_name(&s), "Edit Mode");
    assert_eq!(
        step_names(&s).last().map(String::as_str),
        Some("M:Edit Mode")
    );
}

#[test]
fn edit_mesh_step_restores_a_compact_bmesh_and_its_history() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.ops.mesh.select_all(action='DESELECT')");
    ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='FACE', indices=[2])",
    );
    ok(&mut s, "bpy.ops.mesh.delete(type='VERT')");
    ok(
        &mut s,
        "bpy.ops.frap.select_elements(type='VERT', indices=[0, 3])",
    );
    ok(&mut s, "bpy.ops.transform.translate(value=(1, 0, 0))");
    let me = s.host.state.edit.meshes.values().next().expect("edit mesh");
    let before = Arc::clone(&me.bm);
    assert!(!before.verts.free.is_empty(), "the delete left free slots");
    ok(&mut s, "bpy.ops.ed.undo()");
    let me = s.host.state.edit.meshes.values().next().expect("edit mesh");
    assert!(me.bm.verts.free.is_empty() && me.bm.faces.free.is_empty());
    assert_eq!(me.bm.verts_num(), before.verts_num());
    assert_eq!(me.bm.select_history.len(), 2);
    let r = ok(&mut s, "bpy.ops.ed.redo()");
    assert_eq!(r["edit_mesh"]["Cube"]["select_history_len"], 2);
}

#[test]
fn edit_armature_steps() {
    let mut s = session();
    ok(&mut s, "bpy.ops.object.armature_add()");
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.ops.armature.bone_primitive_add(name='Second')");
    ok(&mut s, "bpy.ops.armature.select_all(action='SELECT')");
    ok(&mut s, "bpy.ops.transform.translate(value=(0, 0, 2))");
    let names = step_names(&s);
    // Entering armature Edit Mode clears `is_memfile_undo_written` (`object_edit.cc`), so
    // its step comes after a skipped memfile step.
    assert_eq!(
        &names[1..4],
        ["M:Add Armature", "M~:MemFile Internal (pre)", "A:Edit Mode"],
        "{names:?}"
    );
    assert!(
        names.iter().skip(4).all(|n| n.starts_with("A:")),
        "{names:?}"
    );
    let head = |s: &mut Session| -> Vec<f64> {
        let v = value(s, "bpy.data.armatures['Armature'].edit_bones['Bone'].head");
        v.as_array()
            .expect("a vector")
            .iter()
            .map(|x| x.as_f64().expect("a number"))
            .collect()
    };
    assert_eq!(head(&mut s), [0.0, 0.0, 2.0]);
    ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(head(&mut s), [0.0, 0.0, 0.0]);
    ok(&mut s, "bpy.ops.ed.undo()");
    ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(
        run(
            &mut s,
            "bpy.data.armatures['Armature'].edit_bones['Second']"
        )[0]["status"],
        "ERROR"
    );
    ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(value(&mut s, "bpy.context.mode"), "OBJECT");
    for _ in 0..4 {
        ok(&mut s, "bpy.ops.ed.redo()");
    }
    assert_eq!(head(&mut s), [0.0, 0.0, 2.0]);
}

#[test]
fn undo_redo_repeats_the_last_registered_operator() {
    let mut s = session();
    let r = run(&mut s, "bpy.ops.ed.undo_redo()");
    assert_eq!(r[0]["error"]["code"], "OP_POLL_FAILED");
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(size=3, location=(0, 5, 0))",
    );
    assert_eq!(ok(&mut s, "bpy.ops.ed.undo_redo.poll()")["value"], true);
    let r = ok(&mut s, "bpy.ops.ed.undo_redo()");
    assert_eq!(r["result"]["repeated"], "mesh.primitive_cube_add");
    // Undone and run again: the same object name, one step.
    assert_eq!(s.main().objects.len(), 4);
    assert_eq!(
        value(&mut s, "bpy.data.objects['Cube.001'].location.y"),
        5.0
    );
    assert_eq!(step_names(&s), ["M:Original", "M:Add Cube"]);
    // ed.undo forgets the operator (`WM_operator_stack_clear`).
    ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(ok(&mut s, "bpy.ops.ed.undo_redo.poll()")["value"], false);
    // object.mode_set registers the toggle operator it calls.
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let r = ok(&mut s, "bpy.ops.ed.undo_redo()");
    assert_eq!(r["result"]["repeated"], "object.editmode_toggle");
    assert_eq!(r["context"]["mode"], "EDIT_MESH");
    // `ed.undo` read the "Original" memfile step: a new `Main`, not yet written to a memfile,
    // so the Edit Mesh step comes after a skipped memfile step.
    assert_eq!(
        step_names(&s),
        ["M:Original", "M~:MemFile Internal (pre)", "E:Edit Mode"]
    );
    // The API entry point repeats with its keyword inputs.
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    let mut kw = indexmap::IndexMap::new();
    kw.insert("size".to_owned(), frap_rna::Input::Float(0.5));
    assert!(s.invoke("mesh.primitive_plane_add", kw).ok());
    let r = s
        .invoke("ed.undo_redo", indexmap::IndexMap::new())
        .to_json();
    assert_eq!(r["status"], "FINISHED", "{r:#}");
    assert_eq!(value(&mut s, "bpy.data.objects['Plane'].dimensions.x"), 0.5);
}

#[test]
fn file_loads_reset_the_history() {
    let mut s = session();
    ok(&mut s, "bpy.ops.mesh.primitive_monkey_add()");
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(&mut s, "bpy.ops.wm.read_homefile()");
    assert_eq!(step_names(&s), ["M:Original"]);
    assert_eq!(ok(&mut s, "bpy.ops.ed.undo.poll()")["value"], false);
    ok(&mut s, "bpy.ops.mesh.primitive_monkey_add()");
    ok(&mut s, "bpy.ops.wm.read_factory_settings(use_empty=True)");
    assert_eq!(step_names(&s), ["M:Original"]);
    assert_eq!(s.main().objects.len(), 0);
}

#[test]
fn a_step_costs_o_ids_and_the_ring_is_bounded() {
    let mut s = session();
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=300, y_subdivisions=300)",
    );
    let grid = s.host.state.main.meshes.find("Grid").expect("grid mesh");
    let t = std::time::Instant::now();
    for i in 0..1000 {
        let r = s.set(
            "bpy.data.objects['Cube'].location.x",
            frap_rna::Input::Float(f64::from(i)),
        );
        assert!(r.ok(), "{:#}", r.to_json());
    }
    let elapsed = t.elapsed();
    // `edit.undo_steps` (32) below the active step.
    assert_eq!(s.undo.stack.steps.len(), super::UNDO_STEPS + 1);
    // Every step shares the 90 601-vertex grid with the session state: nothing was copied.
    let live = s.host.state.main.meshes.get_arc(grid).expect("grid");
    for step in &s.undo.stack.steps {
        let super::steps::Data::Memfile(snap) = &step.data else {
            panic!("memfile steps only");
        };
        let m = snap.main.meshes.get_arc(grid).expect("grid in step");
        assert!(Arc::ptr_eq(&m, &live));
    }
    // 33 steps alive plus the state: the grid is referenced by them, not duplicated.
    assert!(Arc::strong_count(&live) <= 2 + s.undo.stack.steps.len());
    assert!(
        elapsed < std::time::Duration::from_secs(60),
        "1000 pushes took {elapsed:?}"
    );
    // Undo still works at the end of the ring.
    ok(&mut s, "bpy.ops.ed.undo()");
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].location.x"), 998.0);
}

#[test]
fn cancelled_and_failed_undo_statements_change_nothing() {
    let mut s = session();
    ok(&mut s, "bpy.ops.mesh.primitive_cube_add()");
    let dump = s.state_dump();
    // A rolled-back statement drops its request.
    let r = run(&mut s, "bpy.ops.ed.undo(bogus=1)");
    assert_eq!(r[0]["status"], "ERROR");
    assert_eq!(s.state_dump(), dump);
    assert_eq!(active_name(&s), "Add Cube");
    // A stale status (published by another session on this thread, the poll reading it
    // before this session's own): the request is checked against the ring and the call
    // answers with this session's poll error.
    let mut fresh = session();
    super::enter(&mut fresh);
    for op in ["ed.undo", "ed.redo", "ed.undo_redo"] {
        frap_op::set_undo_status(frap_op::UndoStatus {
            can_undo: true,
            can_redo: true,
            can_undo_redo: true,
        });
        let mut r = fresh
            .invoke_values(1, op, vec![], vec![])
            .unwrap_or_else(|_| panic!("{op} runs"));
        assert_eq!(r.status, crate::response::Status::Finished);
        super::finish(&mut fresh, super::Origin::Other, &mut r);
        let j = r.to_json();
        assert_eq!(j["error"]["code"], "OP_POLL_FAILED", "{j:#}");
        assert_eq!(j["state"], "unchanged");
    }
    assert_eq!(step_names(&fresh), ["M:Original"]);
    assert!(!frap_op::undo_status().can_undo);
}

#[test]
fn the_step_limit_chain_matches_blender() {
    // Golden `undo/step_limit`, with the intermediate values the golden does not compare
    // (Blender 5.2.4, `ed.undo_push` after each of 40 writes: the 32 undos give 39, 38, …,
    // 8; the 33rd fails its poll; 32 redos give 40 again).
    let mut s = session();
    for i in 1..=40 {
        ok(
            &mut s,
            &format!("bpy.data.objects['Cube'].location.x = {i}.0"),
        );
    }
    let mut xs = vec![];
    for _ in 0..32 {
        ok(&mut s, "bpy.ops.ed.undo()");
        xs.push(value(&mut s, "bpy.data.objects['Cube'].location.x").as_f64());
    }
    let want: Vec<Option<f64>> = (8..=39).rev().map(|x| Some(f64::from(x))).collect();
    assert_eq!(xs, want);
    assert_eq!(ok(&mut s, "bpy.ops.ed.undo.poll()")["value"], false);
    for _ in 0..32 {
        ok(&mut s, "bpy.ops.ed.redo()");
    }
    assert_eq!(value(&mut s, "bpy.data.objects['Cube'].location.x"), 40.0);
    assert_eq!(ok(&mut s, "bpy.ops.ed.redo.poll()")["value"], false);
}

/// The session of the `--stmt-timeout` tests: a 300 x 300 grid in Edit Mode, three selection
/// steps (decoding one rebuilds the 90 601-vertex edit BMesh, which has cancellation points).
fn big_edit_mesh() -> Session {
    let mut s = session();
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=300, y_subdivisions=300)",
    );
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    for action in ["DESELECT", "SELECT", "INVERT"] {
        ok(
            &mut s,
            &format!("bpy.ops.mesh.select_all(action='{action}')"),
        );
    }
    s
}

/// Runs an `ed.*` operator, then [`super::finish`] (which decodes the steps) under a budget
/// whose deadline has already passed: the statement itself ran, the decode is interrupted.
fn finish_after_the_deadline(s: &mut Session, op: &str) -> J {
    super::enter(s);
    let mut r = s
        .invoke_values(1, op, vec![], vec![])
        .unwrap_or_else(|_| panic!("{op} runs"));
    assert_eq!(r.status, crate::response::Status::Finished);
    let timeout = std::time::Duration::from_nanos(1);
    s.opts.limits.stmt_timeout = Some(timeout);
    let budget = frap_math::budget::scope(frap_math::budget::Budget {
        timeout: Some(timeout),
        max_elements: u64::MAX,
    });
    std::thread::sleep(std::time::Duration::from_millis(1));
    super::finish(s, super::Origin::Other, &mut r);
    drop(budget);
    s.opts.limits.stmt_timeout = None;
    r.to_json()
}

#[test]
fn a_timeout_while_a_step_is_decoded_keeps_the_history() {
    // `ed.undo` decodes the previous step; `ed.undo_redo` loads the step before the repeated
    // operator's, runs it again (which times out) and decodes the step it left (`ED_undo_redo`).
    for op in ["ed.undo", "ed.undo_redo"] {
        let mut s = big_edit_mesh();
        let steps = step_names(&s);
        let active = active_name(&s);
        let dump = s.state_dump();
        assert!(s.undo.last_redo.is_some());
        let r = finish_after_the_deadline(&mut s, op);
        assert_eq!(r["error"]["code"], "TIMEOUT", "{op}: {r:#}");
        assert_eq!(r["state"], "unchanged", "{op}");
        // The statement was rolled back, its history included.
        assert_eq!(s.state_dump(), dump, "{op}");
        assert_eq!(step_names(&s), steps, "{op}");
        assert_eq!(active_name(&s), active, "{op}");
        assert!(s.undo.last_redo.is_some(), "{op}");
        assert!(frap_op::undo_status().can_undo_redo, "{op}");
        // Without the limit, every step can still be undone.
        let r = ok(&mut s, "bpy.ops.ed.undo()");
        assert_eq!(r["result"]["undone"], "(De)select All", "{op}");
        for _ in 0..3 {
            ok(&mut s, "bpy.ops.ed.undo()");
        }
        let r = ok(&mut s, "bpy.ops.ed.undo()");
        assert_eq!(r["result"]["undone"], "Add Grid", "{op}");
        assert_eq!(r["context"]["mode"], "OBJECT", "{op}");
        assert!(s.main().objects.find("Grid").is_none(), "{op}");
    }
}

#[test]
fn a_bug_while_a_step_is_decoded_clears_the_history_and_says_so() {
    let mut s = big_edit_mesh();
    let dump = s.state_dump();
    // A panic in the decode stands for any bug there.
    super::enter(&mut s);
    let mut r = s
        .invoke_values(1, "ed.undo", vec![], vec![])
        .unwrap_or_else(|_| panic!("ed.undo runs"));
    super::steps::PANIC_ON_DECODE.with(|p| p.set(true));
    super::finish(&mut s, super::Origin::Other, &mut r);
    super::steps::PANIC_ON_DECODE.with(|p| p.set(false));
    let j = r.to_json();
    assert_eq!(j["error"]["code"], "INTERNAL", "{j:#}");
    assert!(
        j["reports"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x["message"]
                .as_str()
                .is_some_and(|m| m.contains("undo history was cleared")))),
        "{j:#}"
    );
    assert_eq!(s.state_dump(), dump);
    // The new history starts at the statement's result.
    assert_eq!(step_names(&s), ["M:Original", "E:Original Mode"]);
    assert_eq!(active_name(&s), "Original Mode");
}

#[test]
fn a_limit_while_a_step_is_decoded_keeps_the_history() {
    // `--max-elements` below the grid's corner count: rebuilding the edit BMesh of a step
    // exceeds it (`budget::check_corners`).
    let mut s = big_edit_mesh();
    let steps = step_names(&s);
    let dump = s.state_dump();
    s.opts.limits.max_elements = 1000;
    let r = run(&mut s, "bpy.ops.ed.undo()")
        .pop()
        .expect("one response");
    assert_ne!(r["status"], "FINISHED", "{r:#}");
    assert_eq!(r["error"]["code"], "LIMIT_EXCEEDED", "{r:#}");
    assert_eq!(s.state_dump(), dump);
    assert_eq!(step_names(&s), steps);
}
