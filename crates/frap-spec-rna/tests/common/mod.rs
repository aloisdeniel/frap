// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared drift checks (ARCHITECTURE §4.2): a compiled `PropDef` / `OperatorSpec` against the raw
//! Blender dump JSON in `spec/blender-5.2/`. Included by path from every `frap-spec-*` crate's
//! drift test (`#[path = "../../frap-spec-rna/tests/common/mod.rs"] mod common;`), so the facts
//! are checked independently of the generator's own normalisation.
#![allow(dead_code)]

use std::collections::BTreeSet;

use frap_rna::{
    ArrayShape, Defaults, EnumDefault, OperatorSpec, PollCell, PropDef, PropFlags, PropKind,
};
use serde_json::Value as J;

/// `spec/` of the workspace.
pub fn spec_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec")
}

/// Reads `spec/<rel>` as JSON.
pub fn load(rel: &str) -> J {
    let p = spec_dir().join(rel);
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn num(j: &J) -> f64 {
    match j {
        J::Number(n) => n.as_f64().unwrap(),
        J::Bool(b) => *b as i64 as f64,
        J::Object(o) => match o["$float"].as_str().unwrap() {
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            _ => f64::NAN,
        },
        _ => panic!("not a number: {j}"),
    }
}

fn flat(j: &J) -> Vec<J> {
    match j {
        J::Array(a) => a.iter().flat_map(flat).collect(),
        x => vec![x.clone()],
    }
}

fn same_f64(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// Flags frap synthesises (not dumped by Blender).
const SYNTH: PropFlags = PropFlags::DYNAMIC_ENUM
    .union(PropFlags::PYTHON_ONLY)
    .union(PropFlags::CONTEXT_MEMBER);

/// Checks one compiled property against its raw dump.
pub fn check_prop(ctx: &str, p: &PropDef, j: &J) {
    let ctx = format!("{ctx}.{}", p.identifier);
    assert_eq!(j["identifier"], p.identifier, "{ctx}");
    assert_eq!(j["name"].as_str().unwrap_or(""), p.name, "{ctx} name");
    assert_eq!(
        j["description"].as_str().unwrap_or(""),
        p.description,
        "{ctx} description"
    );
    assert_eq!(j["type"], p.type_str(), "{ctx} type");
    let sub = j["subtype"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("NONE");
    assert_eq!(sub, p.subtype.as_str(), "{ctx} subtype");
    assert_eq!(
        j["unit"].as_str().unwrap_or("NONE"),
        p.unit.as_str(),
        "{ctx} unit"
    );
    let shape = match j.get("array").and_then(J::as_array).map(|a| a.as_slice()) {
        None | Some([]) => ArrayShape::Scalar,
        Some([J::String(_)]) => ArrayShape::Dynamic,
        Some([n]) => ArrayShape::Fixed(n.as_u64().unwrap() as u16),
        Some([r, c]) => ArrayShape::Matrix(r.as_u64().unwrap() as u16, c.as_u64().unwrap() as u16),
        Some(other) => panic!("{ctx}: shape {other:?}"),
    };
    assert_eq!(shape, p.array, "{ctx} array");
    let dumped: BTreeSet<&str> = j["flags"]
        .as_array()
        .map(|a| a.iter().filter_map(J::as_str).collect())
        .unwrap_or_default();
    let known: BTreeSet<&str> = dumped
        .iter()
        .copied()
        .filter(|f| PropFlags::from_name(f).is_some())
        .collect();
    let compiled: BTreeSet<&str> = p.flags.difference(SYNTH).names().into_iter().collect();
    assert_eq!(known, compiled, "{ctx} flags");
    match (&j.get("deprecated"), p.deprecated) {
        (Some(d), Some(x)) => assert_eq!(d["note"], x.note, "{ctx} deprecation"),
        (None, None) => {}
        _ => panic!("{ctx}: deprecation mismatch"),
    }
    match &p.kind {
        PropKind::Bool { default } => match default {
            Defaults::Scalar(b) => assert_eq!(j["default"].as_bool(), Some(*b), "{ctx} default"),
            Defaults::Array(a) => {
                let w: Vec<bool> = flat(&j["default"])
                    .iter()
                    .map(|x| x.as_bool().unwrap())
                    .collect();
                assert_eq!(a.to_vec(), w, "{ctx} default");
            }
        },
        PropKind::Int {
            default,
            hard,
            soft,
        } => {
            let i = |k: &str| j[k].as_i64().unwrap();
            assert_eq!(*hard, (i("hard_min"), i("hard_max")), "{ctx} hard");
            assert_eq!(*soft, (i("soft_min"), i("soft_max")), "{ctx} soft");
            let w: Vec<i64> = flat(&j["default"]).iter().map(|x| num(x) as i64).collect();
            match default {
                Defaults::Scalar(v) => assert_eq!(vec![*v], w, "{ctx} default"),
                Defaults::Array(a) => assert_eq!(a.to_vec(), w, "{ctx} default"),
            }
        }
        PropKind::Float {
            default,
            hard,
            soft,
            precision,
            ..
        } => {
            let f = |k: &str| num(&j[k]);
            assert!(
                same_f64(hard.0, f("hard_min")) && same_f64(hard.1, f("hard_max")),
                "{ctx} hard"
            );
            assert!(
                same_f64(soft.0, f("soft_min")) && same_f64(soft.1, f("soft_max")),
                "{ctx} soft"
            );
            // -1 ("automatic", a few modifier props) is stored as 0.
            assert_eq!(
                *precision as i64,
                j["precision"].as_i64().unwrap().max(0),
                "{ctx} precision"
            );
            let w: Vec<f64> = flat(&j["default"]).iter().map(num).collect();
            let got: Vec<f64> = match default {
                Defaults::Scalar(v) => vec![*v],
                Defaults::Array(a) => a.to_vec(),
            };
            assert_eq!(got.len(), w.len(), "{ctx} default");
            assert!(
                got.iter().zip(&w).all(|(a, b)| same_f64(*a, *b)),
                "{ctx} default"
            );
        }
        PropKind::String { default, max_bytes } => {
            assert_eq!(
                j["default"].as_str().unwrap_or(""),
                *default,
                "{ctx} default"
            );
            assert_eq!(
                j["length_max"].as_u64().unwrap_or(0),
                *max_bytes as u64,
                "{ctx} max_bytes"
            );
        }
        PropKind::Enum {
            items,
            default,
            dynamic,
        } => {
            let dumped: Vec<(String, String, String, i64)> = j["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| {
                    (
                        i["identifier"].as_str().unwrap().to_owned(),
                        i["name"].as_str().unwrap().to_owned(),
                        i["description"].as_str().unwrap().to_owned(),
                        i["value"].as_i64().unwrap(),
                    )
                })
                .collect();
            let compiled: Vec<(String, String, String, i64)> = items
                .iter()
                .map(|i| {
                    (
                        i.identifier.into(),
                        i.name.into(),
                        i.description.into(),
                        i.value as i64,
                    )
                })
                .collect();
            let is_dyn = j.get("items_static").is_some()
                || j.get("dynamic_hint").is_some()
                || (dumped.is_empty() && !p.flags.contains(PropFlags::ENUM_FLAG));
            if is_dyn {
                assert!(dynamic.is_some(), "{ctx}: dynamic enum without overlay key");
            }
            match dynamic {
                None => {
                    assert_eq!(compiled, dumped, "{ctx} items");
                    match default {
                        EnumDefault::Item(d) => assert_eq!(j["default"], *d, "{ctx} default"),
                        EnumDefault::Flags(f) => {
                            let w: Vec<&str> = j["default"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|x| x.as_str().unwrap())
                                .collect();
                            assert_eq!(f.to_vec(), w, "{ctx} default");
                        }
                    }
                }
                Some(_) => {
                    assert!(
                        p.flags.contains(PropFlags::DYNAMIC_ENUM),
                        "{ctx}: DYNAMIC_ENUM flag"
                    );
                    // The superset contains the dumped items, except a 0/1-item placeholder
                    // (`'NONE'`, `'DEFAULT'`) that the class-level -b dump reports.
                    if dumped.len() > 1 {
                        let ids: BTreeSet<&str> = items.iter().map(|i| i.identifier).collect();
                        for d in &dumped {
                            assert!(ids.contains(d.0.as_str()), "{ctx}: superset lacks {}", d.0);
                        }
                    }
                }
            }
        }
        PropKind::Pointer { fixed_type, is_id } => {
            assert_eq!(
                j["fixed_type"].as_str().unwrap_or("AnyType"),
                *fixed_type,
                "{ctx} fixed_type"
            );
            assert_eq!(j["is_id"].as_bool().unwrap_or(false), *is_id, "{ctx} is_id");
        }
        PropKind::Collection { item_type, api } => {
            assert_eq!(
                j["fixed_type"].as_str().unwrap_or("AnyType"),
                *item_type,
                "{ctx} item_type"
            );
            assert_eq!(j["api"].as_str(), *api, "{ctx} api");
        }
        other => panic!("{ctx}: unexpected kind {other:?}"),
    }
}

/// The raw dump of an operator (Blender namespaces and `spec/frap/ops/`).
pub fn op_dump(idname: &str) -> J {
    let ns = idname.split('.').next().unwrap();
    let files = if ns == "frap" {
        frap_spec_files()
    } else {
        vec![format!("blender-5.2/ops/{ns}.json")]
    };
    files
        .iter()
        .flat_map(|f| load(f)["operators"].as_array().unwrap().clone())
        .find(|o| o["idname"] == idname)
        .unwrap_or_else(|| panic!("{idname} not dumped"))
}

/// `spec/frap/ops/*.json` (one file per frap operator since M2 W0, plus `frap.json`), sorted.
pub fn frap_spec_files() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(spec_dir().join("frap/ops"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .map(|n| format!("frap/ops/{n}"))
        .collect();
    v.sort();
    v
}

/// Checks an operator spec against its dump (props, options, macro steps, poll matrix).
pub fn check_op(s: &OperatorSpec) {
    let j = op_dump(s.idname);
    let ctx = s.idname;
    assert_eq!(j["bl_idname"], s.bl_idname, "{ctx}");
    assert_eq!(j["label"], s.label, "{ctx}");
    assert_eq!(j["description"], s.description, "{ctx}");
    let mut opts: Vec<&str> = j["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    opts.sort_unstable();
    assert_eq!(opts, s.options.names(), "{ctx} options");
    let steps: Vec<&str> = j["macro_steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(steps, s.macro_steps.to_vec(), "{ctx} macro_steps");
    let jp = j["props"].as_array().unwrap();
    assert!(s.props.len() >= jp.len(), "{ctx}: props missing");
    for (p, jp) in s.props.iter().zip(jp) {
        check_prop(ctx, p, jp);
    }
    for extra in &s.props[jp.len()..] {
        assert!(
            extra.identifier.starts_with("frap_"),
            "{ctx}: extra prop {}",
            extra.identifier
        );
    }
    match (&j["poll"], &s.poll_matrix) {
        (J::Null, None) => {}
        (J::Object(m), Some(pm)) => {
            let cell = |c: PollCell| match c {
                PollCell::Y => "Y",
                PollCell::N => "N",
                PollCell::Raised => "X",
            };
            for (k, ctxn) in ["A", "B", "C", "D", "E"].iter().enumerate() {
                let want = |key: String| {
                    let v = m[&key].as_str().unwrap();
                    if v.starts_with("X") { "X" } else { v }
                };
                assert_eq!(
                    cell(pm.plain[k]),
                    want(ctxn.to_string()),
                    "{ctx} poll {ctxn}"
                );
                assert_eq!(
                    cell(pm.view3d[k]),
                    want(format!("{ctxn}V")),
                    "{ctx} poll {ctxn}V"
                );
            }
        }
        other => panic!("{ctx}: poll matrix mismatch {other:?}"),
    }
}

/// Every operator idname of the dumps and of `spec/frap/ops/`.
pub fn all_dumped_idnames() -> BTreeSet<String> {
    let meta = load("blender-5.2/ops/_meta.json");
    let mut out = BTreeSet::new();
    for ns in meta["namespaces"].as_array().unwrap() {
        let d = load(&format!("blender-5.2/ops/{}.json", ns.as_str().unwrap()));
        for o in d["operators"].as_array().unwrap() {
            out.insert(o["idname"].as_str().unwrap().to_owned());
        }
    }
    for f in frap_spec_files() {
        for o in load(&f)["operators"].as_array().unwrap() {
            out.insert(o["idname"].as_str().unwrap().to_owned());
        }
    }
    out
}
