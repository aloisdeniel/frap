// SPDX-License-Identifier: GPL-3.0-or-later
//! §21.7: "a property read in Edit Mode never triggers modifier re-evaluation (§6.3)", counted
//! with `frap_eval::EvalCache::stack_evaluations` (modifier-stack evaluations on this thread).
//!
//! - Reads of mesh element data in Edit Mode go through the cached `to_mesh` view of the edit
//!   BMesh: they evaluate no modifier stack, whatever they read and however often, and they
//!   leave the object's evaluation fingerprint alone.
//! - Derived reads (`dimensions`, `bound_box`, `matrix_world`, the evaluated mesh) evaluate
//!   the stack at most once per edit: the first read after an edit misses the generation
//!   cache, every later one hits it.
//! - With `--verbosity full` (evaluated counts in every response's `context`) the same holds:
//!   one evaluation after an edit, none for the reads after it.
//! - The same holds over many objects: derived reads cycling over 100 modifier objects in
//!   Edit Mode evaluate each stack once, then never again until an edit.

mod support;

use std::time::Instant;

use frap_eval::EvalCache;
use frap_rna::{IdRef, ObjectId};
use frap_scene::EvalMode;
use frap_session::{Session, SessionOptions, Verbosity};
use serde_json::json;
use support::*;

/// Element reads of the active mesh (in Edit Mode: the edit BMesh's view).
const ELEMENT_READS: &[&str] = &[
    "bpy.context.object.data.vertices[0].co",
    "bpy.context.object.data.vertices[7].normal",
    "bpy.context.object.data.vertices[3].select",
    "bpy.context.object.data.vertices",
    "bpy.context.object.data.edges",
    "bpy.context.object.data.polygons",
    "bpy.context.object.data.loops",
    "bpy.context.object.data.edges[4].vertices[1]",
    "bpy.context.object.data.polygons[2].normal",
    "bpy.context.object.data.polygons[2].center",
    "bpy.context.object.data.polygons[2].area",
    "bpy.context.object.data.loops[5].vertex_index",
    "bpy.context.object.data.uv_layers.active.data[3].uv",
    "bpy.context.object.data.attributes['position'].data[1].vector",
    "bpy.context.object.data.total_vert_sel",
    "bpy.context.object.data.name",
    "bpy.context.object.location",
    "bpy.context.object.mode",
    "bpy.context.object.modifiers['Subdivision'].levels",
    "bpy.context.object.modifiers['Bevel'].width",
    "bpy.context.mode",
];

/// Derived reads: they need the evaluated object.
const DERIVED_READS: &[&str] = &[
    "bpy.context.object.dimensions",
    "bpy.context.object.bound_box[6][0]",
    "bpy.context.object.matrix_world[0][0]",
];

fn active(s: &Session) -> IdRef<ObjectId> {
    let main = s.main();
    main.objects
        .iter()
        .map(|(r, _)| r)
        .find(|&r| main.objects.name_of(r) == Some("Cube"))
        .expect("the default cube")
}

fn fingerprint(s: &Session) -> u64 {
    EvalCache::fingerprint(s.host(), active(s), EvalMode::Viewport).expect("cube exists")
}

/// The default cube with a SUBSURF (levels 2) and a BEVEL modifier, in Edit Mode.
fn scene(verbosity: Verbosity) -> Session {
    let mut o = SessionOptions::default();
    o.cwd = std::env::temp_dir();
    o.verbosity = verbosity;
    let mut s = Session::new(o).expect("session");
    ok(&mut s, "bpy.ops.object.modifier_add(type='SUBSURF')");
    ok(
        &mut s,
        "bpy.context.object.modifiers['Subdivision'].levels = 2",
    );
    ok(&mut s, "bpy.ops.object.modifier_add(type='BEVEL')");
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    s
}

/// Runs `reads` `rounds` times; the stack evaluations they caused.
fn evaluations(s: &mut Session, reads: &[&str], rounds: usize) -> u64 {
    let before = EvalCache::stack_evaluations();
    for _ in 0..rounds {
        for r in reads {
            ok(s, r);
        }
    }
    EvalCache::stack_evaluations() - before
}

fn check(verbosity: Verbosity, name: &str) {
    let _g = serial();
    let mut s = scene(verbosity);
    // The counter counts: an edit followed by a derived read evaluates the stack.
    ok(&mut s, "bpy.ops.mesh.subdivide()");
    let fp = fingerprint(&s);
    let t = Instant::now();
    let element = evaluations(&mut s, ELEMENT_READS, 5);
    let reads_ms = ms(t.elapsed());
    assert_eq!(
        element, 0,
        "{name}: element reads in Edit Mode evaluated the stack"
    );
    assert_eq!(fingerprint(&s), fp, "{name}: reads changed the fingerprint");

    let first = evaluations(&mut s, &DERIVED_READS[..1], 1);
    let again = evaluations(&mut s, DERIVED_READS, 10);
    let element_after = evaluations(&mut s, ELEMENT_READS, 2);
    match verbosity {
        // The edit's own response (`full`: evaluated counts) already evaluated.
        Verbosity::Full => assert!(first <= 1, "{name}: first derived read: {first}"),
        _ => assert_eq!(first, 1, "{name}: first derived read after an edit"),
    }
    assert_eq!(again, 0, "{name}: repeated derived reads re-evaluated");
    assert_eq!(element_after, 0, "{name}: element reads re-evaluated");

    // A new edit invalidates once more; a modifier property write too.
    ok(&mut s, "bpy.ops.transform.translate(value=(0.1, 0, 0))");
    let after_edit = evaluations(&mut s, DERIVED_READS, 3);
    assert!(after_edit <= 1, "{name}: after an edit: {after_edit}");
    ok(
        &mut s,
        "bpy.context.object.modifiers['Subdivision'].levels = 1",
    );
    let after_write = evaluations(&mut s, DERIVED_READS, 3);
    assert!(
        after_write <= 1,
        "{name}: after a modifier write: {after_write}"
    );
    let total = evaluations(&mut s, ELEMENT_READS, 1);
    assert_eq!(total, 0, "{name}: element reads after a modifier write");
    report(
        name,
        reads_ms,
        None,
        json!({
            "element_reads": ELEMENT_READS.len() * 5,
            "evaluations_element_reads": element,
            "evaluations_repeated_derived_reads": again,
        }),
    );
}

#[test]
fn editmode_reads_never_reevaluate_modifiers() {
    check(Verbosity::Normal, "editmode_reads.evaluations");
}

#[test]
fn editmode_reads_never_reevaluate_modifiers_verbosity_full() {
    check(Verbosity::Full, "editmode_reads.evaluations_full");
}

#[test]
fn the_counter_counts_stack_evaluations() {
    let _g = serial();
    let mut s = scene(Verbosity::Normal);
    ok(&mut s, "bpy.ops.object.mode_set(mode='OBJECT')");
    // Object Mode reads of the evaluated mesh: one evaluation per change, none on a hit.
    assert_eq!(evaluations(&mut s, DERIVED_READS, 4), 1);
    ok(&mut s, "bpy.context.object.location.x = 2.0");
    assert_eq!(evaluations(&mut s, DERIVED_READS, 4), 1);
    // No modifiers, no stack: nothing to count.
    ok(
        &mut s,
        "bpy.ops.object.modifier_remove(modifier='Subdivision')",
    );
    ok(&mut s, "bpy.ops.object.modifier_remove(modifier='Bevel')");
    ok(&mut s, "bpy.context.object.location.x = 3.0");
    assert_eq!(evaluations(&mut s, DERIVED_READS, 4), 0);
}

/// Derived reads that cycle over more objects than the read cache's prune threshold (64):
/// every object keeps its cached result (one entry per object and mode, replaced when its
/// fingerprint changes), so the second pass evaluates nothing. Before the WP 8.8 fix round the
/// per-thread cache was cleared whole past 64 entries, and such a pass re-evaluated the stack
/// on almost every read.
#[test]
fn derived_reads_over_many_modifier_objects_evaluate_once() {
    const N: usize = 100;
    let _g = serial();
    let mut s = factory();
    ok(&mut s, "bpy.ops.wm.read_factory_settings(use_empty=True)");
    for i in 0..N {
        ok(
            &mut s,
            &format!(
                "bpy.ops.mesh.primitive_cube_add(size=1.0, location=({}, 0, 0))",
                3 * i
            ),
        );
        ok(&mut s, "bpy.ops.object.modifier_add(type='SUBSURF')");
    }
    // The last cube is active; Edit Mode on it.
    ok(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    let names: Vec<String> = (0..N)
        .map(|i| {
            if i == 0 {
                "Cube".to_owned()
            } else {
                format!("Cube.{i:03}")
            }
        })
        .collect();
    let reads: Vec<String> = names
        .iter()
        .map(|n| format!("bpy.data.objects['{n}'].dimensions"))
        .collect();
    let reads: Vec<&str> = reads.iter().map(String::as_str).collect();
    let first = evaluations(&mut s, &reads, 1);
    assert!(
        first <= N as u64,
        "first pass: {first} evaluations for {N} objects"
    );
    let t = Instant::now();
    let second = evaluations(&mut s, &reads, 3);
    let reads_ms = ms(t.elapsed());
    assert_eq!(
        second, 0,
        "derived reads over {N} modifier objects in Edit Mode re-evaluated"
    );
    // An edit of the edited object invalidates that object alone.
    ok(&mut s, "bpy.ops.transform.translate(value=(0.1, 0, 0))");
    let after_edit = evaluations(&mut s, &reads, 2);
    assert!(after_edit <= 1, "after an edit: {after_edit}");
    report(
        "editmode_reads.many_objects",
        reads_ms,
        None,
        json!({
            "objects": N,
            "reads": 3 * N,
            "evaluations_first_pass": first,
            "evaluations_repeated_reads": second,
        }),
    );
}
