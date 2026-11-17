// SPDX-License-Identifier: GPL-3.0-or-later
//! A small JSON Schema 2020-12 instance validator for the keywords frap emits (no `$ref`:
//! frap's schemas are flattened, §14). Used to check operator arguments and tool calls against
//! `--describe=jsonschema` and responses against `--describe=protocol`.
//!
//! Supported: `type` (string or array), `enum`, `const`, `properties`, `required`,
//! `additionalProperties` (bool or schema), `items`, `prefixItems`, `minItems`, `maxItems`,
//! `uniqueItems`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `minLength`,
//! `maxLength`, `anyOf`, `oneOf`, `allOf`, `not`, boolean schemas. Annotations (`title`,
//! `description`, `default`, `format`, `$comment`, …) are ignored. `$ref` is an error.

use serde_json::Value as J;

/// Validates `instance` against `schema`; returns every violation as `<json pointer>: <why>`.
pub fn validate(schema: &J, instance: &J) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    check(schema, instance, "", &mut errs);
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn type_ok(t: &str, v: &J) -> bool {
    match t {
        "null" => v.is_null(),
        "boolean" => v.is_boolean(),
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "number" => v.is_number(),
        "integer" => match v {
            J::Number(n) => {
                n.is_i64() || n.is_u64() || n.as_f64().is_some_and(|f| f.fract() == 0.0)
            }
            _ => false,
        },
        _ => false,
    }
}

fn num(v: &J) -> Option<f64> {
    v.as_f64()
}

fn check(schema: &J, v: &J, at: &str, errs: &mut Vec<String>) {
    let s = match schema {
        J::Bool(true) => return,
        J::Bool(false) => {
            errs.push(format!("{at}: no value is allowed here"));
            return;
        }
        J::Object(s) => s,
        _ => {
            errs.push(format!("{at}: invalid schema"));
            return;
        }
    };
    if s.contains_key("$ref") {
        errs.push(format!(
            "{at}: $ref is not supported (frap schemas are flattened)"
        ));
        return;
    }
    if let Some(t) = s.get("type") {
        let ok = match t {
            J::String(t) => type_ok(t, v),
            J::Array(ts) => ts.iter().filter_map(J::as_str).any(|t| type_ok(t, v)),
            _ => true,
        };
        if !ok {
            errs.push(format!("{at}: expected type {t}, got {v}"));
            return;
        }
    }
    if let Some(J::Array(e)) = s.get("enum")
        && !e.contains(v)
    {
        errs.push(format!("{at}: {v} is not one of {}", J::Array(e.clone())));
    }
    if let Some(c) = s.get("const")
        && c != v
    {
        errs.push(format!("{at}: expected {c}, got {v}"));
    }
    if let Some(x) = num(v) {
        if let Some(m) = s.get("minimum").and_then(num)
            && x < m
        {
            errs.push(format!("{at}: {x} < minimum {m}"));
        }
        if let Some(m) = s.get("maximum").and_then(num)
            && x > m
        {
            errs.push(format!("{at}: {x} > maximum {m}"));
        }
        if let Some(m) = s.get("exclusiveMinimum").and_then(num)
            && x <= m
        {
            errs.push(format!("{at}: {x} <= exclusiveMinimum {m}"));
        }
        if let Some(m) = s.get("exclusiveMaximum").and_then(num)
            && x >= m
        {
            errs.push(format!("{at}: {x} >= exclusiveMaximum {m}"));
        }
    }
    if let J::String(st) = v {
        let n = st.chars().count() as u64;
        if let Some(m) = s.get("minLength").and_then(J::as_u64)
            && n < m
        {
            errs.push(format!("{at}: string shorter than {m}"));
        }
        if let Some(m) = s.get("maxLength").and_then(J::as_u64)
            && n > m
        {
            errs.push(format!("{at}: string longer than {m}"));
        }
    }
    if let J::Array(a) = v {
        let n = a.len() as u64;
        if let Some(m) = s.get("minItems").and_then(J::as_u64)
            && n < m
        {
            errs.push(format!("{at}: {n} items, expected at least {m}"));
        }
        if let Some(m) = s.get("maxItems").and_then(J::as_u64)
            && n > m
        {
            errs.push(format!("{at}: {n} items, expected at most {m}"));
        }
        if s.get("uniqueItems") == Some(&J::Bool(true)) {
            for (i, x) in a.iter().enumerate() {
                if a[..i].contains(x) {
                    errs.push(format!("{at}/{i}: duplicate item {x}"));
                }
            }
        }
        let prefix = s.get("prefixItems").and_then(J::as_array);
        let np = prefix.map_or(0, Vec::len);
        if let Some(p) = prefix {
            for (i, (ps, x)) in p.iter().zip(a).enumerate() {
                check(ps, x, &format!("{at}/{i}"), errs);
            }
        }
        if let Some(items) = s.get("items") {
            for (i, x) in a.iter().enumerate().skip(np) {
                check(items, x, &format!("{at}/{i}"), errs);
            }
        }
    }
    if let J::Object(o) = v {
        let props = s.get("properties").and_then(J::as_object);
        if let Some(J::Array(req)) = s.get("required") {
            for r in req.iter().filter_map(J::as_str) {
                if !o.contains_key(r) {
                    errs.push(format!("{at}: missing required property '{r}'"));
                }
            }
        }
        for (k, x) in o {
            let path = format!("{at}/{k}");
            match props.and_then(|p| p.get(k)) {
                Some(ps) => check(ps, x, &path, errs),
                None => match s.get("additionalProperties") {
                    Some(J::Bool(false)) => {
                        errs.push(format!("{at}: unexpected property '{k}'"));
                    }
                    Some(ap) => check(ap, x, &path, errs),
                    None => {}
                },
            }
        }
    }
    if let Some(J::Array(all)) = s.get("allOf") {
        for sub in all {
            check(sub, v, at, errs);
        }
    }
    if let Some(J::Array(any)) = s.get("anyOf")
        && !any.iter().any(|sub| validate(sub, v).is_ok())
    {
        errs.push(format!("{at}: matches none of anyOf"));
    }
    if let Some(J::Array(one)) = s.get("oneOf") {
        let n = one.iter().filter(|sub| validate(sub, v).is_ok()).count();
        if n != 1 {
            errs.push(format!("{at}: matches {n} of oneOf, expected exactly 1"));
        }
    }
    if let Some(not) = s.get("not")
        && validate(not, v).is_ok()
    {
        errs.push(format!("{at}: matches a 'not' schema"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keywords() {
        let s = json!({
            "type": "object",
            "properties": {
                "a": {"type": "integer", "minimum": 0, "maximum": 3},
                "b": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2},
                "c": {"type": "array", "items": {"enum": ["X", "Y"]}, "uniqueItems": true},
                "d": {"anyOf": [{"type": "string"}, {"type": "null"}]},
                "e": {"const": "k"},
            },
            "required": ["a"],
            "additionalProperties": false,
        });
        assert!(
            validate(
                &s,
                &json!({"a": 1, "b": [1, 2.5], "c": ["X"], "d": null, "e": "k"})
            )
            .is_ok()
        );
        assert!(validate(&s, &json!({})).is_err());
        assert!(validate(&s, &json!({"a": 4})).is_err());
        assert!(validate(&s, &json!({"a": 1.5})).is_err());
        assert!(validate(&s, &json!({"a": 1, "b": [1]})).is_err());
        assert!(validate(&s, &json!({"a": 1, "c": ["X", "X"]})).is_err());
        assert!(validate(&s, &json!({"a": 1, "c": ["Z"]})).is_err());
        assert!(validate(&s, &json!({"a": 1, "d": 3})).is_err());
        assert!(validate(&s, &json!({"a": 1, "e": "j"})).is_err());
        assert!(validate(&s, &json!({"a": 1, "zz": 0})).is_err());
        assert!(validate(&json!({"$ref": "#/x"}), &json!(1)).is_err());
        assert!(
            validate(
                &json!({"oneOf": [{"type": "number"}, {"type": "integer"}]}),
                &json!(1)
            )
            .is_err()
        );
        assert!(validate(&json!(true), &json!(1)).is_ok());
        assert!(validate(&json!(false), &json!(1)).is_err());
    }
}
