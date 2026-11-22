// SPDX-License-Identifier: GPL-3.0-or-later
//! §21.7: "a 100k-face edit-mode operator < 200 ms" (`mesh.subdivide`, `mesh.extrude_region`,
//! `mesh.bevel`), measured through a real `Session` as one statement plus its JSON response.
//!
//! **Scene.** A 316 × 316 grid (99,856 faces) in Edit Mode, plus a 12 × 12 grid added in Edit
//! Mode, which deselects the big one and selects the new 144 faces: 100,000 faces, 144
//! selected. The operator works on a local selection of a 100k-face mesh, which is what the
//! budget is about: everything the operator pays for the rest of the mesh (copy-on-write of
//! the edit BMesh, selection flushing, the undo step, the response's `edit_mesh` counts).
//! Blender 5.2.4 runs the same three statements in 30 ms, 29 ms and 36 ms (M1 Max, warm,
//! `xtask/perf/blender_reference.py`); with the whole 317 × 317 grid selected it needs 262 ms
//! (subdivide, 400k faces out) and 108 ms (extrude_region), and 2.5 s for bevel, so a
//! whole-mesh result is not what a 200 ms budget can mean. The whole-mesh variants are
//! measured too ([`whole_mesh_subdivide`], [`whole_mesh_extrude_region`]), as tracked values
//! without a budget.
//!
//! **Golden.** The element and selection counts after each operator are Blender 5.2.4's (the
//! same script): the selection is local, so the deltas do not depend on the big grid's size,
//! and the debug run (a 40 × 40 grid) checks the same deltas. The whole-mesh counts are
//! Blender's closed forms for an `n × n` grid, checked against its numbers at `n = 317`.

mod support;

use std::time::Duration;

use serde_json::json;
use support::*;

/// Faces per side of the big grid (99,856 faces in release).
fn big() -> u32 {
    if release() { 316 } else { 40 }
}

/// Fresh session with the scene of the module docs, in Edit Mode.
fn scene() -> frap_session::Session {
    let mut s = factory();
    ok(&mut s, "bpy.ops.wm.read_factory_settings(use_empty=True)");
    let n = big();
    ok(
        &mut s,
        &format!("bpy.ops.mesh.primitive_grid_add(x_subdivisions={n}, y_subdivisions={n}, size=2.0)"),
    );
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    ok(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=12, y_subdivisions=12, size=0.5, location=(0, 0, 0.5))",
    );
    s
}

/// `(vertices, edges, faces)` of the scene before the operator.
fn base_counts() -> (i64, i64, i64) {
    let n = i64::from(big());
    let (sv, se, sf) = (13 * 13, 2 * 12 * 13, 144);
    (
        (n + 1) * (n + 1) + sv,
        2 * n * (n + 1) + se,
        n * n + sf,
    )
}

/// Runs `stmt` on fresh scenes (one warm-up, then `reps`); checks the result counts against
/// Blender's deltas and reports the median.
fn measure(name: &str, stmt: &str, delta: (i64, i64, i64), sel: (i64, i64, i64)) {
    let _g = serial();
    let reps = if release() { 7 } else { 1 };
    let mut times: Vec<Duration> = Vec::new();
    for rep in 0..=reps {
        let mut s = scene();
        assert_eq!(counts(&mut s), base_counts(), "scene");
        let (d, r) = timed(&mut s, stmt);
        let (v0, e0, f0) = base_counts();
        assert_eq!(
            counts(&mut s),
            (v0 + delta.0, e0 + delta.1, f0 + delta.2),
            "{stmt}: element counts differ from Blender 5.2.4"
        );
        let em = &r["edit_mesh"]["Grid"]["selected"];
        assert_eq!(
            (
                em["verts"].as_i64(),
                em["edges"].as_i64(),
                em["faces"].as_i64()
            ),
            (Some(sel.0), Some(sel.1), Some(sel.2)),
            "{stmt}: selection differs from Blender 5.2.4: {em}"
        );
        if rep > 0 {
            times.push(d);
        }
    }
    let med = median(&mut times);
    report(
        name,
        ms(med),
        Some(200.0),
        json!({
            "faces": base_counts().2,
            "selected_faces": 144,
            "min_ms": ms(times[0]),
            "max_ms": ms(*times.last().expect("reps")),
        }),
    );
}

#[test]
fn subdivide_on_100k_faces() {
    // Blender 5.2.4: 101114 / 201544 / 100432 elements, 625 / 1200 / 576 selected.
    measure(
        "edit_op_100k.subdivide",
        "bpy.ops.mesh.subdivide()",
        (101_114 - 100_658, 201_544 - 200_656, 100_432 - 100_000),
        (625, 1200, 576),
    );
}

#[test]
fn extrude_region_on_100k_faces() {
    // Blender 5.2.4: 100827 / 201016 / 100192 elements, 169 / 312 / 144 selected.
    measure(
        "edit_op_100k.extrude_region",
        "bpy.ops.mesh.extrude_region()",
        (100_827 - 100_658, 201_016 - 200_656, 100_192 - 100_000),
        (169, 312, 144),
    );
}

#[test]
fn bevel_on_100k_faces() {
    // Blender 5.2.4: 101065 / 201448 / 100385 elements, 572 / 1096 / 525 selected.
    measure(
        "edit_op_100k.bevel",
        "bpy.ops.mesh.bevel(offset=0.01)",
        (101_065 - 100_658, 201_448 - 200_656, 100_385 - 100_000),
        (572, 1096, 525),
    );
}

/// Whole-mesh variant: `stmt` on an `n × n` grid with everything selected; checks the counts
/// `expect(n)` and reports the median as a tracked value (no budget).
fn measure_whole(name: &str, stmt: &str, expect: fn(i64) -> [i64; 6]) {
    let _g = serial();
    let n = big() + 1;
    let reps = if release() { 3 } else { 1 };
    let mut times = Vec::new();
    for rep in 0..=reps {
        let mut s = factory();
        ok(&mut s, "bpy.ops.wm.read_factory_settings(use_empty=True)");
        ok(
            &mut s,
            &format!("bpy.ops.mesh.primitive_grid_add(x_subdivisions={n}, y_subdivisions={n})"),
        );
        ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
        ok(&mut s, "bpy.ops.mesh.select_all(action='SELECT')");
        let (d, r) = timed(&mut s, stmt);
        let (v, e, f) = counts(&mut s);
        let sel = &r["edit_mesh"]["Grid"]["selected"];
        let got = [
            v,
            e,
            f,
            sel["verts"].as_i64().unwrap_or(-1),
            sel["edges"].as_i64().unwrap_or(-1),
            sel["faces"].as_i64().unwrap_or(-1),
        ];
        assert_eq!(got, expect(i64::from(n)), "{stmt} on a {n} x {n} grid");
        if rep > 0 {
            times.push(d);
        }
    }
    report(
        name,
        ms(median(&mut times)),
        None,
        json!({ "faces": i64::from(n * n) }),
    );
}

#[test]
fn whole_mesh_subdivide() {
    // Blender 5.2.4 at n = 317: 403225 / 805180 / 401956, all selected.
    let f = |n: i64| {
        let (v, e, f) = ((2 * n + 1) * (2 * n + 1), 2 * (2 * n) * (2 * n + 1), 4 * n * n);
        [v, e, f, v, e, f]
    };
    assert_eq!(f(317), [403_225, 805_180, 401_956, 403_225, 805_180, 401_956]);
    measure_whole("edit_op_100k_all.subdivide", "bpy.ops.mesh.subdivide()", f);
}

#[test]
fn whole_mesh_extrude_region() {
    // Blender 5.2.4 at n = 317: 202248 / 404492 / 202246; 101124 / 201612 / 100489 selected.
    let f = |n: i64| {
        let (v, e) = ((n + 1) * (n + 1), 2 * n * (n + 1));
        [2 * v, 2 * e + 4 * n, 2 * n * n + 4 * n, v, e, n * n]
    };
    assert_eq!(f(317), [202_248, 404_492, 202_246, 101_124, 201_612, 100_489]);
    measure_whole("edit_op_100k_all.extrude_region", "bpy.ops.mesh.extrude_region()", f);
}
