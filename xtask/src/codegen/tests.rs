// SPDX-License-Identifier: GPL-3.0-or-later
//! Codegen tests (`cargo test -p xtask`): generated code is current, typed props structs exist
//! exactly for OPERATORS.md operators with `M` ≤ `spec/current_milestone`, the frap-option
//! audit (§4.2), overlay consistency, and the dump preconditions of §4.1.

use std::collections::BTreeSet;

use super::model::{self, Kind, Spec};
use super::*;

fn spec() -> Spec {
    model::load(&crate::workspace_root()).expect("spec loads")
}

#[test]
fn generated_code_is_up_to_date() {
    let root = crate::workspace_root();
    let files = generate(&root).expect("codegen runs");
    let (changed, stale) = drift(&root, &files);
    assert!(
        changed.is_empty() && stale.is_empty(),
        "run `cargo xtask codegen`: changed {changed:?}, stale {stale:?}"
    );
}

#[test]
fn generation_is_deterministic() {
    let root = crate::workspace_root();
    assert_eq!(generate(&root).unwrap(), generate(&root).unwrap());
}

/// Typed props structs exist exactly for OPERATORS.md operators with M ≤ current milestone; every
/// other operator of the curated namespaces is in the catalog.
#[test]
fn typed_ops_match_operators_md() {
    let s = spec();
    let md = std::fs::read_to_string(s.root.join("docs/OPERATORS.md")).unwrap();
    let listed = model::parse_operators_md(&md).unwrap();
    let want: BTreeSet<&str> = listed
        .iter()
        .filter(|(_, m)| m.is_some_and(|m| m <= s.milestone))
        .map(|(k, _)| k.as_str())
        .collect();
    let typed: BTreeSet<&str> = s
        .ops
        .iter()
        .filter(|o| s.is_typed(o))
        .map(|o| o.idname.as_str())
        .collect();
    assert_eq!(typed, want);
    assert_eq!(s.milestone, 2, "spec/current_milestone");
    // 63 M1 operators (decision log S1-1) plus the 125 M2 operators (M2 W0).
    assert_eq!(typed.len(), 188, "OPERATORS.md: operators with M <= 2");
    // Every typed op has a generated struct in its group's namespace file.
    let files = generate(&s.root).unwrap();
    for op in s.ops.iter().filter(|o| s.is_typed(o)) {
        let path = s
            .root
            .join(gen_ops::group_dir(op.group))
            .join(format!("{}.rs", op.ns));
        let src = String::from_utf8(files[&path].clone()).unwrap();
        assert!(
            src.contains(&format!("pub struct {} {{", gen_ops::struct_name(&s, op))),
            "{}: no typed struct",
            op.idname
        );
        assert!(src.contains(&format!("const IDNAME: &'static str = \"{}\";", op.idname)));
    }
    // The catalog holds every other operator, and nothing typed.
    let catalog: serde_json::Value = serde_json::from_str(&gen_ops::catalog_json(&s)).unwrap();
    let in_catalog: BTreeSet<String> = catalog
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["idname"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(in_catalog.len() + typed.len(), s.ops.len());
    for op in &s.ops {
        assert_ne!(
            in_catalog.contains(&op.idname),
            typed.contains(op.idname.as_str())
        );
    }
    // Every dumped namespace of §4.1 is present.
    for ns in [
        "object",
        "mesh",
        "uv",
        "transform",
        "view3d",
        "anim",
        "screen",
        "render",
        "wm",
        "material",
        "image",
        "world",
        "collection",
        "scene",
        "geometry",
        "armature",
        "pose",
        "nla",
        "export_scene",
        "import_scene",
        "ed",
    ] {
        assert!(
            s.ops.iter().any(|o| o.ns == ns),
            "namespace {ns} not dumped"
        );
    }
}

/// §4.2 frap-option audit: a `frap_*` option must not duplicate a Blender property.
#[test]
fn frap_option_audit() {
    let s = spec();
    let errors = audit::frap_options(&s);
    assert!(errors.is_empty(), "{errors:#?}");
    assert!(!s.frap_options.is_empty(), "frap_options.toml seeded");
}

/// The audit catches the duplicate that motivated it (`frap_instancing`, §17.2).
#[test]
fn frap_option_audit_catches_duplicates() {
    let mut s = spec();
    let mut dup = s.frap_options[0].clone();
    dup.identifier = "frap_instancing".into();
    dup.feature = "gpu_instancing".into();
    s.frap_options.push(dup);
    let mut dup2 = s.frap_options[0].clone();
    dup2.identifier = "frap_image_format".into();
    dup2.feature = "compat".into();
    s.frap_options.push(dup2);
    let errors = audit::frap_options(&s);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("frap_instancing") && e.contains("export_gpu_instances")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.contains("frap_image_format") && e.contains("export_image_format")),
        "{errors:?}"
    );
}

/// Every removed-4.x member of `compat_4x.toml` is really absent from the 5.2 dumps.
#[test]
fn compat_4x_members_are_absent() {
    let s = spec();
    assert!(!s.compat.is_empty());
    let errors = audit::compat_4x(&s);
    assert!(errors.is_empty(), "{errors:#?}");
    // A member that does exist is reported.
    let mut s2 = spec();
    s2.compat[0].identifier = "location".into();
    s2.compat[0].struct_ = "Object".into();
    assert_eq!(audit::compat_4x(&s2).len(), 1);
}

/// Python-only members are added by the overlay, never dumped by `bl_rna`.
#[test]
fn python_only_members_are_overlay_only() {
    let s = spec();
    let root = s.root.join("spec/blender-5.2/rna");
    for (st, id) in &s.python_only_props {
        let raw = model::read_json(&root.join(format!("{st}.json"))).unwrap();
        assert!(
            !raw["props"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["identifier"] == id.as_str()),
            "{st}.{id} is dumped"
        );
        let p = s.structs[st]
            .props
            .iter()
            .find(|p| &p.identifier == id)
            .unwrap();
        assert!(p.has_flag("PYTHON_ONLY") && p.has_flag("READONLY"));
    }
    let from_pydata = s.structs["Mesh"]
        .funcs
        .iter()
        .find(|f| f.identifier == "from_pydata")
        .unwrap();
    assert_eq!(from_pydata.params.len(), 4);
    let kf = s
        .bpy_struct_funcs
        .iter()
        .find(|f| f.identifier == "keyframe_insert")
        .unwrap();
    assert_eq!(kf.keyword_only_from, Some(1));
}

/// §4.1: the dumps were taken with the glTF variants preference on, so the variant RNA exists.
#[test]
fn dumps_taken_with_gltf_variants_preference() {
    let s = spec();
    assert_eq!(
        s.rna_meta["addon_prefs"]["io_scene_gltf2"]["KHR_materials_variants_ui"],
        serde_json::Value::Bool(true)
    );
    let ops_meta = model::read_json(&s.root.join("spec/blender-5.2/ops/_meta.json")).unwrap();
    assert_eq!(
        ops_meta["addon_prefs"]["io_scene_gltf2"]["KHR_materials_variants_ui"],
        serde_json::Value::Bool(true)
    );
    assert!(
        s.structs["Scene"]
            .props
            .iter()
            .any(|p| p.identifier == "gltf2_KHR_materials_variants_variants"),
        "Scene.gltf2_KHR_materials_variants_variants missing: dumped without the preference"
    );
    assert!(s.structs.contains_key("GLTF_AddonPreferences"));
}

/// Dynamic enums: every overlay entry is used and the M1 ones resolve to the right supersets.
#[test]
fn dynamic_enums_resolve() {
    let s = spec();
    assert_eq!(s.dynamic_overlay_keys, s.used_dynamic_overlay_keys);
    let engine = s.structs["RenderSettings"]
        .props
        .iter()
        .find(|p| p.identifier == "engine")
        .unwrap();
    let Kind::Enum { items, dynamic, .. } = &engine.kind else {
        panic!()
    };
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["BLENDER_EEVEE", "BLENDER_WORKBENCH", "CYCLES"]
    );
    assert_eq!(dynamic.as_deref(), Some("RenderSettings.engine"));
    let rotate = s
        .ops
        .iter()
        .find(|o| o.idname == "transform.rotate")
        .unwrap();
    let ot = rotate
        .props
        .iter()
        .find(|p| p.identifier == "orient_type")
        .unwrap();
    let Kind::Enum { items, runtime, .. } = &ot.kind else {
        panic!()
    };
    assert_eq!(items.len(), 7);
    assert!(*runtime);
    assert!(ot.has_flag("DYNAMIC_ENUM"));
}

/// Context members (F-DEF-5) are synthesised with `READONLY | CONTEXT_MEMBER`.
#[test]
fn context_members_synthesised() {
    let s = spec();
    let ctx = &s.structs["Context"];
    for m in [
        "object",
        "active_object",
        "selected_objects",
        "edit_object",
        "material",
    ] {
        let p = ctx.props.iter().find(|p| p.identifier == m).unwrap();
        assert!(
            p.has_flag("CONTEXT_MEMBER") && p.has_flag("READONLY"),
            "{m}"
        );
    }
    let scene = ctx.props.iter().find(|p| p.identifier == "scene").unwrap();
    assert!(!scene.has_flag("CONTEXT_MEMBER"));
}
