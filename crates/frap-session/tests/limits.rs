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

/// M5+M6 stabilization round 4: a Subdivision Surface evaluation (`levels = 10` on the cube,
/// 25M corners, OpenSubdiv's limit surface) polls the deadline in its per-corner and
/// per-vertex loops: `frap.scene_summary` answers `TIMEOUT` within about twice the deadline,
/// wherever the deadline falls (it used to run unchecked for minutes past ~2.5 s).
#[test]
fn stmt_timeout_cancels_subsurf_evaluation() {
    for ms in [1500u64, 3000] {
        let mut s = session_with(|o| o.limits.stmt_timeout = Some(Duration::from_millis(ms)));
        for src in [
            "bpy.data.objects['Cube'].modifiers.new('S', 'SUBSURF')",
            "bpy.data.objects['Cube'].modifiers['S'].levels = 10",
        ] {
            assert_eq!(one(&mut s, src)["ok"], true, "{src}");
        }
        let t = Instant::now();
        let r = one(&mut s, "bpy.ops.frap.scene_summary()");
        let took = t.elapsed();
        assert_eq!(r["error"]["code"], "TIMEOUT", "{ms} ms: {r}");
        assert!(
            took < Duration::from_millis(2 * ms + 1000),
            "{ms} ms: {took:?}"
        );
    }
}

/// M5+M6 stabilization round 4: the glTF exporter's animation sampling (`SCENE` mode over a
/// million frames) polls the deadline per sampled frame and in its per-frame channel loops.
#[test]
fn stmt_timeout_cancels_scene_animation_export() {
    let dir = std::env::temp_dir().join(format!("frap-anim-timeout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session_with(|o| {
        o.cwd = dir.clone();
        o.limits.stmt_timeout = Some(Duration::from_millis(1000));
    });
    for src in [
        "bpy.data.objects['Cube'].keyframe_insert('location', frame=1)",
        "bpy.data.objects['Cube'].location = (4.0, 0.0, 0.0)",
        "bpy.data.objects['Cube'].keyframe_insert('location', frame=11)",
        "bpy.context.scene.frame_end = 1048574",
    ] {
        assert_eq!(one(&mut s, src)["ok"], true, "{src}");
    }
    let t = Instant::now();
    let r = one(
        &mut s,
        "bpy.ops.export_scene.gltf(filepath='a.glb', export_animation_mode='SCENE')",
    );
    let took = t.elapsed();
    assert_eq!(r["error"]["code"], "TIMEOUT", "{r}");
    assert!(took < Duration::from_millis(3000), "{took:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// D-52: a modifier whose result exceeds `--max-elements` passes the mesh through and is a
/// WARNING of every statement that evaluates it, not only of a data read: `frap.scene_summary`
/// and `export_scene.gltf(export_apply=True)` report it too.
#[test]
fn modifier_limit_is_a_warning_of_evaluating_statements() {
    let dir = std::env::temp_dir().join(format!("frap-limits-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session_with(|o| {
        o.cwd = dir.clone();
        o.limits.max_elements = 10_000;
    });
    for src in [
        "bpy.data.objects['Cube'].modifiers.new('S', 'SUBSURF')",
        "bpy.data.objects['Cube'].modifiers['S'].levels = 6",
    ] {
        assert_eq!(one(&mut s, src)["ok"], true, "{src}");
    }
    let expected = "Modifier 'S': frap: limit exceeded (max_elements): the statement would create 98304 face corners (limit 10000)";
    for src in [
        "bpy.ops.frap.scene_summary()",
        "bpy.ops.export_scene.gltf(filepath='limit.glb', export_apply=True)",
    ] {
        let r = one(&mut s, src);
        assert_eq!(r["status"], "FINISHED", "{src}: {r}");
        assert_eq!(
            r["reports"],
            serde_json::json!([{"type": "WARNING", "message": expected}]),
            "{src}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// M2 stabilization round 3: `--max-elements` in Edit Mode. The multiplicative edit operators
/// pre-estimate their output, so `subdivide(number_cuts=100)` twice fails at once with the
/// default flags (it used to grow past 4 GB), and the session can still leave Edit Mode.
#[test]
fn edit_mode_subdivide_is_limited_with_default_flags() {
    let mut s = session();
    for src in [
        "bpy.ops.object.mode_set(mode='EDIT')",
        "bpy.ops.mesh.select_all(action='SELECT')",
        "bpy.ops.mesh.subdivide(number_cuts=100)",
    ] {
        assert_eq!(one(&mut s, src)["status"], "FINISHED", "{src}");
    }
    let before = s.state_dump();
    let t = Instant::now();
    let (exc, code, msg) = err(&mut s, "bpy.ops.mesh.subdivide(number_cuts=100)");
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("RuntimeError", "LIMIT_EXCEEDED")
    );
    // 61206 quads, each a 101 x 101 grid.
    assert_eq!(
        msg,
        "Error: frap: limit exceeded (max_elements): the mesh would need more than 2147483647 elements, more than a Blender mesh can index"
    );
    assert_eq!(s.state_dump(), before);
    let r = one(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(r["status"], "FINISHED", "{r}");
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].polygons")["len"],
        61206
    );
}

/// The statement that grows an edit mesh past `--max-elements` fails (rolled back), not the
/// later `mode_set`: pre-estimates for subdivide / loopcut / spin / screw / bevel.
#[test]
fn edit_mode_growth_fails_the_growing_statement() {
    let mut s = session_with(|o| o.limits.max_elements = 1000);
    for src in [
        "bpy.ops.object.mode_set(mode='EDIT')",
        "bpy.ops.mesh.select_all(action='SELECT')",
    ] {
        assert_eq!(one(&mut s, src)["status"], "FINISHED", "{src}");
    }
    let before = s.state_dump();
    for (src, corners) in [
        ("bpy.ops.mesh.subdivide(number_cuts=100)", 244824u64),
        ("bpy.ops.mesh.subdivide_edgering(number_cuts=100)", 0),
        (
            "bpy.ops.mesh.loopcut(number_cuts=1000, object_index=0, edge_index=0)",
            0,
        ),
        ("bpy.ops.mesh.spin(steps=1000, angle=3, axis=(0, 0, 1))", 0),
        ("bpy.ops.mesh.bevel(offset=0.1, segments=1000)", 0),
    ] {
        let t = Instant::now();
        let r = one(&mut s, src);
        assert!(
            t.elapsed() < Duration::from_secs(2),
            "{src}: {:?}",
            t.elapsed()
        );
        assert_eq!(r["status"], "ERROR", "{src}: {r}");
        assert_eq!(r["state"], "unchanged", "{src}: {r}");
        assert_eq!(r["error"]["code"], "LIMIT_EXCEEDED", "{src}: {r}");
        assert_eq!(r["error"]["detail"]["max_elements"], 1000, "{src}: {r}");
        assert_eq!(
            r["error"]["hint"], "raise --max-elements, or build less geometry per statement",
            "{src}: {r}"
        );
        if corners > 0 {
            assert_eq!(
                r["error"]["message"],
                format!(
                    "Error: frap: limit exceeded (max_elements): the statement would create {corners} face corners (limit 1000)"
                ),
                "{src}"
            );
        }
        assert_eq!(s.state_dump(), before, "{src}");
    }
    // Screw needs an open string of vertices: one edge.
    for src in [
        "bpy.ops.mesh.select_all(action='DESELECT')",
        "bpy.ops.frap.select_elements(type='EDGE', indices=[0])",
    ] {
        assert_eq!(one(&mut s, src)["status"], "FINISHED", "{src}");
    }
    let r = one(&mut s, "bpy.ops.mesh.screw(steps=1000, turns=10)");
    assert_eq!(r["error"]["code"], "LIMIT_EXCEEDED", "{r}");
    // Growth within the limit still works, and Edit Mode can be left.
    let r = one(&mut s, "bpy.ops.mesh.subdivide(number_cuts=2)");
    assert_eq!(r["status"], "FINISHED", "{r}");
    let r = one(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(r["status"], "FINISHED", "{r}");
}

/// Growth no operator estimates is stopped by the BMesh allocation backstop (one mesh past the
/// limit) or by the post-statement check, which counts every edit BMesh the statement touched
/// (two meshes, each under the limit, together over it). Either way the growing statement
/// fails and rolls back, and Edit Mode can still be left.
#[test]
fn edit_mode_backstop_and_post_check_count_edit_bmeshes() {
    // One cube (24 corners): individual inset makes 30 quads (120 corners) > 100.
    let mut s = session_with(|o| o.limits.max_elements = 100);
    for src in [
        "bpy.ops.object.mode_set(mode='EDIT')",
        "bpy.ops.mesh.select_all(action='SELECT')",
    ] {
        assert_eq!(one(&mut s, src)["status"], "FINISHED", "{src}");
    }
    let before = s.state_dump();
    let (_, code, msg) = err(
        &mut s,
        "bpy.ops.mesh.inset(thickness=0.1, use_individual=True)",
    );
    assert_eq!(code, "LIMIT_EXCEEDED");
    assert_eq!(
        msg,
        "Error: frap: limit exceeded (max_elements): the statement would create 101 face corners (limit 100)"
    );
    assert_eq!(s.state_dump(), before);
    let r = one(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(r["status"], "FINISHED", "{r}");

    // Two cubes in Edit Mode: poke makes 72 corners each, 144 together > 100.
    let mut s = session_with(|o| o.limits.max_elements = 100);
    for src in [
        "bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))",
        "bpy.ops.object.select_all(action='SELECT')",
        "bpy.ops.object.mode_set(mode='EDIT')",
        "bpy.ops.mesh.select_all(action='SELECT')",
    ] {
        assert_eq!(one(&mut s, src)["status"], "FINISHED", "{src}");
    }
    let before = s.state_dump();
    let r = one(&mut s, "bpy.ops.mesh.poke()");
    assert_eq!(r["status"], "ERROR", "{r}");
    assert_eq!(r["state"], "unchanged", "{r}");
    assert_eq!(r["error"]["code"], "LIMIT_EXCEEDED", "{r}");
    assert_eq!(
        r["error"]["message"],
        "Error: frap: limit exceeded (max_elements)"
    );
    assert_eq!(r["error"]["detail"]["corners"], 144, "{r}");
    assert_eq!(s.state_dump(), before);
    let r = one(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    assert_eq!(r["status"], "FINISHED", "{r}");
}

/// One policy for every modifier evaluator (M3+M4 stabilization round 2): a modifier whose
/// result would exceed `--max-elements` is that modifier's error (WARNING, `modifier_status`
/// `ERROR`) and passes the mesh through, whether the evaluator refuses before building
/// (SUBSURF, ARRAY, BEVEL, SCREW) or the BMesh backstop stops it; the read succeeds.
/// `object.modifier_apply` of it answers `LIMIT_EXCEEDED`, with rollback.
#[test]
fn modifiers_over_max_elements_are_modifier_errors() {
    let cases = [
        ("SUBSURF", "modifiers['M'].levels = 11"),
        ("ARRAY", "modifiers['M'].count = 1000"),
        ("BEVEL", "modifiers['M'].segments = 1000"),
        ("SCREW", "modifiers['M'].steps = 10000"),
    ];
    for (ty, set) in cases {
        let mut s = session_with(|o| o.limits.max_elements = 20_000);
        for src in [
            format!("bpy.data.objects['Cube'].modifiers.new(name='M', type='{ty}')"),
            format!("bpy.data.objects['Cube'].{set}"),
        ] {
            let r = one(&mut s, &src);
            assert_eq!(r["ok"], true, "{src}: {r}");
        }
        if ty == "BEVEL" {
            let r = one(
                &mut s,
                "bpy.data.objects['Cube'].modifiers['M'].affect = 'VERTICES'",
            );
            assert_eq!(r["ok"], true, "{r}");
        }
        let r = one(
            &mut s,
            "bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[0].co",
        );
        assert_eq!(r["ok"], true, "{ty}: {r}");
        let warned = r["reports"].as_array().unwrap().iter().any(|w| {
            w["type"] == "WARNING"
                && w["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("Modifier 'M': frap: limit exceeded (max_elements)")
        });
        assert!(warned, "{ty}: {r}");
        let (exc, code, msg) = err(&mut s, "bpy.ops.object.modifier_apply(modifier='M')");
        assert_eq!(
            (exc.as_str(), code.as_str()),
            ("RuntimeError", "LIMIT_EXCEEDED"),
            "{ty}: {msg}"
        );
        assert_eq!(
            val(&mut s, "bpy.data.objects['Cube'].modifiers.keys()"),
            serde_json::json!(["M"]),
            "{ty}: rolled back"
        );
    }
}
