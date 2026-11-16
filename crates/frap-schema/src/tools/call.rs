// SPDX-License-Identifier: GPL-3.0-or-later
//! Turns a call of a per-namespace tool (`frap_ops_mesh {"call": {"op": …, "args": {…}}}`) into
//! the bpy-line statement it stands for, so a harness or `frap mcp` runs it through the normal
//! pipeline (same errors, same responses). `frap_exec` and `frap_describe` need no conversion.
//! A `null` argument (OpenAI strict mode) means "leave unset".

use frap_rna::{PropDef, PropFlags, PropKind};
use serde_json::Value as J;

use crate::manifest::find_op;

use super::NS_PREFIX;

/// A tool call that does not map to a statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallError(pub String);

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CallError {}

fn py_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('\'');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\'' => o.push_str("\\'"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('\'');
    o
}

/// A JSON value as a Python literal: arrays become tuples, objects dicts, enum flags sets.
fn py(v: &J, prop: Option<&PropDef>) -> Result<String, CallError> {
    Ok(match v {
        J::Null => "None".into(),
        J::Bool(b) => if *b { "True" } else { "False" }.into(),
        J::Number(n) => n.to_string(),
        J::String(s) => py_str(s),
        J::Array(a) => {
            let flag = prop.is_some_and(|p| p.flags.contains(PropFlags::ENUM_FLAG));
            let coll = prop.is_some_and(|p| matches!(p.kind, PropKind::Collection { .. }));
            let items = a
                .iter()
                .map(|x| py(x, None))
                .collect::<Result<Vec<_>, _>>()?;
            if flag {
                if items.is_empty() {
                    "set()".into()
                } else {
                    format!("{{{}}}", items.join(", "))
                }
            } else if coll {
                format!("[{}]", items.join(", "))
            } else if items.len() == 1 {
                format!("({},)", items[0])
            } else {
                format!("({})", items.join(", "))
            }
        }
        J::Object(o) => {
            let mut parts = Vec::new();
            for (k, x) in o {
                if x.is_null() {
                    continue;
                }
                parts.push(format!("{}: {}", py_str(k), py(x, None)?));
            }
            format!("{{{}}}", parts.join(", "))
        }
    })
}

/// An ASCII Python identifier (`[A-Za-z_][A-Za-z0-9_]*`): the only keys written into the
/// statement unchecked against the spec, so a key cannot inject statement text.
fn is_identifier(k: &str) -> bool {
    let mut c = k.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
}

/// The statement of a `frap_ops_<ns>` tool call.
pub fn call_to_statement(tool: &str, input: &J) -> Result<String, CallError> {
    let ns = tool
        .strip_prefix(NS_PREFIX)
        .ok_or_else(|| CallError(format!("'{tool}' is not a frap_ops_<namespace> tool")))?;
    let call = input
        .get("call")
        .ok_or_else(|| CallError("missing 'call'".into()))?;
    let op = call
        .get("op")
        .and_then(J::as_str)
        .ok_or_else(|| CallError("missing 'call.op'".into()))?;
    let idname = format!("{ns}.{op}");
    let entry = find_op(&idname).ok_or_else(|| {
        CallError(format!(
            "Calling operator \"bpy.ops.{idname}\" error, could not be found"
        ))
    })?;
    let mut args = Vec::new();
    if let Some(a) = call.get("args") {
        let a = a
            .as_object()
            .ok_or_else(|| CallError("'call.args' must be an object".into()))?;
        // Keyword order follows the operator's RNA order, then unknown keys (which the
        // registry reports with Blender's wording).
        for p in entry.spec.props {
            if let Some(v) = a.get(p.identifier).filter(|v| !v.is_null()) {
                args.push(format!("{}={}", p.identifier, py(v, Some(p))?));
            }
        }
        for (k, v) in a {
            if entry.spec.prop(k).is_none() && !v.is_null() {
                if !is_identifier(k) {
                    return Err(CallError(format!(
                        "'call.args' key {} is not a Python identifier",
                        py_str(k)
                    )));
                }
                args.push(format!("{k}={}", py(v, None)?));
            }
        }
    }
    Ok(format!("bpy.ops.{idname}({})", args.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn statements() {
        let s = call_to_statement(
            "frap_ops_mesh",
            &json!({"call": {"op": "primitive_cube_add", "args": {"location": [0, 0, 1], "size": 2, "align": null, "calc_uvs": false}}}),
        )
        .unwrap();
        assert_eq!(
            s,
            "bpy.ops.mesh.primitive_cube_add(size=2, calc_uvs=False, location=(0, 0, 1))"
        );
        let s = call_to_statement(
            "frap_ops_mesh",
            &json!({"call": {"op": "extrude_region_move", "args": {"TRANSFORM_OT_translate": {"value": [0, 0, 1], "orient_type": null}}}}),
        )
        .unwrap();
        assert_eq!(
            s,
            "bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})"
        );
        let s = call_to_statement(
            "frap_ops_object",
            &json!({"call": {"op": "select_pattern", "args": {"pattern": "it's\\"}}}),
        )
        .unwrap();
        assert_eq!(s, r"bpy.ops.object.select_pattern(pattern='it\'s\\')");
        assert!(call_to_statement("frap_exec", &json!({})).is_err());
        assert!(call_to_statement("frap_ops_mesh", &json!({"call": {"op": "nope"}})).is_err());
    }

    #[test]
    fn unknown_keys_must_be_identifiers() {
        // An unknown identifier is passed through so the registry reports OP_ARG_UNKNOWN.
        let s = call_to_statement(
            "frap_ops_mesh",
            &json!({"call": {"op": "primitive_cube_add", "args": {"sizee": 2}}}),
        )
        .unwrap();
        assert_eq!(s, "bpy.ops.mesh.primitive_cube_add(sizee=2)");
        for bad in [
            "x=1); bpy.ops.wm.read_factory_settings(",
            "a b",
            "1x",
            "",
            "caf\u{e9}",
        ] {
            let e = call_to_statement(
                "frap_ops_mesh",
                &json!({"call": {"op": "primitive_cube_add", "args": {bad: 1}}}),
            )
            .unwrap_err();
            assert!(e.0.contains("not a Python identifier"), "{bad}: {e}");
        }
        assert!(is_identifier("_a1") && !is_identifier("é"));
    }
}
