// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 6.5: an armature that is a child of the mesh it deforms. An OBJECT parent is no cycle
//! (the armature needs the mesh's transform, not its geometry; Blender 5.2.4 evaluates it, the
//! `armature/deform/dependency_cycle` golden pins the result); a VERTEX parent is a real cycle,
//! which must be broken with a WARNING instead of recursing until the stack overflows. The same
//! holds for a SIMPLE_DEFORM origin that is a child of the deformed mesh.

mod support;

use serde_json::json;
use support::*;

const EV: &str = "bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get())";

fn rig_parented_to_mesh() -> frap_session::Session {
    let mut s = session();
    for line in [
        "bpy.ops.object.armature_add(location=(0, 0, -1))",
        "bpy.data.objects['Cube'].vertex_groups.new(name='Bone')",
        "bpy.data.objects['Cube'].vertex_groups['Bone'].add([0, 1, 2, 3], 1.0, 'REPLACE')",
        "bpy.data.objects['Cube'].modifiers.new('Rig', 'ARMATURE')",
        "bpy.data.objects['Cube'].modifiers['Rig'].object = bpy.data.objects['Armature']",
        "bpy.data.objects['Armature'].parent = bpy.data.objects['Cube']",
        "bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)",
    ] {
        val(&mut s, line);
    }
    s
}

fn close(a: &serde_json::Value, b: [f64; 3]) -> bool {
    a.as_array().is_some_and(|v| {
        v.len() == 3
            && v.iter()
                .zip(b)
                .all(|(x, y)| (x.as_f64().unwrap_or(f64::NAN) - y).abs() < 1e-4)
    })
}

#[test]
fn armature_child_of_its_mesh_deforms() {
    let mut s = rig_parented_to_mesh();
    // Blender 5.2.4: (0.4894, 0.5319, 1.3404).
    let co = val(&mut s, &format!("{EV}.data.vertices[0].co"));
    assert!(close(&co, [0.489_361_6, 0.531_914_8, 1.340_425_5]), "{co}");
    // Reading the armature afterwards hits the same result (no stale cycle state).
    let co2 = val(&mut s, &format!("{EV}.data.vertices[0].co"));
    assert_eq!(co, co2);
}

#[test]
fn vertex_parent_cycle_is_broken() {
    let mut s = rig_parented_to_mesh();
    val(
        &mut s,
        "bpy.data.objects['Armature'].parent_type = 'VERTEX'",
    );
    let co = val(&mut s, &format!("{EV}.data.vertices[0].co"));
    assert!(co.as_array().is_some_and(|v| v.len() == 3), "{co}");
    // The session survives and keeps answering.
    assert_eq!(
        val(&mut s, "bpy.data.objects['Armature'].parent_type"),
        json!("VERTEX")
    );
    let arm = val(
        &mut s,
        "bpy.data.objects['Armature'].evaluated_get(bpy.context.evaluated_depsgraph_get()).matrix_world.translation",
    );
    assert!(arm.as_array().is_some_and(|v| v.len() == 3), "{arm}");
}

#[test]
fn simple_deform_origin_child_of_its_mesh() {
    let mut s = session();
    for line in [
        "bpy.data.objects.new('E', None)",
        "bpy.context.scene.collection.objects.link(bpy.data.objects['E'])",
        "bpy.data.objects['Cube'].modifiers.new('SD', 'SIMPLE_DEFORM')",
        "bpy.data.objects['Cube'].modifiers['SD'].origin = bpy.data.objects['E']",
        "bpy.data.objects['E'].parent = bpy.data.objects['Cube']",
    ] {
        val(&mut s, line);
    }
    // Blender 5.2.4 (E at the origin, TWIST 45°).
    let co = val(&mut s, &format!("{EV}.data.vertices[0].co"));
    assert!(close(&co, [1.0, 0.541_196_05, 1.306_562_9]), "{co}");
    val(&mut s, "bpy.data.objects['E'].parent_type = 'VERTEX'");
    let co = val(&mut s, &format!("{EV}.data.vertices[0].co"));
    assert!(co.as_array().is_some_and(|v| v.len() == 3), "{co}");
}
