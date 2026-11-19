// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 4.1 (M4, modifier stack) through the session: what agents see in responses. Evaluated
//! reads (`ob.evaluated_get(dg).data`, `to_mesh()`) give the evaluated mesh; a modifier frap
//! cannot evaluate is never dropped silently (WARNING reports, `OP_NOT_IMPLEMENTED`);
//! `object.modifier_apply` counts real users (the fake user does not make data multi-user).
//! Blender behaviour verified on 5.2.4 (the goldens of `tests/golden/cases/modifiers/stack`
//! pin the same outcomes against Blender).

mod support;

use serde_json::{Value as J, json};
use support::*;

const EV: &str = "bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get())";

/// The WARNING messages of a response.
fn warnings(r: &J) -> Vec<String> {
    r["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["type"] == "WARNING")
        .map(|x| x["message"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn evaluated_data_is_the_evaluated_mesh() {
    let mut s = session();
    val(&mut s, "bpy.ops.object.shade_auto_smooth(angle=0.4)");
    // The original has no `sharp_edge`: the modifier wrote it on the evaluated mesh only.
    let (exc, _, _) = err(
        &mut s,
        "bpy.data.objects['Cube'].data.attributes['sharp_edge']",
    );
    assert_eq!(exc, "KeyError");
    assert_eq!(
        val(
            &mut s,
            &format!("{EV}.data.attributes['sharp_edge'].data[0].value")
        ),
        json!(true)
    );
    assert_eq!(val(&mut s, &format!("{EV}.data.edges"))["len"], json!(12));
    assert_eq!(val(&mut s, &format!("{EV}.data.name")), json!("Cube"));
    assert_eq!(
        val(&mut s, &format!("{EV}.data.is_evaluated")),
        json!(false)
    );
    assert_eq!(
        val(&mut s, &format!("{EV}.data.original.is_evaluated")),
        json!(false)
    );
    // Blender's evaluated mesh is its own original (`ev.data.original == ev.data`).
    assert_eq!(
        val(
            &mut s,
            &format!("{EV}.data.original.attributes['sharp_edge'].data[0].value")
        ),
        json!(true)
    );
    // Writes through the evaluated mesh stay refused (D-41).
    let (exc, code, _) = err(&mut s, &format!("{EV}.data.vertices[0].co.x = 42"));
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "NOT_IMPLEMENTED")
    );
    // Always fresh: a modifier change is seen at once (cached by fingerprint).
    val(
        &mut s,
        "bpy.data.objects['Cube'].modifiers['Smooth by Angle'].show_viewport = False",
    );
    let (exc, _, _) = err(&mut s, &format!("{EV}.data.attributes['sharp_edge']"));
    assert_eq!(exc, "KeyError");
}

#[test]
fn modifier_apply_ignores_the_fake_user() {
    let mut s = session();
    val(&mut s, "bpy.data.meshes['Cube'].use_fake_user = True");
    val(&mut s, "bpy.ops.object.shade_auto_smooth(angle=0.4)");
    let r = one(
        &mut s,
        "bpy.ops.object.modifier_apply(modifier='Smooth by Angle', single_user=True)",
    );
    assert_eq!(r["status"], "FINISHED", "{r}");
    // `single_user` did not copy data whose only other user is the fake user.
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].data.name"),
        json!("Cube")
    );
    assert_eq!(val(&mut s, "bpy.data.meshes")["len"], json!(1));
    assert_eq!(
        val(
            &mut s,
            "bpy.data.meshes['Cube'].attributes['sharp_edge'].data[0].value"
        ),
        json!(true)
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].modifiers")["len"],
        json!(0)
    );
    // A real second user is still refused.
    val(&mut s, "bpy.ops.object.shade_auto_smooth(angle=0.4)");
    val(
        &mut s,
        "bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])",
    );
    let (exc, _, msg) = err(
        &mut s,
        "bpy.ops.object.modifier_apply(modifier='Smooth by Angle')",
    );
    assert_eq!(exc, "RuntimeError");
    assert!(msg.contains("multi-user data"), "{msg}");
}

#[test]
fn a_modifier_without_evaluator_warns_and_is_never_dropped() {
    let mut s = session();
    let r = one(&mut s, "bpy.ops.object.modifier_add(type='REMESH')");
    assert_eq!(r["status"], "FINISHED", "{r}");
    let w = warnings(&r);
    assert!(
        w.iter()
            .any(|m| m == "Modifier 'Remesh' (REMESH) is not evaluated by frap; passed through"),
        "{r}"
    );
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].modifiers.new('Ocean', 'OCEAN')",
    );
    assert!(
        warnings(&r)
            .iter()
            .any(|m| m.contains("(OCEAN) is not evaluated by frap")),
        "{r}"
    );
    // Evaluated reads report the stack's warnings.
    let r = one(&mut s, &format!("{EV}.to_mesh().vertices"));
    assert_eq!(r["status"], "FINISHED", "{r}");
    let w = warnings(&r);
    assert!(w.iter().any(|m| m.contains("(REMESH)")), "{r}");
    assert!(w.iter().any(|m| m.contains("(OCEAN)")), "{r}");
    // convert would drop them unapplied: refused, nothing changes.
    let (exc, code, msg) = err(&mut s, "bpy.ops.object.convert(target='MESH')");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "OP_NOT_IMPLEMENTED")
    );
    assert!(msg.contains("'Remesh'"), "{msg}");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].modifiers")["len"],
        json!(2)
    );
    // So does modifier_apply.
    let (exc, code, _) = err(&mut s, "bpy.ops.object.modifier_apply(modifier='Remesh')");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "OP_NOT_IMPLEMENTED")
    );
}

#[test]
fn an_unsupported_node_group_is_a_frap_limitation() {
    let mut s = session();
    // A local copy of the Essentials group (it shadows the linked one by name) is not the group
    // frap evaluates.
    val(&mut s, "bpy.ops.object.shade_auto_smooth()");
    val(&mut s, "bpy.data.node_groups['Smooth by Angle'].copy()");
    val(
        &mut s,
        "bpy.data.objects['Cube'].modifiers.new('GN', 'NODES')",
    );
    val(
        &mut s,
        "bpy.data.objects['Cube'].modifiers['GN'].node_group = bpy.data.node_groups['Smooth by Angle']",
    );
    let r = one(&mut s, &format!("{EV}.name"));
    assert!(
        warnings(&r)
            .iter()
            .any(|m| m.contains("Unsupported node group \"Smooth by Angle\"")),
        "{r}"
    );
    let (exc, code, msg) = err(&mut s, "bpy.ops.object.modifier_apply(modifier='GN')");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "OP_NOT_IMPLEMENTED"),
        "{msg}"
    );
    assert!(msg.contains("Unsupported node group"), "{msg}");
    let (exc, code, _) = err(&mut s, "bpy.ops.object.convert(target='MESH')");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "OP_NOT_IMPLEMENTED")
    );
}
