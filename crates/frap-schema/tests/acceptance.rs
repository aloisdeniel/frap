// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.18 acceptance (ARCHITECTURE §22.3): `poll.requires` per operator, angle units shown as
//! `ROTATION (radians)`, unimplemented operators visible with `not_implemented`, operator
//! schemas self-consistent, tool formats inside the provider JSON-Schema subsets.

use frap_rna::{PropKind, Unit};
use frap_schema::jsonschema::{op_input_schema, validate::validate};
use frap_schema::manifest::{
    self, DEFAULTS_THAT_SURPRISE, DescribeOptions, SurpriseKind, Target, all_ops, describe,
    describe_entry, find_op, namespaces, poll_req_name,
};
use frap_schema::tools;
use serde_json::{Map, Value as J, json};

fn link() {
    frap_ops_all::link();
}

fn all_props_of(v: &J) -> Vec<&J> {
    v["properties"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

#[test]
fn poll_requires_per_operator() {
    link();
    let mut implemented = 0;
    for e in all_ops() {
        let v = describe_entry(e);
        let poll = &v["poll"];
        match e.def {
            Some(def) => {
                implemented += 1;
                let want: Vec<J> = def
                    .poll
                    .requires
                    .iter()
                    .map(|r| J::from(poll_req_name(r)))
                    .collect();
                assert_eq!(poll["requires"], J::Array(want), "{}", e.spec.idname);
                assert!(poll["text"].as_str().is_some_and(|t| !t.is_empty()));
            }
            None => {
                assert!(poll["requires"].is_null(), "{}", e.spec.idname);
                assert!(poll["text"].as_str().is_some_and(|t| !t.is_empty()));
            }
        }
        if e.spec.poll_matrix.is_some() {
            let m = &poll["matrix"];
            for k in ["plain", "view3d"] {
                let s = m[k].as_str().unwrap();
                assert_eq!(s.len(), 5);
                assert!(s.chars().all(|c| matches!(c, 'Y' | 'N' | 'X')));
            }
        } else {
            assert!(
                e.spec.idname.starts_with("frap."),
                "{} has no poll matrix",
                e.spec.idname
            );
        }
    }
    assert!(implemented >= 2, "M0 operators must be registered");
    // The worked example: primitives always poll.
    let cube = describe(
        &Target::Op("mesh.primitive_cube_add".into()),
        &DescribeOptions::default(),
    )
    .unwrap();
    assert_eq!(cube["poll"]["requires"], json!([]));
    assert_eq!(cube["poll"]["text"], "always available");
    assert_eq!(cube["poll"]["matrix"]["plain"], "YYYYY");
}

#[test]
fn angles_are_shown_as_rotation_radians() {
    link();
    let check = |p: &J, unit: Unit, at: &str| {
        let u = p["unit"].as_str().unwrap();
        assert_ne!(u, "ROTATION", "{at}");
        if unit == Unit::Rotation {
            assert_eq!(u, "ROTATION (radians)", "{at}");
        }
    };
    let mut seen = 0;
    for e in all_ops() {
        let v = describe_entry(e);
        for (p, def) in all_props_of(&v).into_iter().zip(e.spec.props) {
            check(p, def.unit, e.spec.idname);
            seen += usize::from(def.unit == Unit::Rotation);
        }
    }
    for s in frap_spec_rna::ALL_STRUCTS.iter() {
        let v = manifest::describe_struct(s);
        for (p, def) in all_props_of(&v).into_iter().zip(s.props) {
            check(p, def.unit, s.identifier);
            seen += usize::from(def.unit == Unit::Rotation);
        }
    }
    assert!(seen > 50, "only {seen} rotation props");
    let rot = describe(
        &Target::Op("transform.rotate".into()),
        &DescribeOptions::default(),
    )
    .unwrap();
    let value = rot["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["identifier"] == "value")
        .unwrap();
    assert_eq!(value["unit"], "ROTATION (radians)");
    let loc = describe(
        &Target::Prop("Object".into(), "rotation_euler".into()),
        &DescribeOptions::default(),
    )
    .unwrap();
    assert_eq!(loc["unit"], "ROTATION (radians)");
    assert_eq!(loc["struct"], "Object");
}

#[test]
fn unimplemented_operators_are_listed_as_not_implemented() {
    link();
    let opts = DescribeOptions::default();
    // mesh.bevel was the example until WP 2.11 implemented it.
    let unwrap = describe(&Target::Op("uv.smart_project".into()), &opts).unwrap();
    assert_eq!(unwrap["frap"]["status"], "not_implemented");
    assert_eq!(unwrap["frap"]["planned"], "M3");
    assert!(
        unwrap["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["identifier"] == "island_margin")
    );
    let cube = describe(&Target::Op("mesh.primitive_cube_add".into()), &opts).unwrap();
    assert_eq!(cube["frap"]["status"], "implemented");
    assert_eq!(cube["frap"]["tier"], "T1");
    let m = manifest::manifest(&opts);
    let ops = m["operators"].as_array().unwrap();
    let not_impl = ops
        .iter()
        .filter(|o| o["frap"]["status"] == "not_implemented")
        .count();
    let registered = frap_op::OPERATORS.len();
    assert_eq!(ops.len(), all_ops().len());
    assert_eq!(not_impl, ops.len() - registered);
    assert_eq!(m["counts"]["not_implemented"], not_impl);
    for section in [
        "structs",
        "context_members",
        "modifier_types",
        "node_types",
        "defaults_that_surprise",
        "bpy_struct_functions",
    ] {
        assert!(
            m[section].as_array().is_some_and(|a| !a.is_empty()),
            "{section}"
        );
    }
    let only = manifest::manifest(&DescribeOptions {
        implemented_only: true,
        ..Default::default()
    });
    assert_eq!(only["operators"].as_array().unwrap().len(), registered);
    let filtered = manifest::manifest(&DescribeOptions {
        filter: Some("mesh.primitive_*".into()),
        ..Default::default()
    });
    let n = all_ops()
        .iter()
        .filter(|e| e.spec.idname.starts_with("mesh.primitive_"))
        .count();
    assert_eq!(filtered["operators"].as_array().unwrap().len(), n);
    assert!(n >= 10);
    assert!(filtered.get("structs").is_none());
    // An operator Blender does not have is an error with suggestions, not an entry.
    let err = describe(&Target::Op("mesh.bevell".into()), &opts).unwrap_err();
    assert_eq!(
        err.did_you_mean.first().map(String::as_str),
        Some("mesh.bevel")
    );
    let fe = err.into_frap();
    assert_eq!(fe.code.0, "DESCRIBE_TARGET");
}

/// Which `*.primitive_*_add` operators apply `scale`, as measured on Blender 5.2.4
/// `-b --factory-startup` (`scale=(3, 1, 0.5)` against unset: dimensions change only for the
/// APPLIES rows; object scale stays 1 everywhere). `mesh.primitive_torus_add` has no `scale`.
const SCALE_ON_5_2_4: &[(&str, bool)] = &[
    ("mesh.primitive_cube_add", true),
    ("mesh.primitive_cone_add", true),
    ("mesh.primitive_cylinder_add", true),
    ("mesh.primitive_uv_sphere_add", true),
    ("mesh.primitive_ico_sphere_add", true),
    ("mesh.primitive_plane_add", false),
    ("mesh.primitive_grid_add", false),
    ("mesh.primitive_circle_add", false),
    ("mesh.primitive_monkey_add", false),
    ("curve.primitive_bezier_curve_add", false),
    ("curve.primitive_bezier_circle_add", false),
    ("curve.primitive_nurbs_curve_add", false),
    ("curve.primitive_nurbs_circle_add", false),
    ("curve.primitive_nurbs_path_add", false),
    ("surface.primitive_nurbs_surface_curve_add", false),
    ("surface.primitive_nurbs_surface_circle_add", false),
    ("surface.primitive_nurbs_surface_surface_add", false),
    ("surface.primitive_nurbs_surface_cylinder_add", false),
    ("surface.primitive_nurbs_surface_sphere_add", false),
    ("surface.primitive_nurbs_surface_torus_add", false),
];

#[test]
fn primitive_scale_semantics_match_blender() {
    link();
    // The curated lists are exactly the measured table.
    let mut applying: Vec<&str> = SCALE_ON_5_2_4.iter().filter(|r| r.1).map(|r| r.0).collect();
    let mut ignoring: Vec<&str> = SCALE_ON_5_2_4
        .iter()
        .filter(|r| !r.1)
        .map(|r| r.0)
        .collect();
    let sorted = |l: &[&'static str]| {
        let mut v = l.to_vec();
        v.sort_unstable();
        v
    };
    applying.sort_unstable();
    ignoring.sort_unstable();
    assert_eq!(sorted(manifest::PRIMITIVES_APPLYING_SCALE), applying);
    assert_eq!(sorted(manifest::PRIMITIVES_IGNORING_SCALE), ignoring);
    // Every primitive add operator whose spec has generic add props and a scale is either in
    // the table or gets no claim (cube_add_gizmo needs a 3D view and cannot be probed headless).
    let mut claimed = 0;
    for e in all_ops() {
        let spec = e.spec;
        if !spec.idname.contains(".primitive_") || spec.prop("scale").is_none() {
            continue;
        }
        let u = manifest::unset_semantics(spec, "scale");
        match SCALE_ON_5_2_4.iter().find(|r| r.0 == spec.idname) {
            Some(&(op, applies)) if spec.prop("align").is_some() => {
                let u = u.unwrap_or_else(|| panic!("{op}"));
                assert_eq!(u.starts_with("unset: (1, 1, 1)"), applies, "{op}: {u}");
                assert_eq!(u.starts_with("ignored:"), !applies, "{op}: {u}");
                let schema = op_input_schema(spec);
                let d = schema["properties"]["scale"]["description"]
                    .as_str()
                    .unwrap();
                assert_eq!(d.contains("If unset: (1, 1, 1)"), applies, "{op}: {d}");
                assert_eq!(d.contains("Ignored: this primitive"), !applies, "{op}: {d}");
                claimed += 1;
            }
            Some(_) => {}
            None => assert_eq!(
                (spec.idname, u),
                ("mesh.primitive_cube_add_gizmo", None),
                "unverified primitive with a scale"
            ),
        }
    }
    // At least every mesh primitive of the table has a full typed spec.
    assert!(claimed >= 9, "{claimed}");
    assert!(
        find_op("mesh.primitive_torus_add")
            .unwrap()
            .spec
            .prop("scale")
            .is_none()
    );
    let cube = DEFAULTS_THAT_SURPRISE
        .iter()
        .find(|s| s.target == "mesh.primitive_cube_add.scale")
        .unwrap();
    assert!(!cube.why.contains("every mesh.primitive"), "{}", cube.why);
    assert!(
        cube.why.contains("plane, grid, circle, monkey"),
        "{}",
        cube.why
    );
}

#[test]
fn defaults_that_surprise_match_the_specs() {
    for s in DEFAULTS_THAT_SURPRISE {
        let want: J = serde_json::from_str(s.default_json).unwrap();
        match s.kind {
            SurpriseKind::OperatorProperty => {
                let (op, prop) = s.target.rsplit_once('.').unwrap();
                let e = find_op(op).unwrap_or_else(|| panic!("{op}"));
                let p = e.spec.prop(prop).unwrap_or_else(|| panic!("{}", s.target));
                assert_eq!(manifest::default_json(p), want, "{}", s.target);
            }
            SurpriseKind::Data => match s.target {
                "Light.energy" => {
                    let l = frap_scene::generated::plain::LightFields::default();
                    assert_eq!(J::from(l.energy as i64), want);
                    assert_eq!(l.energy.fract(), 0.0);
                }
                "Scene.frame_current" => {
                    let p = frap_spec_rna::find_struct("Scene")
                        .unwrap()
                        .find_prop("frame_current")
                        .unwrap();
                    assert_eq!(manifest::default_json(p), want);
                }
                other => panic!("no check for data default {other}"),
            },
        }
    }
    // The §14 list: export_apply, export_cameras, export_lights, light_add 10 W, bevel
    // offset=0, vertices_smooth factor=0, rest frame 0.
    for t in [
        "export_scene.gltf.export_apply",
        "export_scene.gltf.export_cameras",
        "export_scene.gltf.export_lights",
        "Light.energy",
        "mesh.bevel.offset",
        "mesh.vertices_smooth.factor",
        "export_scene.gltf.export_current_frame",
    ] {
        assert!(
            DEFAULTS_THAT_SURPRISE.iter().any(|s| s.target == t),
            "{t} missing"
        );
    }
}

/// The arguments object made of every `default` the schema advertises.
fn defaults_object(schema: &J) -> J {
    let mut m = Map::new();
    for (k, p) in schema["properties"].as_object().unwrap() {
        if let Some(d) = p.get("default") {
            m.insert(k.clone(), d.clone());
        }
    }
    J::Object(m)
}

fn walk_no_ref(v: &J, at: &str) {
    match v {
        J::Object(o) => {
            assert!(!o.contains_key("$ref") && !o.contains_key("$defs"), "{at}");
            for (k, x) in o {
                walk_no_ref(x, &format!("{at}/{k}"));
            }
        }
        J::Array(a) => a.iter().for_each(|x| walk_no_ref(x, at)),
        _ => {}
    }
}

#[test]
fn operator_schemas_accept_their_defaults() {
    link();
    let mut failures = Vec::new();
    for e in all_ops() {
        let s = op_input_schema(e.spec);
        walk_no_ref(&s, e.spec.idname);
        assert_eq!(s["additionalProperties"], false);
        for p in e.spec.props {
            let ps = &s["properties"][p.identifier];
            if p.flags.contains(frap_rna::PropFlags::ENUM_FLAG) {
                assert_eq!(ps["type"], "array", "{}.{}", e.spec.idname, p.identifier);
                assert_eq!(ps["uniqueItems"], true);
            } else if let frap_rna::ArrayShape::Fixed(n) = p.array
                && !matches!(p.kind, PropKind::Enum { .. } | PropKind::String { .. })
            {
                assert_eq!(ps["minItems"], n);
                assert_eq!(ps["maxItems"], n);
            }
        }
        let args = defaults_object(&s);
        if let Err(errs) = validate(&s, &args) {
            failures.push(format!("{}: {errs:?}", e.spec.idname));
        }
    }
    assert!(
        failures.is_empty(),
        "{} operators reject their own defaults:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn fixed_tools_validate_against_provider_subsets() {
    link();
    let opts = DescribeOptions::default();
    let a = tools::anthropic(&opts).unwrap();
    tools::check_anthropic(&a).unwrap();
    let o = tools::openai(&opts).unwrap();
    tools::check_openai(&o).unwrap();
    let names: Vec<&str> = a
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["frap_exec", "frap_describe"]);
    // MCP serves the same tools as --describe=tools-anthropic (`inputSchema` key).
    let m = tools::mcp(&opts).unwrap();
    for (x, y) in a
        .as_array()
        .unwrap()
        .iter()
        .zip(m["tools"].as_array().unwrap())
    {
        assert_eq!(x["name"], y["name"]);
        assert_eq!(x["description"], y["description"]);
        assert_eq!(x["input_schema"], y["inputSchema"]);
    }
    assert_eq!(describe(&Target::ToolsMcp, &opts).unwrap(), m);
}

#[test]
fn namespace_tools_validate_against_provider_subsets() {
    link();
    let mut openai_limits = Vec::new();
    for ns in namespaces() {
        for implemented_only in [true, false] {
            let opts = DescribeOptions {
                tool_namespaces: vec![ns.to_owned()],
                implemented_only,
                ..Default::default()
            };
            let a = tools::anthropic(&opts).unwrap();
            if let Err(e) = tools::check_anthropic(&a) {
                panic!("anthropic {ns}: {:?}", &e[..e.len().min(5)]);
            }
            let o = tools::openai(&opts).unwrap();
            if let Err(e) = tools::check_openai(&o) {
                // Only OpenAI's size limits may fail, and only for whole catalog namespaces.
                assert!(!implemented_only, "openai {ns}: {:?}", &e[..e.len().min(5)]);
                assert!(
                    e.iter()
                        .all(|m| m.contains("limit") || m.contains("exceeds")),
                    "openai {ns}: {:?}",
                    &e[..e.len().min(5)]
                );
                openai_limits.push(ns);
            }
        }
    }
    eprintln!(
        "namespaces over OpenAI's strict-mode size limits when listing every Blender operator: {openai_limits:?}"
    );
    assert!(
        tools::anthropic(&DescribeOptions {
            tool_namespaces: vec!["meshh".into()],
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn tool_calls_round_trip_through_the_operator_schema() {
    link();
    let opts = DescribeOptions {
        tool_namespaces: vec!["mesh".into()],
        implemented_only: true,
        ..Default::default()
    };
    let a = tools::anthropic(&opts).unwrap();
    let t = &a.as_array().unwrap()[2];
    assert_eq!(t["name"], "frap_ops_mesh");
    let input =
        json!({"call": {"op": "primitive_cube_add", "args": {"size": 2, "location": [0, 0, 1]}}});
    validate(&t["input_schema"], &input).unwrap();
    assert!(
        validate(
            &t["input_schema"],
            &json!({"call": {"op": "primitive_cube_add", "args": {"sizee": 2}}})
        )
        .is_err()
    );
    let stmt = tools::call_to_statement("frap_ops_mesh", &input).unwrap();
    assert_eq!(
        stmt,
        "bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))"
    );
    // OpenAI strict: optional args are sent as null and dropped.
    let o = tools::openai(&opts).unwrap();
    let params = &o.as_array().unwrap()[2]["function"]["parameters"];
    let mut args = Map::new();
    for p in frap_spec_ops_mesh::mesh::OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD.props {
        args.insert(p.identifier.into(), J::Null);
    }
    args.insert("size".into(), json!(3));
    let input = json!({"call": {"op": "primitive_cube_add", "args": args}});
    validate(params, &input).unwrap();
    assert_eq!(
        tools::call_to_statement("frap_ops_mesh", &input).unwrap(),
        "bpy.ops.mesh.primitive_cube_add(size=3)"
    );
}

#[test]
fn errors_list_registered_codes() {
    let e = describe(&Target::Errors, &DescribeOptions::default()).unwrap();
    let codes: Vec<&str> = e["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"DESCRIBE_TARGET"));
    assert!(codes.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn structs_and_members() {
    let opts = DescribeOptions::default();
    let ob = describe(&Target::Struct("Object".into()), &opts).unwrap();
    assert_eq!(ob["base"], "ID");
    assert!(
        ob["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["identifier"] == "location")
    );
    let name = describe(&Target::Prop("Object".into(), "name".into()), &opts).unwrap();
    assert_eq!(name["struct"], "ID");
    let f = describe(&Target::Prop("Object".into(), "select_set".into()), &opts).unwrap();
    assert_eq!(f["kind"], "function");
    let kf = describe(
        &Target::Prop("Object".into(), "keyframe_insert".into()),
        &opts,
    )
    .unwrap();
    assert_eq!(kf["struct"], "bpy_struct");
    let e = describe(&Target::Prop("Object".into(), "locaton".into()), &opts).unwrap_err();
    assert!(e.did_you_mean.contains(&"location".to_owned()));
    assert!(describe(&Target::Struct("Objekt".into()), &opts).is_err());
    let ctx = manifest::context_members();
    assert!(
        ctx.as_array()
            .unwrap()
            .iter()
            .any(|m| m["identifier"] == "active_object" && m["type"] == "Object")
    );
}

#[test]
fn agents_guide_covers_the_agent_surface() {
    let guide = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/agents.md"));
    for needle in [
        "frap serve --jsonl",
        "frap mcp",
        "on_error",
        "skip_rest",
        "SKIPPED",
        ".poll()",
        "poll.requires",
        "foreach_get",
        "--max-array",
        "frap_exec",
        "frap_describe",
        "tools-anthropic",
        "tools-openai",
        "ROTATION (radians)",
        "defaults_that_surprise",
        "not_implemented",
    ] {
        assert!(
            guide.contains(needle),
            "docs/agents.md does not mention {needle}"
        );
    }
}
