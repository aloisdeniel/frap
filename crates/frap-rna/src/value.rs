// SPDX-License-Identifier: GPL-3.0-or-later
//! Values and coercion (ARCHITECTURE §9.2, CONTRACT frozen at M1 W0, amended by RFC 0001
//! F-VAL-1..4).
//!
//! `coerce` is the only place where Blender's assignment rules live. Messages are Blender's
//! (verified on 5.2.4, see `crates/frap-rna/tests/coerce.rs`); the caller adds the context
//! prefix through [`RnaError::into_frap`].

use std::collections::BTreeSet;

use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

pub use frap_math::EulerOrder;

use crate::def::{ArrayShape, Defaults, EnumDefault, PropDef, PropKind, Subtype};
use crate::error::{PyExc, RnaError, RnaErrorKind};
use crate::id::{IdAny, IdKind, IdRef};
use crate::ptr::RnaPtr;

/// Untyped value produced by front-ends (parsers never see the scene).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Input {
    /// `None`
    None,
    /// `True`/`False`
    Bool(bool),
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// Quoted string.
    Str(String),
    /// Short-form unquoted word: valid only for String/Enum/EnumSet targets.
    Bare(SmolStr),
    /// `(a, b)`
    Tuple(Vec<Input>),
    /// `[a, b]`
    List(Vec<Input>),
    /// `{a, b}`
    Set(Vec<Input>),
    /// `{"k": v}`
    Dict(IndexMap<String, Input>),
    /// `mathutils` value: Vector, Euler(order), Quaternion, Matrix, Color.
    Math(MathValue),
    /// Evaluated path expression (an ID or struct).
    Ptr(RnaPtr),
}

/// A `mathutils` value built by the executor from `Vector(...)`, `Euler(...)`, … (§11.3).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MathValue {
    /// `Vector((x, y, z))`
    Vector(Vec<f64>),
    /// `Euler((x, y, z), 'XYZ')`
    Euler([f64; 3], EulerOrder),
    /// `Quaternion((w, x, y, z))`
    Quaternion([f64; 4]),
    /// `Matrix(rows)`
    Matrix(Vec<Vec<f64>>),
    /// `Color((r, g, b))`
    Color(Vec<f64>),
}

impl MathValue {
    /// Flat row-major scalars.
    pub fn flat(&self) -> Vec<f64> {
        match self {
            MathValue::Vector(v) | MathValue::Color(v) => v.clone(),
            MathValue::Euler(e, _) => e.to_vec(),
            MathValue::Quaternion(q) => q.to_vec(),
            MathValue::Matrix(rows) => rows.iter().flatten().copied().collect(),
        }
    }
}

impl Input {
    /// Python type name, as used in Blender's error messages.
    pub fn py_type_name(&self) -> &'static str {
        match self {
            Input::None => "NoneType",
            Input::Bool(_) => "bool",
            Input::Int(_) => "int",
            Input::Float(_) => "float",
            Input::Str(_) | Input::Bare(_) => "str",
            Input::Tuple(_) => "tuple",
            Input::List(_) => "list",
            Input::Set(_) => "set",
            Input::Dict(_) => "dict",
            Input::Math(MathValue::Vector(_)) => "Vector",
            Input::Math(MathValue::Euler(..)) => "Euler",
            Input::Math(MathValue::Quaternion(_)) => "Quaternion",
            Input::Math(MathValue::Matrix(_)) => "Matrix",
            Input::Math(MathValue::Color(_)) => "Color",
            Input::Ptr(p) => p.srna.identifier,
        }
    }
    fn as_number(&self) -> Option<f64> {
        match self {
            Input::Bool(b) => Some(*b as i64 as f64),
            Input::Int(i) => Some(*i as f64),
            Input::Float(f) => Some(*f),
            _ => None,
        }
    }
}

/// Math type of a float array (drives `rna.math` in JSON, §13.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MathTag {
    /// Plain `bpy_prop_array`.
    None,
    /// `mathutils.Vector`
    Vector,
    /// `mathutils.Euler` with its order (from the owner's `rotation_mode`).
    Euler(EulerOrder),
    /// `mathutils.Quaternion`
    Quaternion,
    /// Axis-angle array.
    AxisAngle,
    /// `mathutils.Matrix`
    Matrix,
    /// `mathutils.Color`
    Color,
}

impl MathTag {
    /// Blender's math wrapper for a float array of this subtype.
    pub fn for_subtype(s: Subtype) -> MathTag {
        match s {
            Subtype::Translation
            | Subtype::Direction
            | Subtype::Velocity
            | Subtype::Acceleration
            | Subtype::Xyz
            | Subtype::XyzLength
            | Subtype::Coordinates => MathTag::Vector,
            Subtype::Euler => MathTag::Euler(EulerOrder::XYZ),
            Subtype::Quaternion => MathTag::Quaternion,
            Subtype::AxisAngle => MathTag::AxisAngle,
            Subtype::Matrix => MathTag::Matrix,
            Subtype::Color | Subtype::ColorGamma => MathTag::Color,
            _ => MathTag::None,
        }
    }
    /// Name used in `rna.math`.
    pub fn as_str(self) -> &'static str {
        match self {
            MathTag::None => "Array",
            MathTag::Vector => "Vector",
            MathTag::Euler(_) => "Euler",
            MathTag::Quaternion => "Quaternion",
            MathTag::AxisAngle => "AxisAngle",
            MathTag::Matrix => "Matrix",
            MathTag::Color => "Color",
        }
    }
}

/// Typed RNA value.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// None
    None,
    /// BOOLEAN
    Bool(bool),
    /// INT
    Int(i64),
    /// FLOAT (f32 storage)
    Float(f32),
    /// FLOAT (f64 storage)
    Double(f64),
    /// STRING
    Str(SmolStr),
    /// ENUM
    Enum(SmolStr),
    /// ENUM flag
    EnumSet(BTreeSet<SmolStr>),
    /// BOOLEAN array
    BoolArray(SmallVec<[bool; 4]>),
    /// INT array
    IntArray(SmallVec<[i64; 4]>),
    /// FLOAT array
    FloatArray {
        /// Row-major data.
        data: SmallVec<[f32; 16]>,
        /// Shape.
        shape: ArrayShape,
        /// Math wrapper.
        tag: MathTag,
    },
    /// ID pointer (`{"$ref": …}`; `None` = null). RFC 0001 F-VAL-4.
    Id(Option<IdAny>),
    /// Pointer to a non-ID struct (boxed: RFC 0001 F-VAL-1).
    Ptr(Box<RnaPtr>),
    /// Collection.
    Coll(Box<RnaPtr>),
    /// Tuple of values (multiple returns, `print`).
    Tuple(Vec<Value>),
    /// Operator collection props (`files=[{"name": "a.glb"}]`). RFC 0001 F-VAL-3.
    Structs(Vec<IndexMap<SmolStr, Value>>),
}

impl<K: IdKind> From<IdRef<K>> for Value {
    fn from(r: IdRef<K>) -> Value {
        Value::Id(Some(r.any()))
    }
}
impl<K: IdKind> From<Option<IdRef<K>>> for Value {
    fn from(r: Option<IdRef<K>>) -> Value {
        Value::Id(r.map(|r| r.any()))
    }
}
impl From<IdAny> for Value {
    fn from(a: IdAny) -> Value {
        Value::Id(Some(a))
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Value {
        Value::Int(i)
    }
}
impl From<i32> for Value {
    fn from(i: i32) -> Value {
        Value::Int(i as i64)
    }
}
impl From<usize> for Value {
    fn from(i: usize) -> Value {
        Value::Int(i as i64)
    }
}
impl From<f32> for Value {
    fn from(f: f32) -> Value {
        Value::Float(f)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.into())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s.into())
    }
}
impl From<SmolStr> for Value {
    fn from(s: SmolStr) -> Value {
        Value::Str(s)
    }
}

impl Value {
    /// `[f32; 3]` view of a 3-float array.
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match self {
            Value::FloatArray { data, .. } if data.len() == 3 => Some([data[0], data[1], data[2]]),
            _ => None,
        }
    }
    /// Float view.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Double(f) => Some(*f as f32),
            Value::Int(i) => Some(*i as f32),
            _ => None,
        }
    }
    /// Bool view.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    /// String/enum view.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Enum(s) => Some(s),
            _ => None,
        }
    }

    /// Default value of a property.
    pub fn default_for(def: &PropDef) -> Value {
        match &def.kind {
            PropKind::Bool { default } => match default {
                Defaults::Scalar(b) => Value::Bool(*b),
                Defaults::Array(a) => Value::BoolArray(a.iter().copied().collect()),
            },
            PropKind::Int { default, .. } => match default {
                Defaults::Scalar(i) => Value::Int(*i),
                Defaults::Array(a) => Value::IntArray(a.iter().copied().collect()),
            },
            PropKind::Float { default, .. } => match default {
                Defaults::Scalar(f) => Value::Float(*f as f32),
                Defaults::Array(a) => Value::FloatArray {
                    data: a.iter().map(|v| *v as f32).collect(),
                    shape: def.array,
                    tag: MathTag::for_subtype(def.subtype),
                },
            },
            PropKind::String { default, .. } => Value::Str((*default).into()),
            PropKind::Enum { default, .. } => match default {
                EnumDefault::Item(s) => Value::Enum((*s).into()),
                EnumDefault::Flags(f) => {
                    Value::EnumSet(f.iter().map(|s| SmolStr::from(*s)).collect())
                }
            },
            PropKind::Pointer { .. } | PropKind::Collection { .. } => Value::None,
        }
    }
}

/// Where a value is being assigned: Blender's message prefix and qualifier differ
/// (RFC 0001 F-VAL-2, verified wording).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoerceCx {
    /// `bpy_struct: item.attr = val: Object.name expected a string type, not int`
    RnaSet {
        /// Struct identifier ("Object").
        owner: &'static str,
    },
    /// `Converting py args to operator properties: MESH_OT_primitive_cube_add.size expected …`
    OpArgs {
        /// Operator `bl_idname` ("MESH_OT_primitive_cube_add").
        owner: &'static str,
    },
}

impl CoerceCx {
    /// Owner used in qualified messages.
    pub fn owner(&self) -> &'static str {
        match *self {
            CoerceCx::RnaSet { owner } | CoerceCx::OpArgs { owner } => owner,
        }
    }
    /// Blender's message prefix for this site. `coerce` returns unprefixed messages; callers
    /// convert with `RnaError::into_frap(cx.prefix(), …)`.
    pub fn prefix(&self) -> &'static str {
        match self {
            CoerceCx::RnaSet { .. } => "bpy_struct: item.attr = val: ",
            CoerceCx::OpArgs { .. } => "Converting py args to operator properties: ",
        }
    }
}

/// Coercion result.
#[derive(Clone, Debug, PartialEq)]
pub struct Coerced {
    /// Typed value.
    pub value: Value,
    /// True when a number was clamped to the hard range (frap adds an INFO report).
    pub clamped: bool,
}

fn type_err(msg: String) -> RnaError {
    RnaError::new(RnaErrorKind::Type, PyExc::TypeError, msg)
}

fn sequence_items(input: &Input) -> Option<Vec<Input>> {
    match input {
        Input::Tuple(v) | Input::List(v) => Some(v.clone()),
        // Python strings are sequences of 1-char strings (Blender reports their length).
        Input::Str(s) => Some(s.chars().map(|c| Input::Str(c.to_string())).collect()),
        Input::Bare(s) => Some(s.chars().map(|c| Input::Str(c.to_string())).collect()),
        Input::Math(m) => Some(m.flat().into_iter().map(Input::Float).collect()),
        _ => None,
    }
}

fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Python `repr` of a float (`5000.0`, `0.5`, `1e+16`, `inf`), for frap-authored messages.
pub fn py_float_repr(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let a = v.abs();
    if a == 0.0 || (1e-4..1e16).contains(&a) {
        // Shortest round-trip digits; Python always shows a fractional part.
        let s = format!("{v:?}");
        return if s.contains('.') { s } else { format!("{s}.0") };
    }
    // Python: `1e+16`, `1.5e-05` (exponent signed, at least two digits).
    let s = format!("{v:e}");
    let (m, e) = s.split_once('e').expect("invariant: {:e} has an exponent");
    let e: i32 = e.parse().expect("invariant: integer exponent");
    let sign = if e < 0 { '-' } else { '+' };
    format!("{m}e{sign}{:02}", e.unsigned_abs())
}

/// The input as frap echoes it in reports (`-5`, `5000.0`, `(1, 2.5, 0)`; non-numbers by type).
pub fn describe_input(i: &Input) -> String {
    match i {
        Input::Bool(b) => if *b { "True" } else { "False" }.into(),
        Input::Int(v) => v.to_string(),
        Input::Float(v) => py_float_repr(*v),
        Input::Tuple(v) | Input::List(v) => {
            let items: Vec<String> = v.iter().map(describe_input).collect();
            if v.len() == 1 {
                format!("({},)", items[0])
            } else {
                format!("({})", items.join(", "))
            }
        }
        Input::Math(m) => format!(
            "({})",
            m.flat()
                .into_iter()
                .map(py_float_repr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.py_type_name().to_owned(),
    }
}

/// Text of frap's INFO report for a value [`coerce`] clamped (deviation D-03, ARCHITECTURE
/// §9.2), the same for assignments and operator arguments:
///
/// - scalar below the hard minimum: `frap: size=-5 clamped to hard min 0.0`
/// - scalar above the hard maximum: `frap: size=10000000000000.0 clamped to hard max 999999995904.0`
/// - array (any element out of range): `frap: location=(1e+16, 0, 0) clamped to hard range
///   [-999999995904.0, 999999995904.0]`
///
/// `input` is the value as written (before coercion). Bounds use Python's `repr`.
pub fn clamp_report(def: &PropDef, input: &Input) -> String {
    let shown = describe_input(input);
    let (lo, hi, lo_s, hi_s) = match &def.kind {
        PropKind::Float { hard, .. } => {
            (hard.0, hard.1, py_float_repr(hard.0), py_float_repr(hard.1))
        }
        PropKind::Int { hard, .. } => (
            hard.0 as f64,
            hard.1 as f64,
            hard.0.to_string(),
            hard.1.to_string(),
        ),
        _ => (
            f64::NEG_INFINITY,
            f64::INFINITY,
            "-inf".into(),
            "inf".into(),
        ),
    };
    let bound = match (matches!(def.array, ArrayShape::Scalar), input.as_number()) {
        (true, Some(v)) if v < lo => format!("hard min {lo_s}"),
        (true, Some(v)) if v > hi => format!("hard max {hi_s}"),
        _ => format!("hard range [{lo_s}, {hi_s}]"),
    };
    format!("frap: {}={shown} clamped to {bound}", def.identifier)
}

/// Coerces an untyped input against a property definition (Blender's assignment rules).
pub fn coerce(def: &PropDef, input: Input, cx: &CoerceCx) -> Result<Coerced, RnaError> {
    let qual = format!("{}.{}", cx.owner(), def.identifier);
    let is_array = !matches!(def.array, ArrayShape::Scalar);
    match &def.kind {
        PropKind::Float { hard, .. } if is_array => {
            let n = def.array_len();
            let Some(items) = sequence_items(&input) else {
                // Blender: "bpy_struct: item.attr = val: " with nothing after the prefix.
                return Err(type_err(String::new()).with_hint(format!(
                    "{} expects a sequence of {n} numbers, e.g. (0, 0, 1)",
                    def.identifier
                )));
            };
            if items.len() != n {
                return Err(RnaError::new(
                    RnaErrorKind::Length,
                    PyExc::ValueError,
                    format!(
                        "sequences of dimension 0 should contain {n} items, not {}",
                        items.len()
                    ),
                ));
            }
            let mut data = SmallVec::new();
            let mut clamped = false;
            for it in &items {
                let Some(v) = it.as_number() else {
                    return Err(type_err(format!(
                        "expected sequence items of type float, not {}",
                        it.py_type_name()
                    )));
                };
                let c = v.clamp(hard.0, hard.1);
                clamped |= c != v;
                data.push(c as f32);
            }
            Ok(Coerced {
                value: Value::FloatArray {
                    data,
                    shape: def.array,
                    tag: MathTag::for_subtype(def.subtype),
                },
                clamped,
            })
        }
        PropKind::Float { hard, storage, .. } => {
            let Some(v) = input.as_number() else {
                return Err(type_err(format!(
                    "{qual} expected a float type, not {}",
                    input.py_type_name()
                )));
            };
            let c = v.clamp(hard.0, hard.1);
            let value = match storage {
                crate::def::FloatStorage::F32 => Value::Float(c as f32),
                crate::def::FloatStorage::F64 => Value::Double(c),
            };
            Ok(Coerced {
                value,
                clamped: c != v,
            })
        }
        PropKind::Int { hard, .. } if !is_array => match input {
            Input::Int(_) | Input::Bool(_) => {
                let v = match input {
                    Input::Int(i) => i,
                    Input::Bool(b) => b as i64,
                    _ => unreachable!(),
                };
                let c = v.clamp(hard.0, hard.1);
                Ok(Coerced {
                    value: Value::Int(c),
                    clamped: c != v,
                })
            }
            other => Err(type_err(format!(
                "{qual} expected an int type, not {}",
                other.py_type_name()
            ))),
        },
        PropKind::Bool { .. } if !is_array => match input {
            Input::Bool(b) => Ok(Coerced {
                value: Value::Bool(b),
                clamped: false,
            }),
            Input::Int(i @ (0 | 1)) => Ok(Coerced {
                value: Value::Bool(i == 1),
                clamped: false,
            }),
            other => Err(type_err(format!(
                "{qual} expected True/False or 0/1, not {}",
                other.py_type_name()
            ))),
        },
        PropKind::String { max_bytes, .. } => match input {
            Input::Str(s) => {
                let max = if *max_bytes == 0 {
                    usize::MAX
                } else {
                    *max_bytes as usize - 1
                };
                Ok(Coerced {
                    value: Value::Str(truncate_utf8(&s, max).into()),
                    clamped: false,
                })
            }
            Input::Bare(s) => Ok(Coerced {
                value: Value::Str(s),
                clamped: false,
            }),
            other => Err(type_err(format!(
                "{qual} expected a string type, not {}",
                other.py_type_name()
            ))),
        },
        PropKind::Enum { items, .. } => {
            let s: SmolStr = match input {
                Input::Str(s) => s.into(),
                Input::Bare(s) => s,
                other => {
                    return Err(type_err(format!(
                        "expected a string enum, not {}",
                        other.py_type_name()
                    )));
                }
            };
            if items.iter().any(|i| i.identifier == s) {
                Ok(Coerced {
                    value: Value::Enum(s),
                    clamped: false,
                })
            } else {
                let list = items
                    .iter()
                    .map(|i| format!("'{}'", i.identifier))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut e = RnaError::new(
                    RnaErrorKind::Enum,
                    PyExc::TypeError,
                    format!("enum \"{s}\" not found in ({list})"),
                );
                e.detail.insert(
                    "valid".into(),
                    items
                        .iter()
                        .map(|i| serde_json::Value::from(i.identifier))
                        .collect::<Vec<_>>()
                        .into(),
                );
                Err(e)
            }
        }
        _ => Err(RnaError::new(
            RnaErrorKind::Other,
            PyExc::NotImplementedError,
            format!(
                "frap M0: assigning {} properties is not implemented",
                def.type_str()
            ),
        )),
    }
}

#[cfg(test)]
mod repr_tests {
    use super::py_float_repr;

    #[test]
    fn float_repr_matches_python() {
        // Expected strings are CPython's `repr(float)`.
        for (v, py) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (5000.0, "5000.0"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (1.5e-5, "1.5e-05"),
            (1e13, "10000000000000.0"),
            (1e16, "1e+16"),
            (999999995904.0, "999999995904.0"),
            (3.4028234663852886e38, "3.4028234663852886e+38"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
        ] {
            assert_eq!(py_float_repr(v), py, "{v:?}");
        }
    }
}
