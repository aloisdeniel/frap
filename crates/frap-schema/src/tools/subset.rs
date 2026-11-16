// SPDX-License-Identifier: GPL-3.0-or-later
//! Checkers for the JSON-Schema subsets the LLM providers accept in tool definitions, applied
//! to frap's own tool output by the tests (acceptance: "tool formats validate against the
//! provider JSON-Schema subsets").
//!
//! * **Anthropic** (Messages API tools; the subset of strict tool use / structured outputs, so
//!   `strict: true` may be added): tool keys `name` (`^[a-zA-Z0-9_-]{1,64}$`), `description`,
//!   `input_schema` (root `type: "object"`); keywords `type`, `properties`, `required`,
//!   `additionalProperties` (must be `false` on every object), `items`, `enum`, `const`,
//!   `anyOf`, `allOf`, `description`, `title`, `format` (listed formats), `minItems` 0 or 1.
//!   Not supported: recursive schemas / `$ref` cycles, numeric constraints (`minimum`,
//!   `maximum`, `multipleOf`), string constraints (`minLength`, `maxLength`), complex array
//!   constraints, `additionalProperties` other than `false`. frap also leaves out `oneOf`,
//!   `not`, conditionals and `default`.
//! * **OpenAI** (function calling with `strict: true`, Structured Outputs): tool shape
//!   `{"type": "function", "function": {name, description, parameters, strict}}`; root
//!   `type: "object"` (not `anyOf`); every object has `additionalProperties: false` and lists
//!   **all** its properties in `required` (optional ones are nullable); `anyOf` allowed below the
//!   root; `allOf`, `oneOf`, `not`, `if`/`then`/`else`, `dependentRequired`,
//!   `dependentSchemas`, `patternProperties`, `default`, `uniqueItems`, `minLength`,
//!   `maxLength` are rejected; limits: at most 5000 object properties in total, 10 levels of
//!   nesting, 1000 enum values in total, 15000 characters of enum values for an enum with more
//!   than 250 values, 120000 characters of property names, enum and const values in total.

use serde_json::{Map, Value as J};

fn valid_name(n: &str) -> bool {
    (1..=64).contains(&n.len())
        && n.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

const TYPES: &[&str] = &[
    "object", "array", "string", "integer", "number", "boolean", "null",
];

const ANTHROPIC_FORMATS: &[&str] = &[
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "uri",
    "ipv4",
    "ipv6",
    "uuid",
];

#[derive(Default)]
struct Stats {
    properties: usize,
    enum_values: usize,
    chars: usize,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Provider {
    Anthropic,
    OpenAi,
}

fn allowed(p: Provider, k: &str) -> bool {
    let common = [
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "enum",
        "const",
        "anyOf",
        "description",
        "title",
        "format",
    ];
    if common.contains(&k) {
        return true;
    }
    match p {
        Provider::Anthropic => matches!(k, "allOf" | "minItems"),
        Provider::OpenAi => matches!(
            k,
            "pattern"
                | "minimum"
                | "maximum"
                | "exclusiveMinimum"
                | "exclusiveMaximum"
                | "multipleOf"
                | "minItems"
                | "maxItems"
        ),
    }
}

fn types_of(o: &Map<String, J>) -> Vec<&str> {
    match o.get("type") {
        Some(J::String(t)) => vec![t.as_str()],
        Some(J::Array(a)) => a.iter().filter_map(J::as_str).collect(),
        _ => Vec::new(),
    }
}

fn walk(p: Provider, s: &J, at: &str, depth: usize, stats: &mut Stats, errs: &mut Vec<String>) {
    let Some(o) = s.as_object() else {
        errs.push(format!("{at}: a schema must be an object"));
        return;
    };
    for k in o.keys() {
        if !allowed(p, k) {
            errs.push(format!("{at}: keyword '{k}' is not supported"));
        }
    }
    let types = types_of(o);
    for t in &types {
        if !TYPES.contains(t) {
            errs.push(format!("{at}: unknown type '{t}'"));
        }
    }
    if let Some(J::String(f)) = o.get("format")
        && p == Provider::Anthropic
        && !ANTHROPIC_FORMATS.contains(&f.as_str())
    {
        errs.push(format!("{at}: format '{f}' is not supported"));
    }
    if p == Provider::Anthropic
        && let Some(n) = o.get("minItems")
        && !matches!(n.as_u64(), Some(0 | 1))
    {
        errs.push(format!("{at}: minItems other than 0 or 1 is not supported"));
    }
    if let Some(e) = o.get("enum") {
        match e.as_array() {
            Some(a) if !a.is_empty() => {
                stats.enum_values += a.len();
                let chars: usize = a.iter().filter_map(J::as_str).map(str::len).sum();
                stats.chars += chars;
                if p == Provider::OpenAi && a.len() > 250 && chars > 15_000 {
                    errs.push(format!(
                        "{at}: enum of {} values exceeds 15000 characters",
                        a.len()
                    ));
                }
            }
            _ => errs.push(format!("{at}: enum must be a non-empty array")),
        }
    }
    if let Some(J::String(c)) = o.get("const") {
        stats.chars += c.len();
    }
    let is_object = types.contains(&"object") || o.contains_key("properties");
    if is_object {
        if depth > 10 && p == Provider::OpenAi {
            errs.push(format!("{at}: more than 10 levels of nesting"));
        }
        if o.get("additionalProperties") != Some(&J::Bool(false)) {
            errs.push(format!("{at}: objects need additionalProperties: false"));
        }
        let props = o.get("properties").and_then(J::as_object);
        let keys: Vec<&str> = props
            .map(|p| p.keys().map(String::as_str).collect())
            .unwrap_or_default();
        stats.properties += keys.len();
        stats.chars += keys.iter().map(|k| k.len()).sum::<usize>();
        let req: Vec<&str> = o
            .get("required")
            .and_then(J::as_array)
            .map(|r| r.iter().filter_map(J::as_str).collect())
            .unwrap_or_default();
        for r in &req {
            if !keys.contains(r) {
                errs.push(format!("{at}: required '{r}' is not a property"));
            }
        }
        if p == Provider::OpenAi {
            for k in &keys {
                if !req.contains(k) {
                    errs.push(format!(
                        "{at}: strict mode needs '{k}' in required (make it nullable instead)"
                    ));
                }
            }
        }
        if let Some(props) = props {
            for (k, v) in props {
                walk(
                    p,
                    v,
                    &format!("{at}/properties/{k}"),
                    depth + 1,
                    stats,
                    errs,
                );
            }
        }
    } else if o.contains_key("additionalProperties") || o.contains_key("required") {
        errs.push(format!("{at}: object keywords on a non-object schema"));
    }
    if let Some(items) = o.get("items") {
        walk(p, items, &format!("{at}/items"), depth + 1, stats, errs);
    }
    for key in ["anyOf", "allOf"] {
        if let Some(v) = o.get(key) {
            match v.as_array() {
                Some(a) if !a.is_empty() => {
                    for (i, b) in a.iter().enumerate() {
                        walk(p, b, &format!("{at}/{key}/{i}"), depth + 1, stats, errs);
                    }
                }
                _ => errs.push(format!("{at}: {key} must be a non-empty array")),
            }
        }
    }
}

fn check(
    p: Provider,
    name: &str,
    desc: Option<&J>,
    schema: Option<&J>,
    at: &str,
    errs: &mut Vec<String>,
) {
    if !valid_name(name) {
        errs.push(format!(
            "{at}: name '{name}' does not match ^[a-zA-Z0-9_-]{{1,64}}$"
        ));
    }
    if !desc.and_then(J::as_str).is_some_and(|d| !d.is_empty()) {
        errs.push(format!("{at}: a non-empty description is required"));
    }
    let Some(schema) = schema else {
        errs.push(format!("{at}: missing input schema"));
        return;
    };
    if schema.get("type") != Some(&J::from("object")) {
        errs.push(format!("{at}: the root schema must be type: object"));
    }
    if schema.get("anyOf").is_some() {
        errs.push(format!("{at}: the root schema must not be anyOf"));
    }
    let mut stats = Stats::default();
    walk(p, schema, &format!("{at}/schema"), 1, &mut stats, errs);
    if p == Provider::OpenAi {
        if stats.properties > 5000 {
            errs.push(format!(
                "{at}: {} object properties (limit 5000)",
                stats.properties
            ));
        }
        if stats.enum_values > 1000 {
            errs.push(format!(
                "{at}: {} enum values (limit 1000)",
                stats.enum_values
            ));
        }
        if stats.chars > 120_000 {
            errs.push(format!(
                "{at}: {} characters of names and values (limit 120000)",
                stats.chars
            ));
        }
    }
}

fn tools_array<'a>(tools: &'a J, errs: &mut Vec<String>) -> &'a [J] {
    match tools.as_array() {
        Some(a) => a,
        None => {
            errs.push("tools must be an array".into());
            &[]
        }
    }
}

fn finish(errs: Vec<String>) -> Result<(), Vec<String>> {
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// Checks an Anthropic `tools` array.
pub fn check_anthropic(tools: &J) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let mut names = Vec::new();
    for (i, t) in tools_array(tools, &mut errs).iter().enumerate() {
        let at = format!("/{i}");
        let Some(o) = t.as_object() else {
            errs.push(format!("{at}: a tool must be an object"));
            continue;
        };
        for k in o.keys() {
            if !matches!(
                k.as_str(),
                "name" | "description" | "input_schema" | "strict" | "cache_control"
            ) {
                errs.push(format!("{at}: unknown tool key '{k}'"));
            }
        }
        let name = o.get("name").and_then(J::as_str).unwrap_or("");
        if names.contains(&name) {
            errs.push(format!("{at}: duplicate tool name '{name}'"));
        }
        names.push(name);
        check(
            Provider::Anthropic,
            name,
            o.get("description"),
            o.get("input_schema"),
            &at,
            &mut errs,
        );
    }
    finish(errs)
}

/// Checks an OpenAI (Chat Completions, strict) `tools` array.
pub fn check_openai(tools: &J) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let mut names = Vec::new();
    for (i, t) in tools_array(tools, &mut errs).iter().enumerate() {
        let at = format!("/{i}");
        if t.get("type") != Some(&J::from("function")) {
            errs.push(format!("{at}: type must be 'function'"));
        }
        let Some(f) = t.get("function").and_then(J::as_object) else {
            errs.push(format!("{at}: missing function object"));
            continue;
        };
        for k in f.keys() {
            if !matches!(k.as_str(), "name" | "description" | "parameters" | "strict") {
                errs.push(format!("{at}: unknown function key '{k}'"));
            }
        }
        if f.get("strict") != Some(&J::Bool(true)) {
            errs.push(format!("{at}: strict must be true"));
        }
        let name = f.get("name").and_then(J::as_str).unwrap_or("");
        if names.contains(&name) {
            errs.push(format!("{at}: duplicate tool name '{name}'"));
        }
        names.push(name);
        check(
            Provider::OpenAi,
            name,
            f.get("description"),
            f.get("parameters"),
            &at,
            &mut errs,
        );
    }
    finish(errs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn anth(schema: J) -> Result<(), Vec<String>> {
        check_anthropic(&json!([{"name": "t", "description": "d", "input_schema": schema}]))
    }
    fn oai(schema: J) -> Result<(), Vec<String>> {
        check_openai(
            &json!([{"type": "function", "function": {"name": "t", "description": "d", "parameters": schema, "strict": true}}]),
        )
    }

    #[test]
    fn rejects_what_the_providers_reject() {
        let ok = json!({"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"], "additionalProperties": false});
        assert!(anth(ok.clone()).is_ok());
        assert!(oai(ok).is_ok());
        let open =
            json!({"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]});
        assert!(anth(open.clone()).is_err());
        assert!(oai(open).is_err());
        let min = json!({"type": "object", "properties": {"a": {"type": "number", "minimum": 0}}, "required": ["a"], "additionalProperties": false});
        assert!(anth(min.clone()).is_err());
        assert!(oai(min).is_ok());
        let one_of = json!({"type": "object", "properties": {"a": {"oneOf": [{"type": "string"}]}}, "required": ["a"], "additionalProperties": false});
        assert!(anth(one_of.clone()).is_err());
        assert!(oai(one_of).is_err());
        let optional = json!({"type": "object", "properties": {"a": {"type": "string"}}, "required": [], "additionalProperties": false});
        assert!(anth(optional.clone()).is_ok());
        assert!(oai(optional).is_err());
        let default = json!({"type": "object", "properties": {"a": {"type": "string", "default": "x"}}, "required": ["a"], "additionalProperties": false});
        assert!(anth(default.clone()).is_err());
        assert!(oai(default).is_err());
        let r#ref = json!({"type": "object", "properties": {"a": {"$ref": "#/$defs/x"}}, "required": ["a"], "additionalProperties": false});
        assert!(anth(r#ref).is_err());
        assert!(anth(json!({"anyOf": [{"type": "object"}]})).is_err());
        assert!(check_anthropic(&json!([{"name": "bad name", "description": "d", "input_schema": {"type": "object", "properties": {}, "additionalProperties": false}}])).is_err());
        let many: Map<String, J> = (0..1001)
            .map(|i| (format!("p{i}"), json!({"type": "string", "enum": ["A"]})))
            .collect();
        let keys: Vec<String> = many.keys().cloned().collect();
        assert!(oai(json!({"type": "object", "properties": many, "required": keys, "additionalProperties": false})).is_err());
    }
}
