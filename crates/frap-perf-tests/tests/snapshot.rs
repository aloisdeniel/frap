// SPDX-License-Identifier: GPL-3.0-or-later
//! §21.7: "snapshot clone O(#IDs)", and the undo push of WP 8.4 with it: every statement
//! clones `State` for its rollback and pushes an undo step that shares the state's data.
//! Neither may copy mesh data or an edit BMesh.
//!
//! - **Sharing.** Statements that do not write geometry leave the mesh's position column and
//!   the edit BMesh at the same address (no copy-on-write happened), while the undo ring and
//!   the transaction snapshot hold references to them.
//! - **Independent of data size.** The same one-property statement costs the same on a scene
//!   whose mesh has 1M corners as on the default cube: with a deep copy it would cost several
//!   milliseconds more.
//! - **Linear in #IDs.** On scenes of `N` and `8N` objects the statement's cost (with its
//!   `normal` response, whose context block lists the selected objects) grows at most
//!   linearly (the per-ID cost is bounded), never quadratically. WP 8.8 fixed the one
//!   quadratic step it found: `frap_scene::layers::visibility` searched the view layer's
//!   bases once per object, so every response cost O(#objects²) (3.3 ms at 4,000 objects).

mod support;

use std::time::Duration;

use frap_session::{Session, Verbosity};
use serde_json::json;
use support::*;

/// Median time of `stmt` over `reps` runs (after two warm-up runs).
fn cost(s: &mut Session, stmt: &str, reps: usize) -> Duration {
    for _ in 0..2 {
        timed(s, stmt);
    }
    let mut v: Vec<Duration> = (0..reps).map(|_| timed(s, stmt).0).collect();
    median(&mut v)
}

fn positions_ptr(s: &Session, mesh: &str) -> *const [f32; 3] {
    let main = s.main();
    let (_, me) = main
        .meshes
        .iter()
        .find(|&(r, _)| main.meshes.name_of(r) == Some(mesh))
        .expect("mesh exists");
    me.positions().as_ptr()
}

fn bmesh_ptr(s: &Session) -> *const frap_bmesh::BMesh {
    let edit = s.host().edit();
    assert_eq!(edit.meshes.len(), 1, "one mesh in Edit Mode");
    let em = edit.meshes.values().next().expect("edit mesh");
    std::sync::Arc::as_ptr(&em.bm)
}

/// The one-property statements every test times: an object property (a memfile undo step in
/// Object Mode) and a scene property.
const STMTS: &[(&str, &str)] = &[
    ("object_location", "bpy.context.object.location.x = 0.25"),
    ("scene_frame_end", "bpy.context.scene.frame_end = 120"),
];

fn grid_side() -> u32 {
    if release() { 500 } else { 60 }
}

#[test]
fn statements_share_mesh_data_and_the_edit_bmesh() {
    let _g = serial();
    let mut s = factory();
    let n = grid_side();
    ok(
        &mut s,
        &format!("bpy.ops.mesh.primitive_grid_add(x_subdivisions={n}, y_subdivisions={n})"),
    );
    let p0 = positions_ptr(&s, "Grid");
    for &(_, stmt) in STMTS {
        ok(&mut s, stmt);
        ok(&mut s, "bpy.ops.ed.undo_push(message='step')");
        assert_eq!(positions_ptr(&s, "Grid"), p0, "{stmt}: the mesh was copied");
    }
    // A failed statement restores the snapshot: the same data, still shared.
    let r = s.exec_source("bpy.context.object.location.x = 'x'").remove(0).to_json();
    assert_eq!(r["status"], "ERROR");
    assert_eq!(positions_ptr(&s, "Grid"), p0, "rollback copied the mesh");
    // Undo and redo load steps that share the same column.
    ok(&mut s, "bpy.ops.ed.undo()");
    ok(&mut s, "bpy.ops.ed.redo()");
    assert_eq!(positions_ptr(&s, "Grid"), p0, "undo/redo copied the mesh");

    // Edit Mode: undo steps of non-geometry statements share the edit BMesh.
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let b0 = bmesh_ptr(&s);
    for stmt in [
        "bpy.context.object.data.name = 'GridMesh'",
        "bpy.context.object.location.x = 0.5",
        "bpy.context.scene.frame_end = 90",
        "bpy.ops.ed.undo_push(message='edit step')",
    ] {
        ok(&mut s, stmt);
        assert_eq!(bmesh_ptr(&s), b0, "{stmt}: the edit BMesh was copied");
    }
}

#[test]
fn statement_cost_does_not_depend_on_mesh_size() {
    let _g = serial();
    let reps = if release() { 41 } else { 5 };
    // `minimal` responses: the statement is its transaction snapshot, its execution and its
    // undo push. (A `normal` response in Edit Mode adds the `edit_mesh` summary, which counts
    // the selection: O(elements) by design, ~0.75 ms at 1M corners on an M1 Max.)
    let mut small = factory_with(Verbosity::Minimal);
    let mut big = factory_with(Verbosity::Minimal);
    let n = grid_side();
    ok(&mut big, "bpy.ops.object.delete()");
    ok(&mut big, "bpy.data.meshes.remove(bpy.data.meshes['Cube'])");
    ok(
        &mut big,
        &format!("bpy.ops.mesh.primitive_grid_add(x_subdivisions={n}, y_subdivisions={n})"),
    );
    // Same #IDs on both sides: the factory scene with its cube replaced by a grid.
    assert_eq!(small.main().meshes.len(), big.main().meshes.len());
    for (mode, setup) in [("object", None), ("edit", Some("bpy.ops.object.mode_set(mode='EDIT')"))] {
        if let Some(src) = setup {
            ok(&mut small, src);
            ok(&mut big, src);
        }
        for &(label, stmt) in STMTS {
            let t_small = cost(&mut small, stmt, reps);
            let t_big = cost(&mut big, stmt, reps);
            let name = format!("snapshot.data_size.{mode}.{label}");
            // A deep copy of 1M corners (~40 MB of columns) costs well over a millisecond;
            // the bound leaves room for timer noise on a shared CI runner.
            let bound = t_small * 3 + Duration::from_micros(500);
            if release() {
                assert!(
                    t_big < bound,
                    "{name}: {:.3} ms on a 1M-corner mesh vs {:.3} ms on the cube: the statement copies mesh data",
                    ms(t_big),
                    ms(t_small)
                );
            }
            report(
                &name,
                ms(t_big),
                None,
                json!({ "cube_ms": ms(t_small), "bound_ms": ms(bound), "corners": 4 * n * n }),
            );
        }
    }
}

/// A scene of `n` cubes (shared mesh data: `n` objects, one mesh).
fn objects(n: usize) -> Session {
    let mut s = factory();
    for i in 1..n {
        ok(
            &mut s,
            &format!("bpy.context.collection.objects.link(bpy.data.objects.new('C{i}', bpy.data.meshes['Cube']))"),
        );
    }
    s
}

#[test]
fn statement_cost_is_linear_in_ids() {
    let _g = serial();
    let (n, reps) = if release() { (250, 41) } else { (20, 5) };
    let mut a = objects(n);
    let mut b = objects(8 * n);
    let ids = |s: &Session| s.main().objects.len();
    let (ia, ib) = (ids(&a), ids(&b));
    for &(label, stmt) in STMTS {
        let ta = cost(&mut a, stmt, reps);
        let tb = cost(&mut b, stmt, reps);
        // Linear growth allows 8x for 8x the IDs; twice that for noise. Quadratic would be 64x.
        let ratio = tb.as_secs_f64() / ta.as_secs_f64().max(1e-9);
        let per_id_us = (tb.saturating_sub(ta)).as_secs_f64() * 1e6 / (ib - ia) as f64;
        let name = format!("snapshot.ids.{label}");
        if release() {
            assert!(
                ratio < 16.0,
                "{name}: {:.3} ms with {ib} objects vs {:.3} ms with {ia}: worse than linear",
                ms(tb),
                ms(ta)
            );
            // Cloning an `IdMap` entry, pushing it to the undo ring and diffing it for the
            // response is a few pointer copies per ID.
            assert!(per_id_us < 5.0, "{name}: {per_id_us:.2} µs per ID");
        }
        report(
            &name,
            ms(tb),
            None,
            json!({ "objects": [ia, ib], "ms": [ms(ta), ms(tb)], "ratio": ratio, "per_id_us": per_id_us }),
        );
    }
}
