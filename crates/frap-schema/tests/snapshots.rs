// SPDX-License-Identifier: GPL-3.0-or-later
//! `insta` snapshots of `--describe` (§14, §20.1): per namespace, so namespaces never conflict.
//!
//! * `manifest__ns__<ns>`: one line of dumped facts per operator of the namespace (planned
//!   milestone, poll matrix, property identifiers). Nothing here depends on what frap
//!   implements, so WPs that register operators never touch these files: the full entry of
//!   every *implemented* operator is its own `op__<idname>.snap` (`describe_ops.rs`, §22.1).
//! * `manifest__op__<idname>`: full entries of operators frap does not implement in M1
//!   (planned M2 or later: dynamic enums, macros, angles), so they stay stable during M1.
//! * `manifest__<section>`: the other manifest sections.
//! * `jsonschema__op__<idname>`, `protocol`, `tools__*`: the schemas and tool formats.
//!
//! Bless with `INSTA_UPDATE=always cargo test -p frap-schema --test snapshots`.

use frap_schema::jsonschema::{op_input_schema, protocol};
use frap_schema::manifest::{
    self, DescribeOptions, Target, all_ops, describe, describe_entry, find_op, namespaces,
};
use frap_schema::tools;
use serde_json::{Value as J, json};

fn snap(name: &str, v: &J) {
    insta::with_settings!({ prepend_module_to_snapshot => false, omit_expression => true }, {
        insta::assert_json_snapshot!(name.to_owned(), v);
    });
}

fn summary(v: &J) -> J {
    let props: Vec<&str> = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["identifier"].as_str().unwrap())
        .collect();
    json!(format!(
        "{} | planned {} | poll {}/{} | {}",
        v["idname"].as_str().unwrap(),
        v["frap"]["planned"].as_str().unwrap(),
        v["poll"]["matrix"]["plain"].as_str().unwrap_or("-"),
        v["poll"]["matrix"]["view3d"].as_str().unwrap_or("-"),
        props.join(",")
    ))
}

#[test]
fn manifest_per_namespace() {
    frap_ops_all::link();
    for ns in namespaces() {
        let lines: Vec<J> = all_ops()
            .iter()
            .filter(|e| e.namespace() == ns)
            .map(|e| summary(&describe_entry(e)))
            .collect();
        snap(&format!("manifest__ns__{ns}"), &J::Array(lines));
    }
}

#[test]
fn manifest_selected_operators() {
    frap_ops_all::link();
    for idname in [
        "uv.minimize_stretch",
        "armature.extrude_forked",
        "object.modifier_apply_as_shapekey",
        "mesh.select_similar",
    ] {
        let e = find_op(idname).unwrap();
        assert!(
            !e.implemented(),
            "{idname}: pick an operator planned after M1"
        );
        snap(&format!("manifest__op__{idname}"), &describe_entry(e));
    }
}

#[test]
fn manifest_sections() {
    frap_ops_all::link();
    snap(
        "manifest__defaults_that_surprise",
        &manifest::defaults_that_surprise(),
    );
    snap("manifest__modifier_types", &manifest::modifier_types());
    let nodes: Vec<J> = manifest::node_types()
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            json!(format!(
                "{} | {} | {} | planned {} | in {} | out {}",
                n["bl_idname"].as_str().unwrap(),
                n["name"].as_str().unwrap(),
                n["struct"].as_str().unwrap(),
                n["planned"].as_str().unwrap(),
                n["inputs"].as_array().unwrap().len(),
                n["outputs"].as_array().unwrap().len()
            ))
        })
        .collect();
    snap("manifest__node_types", &J::Array(nodes));
    let ctx: Vec<J> = manifest::context_members()
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            json!(format!(
                "{} | {} {}{}",
                m["identifier"].as_str().unwrap(),
                m["kind"].as_str().unwrap(),
                m["type"].as_str().unwrap(),
                if m["callback_member"] == true {
                    " | callback"
                } else {
                    ""
                }
            ))
        })
        .collect();
    snap("manifest__context_members", &J::Array(ctx));
    let structs: Vec<J> = frap_spec_rna::ALL_STRUCTS
        .iter()
        .map(|s| {
            json!(format!(
                "{} < {} | {} props | {} funcs",
                s.identifier,
                s.base.map_or("-", |b| b.identifier),
                s.props.len(),
                s.funcs.len()
            ))
        })
        .collect();
    snap("manifest__structs_index", &J::Array(structs));
    snap(
        "manifest__struct__Camera",
        &describe(
            &Target::Struct("Camera".into()),
            &DescribeOptions::default(),
        )
        .unwrap(),
    );
    snap(
        "manifest__prop__Object.rotation_euler",
        &describe(
            &Target::Prop("Object".into(), "rotation_euler".into()),
            &DescribeOptions::default(),
        )
        .unwrap(),
    );
    let mut head = manifest::manifest(&DescribeOptions {
        implemented_only: true,
        ..Default::default()
    });
    let o = head.as_object_mut().unwrap();
    for k in [
        "counts",
        "operators",
        "structs",
        "bpy_struct_functions",
        "context_members",
        "modifier_types",
        "node_types",
        "defaults_that_surprise",
    ] {
        o.remove(k);
    }
    snap("manifest__head_implemented_only", &head);
}

#[test]
fn jsonschema_and_protocol() {
    frap_ops_all::link();
    // Implemented since M0, or planned after M1: stable while M1 WPs register operators.
    for idname in [
        "mesh.primitive_cube_add",
        "export_scene.gltf",
        "mesh.extrude_region_move",
        "mesh.bevel",
    ] {
        snap(
            &format!("jsonschema__op__{idname}"),
            &op_input_schema(find_op(idname).unwrap().spec),
        );
    }
    snap("protocol", &protocol());
}

#[test]
fn tools() {
    frap_ops_all::link();
    let d = DescribeOptions::default();
    snap("tools__anthropic", &tools::anthropic(&d).unwrap());
    snap("tools__openai", &tools::openai(&d).unwrap());
    snap("tools__mcp", &tools::mcp(&d).unwrap());
    // export_scene: gltf (implemented since M0) and Blender's other exporters (OUT).
    let ns = DescribeOptions {
        tool_namespaces: vec!["export_scene".into()],
        ..Default::default()
    };
    snap(
        "tools__anthropic_ns_export_scene",
        &tools::anthropic(&ns).unwrap(),
    );
    snap(
        "tools__openai_ns_export_scene",
        &tools::openai(&ns).unwrap(),
    );
}
