// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context.space_data`, `preferences` and `window` through a real session (WP 2.3,
//! ARCHITECTURE §8; OPERATORS.md "Context (preferences, space_data stub)"), and the routing of
//! `View3DShading.studio_light` over its three DNA names. Blender's line outcomes for the same
//! statements are pinned by `tests/golden/cases/render/m2/{space_data_shading,
//! studio_light_routing}.py`; this file checks the values, which the golden runner does not
//! record. No GPU needed.

mod common;

use common::Env;
use serde_json::Value as J;

fn value(e: &mut Env, src: &str) -> J {
    let j = e.run(src);
    assert_eq!(j["ok"], true, "{src}: {j}");
    j["value"].clone()
}

fn error(e: &mut Env, src: &str) -> (String, String, String) {
    let j = e.run(src);
    assert_eq!(j["ok"], false, "{src}: {j}");
    let s = |k: &str| j["error"][k].as_str().unwrap_or_default().to_owned();
    (s("exception"), s("code"), s("message"))
}

#[test]
fn space_data_is_the_virtual_viewport() {
    let mut e = Env::new("ctx_space_data");
    let j = e.run("bpy.context.space_data");
    assert_eq!(j["value"]["type"], "SpaceView3D", "{j}");
    assert_eq!(value(&mut e, "bpy.context.space_data.type"), "VIEW_3D");
    // The factory Layout viewport (`startup.json`, dumped from 5.2.4).
    let startup: J = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../spec/blender-5.2/startup.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let want = &startup["view3d"]["region_3d"]["view_matrix"];
    let got = value(&mut e, "bpy.context.space_data.region_3d.view_matrix");
    for r in 0..4 {
        for c in 0..4 {
            let (a, b) = (got[r][c].as_f64().unwrap(), want[r][c].as_f64().unwrap());
            assert!((a - b).abs() < 1e-6, "view_matrix[{r}][{c}]: {a} vs {b}");
        }
    }
    assert_eq!(
        value(&mut e, "bpy.context.space_data.region_3d.view_perspective"),
        "PERSP"
    );
    assert_eq!(
        value(&mut e, "bpy.context.space_data.region_3d.is_perspective"),
        true
    );
    // The factory viewport shades with an outline; the scene's render shading does not.
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.show_object_outline"),
        true
    );
    assert_eq!(
        value(
            &mut e,
            "bpy.context.scene.display.shading.show_object_outline"
        ),
        false
    );
    // Unbound RegionView3D members and the fixed viewport.
    let (exc, code, _) = error(&mut e, "bpy.context.space_data.region_3d.view_distance");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "RNA_NOT_IMPLEMENTED")
    );
    let (exc, code, _) = error(&mut e, "bpy.context.space_data.type = 'VIEW_3D'");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("AttributeError", "RNA_READONLY")
    );
    // area / region / screen: frap has none (§8).
    for m in ["area", "region", "region_data", "screen"] {
        assert_eq!(value(&mut e, &format!("bpy.context.{m}")), J::Null, "{m}");
    }
    // window: the Window stub, its scene the context scene.
    let j = e.run("bpy.context.window.scene");
    assert_eq!(j["value"]["$ref"], "bpy.data.scenes['Scene']", "{j}");
}

#[test]
fn viewport_shading_is_writable_routed_and_saved() {
    let mut e = Env::new("ctx_shading");
    e.ok("bpy.context.space_data.shading.light = 'MATCAP'
bpy.context.space_data.shading.studio_light = 'clay_brown.exr'
bpy.context.space_data.shading.show_xray = True");
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.studio_light"),
        "clay_brown.exr"
    );
    // The scene's shading is a separate struct.
    assert_eq!(
        value(&mut e, "bpy.context.scene.display.shading.light"),
        "STUDIO"
    );
    assert_eq!(
        value(&mut e, "bpy.context.scene.display.shading.show_xray"),
        false
    );
    let (exc, code, msg) = error(
        &mut e,
        "bpy.context.space_data.shading.studio_light = 'rim.sl'",
    );
    assert_eq!((exc.as_str(), code.as_str()), ("TypeError", "RNA_ENUM"));
    assert!(
        msg.starts_with(
            "bpy_struct: item.attr = val: enum \"rim.sl\" not found in ('basic_bright.exr'"
        ),
        "{msg}"
    );
    e.ok("bpy.context.space_data.shading.light = 'STUDIO'");
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.studio_light"),
        "Default"
    );
    e.ok("bpy.context.space_data.shading.studio_light = 'rim.sl'");
    // Saved in `.frap` (window state, §15) and restored by open.
    e.ok("bpy.ops.wm.save_mainfile(filepath='ui.frap')
bpy.ops.wm.read_factory_settings()");
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.show_xray"),
        false
    );
    e.ok("bpy.ops.wm.open_mainfile(filepath='ui.frap')");
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.show_xray"),
        true
    );
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.studio_light"),
        "rim.sl"
    );
    e.ok("bpy.context.space_data.shading.light = 'MATCAP'");
    assert_eq!(
        value(&mut e, "bpy.context.space_data.shading.studio_light"),
        "clay_brown.exr"
    );
}

#[test]
fn scene_shading_routes_studio_light_by_light() {
    // The WP 2.3 verifier's sequence (5.2.4): MATCAP + clay_brown, STUDIO + rim.sl, back to
    // MATCAP reads (and renders) clay_brown; each mode reads its own default first.
    let mut e = Env::new("ctx_scene_shading");
    let sl = "bpy.context.scene.display.shading.studio_light";
    e.ok("bpy.context.scene.display.shading.light = 'MATCAP'");
    assert_eq!(value(&mut e, sl), "basic_bright.exr");
    e.ok(&format!("{sl} = 'clay_brown.exr'"));
    e.ok("bpy.context.scene.display.shading.light = 'STUDIO'");
    assert_eq!(value(&mut e, sl), "Default");
    let (exc, _, msg) = error(&mut e, &format!("{sl} = 'clay_brown.exr'"));
    assert_eq!(exc, "TypeError");
    assert!(
        msg.ends_with(
            "not found in ('Default', 'basic.sl', 'outdoor.sl', 'paint.sl', 'rim.sl', 'studio.sl')"
        ),
        "{msg}"
    );
    e.ok(&format!("{sl} = 'rim.sl'"));
    e.ok("bpy.context.scene.display.shading.light = 'MATCAP'");
    assert_eq!(value(&mut e, sl), "clay_brown.exr");
    e.ok("bpy.context.scene.display.shading.type = 'MATERIAL'");
    assert_eq!(value(&mut e, sl), "forest.exr");
    e.ok("bpy.context.scene.display.shading.type = 'WIREFRAME'");
    assert_eq!(value(&mut e, sl), "");
    // A refused assignment changes nothing.
    let j = e.run(&format!("{sl} = 'forest.exr'"));
    assert_eq!(j["state"], "unchanged", "{j}");
    // Saved and restored, all three names.
    e.ok("bpy.context.scene.display.shading.type = 'SOLID'
bpy.ops.wm.save_mainfile(filepath='s.frap')
bpy.ops.wm.open_mainfile(filepath='s.frap')");
    assert_eq!(value(&mut e, sl), "clay_brown.exr");
    e.ok("bpy.context.scene.display.shading.light = 'STUDIO'");
    assert_eq!(value(&mut e, sl), "rim.sl");
}

#[test]
fn preferences_are_factory_and_read_only() {
    let mut e = Env::new("ctx_prefs");
    let j = e.run("bpy.context.preferences");
    assert_eq!(j["value"]["type"], "Preferences", "{j}");
    assert_eq!(
        value(&mut e, "bpy.context.preferences.edit.use_duplicate_mesh"),
        true
    );
    assert_eq!(
        value(
            &mut e,
            "bpy.context.preferences.edit.use_duplicate_material"
        ),
        false
    );
    assert_eq!(
        value(
            &mut e,
            "bpy.context.preferences.edit.keyframe_new_handle_type"
        ),
        "AUTO_CLAMPED"
    );
    assert_eq!(
        value(
            &mut e,
            "bpy.context.preferences.edit.keyframe_new_interpolation_type"
        ),
        "BEZIER"
    );
    let (exc, code, msg) = error(
        &mut e,
        "bpy.context.preferences.edit.use_duplicate_mesh = False",
    );
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "NOT_IMPLEMENTED")
    );
    assert!(msg.contains("read-only"), "{msg}");
    assert_eq!(
        value(&mut e, "bpy.context.preferences.edit.use_duplicate_mesh"),
        true
    );
    // The factory add-on modules, in Blender's order.
    assert_eq!(
        value(&mut e, "bpy.context.preferences.addons[0].module"),
        "io_anim_bvh"
    );
    assert_eq!(
        value(&mut e, "bpy.context.preferences.addons[4].module"),
        "io_scene_gltf2"
    );
    let (exc, _, _) = error(&mut e, "bpy.context.preferences.addons['nope']");
    assert_eq!(exc, "KeyError");
    // glTF add-on preferences: readable, writable (coerced), saved in `.frap` (§15).
    let gltf = "bpy.context.preferences.addons['io_scene_gltf2'].preferences";
    assert_eq!(
        value(&mut e, &format!("{gltf}.allow_embedded_format")),
        false
    );
    e.ok(&format!("{gltf}.allow_embedded_format = True"));
    assert_eq!(
        value(&mut e, &format!("{gltf}.allow_embedded_format")),
        true
    );
    let (exc, _, _) = error(&mut e, &format!("{gltf}.allow_embedded_format = 'x'"));
    assert_eq!(exc, "TypeError");
    e.ok("bpy.ops.wm.save_mainfile(filepath='p.frap')
bpy.ops.wm.read_factory_settings()");
    assert_eq!(
        value(&mut e, &format!("{gltf}.allow_embedded_format")),
        false
    );
    e.ok("bpy.ops.wm.open_mainfile(filepath='p.frap')");
    assert_eq!(
        value(&mut e, &format!("{gltf}.allow_embedded_format")),
        true
    );
}
