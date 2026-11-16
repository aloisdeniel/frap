// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF `extras` → ID custom properties (`ID["key"] = value`), with Python's IDProperty
//! conversion rules: 32-bit ints, doubles, bools, strings, int/double/bool arrays, string and
//! dict lists, nested groups; a value Blender refuses (an int beyond 32 bits, a mixed list) is
//! stored as its Python `str()`; `null` (an IDP_NONE in Blender, which frap's `IdProp` has no
//! variant for) is skipped.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/com/extras.py` (`set_extras`, `BLACK_LIST`),
//! Apache-2.0, The glTF-Blender-IO authors; conversion rules verified on Blender 5.2.4
//! (`idprop_py_api.cc`).

use frap_gltf::Extras;
use frap_rna::{IdHeader, IdProp};
use indexmap::IndexMap;
use serde_json::Value as J;
use smol_str::SmolStr;

/// Custom properties never imported.
const BLACK_LIST: [&str; 9] = [
    "cycles",
    "cycles_visibility",
    "cycles_curves",
    "glTF2ExportSettings",
    "gltf2_mesh_applied",
    "gltf2_KHR_materials_variants_variants",
    "gltf2_active_variant",
    "gltf2_variant_default_materials",
    "gltf2_variant_mesh_data",
];

/// Python `repr` of a JSON value (`str(value)` of the dict/list the add-on holds).
fn py_repr(v: &J) -> String {
    match v {
        J::Null => "None".into(),
        J::Bool(true) => "True".into(),
        J::Bool(false) => "False".into(),
        J::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => match n.as_u64() {
                Some(u) => u.to_string(),
                None => py_float(n.as_f64().unwrap_or(0.0)),
            },
        },
        J::String(s) => py_str(s),
        J::Array(a) => format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        J::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", py_str(k), py_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Python `repr(float)`.
fn py_float(x: f64) -> String {
    if x.is_finite() && x == x.trunc() && x.abs() < 1e16 {
        format!("{x:.1}")
    } else {
        let s = format!("{x:?}");
        // Rust prints `1e20`; Python prints `1e+20`.
        match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        }
    }
}

/// Python `repr(str)` (single quotes unless the text contains one and no double quote).
fn py_str(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// `ID[key] = value`: the IDProperty Python would create, `None` when it would refuse.
fn to_idprop(v: &J) -> Option<IdProp> {
    match v {
        J::Null => None,
        J::Bool(b) => Some(IdProp::Bool(*b)),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                i32::try_from(i).ok().map(|i| IdProp::Int(i64::from(i)))
            } else if n.as_u64().is_some() {
                None
            } else {
                n.as_f64().map(IdProp::Float)
            }
        }
        J::String(s) => Some(IdProp::Str(s.as_str().into())),
        J::Object(o) => {
            let mut g = IndexMap::new();
            for (k, v) in o {
                if v.is_null() {
                    continue;
                }
                g.insert(SmolStr::new(k), to_idprop(v)?);
            }
            Some(IdProp::Group(g))
        }
        J::Array(a) => {
            if a.iter().all(|x| x.is_boolean()) && !a.is_empty() {
                return Some(IdProp::List(a.iter().filter_map(to_idprop).collect()));
            }
            if a.iter().all(|x| x.is_number()) {
                if a.iter().all(|x| x.is_i64()) {
                    let ints: Option<Vec<i64>> = a
                        .iter()
                        .map(|x| {
                            x.as_i64()
                                .and_then(|i| i32::try_from(i).ok())
                                .map(i64::from)
                        })
                        .collect();
                    return ints.map(IdProp::IntArray);
                }
                return Some(IdProp::FloatArray(
                    a.iter().filter_map(|x| x.as_f64()).collect(),
                ));
            }
            if a.iter().all(|x| x.is_string()) || a.iter().all(|x| x.is_object()) {
                return a
                    .iter()
                    .map(to_idprop)
                    .collect::<Option<Vec<_>>>()
                    .map(IdProp::List);
            }
            None
        }
    }
}

/// `set_extras(blender_element, extras, exclude)`.
pub(crate) fn set_extras(header: &mut IdHeader, extras: Option<&Extras>, exclude: &[&str]) {
    let Some(J::Object(map)) = extras.map(Extras::to_value) else {
        return;
    };
    for (k, v) in &map {
        if BLACK_LIST.contains(&k.as_str()) || exclude.contains(&k.as_str()) {
            continue;
        }
        if v.is_null() {
            continue;
        }
        let p = to_idprop(v).unwrap_or_else(|| IdProp::Str(py_repr(v).into()));
        header.custom.insert(SmolStr::new(k), p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn python_conversion_rules() {
        let mut h = IdHeader::new("X");
        let extras = Extras::from_value(&json!({
            "i": 5, "big": 1099511627776u64, "f": 1.5, "b": true, "s": "x", "li": [1, 2],
            "lf": [1, 2.5], "ls": ["a", "b"], "lmix": [1, "a"], "d": {"k": 1}, "n": null,
            "e": [], "cycles": 1, "targetNames": ["a"]
        }));
        set_extras(&mut h, Some(&extras), &["targetNames"]);
        let c = &h.custom;
        assert_eq!(c["i"], IdProp::Int(5));
        assert_eq!(c["big"], IdProp::Str("1099511627776".into()));
        assert_eq!(c["f"], IdProp::Float(1.5));
        assert_eq!(c["b"], IdProp::Bool(true));
        assert_eq!(c["li"], IdProp::IntArray(vec![1, 2]));
        assert_eq!(c["lf"], IdProp::FloatArray(vec![1.0, 2.5]));
        assert_eq!(
            c["ls"],
            IdProp::List(vec![IdProp::Str("a".into()), IdProp::Str("b".into())])
        );
        assert_eq!(c["lmix"], IdProp::Str("[1, 'a']".into()));
        assert_eq!(c["e"], IdProp::IntArray(vec![]));
        assert!(matches!(c["d"], IdProp::Group(_)));
        assert!(
            !c.contains_key("n") && !c.contains_key("cycles") && !c.contains_key("targetNames")
        );
    }

    #[test]
    fn python_reprs() {
        assert_eq!(
            py_repr(&json!({"a": [1.0, 2.5, true, null, "it's"]})),
            r#"{'a': [1.0, 2.5, True, None, "it's"]}"#
        );
        assert_eq!(py_float(1e20), "1e+20");
    }
}
