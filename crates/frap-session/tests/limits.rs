// SPDX-License-Identifier: GPL-3.0-or-later
//! `--max-elements` pre-checks and the `--stmt-timeout` deadline (ARCHITECTURE §12): absurd
//! operator values are refused before anything is built, a statement that runs past its
//! deadline is cancelled at a cooperative check point, rolled back, and answers `TIMEOUT`.

mod support;

use std::time::{Duration, Instant};

use support::*;

#[test]
fn absurd_primitive_sizes_are_refused_before_building() {
    let mut s = session();
    let before = s.state_dump();
    for src in [
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=10000000, y_subdivisions=10000000)",
        "bpy.ops.mesh.primitive_uv_sphere_add(segments=100000, ring_count=100000)",
    ] {
        let t = Instant::now();
        let (exc, code, msg) = err(&mut s, src);
        assert!(
            t.elapsed() < Duration::from_secs(2),
            "{src}: {:?}",
            t.elapsed()
        );
        assert_eq!(
            (exc.as_str(), code.as_str()),
            ("RuntimeError", "LIMIT_EXCEEDED")
        );
        assert_eq!(
            msg,
            "Error: frap: limit exceeded (max_elements): the mesh would need more than 2147483647 elements, more than a Blender mesh can index",
            "{src}"
        );
    }
    // Within Blender's range but above the default 50M corners.
    let t = Instant::now();
    let (_, code, msg) = err(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=5000, y_subdivisions=5000)",
    );
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    assert_eq!(code, "LIMIT_EXCEEDED");
    assert_eq!(
        msg,
        "Error: frap: limit exceeded (max_elements): the statement would create 100000000 face corners (limit 50000000)"
    );
    assert_eq!(s.state_dump(), before);
    // A raised limit lets the same request through the pre-check.
    let mut s = session_with(|o| o.limits.max_elements = 4 * 4 * 4);
    let r = one(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)",
    );
    assert_eq!(r["status"], "FINISHED", "{r}");
}

#[test]
fn stmt_timeout_cancels_and_rolls_back() {
    let mut s = session_with(|o| {
        o.limits.stmt_timeout = Some(Duration::from_millis(30));
        o.limits.max_elements = 1 << 30;
    });
    let before = s.state_dump();
    let t = Instant::now();
    let r = one(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=3000, y_subdivisions=3000)",
    );
    let took = t.elapsed();
    assert_eq!(r["status"], "ERROR", "{r}");
    assert_eq!(r["state"], "unchanged", "{r}");
    assert_eq!(r["error"]["exception"], "RuntimeError");
    assert_eq!(r["error"]["code"], "TIMEOUT");
    assert_eq!(
        r["error"]["message"],
        "Error: frap: limit exceeded (stmt_timeout)"
    );
    assert_eq!(r["error"]["detail"]["stmt_timeout_ms"], 30);
    // Cancelled at a check point well before the 9M-vertex grid was built.
    assert!(took < Duration::from_secs(5), "{took:?}");
    assert_eq!(s.state_dump(), before);
    // The session keeps working; small statements fit the deadline.
    let r = one(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))",
    );
    assert_eq!(r["status"], "FINISHED", "{r}");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube.001'].location"),
        serde_json::json!([3, 0, 0])
    );
}

#[test]
fn no_timeout_by_default() {
    let mut s = session();
    let r = one(
        &mut s,
        "bpy.ops.mesh.primitive_grid_add(x_subdivisions=200, y_subdivisions=200)",
    );
    assert_eq!(r["status"], "FINISHED", "{r}");
}

#[test]
fn stmt_timeout_cancels_quadratic_mesh_kernels() {
    // Loop cuts and bevels whose cost grows with the face size (each cut walks an n-gon) are
    // cancelled promptly: the deadline is polled by the work done, not per outer step.
    let mut s = session_with(|o| {
        o.limits.stmt_timeout = Some(Duration::from_millis(500));
        o.limits.max_elements = 1 << 30;
    });
    one(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let before = s.state_dump();
    for src in [
        "bpy.ops.mesh.loopcut(number_cuts=100000, edge_index=0, object_index=0)",
        "bpy.ops.mesh.loopcut(number_cuts=10000, edge_index=0, object_index=0)",
        "bpy.ops.mesh.bevel(offset=0.1, segments=300)",
        "bpy.ops.mesh.bevel(offset=1e30, segments=1000)",
    ] {
        let t = Instant::now();
        let r = one(&mut s, src);
        let took = t.elapsed();
        assert_eq!(r["error"]["code"], "TIMEOUT", "{src}: {r}");
        assert_eq!(r["state"], "unchanged", "{src}: {r}");
        // Unoptimised test builds on a loaded machine: well under the 20+ s it took before.
        assert!(took < Duration::from_secs(6), "{src}: {took:?}");
        assert_eq!(s.state_dump(), before, "{src}");
    }
}
