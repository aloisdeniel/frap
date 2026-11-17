// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe=tools-anthropic|tools-openai`: the small fixed toolset (`frap_exec`, `frap_describe`) (§14). WP 1.18.
//!
//! Hundreds of tools overwhelm agents, so frap emits a **small fixed toolset**:
//!
//! * `frap_exec(statements, on_error?)`: run statements against the session's scene;
//! * `frap_describe(target)`: the manifest fragment of an operator, struct or property;
//! * optionally one tool per operator namespace (`frap_ops_mesh`, [`DescribeOptions::tool_namespaces`])
//!   whose `call` is an `anyOf` over per-operator argument objects (`op` is a `const`, so the
//!   branches are mutually exclusive: the provider-safe spelling of a `oneOf`).
//!
//! The same definitions are rendered for the Anthropic Messages API (`input_schema`), OpenAI
//! function calling in strict mode (`parameters`, every property required, optional ones
//! nullable) and MCP `tools/list` (`inputSchema`; `frap mcp` serves exactly [`mcp`]). Every
//! schema stays inside the providers' structured-output subsets ([`check_anthropic`],
//! [`check_openai`]): no `$ref`, no `oneOf`, `additionalProperties: false` on every object, and
//! numeric/length/array-size constraints and defaults moved into descriptions.

mod call;
mod subset;

pub use call::{CallError, call_to_statement};
pub use subset::{check_anthropic, check_openai};

use serde_json::{Map, Value as J, json};

use crate::jsonschema::{append_description, op_prop_schema};
use crate::manifest::{DescribeError, DescribeOptions, OpEntry, all_ops, namespaces};

/// Name of the statement tool.
pub const EXEC: &str = "frap_exec";
/// Name of the describe tool.
pub const DESCRIBE: &str = "frap_describe";
/// Prefix of the per-namespace operator tools (`frap_ops_mesh`).
pub const NS_PREFIX: &str = "frap_ops_";

const EXEC_DESCRIPTION: &str = "\
Run statements against the persistent frap scene (headless Blender 5.2 bpy API; the scene stays alive between calls). \
Call this to create or edit 3D models, read scene data, or export/render. \
One statement per line (or separated by ';'), bpy-style Python one-liners:\n\
- operator: bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))\n\
- read: bpy.data.objects['Cube'].location\n\
- write: bpy.data.objects['Cube'].location = (1, 2, 3)\n\
- RNA function: bpy.data.objects['Cube'].select_set(True)\n\
- check before calling: bpy.ops.mesh.bevel.poll()\n\
- bulk read: bpy.data.meshes['Cube'].vertices.foreach_get('co') (slices page: vertices[0:1000])\n\
The short form also works: mesh.primitive_cube_add size=2 location=0,0,1 ; data.objects['Cube'].location = 1,2,3. \
D and C are bpy.data and bpy.context. No variables, loops or imports: use full paths. Angles are radians. \
Returns one JSON response per statement (status FINISHED, CANCELLED, ERROR or SKIPPED; value, changes, context; \
on ERROR an error with Blender's message, code, detail.did_you_mean and a hint naming the fix) and a final done record. \
A failing statement leaves the scene unchanged.";

const ON_ERROR_DESCRIPTION: &str = "\
What happens to the remaining statements of this call after one fails: 'skip_rest' (default) answers them with status SKIPPED \
without running them, so a batch cannot cascade into misleading errors; 'continue' runs them.";

const DESCRIBE_DESCRIPTION: &str = "\
Describe the frap/Blender 5.2 API before using it: operator properties with types, defaults, ranges, enum items, units \
(angles are radians), poll requirements and whether frap implements it. \
Call this when unsure of an operator's or property's exact name, arguments or preconditions.";

const TARGET_DESCRIPTION: &str = "\
What to describe: an operator ('op:mesh.bevel' or 'mesh.bevel'), an RNA struct ('struct:Object' or 'Object'), \
a struct member ('Object.location', 'Object.select_set'), or 'errors' for every error code.";

/// A provider-neutral tool: name, description and a strict-subset input schema in which
/// optional properties are simply absent from `required`.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolDef {
    /// Tool name.
    pub name: String,
    /// Description (when and how to call it).
    pub description: String,
    /// Input schema.
    pub schema: J,
}

fn exec_tool() -> ToolDef {
    ToolDef {
        name: EXEC.into(),
        description: EXEC_DESCRIPTION.into(),
        schema: json!({
            "type": "object",
            "properties": {
                "statements": {
                    "type": "string",
                    "description": "One or more frap statements, newline- or ';'-separated.",
                },
                "on_error": {
                    "type": "string",
                    "enum": ["skip_rest", "continue"],
                    "description": ON_ERROR_DESCRIPTION,
                },
            },
            "required": ["statements"],
            "additionalProperties": false,
        }),
    }
}

fn describe_tool() -> ToolDef {
    ToolDef {
        name: DESCRIBE.into(),
        description: DESCRIBE_DESCRIPTION.into(),
        schema: json!({
            "type": "object",
            "properties": {
                "target": {"type": "string", "description": TARGET_DESCRIPTION},
            },
            "required": ["target"],
            "additionalProperties": false,
        }),
    }
}

/// Moves the keywords outside the providers' common subset into the description, recursively.
fn provider_safe(s: &mut J) {
    let J::Object(o) = s else { return };
    let mut notes: Vec<String> = Vec::new();
    let n_min = o.remove("minItems");
    let n_max = o.remove("maxItems");
    match (n_min, n_max) {
        (Some(a), Some(b)) if a == b => notes.push(format!("Exactly {a} items.")),
        (a, b) => {
            if let Some(a) = a {
                notes.push(format!("At least {a} items."));
            }
            if let Some(b) = b {
                notes.push(format!("At most {b} items."));
            }
        }
    }
    if o.remove("uniqueItems") == Some(J::Bool(true)) {
        notes.push("No duplicates.".into());
    }
    let lo = o.remove("minimum").and_then(|v| v.as_f64());
    let hi = o.remove("maximum").and_then(|v| v.as_f64());
    let small = |x: f64| x.abs() < 1e9;
    match (lo.filter(|x| small(*x)), hi.filter(|x| small(*x))) {
        (Some(a), Some(b)) => notes.push(format!("Range [{a}, {b}].")),
        (Some(a), None) => notes.push(format!("Minimum {a}.")),
        (None, Some(b)) => notes.push(format!("Maximum {b}.")),
        (None, None) => {}
    }
    if let Some(m) = o.remove("maxLength") {
        notes.push(format!("At most {m} characters."));
    }
    if let Some(d) = o.remove("default") {
        notes.push(format!("Default: {d}."));
    }
    if o.get("type") == Some(&J::from("object")) {
        o.entry("properties")
            .or_insert_with(|| J::Object(Map::new()));
        o.insert("additionalProperties".into(), J::Bool(false));
    }
    for key in ["properties"] {
        if let Some(J::Object(props)) = o.get_mut(key) {
            for v in props.values_mut() {
                provider_safe(v);
            }
        }
    }
    if let Some(items) = o.get_mut("items") {
        provider_safe(items);
    }
    for key in ["anyOf", "allOf"] {
        if let Some(J::Array(a)) = o.get_mut(key) {
            a.iter_mut().for_each(provider_safe);
        }
    }
    if !notes.is_empty() {
        append_description(s, &notes.join(" "));
    }
}

fn op_branch(e: &OpEntry) -> J {
    let spec = e.spec;
    let mut props = Map::new();
    let mut required = Vec::new();
    for p in spec.props {
        let mut s = op_prop_schema(spec, p, 0);
        provider_safe(&mut s);
        props.insert(p.identifier.into(), s);
        if p.flags.contains(frap_rna::PropFlags::REQUIRED) {
            required.push(J::from(p.identifier));
        }
    }
    let mut desc = if spec.description.is_empty() {
        spec.label.to_owned()
    } else {
        format!("{}: {}", spec.label, spec.description)
    };
    if !e.implemented() {
        desc.push_str(" (not implemented by frap yet: OP_NOT_IMPLEMENTED)");
    }
    json!({
        "type": "object",
        "description": desc,
        "properties": {
            "op": {"type": "string", "const": e.name()},
            "args": {
                "type": "object",
                "description": format!("Keyword arguments of bpy.ops.{}; omit a property (or pass null) to leave it unset.", spec.idname),
                "properties": props,
                "required": required,
                "additionalProperties": false,
            },
        },
        "required": ["op", "args"],
        "additionalProperties": false,
    })
}

/// The tool of one operator namespace, or `None` when no operator is selected.
pub fn namespace_tool(ns: &str, implemented_only: bool) -> Option<ToolDef> {
    let ops: Vec<&OpEntry> = all_ops()
        .iter()
        .filter(|e| e.namespace() == ns && (!implemented_only || e.implemented()))
        .collect();
    if ops.is_empty() {
        return None;
    }
    let names: Vec<&str> = ops.iter().map(|e| e.name()).collect();
    Some(ToolDef {
        name: format!("{NS_PREFIX}{ns}"),
        description: format!(
            "Call one bpy.ops.{ns}.* operator with typed arguments (the same as frap_exec with bpy.ops.{ns}.<op>(...); \
the scene persists between calls). Operators: {}. Angles are radians.",
            names.join(", ")
        ),
        schema: json!({
            "type": "object",
            "properties": {
                "call": {
                    "description": format!("The bpy.ops.{ns} operator to run and its arguments."),
                    "anyOf": ops.iter().map(|e| op_branch(e)).collect::<Vec<_>>(),
                },
            },
            "required": ["call"],
            "additionalProperties": false,
        }),
    })
}

/// The provider-neutral toolset for `opts`: `frap_exec`, `frap_describe`, then one tool per
/// requested namespace (implemented operators only with `implemented_only`).
pub fn tool_defs(opts: &DescribeOptions) -> Result<Vec<ToolDef>, DescribeError> {
    let mut v = vec![exec_tool(), describe_tool()];
    let known = namespaces();
    for ns in &opts.tool_namespaces {
        if !known.contains(&ns.as_str()) {
            return Err(DescribeError {
                message: format!("frap: no operator namespace '{ns}'"),
                did_you_mean: frap_op::suggest(ns, known.iter().copied())
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            });
        }
        if let Some(t) = namespace_tool(ns, opts.implemented_only) {
            v.push(t);
        }
    }
    Ok(v)
}

/// `--describe=tools-anthropic`: Anthropic Messages API `tools` (an array).
pub fn anthropic(opts: &DescribeOptions) -> Result<J, DescribeError> {
    Ok(J::Array(
        tool_defs(opts)?
            .into_iter()
            .map(
                |t| json!({"name": t.name, "description": t.description, "input_schema": t.schema}),
            )
            .collect(),
    ))
}

/// `--describe=tools-mcp`: the MCP `tools/list` result that `frap mcp` serves (the same tools
/// as [`anthropic`], with MCP's `inputSchema` key).
pub fn mcp(opts: &DescribeOptions) -> Result<J, DescribeError> {
    Ok(json!({
        "tools": tool_defs(opts)?
            .into_iter()
            .map(|t| json!({"name": t.name, "description": t.description, "inputSchema": t.schema}))
            .collect::<Vec<_>>(),
    }))
}

fn nullable(s: J) -> J {
    match s {
        J::Object(mut o) => {
            if let Some(J::Array(e)) = o.get_mut("enum") {
                e.push(J::Null);
            }
            match o.get("type").cloned() {
                Some(J::String(t)) if !o.contains_key("const") => {
                    o.insert("type".into(), json!([t, "null"]));
                    J::Object(o)
                }
                _ => {
                    if let Some(J::Array(e)) = o.get_mut("enum") {
                        e.pop();
                    }
                    let desc = o.remove("description");
                    let mut w = Map::new();
                    if let Some(d) = desc {
                        w.insert("description".into(), d);
                    }
                    w.insert("anyOf".into(), json!([J::Object(o), {"type": "null"}]));
                    J::Object(w)
                }
            }
        }
        other => other,
    }
}

/// OpenAI strict mode: every object lists all its properties in `required`; the optional ones
/// become nullable (`null` = leave unset).
fn openai_strict(s: &mut J) {
    let J::Object(o) = s else { return };
    if let Some(J::Object(props)) = o.get_mut("properties") {
        for v in props.values_mut() {
            openai_strict(v);
        }
    }
    if let Some(items) = o.get_mut("items") {
        openai_strict(items);
    }
    if let Some(J::Array(a)) = o.get_mut("anyOf") {
        a.iter_mut().for_each(openai_strict);
    }
    let Some(J::Object(props)) = o.get("properties").cloned() else {
        return;
    };
    let required: Vec<String> = o
        .get("required")
        .and_then(J::as_array)
        .map(|r| r.iter().filter_map(J::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let mut new_props = Map::new();
    for (k, v) in props {
        let v = if required.contains(&k) {
            v
        } else {
            nullable(v)
        };
        new_props.insert(k, v);
    }
    let all: Vec<J> = new_props.keys().map(|k| J::from(k.as_str())).collect();
    o.insert("properties".into(), J::Object(new_props));
    o.insert("required".into(), J::Array(all));
}

/// `--describe=tools-openai`: OpenAI Chat Completions function tools in strict mode
/// (`{"type": "function", "function": {name, description, parameters, strict: true}}`). An
/// optional argument is sent as `null` to leave it unset.
pub fn openai(opts: &DescribeOptions) -> Result<J, DescribeError> {
    Ok(J::Array(
        tool_defs(opts)?
            .into_iter()
            .map(|t| {
                let mut params = t.schema;
                openai_strict(&mut params);
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": params,
                        "strict": true,
                    },
                })
            })
            .collect(),
    ))
}
