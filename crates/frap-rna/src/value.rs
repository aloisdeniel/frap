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

use crate::def::{
    ArrayShape, Defaults, EnumDefault, EnumItem, FloatStorage, PropDef, PropFlags, PropKind,
    Subtype,
};
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

fn length_err(msg: String) -> RnaError {
    RnaError::new(RnaErrorKind::Length, PyExc::ValueError, msg)
}

/// Hint for a short-form unquoted word given to a non-string property (§9.2).
const BARE_HINT: &str = "quote it or pass a number";

/// `STRINGIFY(INT_MIN), STRINGIFY(INT_MAX)` as Blender prints them.
const INT_RANGE: &str = "((-2147483647 -1), 2147483647)";

/// Python's `PySequence_Fast` view of an input: tuples, lists, sets (iteration order), strings
/// (one item per character), dict keys and `mathutils` values (flattened) are sequences;
/// everything else is not.
fn sequence_items(input: &Input) -> Option<Vec<Input>> {
    match input {
        Input::Tuple(v) | Input::List(v) | Input::Set(v) => Some(v.clone()),
        Input::Str(s) => Some(s.chars().map(|c| Input::Str(c.to_string())).collect()),
        Input::Bare(s) => Some(s.chars().map(|c| Input::Str(c.to_string())).collect()),
        Input::Dict(d) => Some(d.keys().map(|k| Input::Str(k.clone())).collect()),
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

/// Enum items as Blender lists them in errors: `'A', 'B'` (separators without identifier
/// skipped).
fn enum_items_repr(items: &[EnumItem]) -> String {
    items
        .iter()
        .filter(|i| !i.identifier.is_empty())
        .map(|i| format!("'{}'", i.identifier))
        .collect::<Vec<_>>()
        .join(", ")
}

fn enum_valid_detail(items: &[EnumItem]) -> serde_json::Value {
    items
        .iter()
        .filter(|i| !i.identifier.is_empty())
        .map(|i| serde_json::Value::from(i.identifier))
        .collect::<Vec<_>>()
        .into()
}

/// Float clamp as Blender does it: the Python number is converted to `float` first
/// (`const float param = PyFloat_AsDouble(value)`), then clamped to the hard range.
fn clamp_f32(v: f64, hard: (f64, f64)) -> (f32, bool) {
    let f = v as f32;
    let (lo, hi) = (hard.0 as f32, hard.1 as f32);
    let c = if f < lo {
        lo
    } else if f > hi {
        hi
    } else {
        f
    };
    (c, f < lo || f > hi)
}

/// Integer conversion of a scalar INT property (`PyLong_AsLongAndOverflow` + range check +
/// `RNA_property_int_clamp`).
fn coerce_int(qual: &str, hard: (i64, i64), input: &Input) -> Result<(i64, bool), RnaError> {
    let v = match input {
        Input::Int(i) => *i,
        Input::Bool(b) => i64::from(*b),
        other => {
            return Err(type_err(format!(
                "{qual} expected an int type, not {}",
                other.py_type_name()
            )));
        }
    };
    if v > i64::from(i32::MAX) || v < i64::from(i32::MIN) {
        return Err(RnaError::new(
            RnaErrorKind::Type,
            PyExc::ValueError,
            format!("{qual} value not in 'int' range {INT_RANGE}"),
        ));
    }
    let c = v.clamp(hard.0, hard.1);
    Ok((c, c != v))
}

fn is_number(i: &Input) -> bool {
    matches!(i, Input::Bool(_) | Input::Int(_) | Input::Float(_))
}

fn is_int(i: &Input) -> bool {
    matches!(i, Input::Bool(_) | Input::Int(_))
}

/// Generic conversion used for dict members (operator collection props, `files=[{"name": …}]`)
/// where no `PropDef` is at hand.
fn untyped_value(input: Input) -> Value {
    match input {
        Input::None => Value::None,
        Input::Bool(b) => Value::Bool(b),
        Input::Int(i) => Value::Int(i),
        Input::Float(f) => Value::Double(f),
        Input::Str(s) => Value::Str(s.into()),
        Input::Bare(s) => Value::Str(s),
        Input::Tuple(v) | Input::List(v) | Input::Set(v) => {
            Value::Tuple(v.into_iter().map(untyped_value).collect())
        }
        Input::Dict(d) => Value::Structs(vec![
            d.into_iter()
                .map(|(k, v)| (SmolStr::from(k), untyped_value(v)))
                .collect(),
        ]),
        Input::Math(m) => Value::FloatArray {
            data: m.flat().into_iter().map(|x| x as f32).collect(),
            shape: ArrayShape::Dynamic,
            tag: MathTag::None,
        },
        Input::Ptr(p) => match (p.root, p.path.is_empty()) {
            (crate::ptr::Root::Id(id), true) => Value::Id(Some(id)),
            _ => Value::Ptr(Box::new(p)),
        },
    }
}

/// Coerces an untyped input against a property definition: Blender's assignment rules
/// (`pyrna_py_to_prop`, `pyrna_py_to_array`, `bpy_rna.cc` / `bpy_rna_array.cc` of 5.2.4),
/// with Blender's messages. The returned error carries no context prefix; callers add
/// [`CoerceCx::prefix`] with [`RnaError::into_frap`].
///
/// frap extensions (§9.2, documented deviations): enum-flag properties also accept a list or
/// a tuple (and a short-form bare word as a one-item set); `Bare` words are accepted for
/// string and enum targets only, and any other target reports Blender's `str` error plus the
/// hint `quote it or pass a number`. Numbers are clamped to the hard range (silently in
/// Blender; `clamped` lets the caller add the [`clamp_report`] INFO).
pub fn coerce(def: &PropDef, input: Input, cx: &CoerceCx) -> Result<Coerced, RnaError> {
    let bare = matches!(input, Input::Bare(_));
    let string_target = matches!(def.kind, PropKind::String { .. } | PropKind::Enum { .. });
    let r = coerce_inner(def, input, cx);
    if bare && !string_target {
        return r.map_err(|e| e.with_hint(BARE_HINT));
    }
    r
}

/// True for the scalar `default_value` of a standard node socket (`NodeSocketFloatFactor`,
/// `NodeSocketIntPercentage`, …) assigned from Python: `rna_NodeSocketStandard_float_range`
/// and `rna_NodeSocketStandard_int_range` make its hard range unbounded (from 0 for unsigned
/// subtypes) and use the socket declaration's range as the soft range only (verified on
/// 5.2.4: Principled `inputs['Metallic'].default_value = 2` reads back 2.0, `Roughness = -0.5`
/// reads back -0.5). The dump records the declaration range as the static hard range.
fn socket_standard_value(def: &PropDef, cx: &CoerceCx) -> bool {
    matches!(cx, CoerceCx::RnaSet { owner } if owner.starts_with("NodeSocket"))
        && def.identifier == "default_value"
}

fn coerce_inner(def: &PropDef, input: Input, cx: &CoerceCx) -> Result<Coerced, RnaError> {
    let qual = format!("{}.{}", cx.owner(), def.identifier);
    let done = |value: Value| {
        Ok(Coerced {
            value,
            clamped: false,
        })
    };
    if !matches!(def.array, ArrayShape::Scalar) {
        return coerce_array(def, input, &qual);
    }
    match &def.kind {
        PropKind::Bool { .. } => match input {
            Input::Bool(b) => done(Value::Bool(b)),
            Input::Int(i @ (0 | 1)) => done(Value::Bool(i == 1)),
            other => Err(type_err(format!(
                "{qual} expected True/False or 0/1, not {}",
                other.py_type_name()
            ))),
        },
        PropKind::Int { hard, .. } => {
            let hard = match socket_standard_value(def, cx) {
                true if def.subtype == Subtype::Unsigned => (0, i64::from(i32::MAX)),
                true => (i64::from(i32::MIN), i64::from(i32::MAX)),
                false => *hard,
            };
            let (v, clamped) = coerce_int(&qual, hard, &input)?;
            Ok(Coerced {
                value: Value::Int(v),
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
            let hard = match socket_standard_value(def, cx) {
                true if def.subtype == Subtype::Unsigned => (0.0, f64::from(f32::MAX)),
                true => (-f64::from(f32::MAX), f64::from(f32::MAX)),
                false => *hard,
            };
            let (c, clamped) = clamp_f32(v, hard);
            let value = match storage {
                FloatStorage::F32 => Value::Float(c),
                // Python assignment goes through a C `float` even for double storage.
                FloatStorage::F64 => Value::Double(f64::from(c)),
            };
            Ok(Coerced { value, clamped })
        }
        PropKind::String { max_bytes, .. } => {
            let max = if *max_bytes == 0 {
                usize::MAX
            } else {
                *max_bytes as usize - 1
            };
            match input {
                Input::Str(s) => done(Value::Str(truncate_utf8(&s, max).into())),
                Input::Bare(s) => done(Value::Str(truncate_utf8(&s, max).into())),
                Input::None if !def.flags.contains(PropFlags::NEVER_NULL) => {
                    done(Value::Str(SmolStr::default()))
                }
                Input::None => Err(type_err(format!(
                    "{qual} doesn't support None from string types"
                ))),
                other => Err(type_err(format!(
                    "{qual} expected a string type, not {}",
                    other.py_type_name()
                ))),
            }
        }
        PropKind::Enum { items, .. } if def.flags.contains(PropFlags::ENUM_FLAG) => {
            coerce_enum_flag(def, items, input, &qual)
        }
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
            if items
                .iter()
                .any(|i| !i.identifier.is_empty() && i.identifier == s)
            {
                done(Value::Enum(s))
            } else {
                let mut e = RnaError::new(
                    RnaErrorKind::Enum,
                    PyExc::TypeError,
                    format!("enum \"{s}\" not found in ({})", enum_items_repr(items)),
                );
                e.detail.insert("valid".into(), enum_valid_detail(items));
                Err(e)
            }
        }
        PropKind::Pointer { fixed_type, is_id } => match input {
            Input::None if def.flags.contains(PropFlags::NEVER_NULL) => Err(type_err(format!(
                "{qual} does not support a 'None' assignment {fixed_type} type"
            ))),
            Input::None => done(if *is_id { Value::Id(None) } else { Value::None }),
            Input::Ptr(p) => {
                if !p.srna.is_a(fixed_type) {
                    return Err(type_err(format!(
                        "{qual} expected a {fixed_type} type, not {}",
                        p.srna.identifier
                    )));
                }
                match (p.root, p.path.is_empty()) {
                    (crate::ptr::Root::Id(id), true) => done(Value::Id(Some(id))),
                    _ => done(Value::Ptr(Box::new(p))),
                }
            }
            // Operator properties / property groups given as a dict (macro sub-operators).
            Input::Dict(d) if !is_id => done(Value::Structs(vec![
                d.into_iter()
                    .map(|(k, v)| (SmolStr::from(k), untyped_value(v)))
                    .collect(),
            ])),
            other => Err(type_err(format!(
                "{qual} expected a {fixed_type} type, not {}",
                other.py_type_name()
            ))),
        },
        // Python-only functions taking plain Python sequences (`Mesh.from_pydata(vertices,
        // edges, faces)`, `spec/overlay/python_only.toml`): the items stay untyped; the
        // function checks them. Python's `len()` is what fails first on a non-sequence.
        PropKind::Collection {
            item_type: "AnyType",
            ..
        } => match input {
            Input::Tuple(v) | Input::List(v) => {
                done(Value::Tuple(v.into_iter().map(untyped_value).collect()))
            }
            other => Err(type_err(format!(
                "object of type '{}' has no len()",
                other.py_type_name()
            ))),
        },
        PropKind::Collection { .. } => {
            // Python's sequence protocol (`PySequence_Check`): tuples, lists, strings and
            // math types are sequences, so `files='a.glb'` fails on its first member; dicts
            // and sets are not (verified, Blender 5.2.4).
            let items = match input {
                Input::Tuple(v) | Input::List(v) => v,
                seq @ (Input::Str(_) | Input::Bare(_) | Input::Math(_)) => {
                    sequence_items(&seq).unwrap_or_default()
                }
                other => {
                    return Err(type_err(format!(
                        "{qual} expected a sequence for an RNA collection, not {}",
                        other.py_type_name()
                    )));
                }
            };
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let Input::Dict(d) = it else {
                    return Err(type_err(format!(
                        "{qual} expected a each sequence member to be a dict for an RNA \
                         collection, not {}",
                        it.py_type_name()
                    )));
                };
                out.push(
                    d.into_iter()
                        .map(|(k, v)| (SmolStr::from(k), untyped_value(v)))
                        .collect(),
                );
            }
            done(Value::Structs(out))
        }
    }
}

fn coerce_enum_flag(
    def: &PropDef,
    items: &[EnumItem],
    input: Input,
    qual: &str,
) -> Result<Coerced, RnaError> {
    let members = match input {
        Input::Set(v) | Input::List(v) | Input::Tuple(v) => v,
        // Short form: `delimit=UV` (frap extension, §9.2).
        Input::Bare(s) => vec![Input::Bare(s)],
        other => {
            return Err(type_err(format!(
                ", {qual} expected a set, not a {}",
                other.py_type_name()
            )));
        }
    };
    if items.is_empty() {
        if members.is_empty() {
            return Ok(Coerced {
                value: Value::EnumSet(BTreeSet::new()),
                clamped: false,
            });
        }
        return Err(type_err(format!(
            ": empty enum \"{}\" could not have any values assigned",
            def.identifier
        )));
    }
    let mut set = BTreeSet::new();
    for m in members {
        let s: SmolStr = match m {
            Input::Str(s) => s.into(),
            Input::Bare(s) => s,
            other => {
                return Err(type_err(format!(
                    "expected a string, not {}",
                    other.py_type_name()
                )));
            }
        };
        if !items
            .iter()
            .any(|i| !i.identifier.is_empty() && i.identifier == s)
        {
            let mut e = RnaError::new(
                RnaErrorKind::Enum,
                PyExc::ValueError,
                format!(": '{s}' not found in ({})", enum_items_repr(items)),
            );
            e.detail.insert("valid".into(), enum_valid_detail(items));
            return Err(e);
        }
        set.insert(s);
    }
    Ok(Coerced {
        value: Value::EnumSet(set),
        clamped: false,
    })
}

/// Array element kinds (`check_item_type` of `bpy_rna_array.cc`).
#[derive(Copy, Clone)]
enum ElemKind {
    Float,
    Int,
    Bool,
}

impl ElemKind {
    fn name(self) -> &'static str {
        match self {
            ElemKind::Float => "float",
            ElemKind::Int => "int",
            ElemKind::Bool => "boolean",
        }
    }
    fn accepts(self, i: &Input) -> bool {
        match self {
            ElemKind::Float => is_number(i),
            ElemKind::Int => is_int(i),
            ElemKind::Bool => matches!(i, Input::Bool(_)),
        }
    }
}

/// `validate_array_type`: checks nested sizes and item types; returns the flattened items in
/// Blender's storage order (outer index first).
fn validate_array_type(
    seq: Vec<Input>,
    dim: usize,
    dims: &[usize],
    dynamic: bool,
    kind: ElemKind,
    out: &mut Vec<Input>,
) -> Result<(), RnaError> {
    if dim + 1 < dims.len() {
        for item in seq {
            let Some(sub) = sequence_items(&item) else {
                return Err(type_err(format!(
                    "expected a sequence of {}, not {}",
                    kind.name(),
                    item.py_type_name()
                )));
            };
            if sub.len() != dims[dim + 1] {
                return Err(length_err(format!(
                    "sequences of dimension {} should contain {} items, not {}",
                    dim + 1,
                    dims[dim + 1],
                    sub.len()
                )));
            }
            validate_array_type(sub, dim + 1, dims, dynamic, kind, out)?;
        }
    } else {
        if seq.len() != dims[dim] && !dynamic {
            return Err(length_err(format!(
                "sequences of dimension {dim} should contain {} items, not {}",
                dims[dim],
                seq.len()
            )));
        }
        for item in seq {
            if !kind.accepts(&item) {
                return Err(type_err(format!(
                    "expected sequence items of type {}, not {}",
                    kind.name(),
                    item.py_type_name()
                )));
            }
            out.push(item);
        }
    }
    Ok(())
}

/// Square matrix storage (`m[col][row]`, what nested sequences fill) to the row-major order of
/// `Value::FloatArray`.
fn col_major_to_row_major<T: Clone>(flat: &[T], cols: usize, rows: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(flat.len());
    for r in 0..rows {
        for c in 0..cols {
            out.push(flat[c * rows + r].clone());
        }
    }
    out
}

fn coerce_array(def: &PropDef, input: Input, qual: &str) -> Result<Coerced, RnaError> {
    let kind = match def.kind {
        PropKind::Float { .. } => ElemKind::Float,
        PropKind::Int { .. } => ElemKind::Int,
        PropKind::Bool { .. } => ElemKind::Bool,
        _ => {
            return Err(RnaError::new(
                RnaErrorKind::Other,
                PyExc::TypeError,
                "not an array type",
            ));
        }
    };
    let dynamic = matches!(def.array, ArrayShape::Dynamic);
    let dims: Vec<usize> = match def.array {
        ArrayShape::Fixed(n) => vec![n as usize],
        ArrayShape::Matrix(a, b) => vec![a as usize, b as usize],
        ArrayShape::Dynamic | ArrayShape::Scalar => vec![0],
    };
    let tag = MathTag::for_subtype(def.subtype);

    // `mathutils.Matrix` assignment (first level only).
    if let Input::Math(MathValue::Matrix(rows)) = &input {
        if !matches!(kind, ElemKind::Float) {
            return Err(length_err(format!(
                "{qual}, matrix assign to non float array"
            )));
        }
        if dims.len() != 2 {
            return Err(length_err(format!(
                "{qual}, matrix assign array with {} dimensions",
                dims.len()
            )));
        }
        let row_num = rows.len();
        let col_num = rows.first().map_or(0, Vec::len);
        if col_num != dims[0] || row_num != dims[1] {
            return Err(length_err(format!(
                "{qual}, matrix assign dimension size mismatch, is {col_num}x{row_num}, \
                 expected be {}x{}",
                dims[0], dims[1]
            )));
        }
        let (lo, hi) = float_hard(def);
        let mut clamped = false;
        let data = rows
            .iter()
            .flatten()
            .map(|v| {
                let (c, cl) = clamp_f32(*v, (lo, hi));
                clamped |= cl;
                c
            })
            .collect();
        return Ok(Coerced {
            value: Value::FloatArray {
                data,
                shape: def.array,
                tag,
            },
            clamped,
        });
    }

    let Some(seq) = sequence_items(&input) else {
        // `PySequence_Fast(seq, error_prefix)`: the message is the prefix alone.
        return Err(type_err(String::new()).with_hint(format!(
            "{} expects a sequence of {} {}s, e.g. {}",
            def.identifier,
            def.array_len(),
            if matches!(kind, ElemKind::Bool) {
                "boolean"
            } else {
                "number"
            },
            example(def.array_len(), kind)
        )));
    };
    let mut flat = Vec::new();
    validate_array_type(seq, 0, &dims, dynamic, kind, &mut flat)?;
    if dims.len() > 1 {
        let len: usize = dims.iter().product();
        if flat.len() != len {
            return Err(length_err(format!(
                "{qual}, sequence must have {len} items total, not {}",
                flat.len()
            )));
        }
        flat = col_major_to_row_major(&flat, dims[0], dims[1]);
    }

    let mut clamped = false;
    let value = match (kind, &def.kind) {
        (ElemKind::Float, _) => {
            let hard = float_hard(def);
            let data = flat
                .iter()
                .map(|i| {
                    let (c, cl) = clamp_f32(i.as_number().unwrap_or(0.0), hard);
                    clamped |= cl;
                    c
                })
                .collect();
            Value::FloatArray {
                data,
                shape: def.array,
                tag,
            }
        }
        (ElemKind::Int, PropKind::Int { hard, .. }) => {
            let mut data = SmallVec::new();
            for i in &flat {
                // Blender 5.2.4 raises a SystemError for out-of-range items; frap reports the
                // scalar wording.
                let (v, cl) = coerce_int(qual, *hard, i)?;
                clamped |= cl;
                data.push(v);
            }
            Value::IntArray(data)
        }
        _ => Value::BoolArray(
            flat.iter()
                .map(|i| matches!(i, Input::Bool(true)))
                .collect(),
        ),
    };
    Ok(Coerced { value, clamped })
}

fn float_hard(def: &PropDef) -> (f64, f64) {
    match def.kind {
        PropKind::Float { hard, .. } => hard,
        _ => (f64::NEG_INFINITY, f64::INFINITY),
    }
}

fn example(n: usize, kind: ElemKind) -> String {
    let one = if matches!(kind, ElemKind::Bool) {
        "False"
    } else {
        "0"
    };
    let mut items = vec![one; n.max(1)];
    if !matches!(kind, ElemKind::Bool) && n >= 3 {
        items[n - 1] = "1";
    }
    format!("({})", items.join(", "))
}

/// Coerces one element of an array property (`prop[i] = x`, `location.x = x`), with the
/// messages of the wrapper Blender uses for that subtype: `mathutils.Vector` (`vector[index] =
/// x: assigned value not a number`), `Euler`, `Quaternion`, `Color`, or a plain
/// `bpy_prop_array` (`expected a float type`, `Python number not a bool (0/1)`, …). Returns a
/// scalar `Value::{Float, Double, Int, Bool}` clamped to the hard range.
pub fn coerce_element(def: &PropDef, input: Input) -> Result<Coerced, RnaError> {
    let not_number = |what: &str| type_err(format!("{what} = x: assigned value not a number"));
    match &def.kind {
        PropKind::Float { hard, storage, .. } => {
            let Some(v) = input.as_number() else {
                return Err(match MathTag::for_subtype(def.subtype) {
                    MathTag::Vector => not_number("vector[index]"),
                    MathTag::Euler(_) => not_number("euler[attribute]"),
                    MathTag::Quaternion => not_number("quaternion[index]"),
                    MathTag::Color if def.array_len() == 3 => not_number("color[item]"),
                    MathTag::Matrix => RnaError::new(
                        RnaErrorKind::NotImplemented,
                        PyExc::TypeError,
                        "frap: assigning one element of a matrix property is not supported; \
                         assign the whole matrix",
                    ),
                    _ => type_err("expected a float type".to_owned()),
                });
            };
            let (c, clamped) = clamp_f32(v, *hard);
            Ok(Coerced {
                value: match storage {
                    FloatStorage::F32 => Value::Float(c),
                    FloatStorage::F64 => Value::Double(f64::from(c)),
                },
                clamped,
            })
        }
        PropKind::Int { hard, .. } => match input {
            Input::Int(_) | Input::Bool(_) => {
                let v = input.as_number().unwrap_or(0.0) as i64;
                if v > i64::from(i32::MAX) || v < i64::from(i32::MIN) {
                    return Err(type_err("expected an int type".to_owned()));
                }
                let c = v.clamp(hard.0, hard.1);
                Ok(Coerced {
                    value: Value::Int(c),
                    clamped: c != v,
                })
            }
            _ => Err(type_err("expected an int type".to_owned())),
        },
        PropKind::Bool { .. } => match input {
            Input::Bool(b) => Ok(Coerced {
                value: Value::Bool(b),
                clamped: false,
            }),
            Input::Int(i @ (0 | 1)) => Ok(Coerced {
                value: Value::Bool(i == 1),
                clamped: false,
            }),
            Input::Int(_) => Err(type_err("Python number not a bool (0/1)".to_owned())),
            other => Err(type_err(format!(
                "'{}' object cannot be interpreted as an integer",
                other.py_type_name()
            ))),
        },
        _ => Err(RnaError::new(
            RnaErrorKind::NoAttribute,
            PyExc::AttributeError,
            "not an array type",
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
