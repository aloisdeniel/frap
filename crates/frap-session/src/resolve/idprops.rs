// SPDX-License-Identifier: GPL-3.0-or-later
//! ID properties (custom properties, `ID["key"]`) on the [`frap_rna::IdHeader::custom`]
//! storage: subscript reads and writes, `keys()`, `values()`, `items()` and `get()` of an ID
//! (Blender's `bpy_struct` mapping protocol, `bpy_rna.cc` `pyrna_struct_subscript` and
//! `idprop_py_api.cc`). glTF `extras` are these properties (WP 3.7).
//!
//! Python's conversion rules (verified on Blender 5.2.4): an `int` must fit 32 bits
//! (`OverflowError: Python int too large to convert to C int`), a `float` is a double, a
//! `bool` stays a bool, a sequence of numbers is an int array (all ints) or a double array, a
//! sequence of bools a bool array, a sequence of strings or of dicts a list, a dict a group;
//! any other sequence is a `TypeError`. Keys are strings of at most 63 bytes.
//!
//! Reads answer plain values: an array as a list, a group as a JSON object (Blender's
//! `IDPropertyArray` / `IDPropertyGroup` wrappers have no JSON form; their `to_list()` /
//! `to_dict()` is what an agent wants). frap's `IdProp` has no `None` or ID-pointer kind:
//! assigning `None` or an ID answers `RNA_NOT_IMPLEMENTED` (deviations §2).

use frap_lang::PathSeg;
use frap_rna::{FrapError, IdAny, IdProp, PyExc, RnaPtr, Root, Value};
use frap_scene::HostRead;
use serde_json::Value as J;

use crate::errors::*;

/// `MAX_IDPROP_NAME - 1`: the longest key in bytes.
pub const MAX_KEY_BYTES: usize = 63;

/// The ID a subscript reads the properties of: an ID itself (not a struct below it).
pub fn owner(ptr: &RnaPtr) -> Option<IdAny> {
    match ptr.root {
        Root::Id(id) if ptr.srna.is_id && ptr.path.is_empty() => Some(id),
        _ => None,
    }
}

/// An embedded ID (`material.node_tree`, `scene.collection`) has ID properties in Blender;
/// frap does not store them yet.
pub fn is_embedded_id(ptr: &RnaPtr) -> bool {
    matches!(ptr.root, Root::Embedded { .. }) && ptr.srna.is_id && ptr.path.is_empty()
}

/// `RNA_NOT_IMPLEMENTED` for an ID property frap cannot hold.
pub fn not_supported(what: &str) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        frap_rna::codes::RNA_NOT_IMPLEMENTED,
        format!("frap: {what} is not supported by frap's ID properties yet"),
    )
}

/// The key of a subscript read (`bpy_struct[key]`).
pub fn read_key(seg: &PathSeg) -> Result<&str, FrapError> {
    match seg {
        PathSeg::Key(k) => Ok(k),
        PathSeg::KeyLib(..) => Err(FrapError::new(
            PyExc::TypeError,
            RNA_TYPE,
            "bpy_struct[key]: only strings are allowed as keys of ID properties",
        )),
        _ => Err(FrapError::new(
            PyExc::TypeError,
            RNA_TYPE,
            "bpy_struct[key]: only strings are allowed as keys of ID properties",
        )),
    }
}

/// The key of a subscript assignment (`bpy_struct[key] = value`).
pub fn write_key(seg: &PathSeg) -> Result<&str, FrapError> {
    let k = match seg {
        PathSeg::Key(k) => k.as_str(),
        other => {
            let ty = match other {
                PathSeg::Index(_) => "int",
                PathSeg::KeyLib(..) => "tuple",
                PathSeg::Slice(..) => "slice",
                _ => "object",
            };
            return Err(FrapError::new(
                PyExc::KeyError,
                RNA_KEY_NOT_FOUND,
                format!("'invalid id-property key, expected a string, not a {ty}'"),
            ));
        }
    };
    if k.len() > MAX_KEY_BYTES {
        return Err(FrapError::new(
            PyExc::KeyError,
            RNA_KEY_NOT_FOUND,
            format!("'the length of IDProperty names is limited to {MAX_KEY_BYTES} characters'"),
        ));
    }
    Ok(k)
}

/// `ID[key]`.
pub fn read(host: &dyn HostRead, id: IdAny, key: &str) -> Result<Value, FrapError> {
    host.main()
        .header(id)
        .and_then(|h| h.custom.get(key))
        .map(to_value)
        .ok_or_else(|| {
            FrapError::new(
                PyExc::KeyError,
                RNA_KEY_NOT_FOUND,
                format!(
                    "'bpy_struct[key]: key \"{}\" not found'",
                    key.replace('"', "\\\"")
                ),
            )
        })
}

/// The ID's properties in key order.
pub fn all(host: &dyn HostRead, id: IdAny) -> Vec<(String, Value)> {
    host.main()
        .header(id)
        .map(|h| {
            h.custom
                .iter()
                .map(|(k, v)| (k.to_string(), to_value(v)))
                .collect()
        })
        .unwrap_or_default()
}

/// An ID property as a value (arrays and lists as tuples, groups as JSON objects).
pub fn to_value(p: &IdProp) -> Value {
    match p {
        IdProp::Int(i) => Value::Int(*i),
        IdProp::Float(f) => Value::Double(*f),
        IdProp::Bool(b) => Value::Bool(*b),
        IdProp::Str(s) => Value::Str(s.clone()),
        IdProp::IntArray(v) => Value::Tuple(v.iter().map(|i| Value::Int(*i)).collect()),
        IdProp::FloatArray(v) => Value::Tuple(v.iter().map(|f| Value::Double(*f)).collect()),
        IdProp::List(v) => Value::Tuple(v.iter().map(to_value).collect()),
        IdProp::Group(_) => Value::Json(Box::new(to_json(p))),
        _ => Value::None,
    }
}

/// An ID property as JSON (`to_dict()` / `to_list()`).
pub fn to_json(p: &IdProp) -> J {
    let num = |f: f64| serde_json::Number::from_f64(f).map_or(J::Null, J::Number);
    match p {
        IdProp::Int(i) => J::from(*i),
        IdProp::Float(f) => num(*f),
        IdProp::Bool(b) => J::Bool(*b),
        IdProp::Str(s) => J::from(s.as_str()),
        IdProp::IntArray(v) => J::Array(v.iter().map(|i| J::from(*i)).collect()),
        IdProp::FloatArray(v) => J::Array(v.iter().map(|f| num(*f)).collect()),
        IdProp::List(v) => J::Array(v.iter().map(to_json).collect()),
        IdProp::Group(g) => J::Object(g.iter().map(|(k, v)| (k.to_string(), to_json(v))).collect()),
        _ => J::Null,
    }
}

/// A JSON member read below a group (`ID["group"]["key"]`).
pub fn json_value(j: &J) -> Value {
    match j {
        J::Null => Value::None,
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Double(n.as_f64().unwrap_or(0.0)),
        },
        J::String(s) => Value::Str(s.as_str().into()),
        J::Array(a) => Value::Tuple(a.iter().map(json_value).collect()),
        J::Object(_) => Value::Json(Box::new(j.clone())),
    }
}
