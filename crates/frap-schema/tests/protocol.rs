// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe=protocol` against the real thing: every response a session produces validates
//! against the published response schema, so the schemars mirror in `src/jsonschema/protocol.rs`
//! cannot drift from `frap_session::Response::to_json` (§13.2).

use frap_schema::jsonschema::{protocol, validate::validate};
use frap_session::{Session, SessionOptions};
use serde_json::json;

const SCRIPT: &str = "\
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
bpy.data.objects['Cube.001'].location
bpy.data.objects['Cube.001'].location = (1, 2, 3)
bpy.ops.mesh.primitive_cube_add(sizee=2)
bpy.ops.mesh.bevel(offset=0.1)
bpy.ops.mesh.primitive_cube_add(rotation=(0, 0, 90))
bpy.data.objects['Nope']
mesh.primitive_cube_add size=1 location=0,0,3
cube = 1
print(bpy.data.objects['Cube'].name)
";

#[test]
fn session_responses_validate_against_the_protocol_schema() {
    let schema = protocol();
    let response = &schema["response"];
    let mut session = Session::new(SessionOptions::default()).expect("session");
    let rs = session.exec_source_at(SCRIPT, 1);
    assert!(rs.len() >= 8, "{} responses", rs.len());
    let mut statuses = Vec::new();
    for r in &rs {
        let j = r.to_json();
        if let Err(e) = validate(response, &j) {
            panic!("response does not match --describe=protocol: {e:?}\n{j:#}");
        }
        statuses.push(j["status"].as_str().unwrap().to_owned());
        if let Some(err) = j.get("error") {
            validate(&schema["error"], err).unwrap();
        }
    }
    assert!(statuses.iter().any(|s| s == "FINISHED"));
    assert!(statuses.iter().any(|s| s == "ERROR"));
}

#[test]
fn documented_examples_validate() {
    let schema = protocol();
    // §13.1 / §13.5 examples.
    validate(
        &schema["events"]["ready"],
        &json!({"event":"ready","frap":"0.1.0","protocol":1,"blender_api":"5.2","startup":"factory"}),
    )
    .unwrap();
    validate(
        &schema["events"]["end"],
        &json!({"event":"end","ok":false,"stats":{"statements":12,"errors":1,"cancelled":0}}),
    )
    .unwrap();
    validate(
        &schema["events"]["done"],
        &json!({"id":7,"event":"done","ok":false,"counts":{"finished":1,"error":1,"cancelled":0,"skipped":1}}),
    )
    .unwrap();
    validate(
        &schema["request"],
        &json!({"id":7,"src":"bpy.ops.mesh.primitive_cube_add()\nbpy.ops.mesh.bevel(offset=0.1)"}),
    )
    .unwrap();
    validate(
        &schema["request"],
        &json!({"id":"a","src":"x","on_error":"continue"}),
    )
    .unwrap();
    assert!(
        validate(
            &schema["request"],
            &json!({"id":1,"src":"x","on_error":"stop"})
        )
        .is_err()
    );
    assert!(validate(&schema["request"], &json!({"src":"x"})).is_err());
    let skipped = json!({"id":7,"seq":3,"line":3,"kind":"op","op":"object.shade_smooth","ok":false,"status":"SKIPPED","reports":[],"state":"unchanged","hints":[]});
    validate(&schema["response"], &skipped).unwrap();
    let bad =
        json!({"seq":1,"line":1,"kind":"op","ok":true,"status":"DONE","reports":[],"hints":[]});
    assert!(validate(&schema["response"], &bad).is_err());
}

#[test]
fn frap_project_manifest_schema() {
    let s = &protocol()["frap_project_manifest"];
    // The frap-project (WP 1.9) documented manifest.json shape.
    let ok = json!({
        "format": "frap",
        "schema_version": 1,
        "blender_api": "5.2",
        "generator": "frap 0.1.0",
        "buffers": [{"hash": "5e1c00000000000000000000000000ab", "size": 28}],
        "images": [{"hash": "77aa00000000000000000000000000cd", "ext": "png", "size": 5120}]
    });
    validate(s, &ok).unwrap();
    for (k, v) in [
        ("format", json!("blend")),
        ("schema_version", json!("1")),
        ("buffers", json!([{"hash": "5e1c", "size": 28, "extra": 1}])),
        ("images", json!([{"hash": "77aa", "size": 1}])),
    ] {
        let mut bad = ok.clone();
        bad[k] = v;
        assert!(validate(s, &bad).is_err(), "{k}");
    }
    let mut extra = ok.clone();
    extra["unknown"] = json!(1);
    assert!(validate(s, &extra).is_err());
    let txt = s.to_string();
    assert!(txt.contains("^[0-9a-f]{32}$"), "{txt}");
}

#[test]
fn protocol_schemas_are_flat() {
    let s = protocol().to_string();
    assert!(
        !s.contains("\"$ref\""),
        "protocol schemas must be inlined (§14: no $ref)"
    );
}
