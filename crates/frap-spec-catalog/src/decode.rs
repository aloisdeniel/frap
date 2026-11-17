// SPDX-License-Identifier: GPL-3.0-or-later
//! Lazy decoding of the catalog blob into leaked `&'static` tables (ARCHITECTURE §4.2, WP 1.1).
//!
//! The blob (`generated/catalog.json.deflate`, written by `cargo xtask codegen`) is a JSON array
//! of operators in the normalised schema of `xtask/src/codegen/gen_ops.rs` (`prop_json`):
//!
//! ```text
//! op:   {idname, bl_idname, label, description, options: [..], props: [prop], macro_steps: [..],
//!        poll: null | {plain: ["Y"|"N"|"X"; 5], view3d: [..; 5]}}
//! prop: {id, name, desc, type, subtype, unit, array: null | [n] | [r, c] | "dynamic", flags,
//!        default, hard, soft, precision, storage, max_bytes, items: [[id, name, desc, value]],
//!        dynamic, fixed_type, is_id, item_type, api, deprecated?}
//! ```
//!
//! Decoding happens once per process (`OnceLock`); every string and table is leaked, so the
//! returned `&'static OperatorSpec` addresses are stable for the process, as for the generated
//! statics of `frap-spec-ops-*`.

use std::sync::OnceLock;

use frap_rna::{
    ArrayShape, Defaults, Deprecation, EnumDefault, EnumItem, FloatStorage, OpOptions,
    OperatorSpec, PollCell, PollMatrix, PropDef, PropFlags, PropKind, Subtype, Unit,
};
use serde_json::Value as J;

use crate::generated::CATALOG_DEFLATE;

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

fn st(j: &J) -> &'static str {
    leak_str(j.as_str().unwrap_or(""))
}

fn f(j: &J) -> f64 {
    match j {
        J::Number(n) => n.as_f64().unwrap_or(0.0),
        J::Object(o) => match o.get("$float").and_then(J::as_str) {
            Some("inf") => f64::INFINITY,
            Some("-inf") => f64::NEG_INFINITY,
            _ => f64::NAN,
        },
        _ => 0.0,
    }
}

fn pair<T>(j: &J, conv: fn(&J) -> T) -> (T, T) {
    (conv(&j[0]), conv(&j[1]))
}

fn i(j: &J) -> i64 {
    j.as_i64().unwrap_or(0)
}

fn defaults<T: 'static>(j: &J, conv: fn(&J) -> T) -> Defaults<T> {
    match j {
        J::Array(a) => Defaults::Array(leak(a.iter().map(conv).collect())),
        x => Defaults::Scalar(conv(x)),
    }
}

fn prop(p: &J) -> Result<PropDef, String> {
    let id = p["id"].as_str().unwrap_or("");
    let mut flags = PropFlags::empty();
    for name in p["flags"].as_array().into_iter().flatten() {
        let name = name.as_str().unwrap_or("");
        flags |= PropFlags::from_name(name).ok_or_else(|| format!("{id}: unknown flag {name}"))?;
    }
    let kind = match p["type"].as_str().unwrap_or("") {
        "BOOLEAN" => PropKind::Bool {
            default: defaults(&p["default"], |j| j.as_bool().unwrap_or(false)),
        },
        "INT" => PropKind::Int {
            default: defaults(&p["default"], i),
            hard: pair(&p["hard"], i),
            soft: pair(&p["soft"], i),
        },
        "FLOAT" => PropKind::Float {
            default: defaults(&p["default"], f),
            hard: pair(&p["hard"], f),
            soft: pair(&p["soft"], f),
            precision: p["precision"].as_u64().unwrap_or(3) as u8,
            storage: if p["storage"] == "F64" {
                FloatStorage::F64
            } else {
                FloatStorage::F32
            },
        },
        "STRING" => PropKind::String {
            default: st(&p["default"]),
            max_bytes: p["max_bytes"].as_u64().unwrap_or(0) as u32,
        },
        "ENUM" => PropKind::Enum {
            items: leak(
                p["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|it| EnumItem {
                        identifier: st(&it[0]),
                        name: st(&it[1]),
                        description: st(&it[2]),
                        value: it[3].as_i64().unwrap_or(0) as i32,
                    })
                    .collect(),
            ),
            default: match &p["default"] {
                J::Array(a) => EnumDefault::Flags(leak(a.iter().map(st).collect())),
                x => EnumDefault::Item(st(x)),
            },
            dynamic: p["dynamic"].as_str().map(leak_str),
        },
        "POINTER" => PropKind::Pointer {
            fixed_type: st(&p["fixed_type"]),
            is_id: p["is_id"].as_bool().unwrap_or(false),
        },
        "COLLECTION" => PropKind::Collection {
            item_type: st(&p["item_type"]),
            api: p["api"].as_str().map(leak_str),
        },
        other => return Err(format!("{id}: unknown type {other}")),
    };
    let array = match &p["array"] {
        J::Null => ArrayShape::Scalar,
        J::String(_) => ArrayShape::Dynamic,
        J::Array(a) if a.len() == 1 => ArrayShape::Fixed(i(&a[0]) as u16),
        J::Array(a) if a.len() == 2 => ArrayShape::Matrix(i(&a[0]) as u16, i(&a[1]) as u16),
        other => return Err(format!("{id}: bad array shape {other}")),
    };
    let ver = |j: &J| -> [u16; 3] { [0, 1, 2].map(|k| j[k].as_u64().unwrap_or(0) as u16) };
    let deprecated = p.get("deprecated").map(|d| {
        &*Box::leak(Box::new(Deprecation {
            note: st(&d["note"]),
            version: ver(&d["version"]),
            removal_version: ver(&d["removal_version"]),
        }))
    });
    let subtype = p["subtype"].as_str().unwrap_or("NONE");
    let unit = p["unit"].as_str().unwrap_or("NONE");
    Ok(PropDef {
        identifier: leak_str(id),
        name: st(&p["name"]),
        description: st(&p["desc"]),
        kind,
        subtype: Subtype::from_blender(subtype)
            .ok_or_else(|| format!("{id}: unknown subtype {subtype}"))?,
        unit: Unit::from_blender(unit).ok_or_else(|| format!("{id}: unknown unit {unit}"))?,
        array,
        flags,
        deprecated,
    })
}

fn cells(j: &J) -> [PollCell; 5] {
    [0, 1, 2, 3, 4].map(|k| match j[k].as_str() {
        Some("Y") => PollCell::Y,
        Some("N") => PollCell::N,
        _ => PollCell::Raised,
    })
}

fn op(o: &J) -> Result<OperatorSpec, String> {
    let idname = o["idname"].as_str().unwrap_or("");
    let mut options = OpOptions::empty();
    for name in o["options"].as_array().into_iter().flatten() {
        let name = name.as_str().unwrap_or("");
        options |=
            OpOptions::from_name(name).ok_or_else(|| format!("{idname}: unknown option {name}"))?;
    }
    let props = o["props"]
        .as_array()
        .into_iter()
        .flatten()
        .map(prop)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{idname}.{e}"))?;
    Ok(OperatorSpec {
        idname: leak_str(idname),
        bl_idname: st(&o["bl_idname"]),
        label: st(&o["label"]),
        description: st(&o["description"]),
        options,
        props: leak(props),
        macro_steps: leak(
            o["macro_steps"]
                .as_array()
                .into_iter()
                .flatten()
                .map(st)
                .collect(),
        ),
        poll_matrix: match &o["poll"] {
            J::Object(m) => Some(PollMatrix {
                plain: cells(&m["plain"]),
                view3d: cells(&m["view3d"]),
            }),
            _ => None,
        },
    })
}

/// Decodes the blob. Panics only on a blob that `cargo xtask codegen` did not write (the
/// catalog tests decode every entry).
fn decode() -> Vec<&'static OperatorSpec> {
    use std::io::Read as _;
    let mut text = Vec::new();
    flate2::read::DeflateDecoder::new(CATALOG_DEFLATE)
        .read_to_end(&mut text)
        .expect("invariant: catalog.json.deflate is valid deflate (cargo xtask codegen)");
    let ops: J = serde_json::from_slice(&text)
        .expect("invariant: catalog.json.deflate holds JSON (cargo xtask codegen)");
    ops.as_array()
        .expect("invariant: the catalog is a JSON array")
        .iter()
        .map(|o| &*Box::leak(Box::new(op(o).unwrap_or_else(|e| panic!("catalog: {e}")))))
        .collect()
}

/// Every catalog operator (each dumped operator without a typed props struct), sorted by
/// idname, decoded once per process. The `&'static` addresses are stable for the process.
pub fn catalog() -> &'static [&'static OperatorSpec] {
    static CATALOG: OnceLock<Vec<&'static OperatorSpec>> = OnceLock::new();
    CATALOG.get_or_init(decode)
}
