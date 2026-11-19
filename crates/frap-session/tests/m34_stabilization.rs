// SPDX-License-Identifier: GPL-3.0-or-later
//! M3+M4 stabilization round 1: the agent-facing protocol details the exit verifier found
//! missing (decision log M34S-7): `did_you_mean` on enum typos, the evaluated-mesh repr,
//! the `Depsgraph` not-bound message and the `FUNC_NOT_CALLED` hint.

mod support;

use serde_json::json;
use support::*;

const EV: &str = "bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get())";

#[test]
fn enum_typos_suggest_the_item() {
    let mut s = session();
    let r = one(&mut s, "bpy.ops.object.modifier_add(type='SUBSURFACE')");
    assert_eq!(r["error"]["code"], "OP_ARG_ENUM", "{r}");
    assert_eq!(
        r["error"]["detail"]["did_you_mean"],
        json!(["SUBSURF"]),
        "{r}"
    );
    run(&mut s, "bpy.ops.object.modifier_add(type='DECIMATE')");
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].modifiers['Decimate'].decimate_type = 'DISOLVE'",
    );
    assert_eq!(r["error"]["code"], "RNA_ENUM", "{r}");
    assert_eq!(
        r["error"]["detail"]["did_you_mean"],
        json!(["DISSOLVE"]),
        "{r}"
    );
    // Lower case: compared upper-cased.
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].modifiers['Decimate'].decimate_type = 'collapse'",
    );
    assert_eq!(
        r["error"]["detail"]["did_you_mean"],
        json!(["COLLAPSE"]),
        "{r}"
    );
    // Enum sets.
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].modifiers['Decimate'].delimit = {'SEEM'}",
    );
    assert_eq!(r["error"]["code"], "RNA_ENUM", "{r}");
    assert_eq!(r["error"]["detail"]["did_you_mean"], json!(["SEAM"]), "{r}");
    // `limit_method='ANGEL'` (BEVEL).
    let r = one(&mut s, "bpy.ops.object.modifier_add(type='BEVEL')\n");
    assert_eq!(r["status"], "FINISHED", "{r}");
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].modifiers['Bevel'].limit_method = 'ANGEL'",
    );
    assert_eq!(
        r["error"]["detail"]["did_you_mean"],
        json!(["ANGLE"]),
        "{r}"
    );
}

#[test]
fn evaluated_mesh_collections_name_the_evaluated_expression() {
    let mut s = session();
    run(
        &mut s,
        "bpy.data.objects['Cube'].modifiers.new(name='S', type='SUBSURF')",
    );
    for src in [
        format!("{EV}.to_mesh().vertices"),
        format!("{EV}.data.vertices"),
    ] {
        let r = val(&mut s, &src);
        let path = format!("{EV}.data.vertices");
        assert_eq!(
            r,
            json!({"$coll": path, "item_type": "MeshVertex", "len": 26}),
            "{src}"
        );
        // The path reads the same evaluated data.
        assert_eq!(val(&mut s, &path)["len"], 26);
    }
    let r = one(&mut s, &format!("{EV}.data"));
    assert_eq!(r["path"], format!("{EV}.data"));
    assert_eq!(r["value"]["$ptr"], format!("{EV}.data"));
    // The original mesh keeps its own repr.
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].data.vertices")["$coll"],
        "bpy.data.meshes['Cube'].vertices"
    );
}

#[test]
fn depsgraph_members_name_themselves_when_not_bound() {
    let mut s = session();
    let (exc, code, msg) = err(&mut s, "bpy.context.evaluated_depsgraph_get().objects");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "RNA_NOT_IMPLEMENTED")
    );
    assert_eq!(
        msg,
        "frap: Depsgraph.objects exists in Blender 5.2 but is not bound by frap yet"
    );
}

#[test]
fn a_bare_function_reference_gets_a_hint() {
    let mut s = session();
    run(&mut s, "bpy.data.images.new('X', 4, 4)");
    for src in ["bpy.data.images['X'].save", "data.images['X'].save"] {
        let r = one(&mut s, src);
        assert_eq!(r["status"], "FINISHED", "{r}");
        assert_eq!(r["value"], json!({"$func": "bpy.data.images['X'].save"}));
        assert_eq!(r["hints"][0]["code"], "FUNC_NOT_CALLED", "{r}");
        assert_eq!(
            r["hints"][0]["message"],
            "bpy.data.images['X'].save is a function and was not called; add parentheses to \
             call it: bpy.data.images['X'].save()"
        );
    }
    // A property read has no hint.
    assert_eq!(one(&mut s, "bpy.data.images['X'].size")["hints"], json!([]));
}

/// `object.modifier_add`, `modifiers.new`, `object.subdivision_set` and
/// `object.shade_auto_smooth` replaced the object with a modified copy that still carried its
/// old generation, so an evaluated read cached before them was served again (stale).
#[test]
fn evaluated_reads_see_modifiers_added_after_a_read() {
    let mut s = session();
    let n = |s: &mut frap_session::Session| val(s, &format!("{EV}.data.vertices"))["len"].clone();
    run(&mut s, "bpy.ops.object.modifier_add(type='SUBSURF')");
    assert_eq!(n(&mut s), 26);
    run(&mut s, "bpy.ops.object.modifier_add(type='SUBSURF')");
    assert_eq!(n(&mut s), 98);
    run(
        &mut s,
        "bpy.data.objects['Cube'].modifiers.new(name='S3', type='SUBSURF')",
    );
    assert_eq!(n(&mut s), 386);
    run(&mut s, "bpy.ops.object.modifier_remove(modifier='S3')");
    assert_eq!(n(&mut s), 98);
    // `subdivision_set` adds a Subdivision modifier when the object has none.
    let mut s = session();
    run(&mut s, "bpy.ops.object.modifier_add(type='TRIANGULATE')");
    assert_eq!(n(&mut s), 8);
    run(&mut s, "bpy.ops.object.subdivision_set(level=1)");
    // The triangulated cube (12 faces) subdivided: 8 + 18 + 12.
    assert_eq!(n(&mut s), 38);
}

/// `--verbosity full` (§13.2): the context block carries the active object's evaluated counts
/// and, with modifiers, `modifier_status`.
#[test]
fn verbosity_full_lists_modifier_status() {
    let mut s = session_with(|o| o.verbosity = frap_session::Verbosity::Full);
    let r = one(&mut s, "bpy.ops.object.modifier_add(type='SUBSURF')");
    assert_eq!(
        r["context"]["evaluated"],
        json!({"vertices": 26, "edges": 48, "faces": 24})
    );
    run(&mut s, "bpy.ops.object.modifier_add(type='BOOLEAN')");
    let r = one(&mut s, "bpy.ops.object.modifier_add(type='SKIN')");
    assert_eq!(
        r["context"]["modifier_status"],
        json!([
            {"name": "Subdivision", "status": "EVALUATED"},
            {"name": "Boolean", "status": "DISABLED", "message": "Boolean: no operand object"},
            {"name": "Skin", "status": "PASSED_THROUGH"},
        ])
    );
    // The default verbosity has neither.
    let mut s = session();
    let r = one(&mut s, "bpy.ops.object.modifier_add(type='SUBSURF')");
    assert!(r["context"].get("modifier_status").is_none(), "{r}");
    assert!(r["context"].get("evaluated").is_none(), "{r}");
}
