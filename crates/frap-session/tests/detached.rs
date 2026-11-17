// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.22 contract change (`RnaPtr::detached`): `evaluated_get()` and `to_mesh()` results
//! stand for Blender copies outside `bpy.data`. frap reads the original through them and
//! refuses writes that would change it (deviations.md), while Blender only changes the copy
//! (verified on 5.2.4: `ob.to_mesh().vertices[0].co.x = 42` and
//! `ob.evaluated_get(dg).location.x = 7` leave the original untouched).

mod support;

use serde_json::json;
use support::*;

const EV: &str = "bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get())";

#[test]
fn evaluated_get_reads_like_blender() {
    let mut s = session();
    assert_eq!(val(&mut s, &format!("{EV}.is_evaluated")), json!(true));
    assert_eq!(
        val(&mut s, &format!("{EV}.original.is_evaluated")),
        json!(false)
    );
    assert_eq!(val(&mut s, &format!("{EV}.name")), json!("Cube"));
    // Blender: `ob.evaluated_get(dg).data.is_evaluated` is False (verified on 5.2.4).
    assert_eq!(
        val(&mut s, &format!("{EV}.data.is_evaluated")),
        json!(false)
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].is_evaluated"),
        json!(false)
    );
    // IDs outside the depsgraph come back as the original.
    run(&mut s, "bpy.data.materials.new('Unused')");
    assert_eq!(
        val(
            &mut s,
            "bpy.data.materials['Unused'].evaluated_get(bpy.context.evaluated_depsgraph_get()).is_evaluated"
        ),
        json!(false)
    );
    assert_eq!(
        val(
            &mut s,
            "bpy.data.materials['Material'].evaluated_get(bpy.context.evaluated_depsgraph_get()).is_evaluated"
        ),
        json!(true)
    );
}

#[test]
fn writes_through_evaluated_ids_are_refused() {
    let mut s = session();
    let (exc, code, msg) = err(&mut s, &format!("{EV}.location.x = 7"));
    assert_eq!(exc, "NotImplementedError");
    assert_eq!(code, "NOT_IMPLEMENTED");
    assert!(msg.contains("evaluated"), "{msg}");
    err(&mut s, &format!("{EV}.location = (7, 0, 0)"));
    err(&mut s, &format!("{EV}.data.vertices[0].co.x = 42"));
    err(&mut s, &format!("{EV}.data.materials.clear()"));
    // `.original` leads back to the writable datablock.
    val(&mut s, &format!("{EV}.original.location.x = 7"));
    assert_eq!(val(&mut s, "bpy.data.objects['Cube'].location.x"), json!(7));
}

#[test]
fn to_mesh_is_a_read_only_view() {
    let mut s = session();
    let tm = "bpy.data.objects['Cube'].to_mesh()";
    assert_eq!(val(&mut s, &format!("{tm}.name")), json!("Cube"));
    assert_eq!(val(&mut s, &format!("{tm}.is_evaluated")), json!(false));
    assert_eq!(val(&mut s, &format!("{tm}.vertices[7].co.x")), json!(-1));
    let (exc, _, msg) = err(&mut s, &format!("{tm}.vertices[0].co.x = 42"));
    assert_eq!(exc, "NotImplementedError");
    assert!(msg.contains("bpy.data.meshes['Cube']"), "{msg}");
    err(&mut s, &format!("{tm}.materials.append(None)"));
    err(
        &mut s,
        &format!("{tm}.vertices.foreach_set('co', [0.0] * 24)"),
    );
    // The original stays untouched and writable.
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co.x"),
        json!(1)
    );
    val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co.x = 2");
    // `to_mesh_clear()` never touches the original data.
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].to_mesh_clear()"),
        json!(null)
    );
    val(&mut s, "bpy.data.objects['Camera'].to_mesh_clear()");
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co.x"),
        json!(2)
    );
    let (exc, _, _) = err(&mut s, "bpy.data.meshes[1]");
    assert_eq!(exc, "IndexError");
}

#[test]
fn the_refusal_names_the_original() {
    let mut s = session();
    let (_, _, msg) = err(
        &mut s,
        "bpy.data.objects['Cube'].to_mesh().vertices[0].co.x = 42",
    );
    assert!(
        msg.starts_with("frap: bpy.data.meshes['Cube'].vertices[0] is part of a temporary"),
        "{msg}"
    );
    assert!(
        msg.ends_with("write to bpy.data.meshes['Cube'] instead"),
        "{msg}"
    );
}
