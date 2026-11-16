// SPDX-License-Identifier: GPL-3.0-or-later
//! `rna_values` (T0, §20.4): the listed properties (`tests/golden/props.toml`) are equal on
//! both sides; floats within `tol`. Covers `objects`, `context` and `materials`.
//!
//! Also home of the shared tolerant JSON comparison every other comparator uses
//! ([`json_close`], [`close`]).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value as Json;

use crate::case::Mismatch;
use crate::{GContext, GMaterial, GObject};

/// Comparator name.
pub const NAME: &str = "rna_values";

/// Materials never compared (D-11: frap's factory startup has no grease-pencil material).
pub const EXCLUDED_MATERIALS: &[&str] = &["Dots Stroke"];

/// `a` and `b` are equal within `tol`: absolute below 1, relative above (f32-aware), NaN equals
/// NaN, infinities must match exactly.
pub fn close(a: f64, b: f64, tol: f64) -> bool {
    if a == b || (a.is_nan() && b.is_nan()) {
        return true;
    }
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    (a - b).abs() <= tol * a.abs().max(b.abs()).max(1.0)
}

/// Compares two JSON trees: numbers within `tol`, objects as key sets (key order ignored),
/// arrays element by element. Appends one mismatch per differing leaf to `out`.
pub fn json_close(
    comparator: &'static str,
    path: &str,
    frap: &Json,
    blender: &Json,
    tol: f64,
    out: &mut Vec<Mismatch>,
) {
    match (frap, blender) {
        (Json::Number(a), Json::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap_or(f64::NAN), b.as_f64().unwrap_or(f64::NAN));
            if !close(a, b, tol) {
                out.push(Mismatch::new(
                    comparator,
                    path,
                    format!("frap {a} != blender {b} (tol {tol})"),
                ));
            }
        }
        (Json::Array(a), Json::Array(b)) => {
            if a.len() != b.len() {
                out.push(Mismatch::new(
                    comparator,
                    path,
                    format!("frap has {} items, blender {}", a.len(), b.len()),
                ));
                return;
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                json_close(comparator, &format!("{path}[{i}]"), x, y, tol, out);
            }
        }
        (Json::Object(a), Json::Object(b)) => {
            for (k, x) in a {
                match b.get(k) {
                    Some(y) => json_close(comparator, &join(path, k), x, y, tol, out),
                    None => out.push(Mismatch::new(
                        comparator,
                        join(path, k),
                        "only in frap".to_string(),
                    )),
                }
            }
            for k in b.keys().filter(|k| !a.contains_key(*k)) {
                out.push(Mismatch::new(comparator, join(path, k), "only in blender"));
            }
        }
        (a, b) if a == b => {}
        (a, b) => out.push(Mismatch::new(
            comparator,
            path,
            format!("frap {} != blender {}", short(a), short(b)),
        )),
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// A JSON value shortened for messages.
pub fn short(v: &Json) -> String {
    let s = v.to_string();
    if s.len() > 120 {
        format!("{}…", s.chars().take(117).collect::<String>())
    } else {
        s
    }
}

fn to_json<T: Serialize>(v: &T) -> Json {
    serde_json::to_value(v).expect("invariant: golden state types serialize")
}

fn compare_maps<T: Serialize>(
    what: &str,
    frap: &BTreeMap<String, T>,
    blender: &BTreeMap<String, T>,
    skip: &[&str],
    tol: f32,
    out: &mut Vec<Mismatch>,
) {
    for (name, f) in frap.iter().filter(|(n, _)| !skip.contains(&n.as_str())) {
        let path = format!("{what}[{name:?}]");
        match blender.get(name) {
            Some(b) => json_close(NAME, &path, &to_json(f), &to_json(b), tol as f64, out),
            None => out.push(Mismatch::new(NAME, path, "only in frap")),
        }
    }
    for name in blender
        .keys()
        .filter(|n| !frap.contains_key(*n) && !skip.contains(&n.as_str()))
    {
        out.push(Mismatch::new(NAME, format!("{what}[{name:?}]"), "only in blender"));
    }
}

/// `compare=objects`: the same object names, every `GObject` field equal (floats within
/// `tol`).
pub fn compare_objects(
    frap: &BTreeMap<String, GObject>,
    blender: &BTreeMap<String, GObject>,
    tol: f32,
) -> Vec<Mismatch> {
    let mut out = Vec::new();
    compare_maps("objects", frap, blender, &[], tol, &mut out);
    out
}

/// `compare=context`.
pub fn compare_context(frap: &GContext, blender: &GContext, tol: f32) -> Vec<Mismatch> {
    let mut out = Vec::new();
    json_close(
        NAME,
        "context",
        &to_json(frap),
        &to_json(blender),
        tol as f64,
        &mut out,
    );
    out
}

/// `compare=materials` (`Dots Stroke` excluded, D-11).
pub fn compare_materials(
    frap: &BTreeMap<String, GMaterial>,
    blender: &BTreeMap<String, GMaterial>,
    tol: f32,
) -> Vec<Mismatch> {
    let mut out = Vec::new();
    compare_maps("materials", frap, blender, EXCLUDED_MATERIALS, tol, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn object(loc: [f32; 3]) -> GObject {
        GObject {
            r#type: "MESH".into(),
            parent: None,
            matrix_world: [
                [1.0, 0.0, 0.0, loc[0]],
                [0.0, 1.0, 0.0, loc[1]],
                [0.0, 0.0, 1.0, loc[2]],
                [0.0, 0.0, 0.0, 1.0],
            ],
            location: loc,
            rotation_mode: "XYZ".into(),
            rotation: vec![0.0; 3],
            scale: [1.0; 3],
            modifiers: vec![],
            materials: vec![Some("Material".into())],
            mode: "OBJECT".into(),
        }
    }

    #[test]
    fn tolerance() {
        assert!(close(1.0, 1.0 + 5e-6, 1e-5));
        assert!(!close(1.0, 1.0 + 5e-5, 1e-5));
        assert!(close(1000.0, 1000.0 + 5e-3, 1e-5)); // relative above 1
        assert!(close(f64::NAN, f64::NAN, 0.0));
        assert!(!close(f64::INFINITY, f64::MAX, 1.0));
    }

    #[test]
    fn objects_equal_under_permutation() {
        let mut a = BTreeMap::new();
        a.insert("Cube".to_string(), object([0.0, 0.0, 1.0]));
        a.insert("Camera".to_string(), object([7.0, -6.0, 5.0]));
        let mut b = BTreeMap::new();
        b.insert("Camera".to_string(), object([7.0, -6.0, 5.0 + 1e-7]));
        b.insert("Cube".to_string(), object([0.0, 0.0, 1.0]));
        assert_eq!(compare_objects(&a, &b, 1e-5), vec![]);
    }

    #[test]
    fn objects_differ_under_one_change() {
        let mut a = BTreeMap::new();
        a.insert("Cube".to_string(), object([0.0, 0.0, 1.0]));
        let mut b = a.clone();
        b.get_mut("Cube").unwrap().location[1] = 0.5;
        let m = compare_objects(&a, &b, 1e-5);
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].path, "objects[\"Cube\"].location[1]");
        let mut c = a.clone();
        c.get_mut("Cube").unwrap().mode = "EDIT".into();
        assert_eq!(compare_objects(&a, &c, 1e-5).len(), 1);
        let mut d = a.clone();
        d.insert("Light".into(), object([0.0; 3]));
        let m = compare_objects(&a, &d, 1e-5);
        assert_eq!(m[0].detail, "only in blender");
    }

    #[test]
    fn json_objects_ignore_key_order() {
        let a = json!({"x": 1, "y": [1.0, 2.0], "z": {"$float": "nan"}});
        let b = json!({"z": {"$float": "nan"}, "y": [1.0, 2.000001], "x": 1});
        let mut out = vec![];
        json_close(NAME, "", &a, &b, 1e-5, &mut out);
        assert!(out.is_empty(), "{out:?}");
        let c = json!({"z": {"$float": "inf"}, "y": [1.0, 2.0], "x": 1});
        json_close(NAME, "", &a, &c, 1e-5, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "z.$float");
    }

    #[test]
    fn materials_skip_dots_stroke() {
        let mat = |v: f64| GMaterial {
            nodes: json!({"Principled BSDF": {"inputs": {"Metallic": v}}}),
            links: json!([]),
        };
        let mut a = BTreeMap::new();
        a.insert("Material".to_string(), mat(0.0));
        let mut b = a.clone();
        b.insert("Dots Stroke".to_string(), mat(1.0));
        assert!(compare_materials(&a, &b, 1e-5).is_empty());
        b.insert("Material".to_string(), mat(1.0));
        assert_eq!(compare_materials(&a, &b, 1e-5).len(), 1);
    }

    #[test]
    fn context_compares_every_field() {
        let a = GContext {
            active: Some("Cube".into()),
            selected: vec!["Cube".into(), "Cube.001".into()],
            mode: "OBJECT".into(),
            select_mode: None,
            cursor: [0.0; 3],
        };
        assert!(compare_context(&a, &a.clone(), 1e-5).is_empty());
        let mut b = a.clone();
        b.selected.reverse(); // base order matters
        assert_eq!(compare_context(&a, &b, 1e-5).len(), 2);
    }
}
