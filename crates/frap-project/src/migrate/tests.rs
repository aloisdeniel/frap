// SPDX-License-Identifier: GPL-3.0-or-later
//! The migration framework, the step table and the v0 → v1 step.

use serde_json::json;

use super::*;

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => panic!("not an object"),
    }
}

#[test]
fn migrations_cover_every_step_file() {
    // The chain is contiguous from 0 and ends at SCHEMA_VERSION.
    assert_eq!(MIGRATIONS.len(), SCHEMA_VERSION as usize);
    for (i, m) in MIGRATIONS.iter().enumerate() {
        assert_eq!(
            m.from as usize, i,
            "MIGRATIONS[{i}] migrates from {}",
            m.from
        );
        assert!(!m.summary.is_empty());
    }
    // Every step file is registered and named after its step.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/migrate/steps");
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|f| !f.starts_with('_'))
        .collect();
    files.sort_by_key(|f| {
        f.trim_start_matches('v')
            .split('_')
            .next()
            .and_then(|n| n.parse::<u32>().ok())
    });
    let expected: Vec<String> = MIGRATIONS
        .iter()
        .map(|m| format!("v{}_to_v{}.rs", m.from, m.from + 1))
        .collect();
    assert_eq!(files, expected);
}

#[test]
fn current_files_are_untouched() {
    let mut manifest =
        obj(json!({"schema_version": SCHEMA_VERSION, "blender_rna_version": "kept"}));
    let mut scene = json!({"meshes": {"M": {"attributes": [{"type": "kept"}]}}});
    let before = (manifest.clone(), scene.clone());
    assert_eq!(migrate(SCHEMA_VERSION, &mut manifest, &mut scene), Ok(None));
    assert_eq!((manifest, scene), before);
}

#[test]
fn newer_versions_are_refused() {
    let err = check_supported(SCHEMA_VERSION + 1).unwrap_err();
    assert_eq!(
        err,
        MigrateError::Newer {
            found: SCHEMA_VERSION + 1,
            supported: SCHEMA_VERSION
        }
    );
    assert_eq!(
        err.to_string(),
        format!(
            "file written by a newer frap (schema_version {}); this build reads schema_version {SCHEMA_VERSION} and older",
            SCHEMA_VERSION + 1
        )
    );
    assert!(check_supported(SCHEMA_VERSION).is_ok());
    assert!(check_supported(0).is_ok());
    let mut m = Map::new();
    let mut s = json!({});
    assert!(matches!(
        migrate(u32::MAX, &mut m, &mut s),
        Err(MigrateError::Newer { .. })
    ));
}

#[test]
fn v0_to_v1() {
    let mut manifest = obj(json!({
        "format": "frap", "schema_version": 0, "blender_rna_version": "5.2",
        "generator": "frap 0.0.1", "buffers": [], "images": [],
    }));
    let mut scene = json!({
        "window": {"scene": "Scene", "view_layer": "ViewLayer"},
        "meshes": {
            "Cube": {"attributes": [
                {"name": "position", "domain": "POINT", "type": "FLOAT_VECTOR", "data": [[0, 0, 0]]},
                {"name": "sharp_face", "domain": "FACE", "type": "BOOLEAN", "data": [true]},
            ], "materials": []},
            "Empty": {"materials": []},
            "linked": {"lib.blend": {"Cube": {"attributes": [
                {"name": "position", "domain": "POINT", "type": "FLOAT_VECTOR", "data": []},
            ]}}},
        },
        "objects": {"Cube": {"type": "MESH"}},
    });
    let report = migrate(0, &mut manifest, &mut scene).unwrap().unwrap();
    assert_eq!(report.from, 0);
    assert_eq!(report.to, 1);
    assert_eq!(report.applied, [MIGRATIONS[0].summary]);
    // Keys renamed in place (same position), schema_version bumped.
    assert_eq!(
        Value::Object(manifest.clone()).to_string(),
        r#"{"format":"frap","schema_version":1,"blender_api":"5.2","generator":"frap 0.0.1","buffers":[],"images":[]}"#
    );
    assert_eq!(
        scene,
        json!({
            "window": {"scene": "Scene", "view_layer": "ViewLayer"},
            "meshes": {
                "Cube": {"attributes": [
                    {"name": "position", "domain": "POINT", "data_type": "FLOAT_VECTOR", "data": [[0, 0, 0]]},
                    {"name": "sharp_face", "domain": "FACE", "data_type": "BOOLEAN", "data": [true]},
                ], "materials": []},
                "Empty": {"materials": []},
                "linked": {"lib.blend": {"Cube": {"attributes": [
                    {"name": "position", "domain": "POINT", "data_type": "FLOAT_VECTOR", "data": []},
                ]}}},
            },
            // Object.type is not an attribute type and is left alone.
            "objects": {"Cube": {"type": "MESH"}},
        })
    );
    let attr_keys: Vec<_> = scene["meshes"]["Cube"]["attributes"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(attr_keys, ["name", "domain", "data_type", "data"]);
    // The result parses as a current manifest.
    crate::manifest::Manifest::from_value(Value::Object(manifest), false).unwrap();
}

#[test]
fn v0_to_v1_reports_malformed_input() {
    let mut manifest = obj(json!({"schema_version": 0}));
    let mut scene = json!({"meshes": {"Cube": {"attributes": {}}}});
    let err = migrate(0, &mut manifest, &mut scene).unwrap_err();
    assert_eq!(
        err.to_string(),
        r#"migrating schema_version 0 to 1: meshes["Cube"].attributes is not an array"#
    );
    let mut manifest =
        obj(json!({"schema_version": 0, "blender_rna_version": "5.2", "blender_api": "5.2"}));
    let mut scene = json!({});
    assert!(matches!(
        migrate(0, &mut manifest, &mut scene),
        Err(MigrateError::Failed { from: 0, .. })
    ));
}

#[test]
fn framework_chains_steps_in_order() {
    fn push(cx: &mut MigrationCx<'_>, tag: &str) {
        cx.scene["log"]
            .as_array_mut()
            .unwrap()
            .push(Value::from(tag));
    }
    fn a(cx: &mut MigrationCx<'_>) -> Result<(), String> {
        assert_eq!(cx.manifest["schema_version"], 0);
        push(cx, "0->1");
        Ok(())
    }
    fn b(cx: &mut MigrationCx<'_>) -> Result<(), String> {
        assert_eq!(cx.manifest["schema_version"], 1);
        push(cx, "1->2");
        Ok(())
    }
    fn c(cx: &mut MigrationCx<'_>) -> Result<(), String> {
        push(cx, "2->3");
        Err("boom".into())
    }
    // Table order does not matter; `from` does.
    let steps = [
        Migration {
            from: 1,
            summary: "b",
            apply: b,
        },
        Migration {
            from: 0,
            summary: "a",
            apply: a,
        },
        Migration {
            from: 2,
            summary: "c",
            apply: c,
        },
    ];
    let mut manifest = obj(json!({"schema_version": 0}));
    let mut scene = json!({"log": []});
    let report = migrate_with(&steps, 2, 0, &mut manifest, &mut scene)
        .unwrap()
        .unwrap();
    assert_eq!(report.applied, ["a", "b"]);
    assert_eq!(scene["log"], json!(["0->1", "1->2"]));
    assert_eq!(manifest["schema_version"], 2);

    // Starting midway runs only the remaining steps.
    let mut manifest = obj(json!({"schema_version": 1}));
    let mut scene = json!({"log": []});
    migrate_with(&steps, 2, 1, &mut manifest, &mut scene).unwrap();
    assert_eq!(scene["log"], json!(["1->2"]));

    // A failing step stops the chain and names the version.
    let mut scene = json!({"log": []});
    let err = migrate_with(&steps, 3, 2, &mut obj(json!({})), &mut scene).unwrap_err();
    assert_eq!(
        err,
        MigrateError::Failed {
            from: 2,
            message: "boom".into()
        }
    );
    assert_eq!(err.to_string(), "migrating schema_version 2 to 3: boom");

    // A gap in the chain is reported.
    assert_eq!(
        migrate_with(
            &steps,
            5,
            0,
            &mut obj(json!({"schema_version": 0})),
            &mut json!({"log": []})
        ),
        Err(MigrateError::Failed {
            from: 2,
            message: "boom".into()
        })
    );
    assert_eq!(
        migrate_with(
            &steps[..2],
            3,
            0,
            &mut obj(json!({"schema_version": 0})),
            &mut json!({"log": []})
        ),
        Err(MigrateError::NoStep(2))
    );
}

#[test]
fn rename_key_keeps_position() {
    let mut m = obj(json!({"a": 1, "b": 2, "c": 3}));
    assert_eq!(rename_key(&mut m, "b", "x"), Ok(true));
    assert_eq!(
        Value::Object(m.clone()).to_string(),
        r#"{"a":1,"x":2,"c":3}"#
    );
    assert_eq!(rename_key(&mut m, "missing", "y"), Ok(false));
    assert!(rename_key(&mut m, "a", "c").is_err());
}
