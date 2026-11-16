// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe=jsonschema`: one flattened schema per operator input + the protocol envelope (§14). WP 1.18.
//!
//! Rules (§14): JSON Schema 2020-12, no `$ref` (every schema is self-contained); enums →
//! `enum`; enum flags → `array` + `uniqueItems`; fixed arrays → `minItems = maxItems`; matrices →
//! arrays of rows; defaults included; hard ranges → `minimum`/`maximum`; macro sub-operator
//! properties (`TRANSFORM_OT_translate={...}`) → nested objects with the sub-operator's schema;
//! operator collections (`files`) → arrays of objects of the item struct's writable properties.
//! The instance encoding is the one an agent writes in a bpy call (and frap answers with,
//! §13.3), so `bpy.ops.ns.op(**args)` is valid exactly when `args` validates (up to dynamic
//! enum narrowing, which depends on the live context).

mod protocol;
pub mod validate;

pub use protocol::protocol;

use frap_rna::{ArrayShape, OperatorSpec, PropDef, PropFlags, PropKind, Unit};
use serde_json::{Map, Value as J, json};

use crate::manifest::{self, DescribeOptions, default_json, find_op, ranges};

/// The JSON Schema dialect frap emits.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// Nesting limit for macro steps and collection items (Blender's operator props nest at most
/// one macro level; collection items hold plain values).
const MAX_DEPTH: u8 = 3;

fn finite(j: &J) -> Option<J> {
    j.is_number().then(|| j.clone())
}

/// Description of a property for schemas and tools: Blender's text plus the unit when it
/// changes how a number is read (angles are radians).
pub fn prop_description(p: &PropDef) -> String {
    let mut d = if p.description.is_empty() {
        p.name.to_owned()
    } else {
        p.description.to_owned()
    };
    if p.unit == Unit::Rotation {
        d.push_str(" (radians)");
    }
    d
}

fn scalar_schema(p: &PropDef) -> J {
    match &p.kind {
        PropKind::Bool { .. } => json!({"type": "boolean"}),
        PropKind::Int { .. } | PropKind::Float { .. } => {
            let mut m = Map::new();
            m.insert(
                "type".into(),
                J::from(if matches!(p.kind, PropKind::Int { .. }) {
                    "integer"
                } else {
                    "number"
                }),
            );
            if let Some([hmin, hmax, _, _]) = ranges(p) {
                if let Some(x) = finite(&hmin) {
                    m.insert("minimum".into(), x);
                }
                if let Some(x) = finite(&hmax) {
                    m.insert("maximum".into(), x);
                }
            }
            J::Object(m)
        }
        _ => json!({}),
    }
}

fn sized(items: J, n: u16) -> J {
    json!({"type": "array", "items": items, "minItems": n, "maxItems": n})
}

fn struct_items_schema(item_type: &str, depth: u8) -> J {
    let Some(s) = frap_spec_rna::find_struct(item_type) else {
        return json!({"type": "object"});
    };
    if depth >= MAX_DEPTH {
        return json!({"type": "object"});
    }
    let mut props = Map::new();
    for p in s.all_props() {
        if p.is_readonly()
            || matches!(
                p.kind,
                PropKind::Pointer { .. } | PropKind::Collection { .. }
            )
        {
            continue;
        }
        props.insert(p.identifier.into(), prop_schema(p, depth + 1));
    }
    json!({"type": "object", "title": s.identifier, "properties": props, "additionalProperties": false})
}

/// The schema of one operator property (the value an agent passes for it).
pub fn prop_schema(p: &PropDef, depth: u8) -> J {
    let mut s = match (&p.kind, p.array) {
        (PropKind::Enum { items, .. }, _) => {
            let ids: Vec<&str> = items.iter().map(|i| i.identifier).collect();
            let item = if ids.is_empty() {
                json!({"type": "string"})
            } else {
                json!({"type": "string", "enum": ids})
            };
            if p.flags.contains(PropFlags::ENUM_FLAG) {
                json!({"type": "array", "items": item, "uniqueItems": true})
            } else {
                item
            }
        }
        (PropKind::String { max_bytes, .. }, _) => {
            let mut m = json!({"type": "string"});
            if *max_bytes > 1 {
                // `length_max` counts bytes including the NUL; a code point is at least one byte.
                m["maxLength"] = J::from(max_bytes - 1);
            }
            m
        }
        (PropKind::Pointer { fixed_type, .. }, _) => {
            match find_op(fixed_type).filter(|_| depth < MAX_DEPTH) {
                // Macro step: `TRANSFORM_OT_translate={...}` takes the sub-operator's props.
                Some(e) => object_schema(e.spec, depth + 1),
                None => json!({"type": "object"}),
            }
        }
        (PropKind::Collection { item_type, .. }, _) => {
            json!({"type": "array", "items": struct_items_schema(item_type, depth)})
        }
        (_, ArrayShape::Scalar) => scalar_schema(p),
        (_, ArrayShape::Fixed(n)) => sized(scalar_schema(p), n),
        (_, ArrayShape::Matrix(r, c)) => sized(sized(scalar_schema(p), c), r),
        (_, ArrayShape::Dynamic) => json!({"type": "array", "items": scalar_schema(p)}),
    };
    let o = s
        .as_object_mut()
        .expect("invariant: property schemas are objects");
    o.insert("title".into(), J::from(p.name));
    o.insert("description".into(), J::from(prop_description(p)));
    if !matches!(p.kind, PropKind::Pointer { .. }) {
        let d = default_json(p);
        // Some dumped defaults are not valid values (an enum default '' outside its items, an
        // int array of zeros under a minimum of 1): Blender substitutes at run time. A schema
        // `default` must be a valid instance, so those are left out here (the manifest keeps
        // the dumped value).
        if !contains_non_finite(&d) && validate::validate(&s, &d).is_ok() {
            s.as_object_mut()
                .expect("invariant: property schemas are objects")
                .insert("default".into(), d);
        }
    }
    s
}

fn contains_non_finite(v: &J) -> bool {
    match v {
        J::Object(o) => o.contains_key("$float"),
        J::Array(a) => a.iter().any(contains_non_finite),
        _ => false,
    }
}

/// Appends `text` to a schema's description as a new sentence.
pub fn append_description(s: &mut J, text: &str) {
    let Some(o) = s.as_object_mut() else { return };
    let d = o
        .get("description")
        .and_then(J::as_str)
        .unwrap_or("")
        .trim_end()
        .to_owned();
    let joined = if d.is_empty() {
        text.to_owned()
    } else if d.ends_with(['.', '!', '?', ')', ':']) {
        format!("{d} {text}")
    } else {
        format!("{d}. {text}")
    };
    o.insert("description".into(), J::from(joined));
}

/// [`prop_schema`] of an operator property, with what leaving it unset means (§14
/// `unset_semantics`) appended to its description.
pub fn op_prop_schema(spec: &OperatorSpec, p: &PropDef, depth: u8) -> J {
    let mut s = prop_schema(p, depth);
    if let Some(u) = manifest::unset_semantics(spec, p.identifier) {
        let u = u.strip_prefix("unset: ").unwrap_or(u);
        append_description(&mut s, &format!("If unset: {u}."));
    }
    s
}

fn object_schema(spec: &OperatorSpec, depth: u8) -> J {
    let mut props = Map::new();
    let mut required = Vec::new();
    for p in spec.props {
        props.insert(p.identifier.into(), op_prop_schema(spec, p, depth));
        if p.flags.contains(PropFlags::REQUIRED) {
            required.push(J::from(p.identifier));
        }
    }
    json!({
        "type": "object",
        "title": spec.bl_idname,
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

/// The flattened input schema of one operator: the keyword arguments of `bpy.ops.<idname>(…)`.
pub fn op_input_schema(spec: &'static OperatorSpec) -> J {
    let entry = find_op(spec.idname);
    let mut s = object_schema(spec, 0);
    let o = s.as_object_mut().expect("invariant: object schema");
    let mut head = Map::new();
    head.insert("$schema".into(), J::from(DIALECT));
    head.insert(
        "$id".into(),
        J::from(format!("urn:frap:op:{}", spec.idname)),
    );
    head.insert("title".into(), J::from(format!("bpy.ops.{}", spec.idname)));
    head.insert(
        "description".into(),
        J::from(if spec.description.is_empty() {
            spec.label.to_owned()
        } else {
            format!("{}: {}", spec.label, spec.description)
        }),
    );
    if !entry.is_some_and(|e| e.implemented()) {
        head.insert(
            "$comment".into(),
            J::from(format!(
                "not implemented by frap yet (planned: {}); calling it gives OP_NOT_IMPLEMENTED",
                manifest::operator_milestone(spec.idname)
            )),
        );
    }
    o.remove("title");
    for (k, v) in std::mem::take(o) {
        head.insert(k, v);
    }
    J::Object(head)
}

/// `--describe=jsonschema`.
pub fn describe_jsonschema(opts: &DescribeOptions) -> J {
    let ops: Map<String, J> = manifest::selected_ops(opts)
        .map(|e| (e.spec.idname.to_owned(), op_input_schema(e.spec)))
        .collect();
    json!({
        "$schema": DIALECT,
        "$comment": "frap --describe=jsonschema: operators.<idname> validates the keyword arguments of bpy.ops.<idname>(...); protocol holds the serve request, response and event schemas",
        "protocol": protocol(),
        "operators": ops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> J {
        op_input_schema(&frap_spec_ops_mesh::mesh::OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD)
    }

    #[test]
    fn cube_schema_validates_bpy_arguments() {
        let s = cube();
        let ok = |v: J| validate::validate(&s, &v);
        assert!(ok(json!({})).is_ok());
        assert!(ok(json!({"size": 2, "location": [0, 0, 1], "align": "CURSOR"})).is_ok());
        assert!(ok(json!({"size": "2"})).is_err());
        assert!(ok(json!({"location": [0, 0]})).is_err());
        assert!(ok(json!({"align": "SIDEWAYS"})).is_err());
        assert!(ok(json!({"sizee": 2})).is_err());
        assert!(ok(json!({"size": -1})).is_err());
        let rot = s["properties"]["rotation"]["description"].as_str().unwrap();
        assert!(
            rot.starts_with("Rotation for the newly added object (radians) If unset: from align"),
            "{rot}"
        );
        assert_eq!(s["properties"]["location"]["minItems"], 3);
        assert_eq!(s["properties"]["location"]["maxItems"], 3);
    }
}
