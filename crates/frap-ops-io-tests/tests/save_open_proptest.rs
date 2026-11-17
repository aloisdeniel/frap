// SPDX-License-Identifier: GPL-3.0-or-later
//! Proptest (WP 1.23 acceptance, §15): random sequences of M1 operations and RNA writes →
//! `wm.save_as_mainfile` → `wm.open_mainfile` give back the identical state (orphans excepted,
//! which a save drops), and saving the reopened state gives the same bytes.

mod support;

use proptest::prelude::*;
use support::*;

const PRIMITIVES: [&str; 10] = [
    "plane",
    "cube",
    "circle",
    "uv_sphere",
    "ico_sphere",
    "cylinder",
    "cone",
    "torus",
    "grid",
    "monkey",
];
const OBJECTS: [&str; 12] = [
    "Cube", "Camera", "Light", "Plane", "Cone", "Cylinder", "Sphere", "Torus", "Suzanne", "Grid",
    "Cube.001", "Empty",
];
const MATERIALS: [&str; 3] = ["Material", "Red", "Steel"];

fn object() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&OBJECTS[..])
}
fn material() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&MATERIALS[..])
}
fn coord() -> impl Strategy<Value = f32> {
    (-40i32..40).prop_map(|i| i as f32 * 0.25)
}

fn statement() -> impl Strategy<Value = String> {
    prop_oneof![
        (prop::sample::select(&PRIMITIVES[..]), coord(), coord())
            .prop_map(|(p, x, y)| format!("bpy.ops.mesh.primitive_{p}_add(location=({x}, {y}, 0))")),
        Just("bpy.ops.object.select_all(action='SELECT')".to_owned()),
        Just("bpy.ops.object.select_all(action='DESELECT')".to_owned()),
        Just("bpy.ops.object.select_all(action='INVERT')".to_owned()),
        Just("bpy.ops.object.delete()".to_owned()),
        (object(), coord(), coord(), coord())
            .prop_map(|(o, a, b, c)| format!("bpy.data.objects['{o}'].location = ({a}, {b}, {c})")),
        (object(), coord(), coord())
            .prop_map(|(o, a, b)| format!("bpy.data.objects['{o}'].rotation_euler = ({a}, {b}, 0.5)")),
        (object(), prop::sample::select(&["XYZ", "ZXY", "QUATERNION", "AXIS_ANGLE"][..]))
            .prop_map(|(o, m)| format!("bpy.data.objects['{o}'].rotation_mode = '{m}'")),
        (object(), coord()).prop_map(|(o, s)| format!("bpy.data.objects['{o}'].scale = (1, {s}, 2)")),
        (object(), object())
            .prop_map(|(a, b)| format!("bpy.data.objects['{a}'].parent = bpy.data.objects['{b}']")),
        object().prop_map(|o| format!("bpy.data.objects['{o}'].select_set(True)")),
        object().prop_map(|o| format!("bpy.data.objects['{o}'].hide_set(True)")),
        object().prop_map(|o| format!("bpy.context.view_layer.objects.active = bpy.data.objects['{o}']")),
        object().prop_map(|o| format!("bpy.data.objects['{o}'].data.shade_smooth()")),
        (object(), object()).prop_map(|(a, b)| format!("bpy.data.objects['{a}'].name = '{b}'")),
        material().prop_map(|m| format!("bpy.data.materials.new('{m}')")),
        (object(), material()).prop_map(|(o, m)| format!(
            "bpy.data.objects['{o}'].data.materials.append(bpy.data.materials['{m}'])"
        )),
        (material(), 0i32..=4).prop_map(|(m, v)| format!(
            "bpy.data.materials['{m}'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = {}",
            v as f32 / 4.0
        )),
        (material(), coord()).prop_map(|(m, v)| format!("bpy.data.materials['{m}'].diffuse_color = (0.5, {}, 0.25, 1)", v.abs() / 10.0)),
        Just("bpy.data.objects.new('Empty', None)".to_owned()),
        Just("bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])".to_owned()),
        Just("bpy.data.collections.new('Group')".to_owned()),
        Just("bpy.context.scene.collection.children.link(bpy.data.collections['Group'])".to_owned()),
        object().prop_map(|o| format!("bpy.data.collections['Group'].objects.link(bpy.data.objects['{o}'])")),
        (0i32..250).prop_map(|f| format!("bpy.context.scene.frame_current = {f}")),
        (coord(), coord()).prop_map(|(a, b)| format!("bpy.context.scene.cursor.location = ({a}, {b}, 1)")),
        Just("bpy.data.images.new('Img', 4, 4)".to_owned()),
        Just("bpy.data.materials['Material'].use_fake_user = True".to_owned()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 24,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn save_open_gives_the_identical_state(stmts in prop::collection::vec(statement(), 1..25)) {
        let d = Scratch::new("prop");
        let mut s = session_in(d.path(), false);
        // Failing statements (missing names, cycles) roll back; they are part of the mix.
        let _ = s.exec_source(&stmts.join("\n"));
        let r = run(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='p.frap')");
        prop_assert!(r.ok(), "{}", r.to_json());
        let expected = saved_state(&s);
        let r = run(&mut s, "bpy.ops.wm.open_mainfile(filepath='p.frap')");
        prop_assert!(r.ok(), "{}", r.to_json());
        assert_same_state(&state(&s), &expected, &stmts.join("\n"));
        let r = run(&mut s, "bpy.ops.wm.save_as_mainfile(filepath='q.frap')");
        prop_assert!(r.ok(), "{}", r.to_json());
        // Users are counted at save time, as in Blender: data used only by a dropped orphan
        // (the material of a deleted object's mesh) is written once and dropped by the next
        // save. Without such second-generation orphans the bytes are identical.
        let dropped = r.result.as_ref().unwrap()["orphans_dropped"].as_i64().unwrap();
        if dropped == 0 {
            let p = std::fs::read(d.path().join("p.frap")).unwrap();
            let q = std::fs::read(d.path().join("q.frap")).unwrap();
            prop_assert!(p == q, "the reopened state saves to other bytes");
        }
    }
}
