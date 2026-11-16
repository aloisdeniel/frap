// SPDX-License-Identifier: GPL-3.0-or-later
//! §9.2 Values (`frap-rna/src/value.rs`, CONTRACT) and a reduced `coerce` that is enough to
//! drive the probe tests with Blender's wording (verified on 5.2.4, see RFC 0001 appendix).

use std::collections::BTreeSet;

use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::{ArrayShape, Defaults, EnumDefault, PropDef, PropFlags, PropKind, Subtype};
use crate::error::{PyExc, RnaError, codes};
use crate::ptr::RnaPtr;

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bare(SmolStr),
    Tuple(Vec<Input>),
    List(Vec<Input>),
    Set(Vec<Input>),
    Dict(IndexMap<String, Input>),
    Math(MathValue),
    Ptr(RnaPtr),
}

/// Not defined by the contract; minimal.
#[derive(Clone, Debug, PartialEq)]
pub enum MathValue {
    Vector(Vec<f64>),
    Euler(Vec<f64>, EulerOrder),
    Quaternion([f64; 4]),
    Matrix(Vec<Vec<f64>>),
    Color(Vec<f64>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EulerOrder {
    XYZ,
    XZY,
    YXZ,
    YZX,
    ZXY,
    ZYX,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MathTag {
    Vector,
    Euler(EulerOrder),
    Quaternion,
    AxisAngle,
    Matrix,
    Color,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    Float(f32),
    Double(f64),
    Str(SmolStr),
    Enum(SmolStr),
    EnumSet(BTreeSet<SmolStr>),
    BoolArray(SmallVec<[bool; 4]>),
    IntArray(SmallVec<[i64; 4]>),
    FloatArray {
        data: SmallVec<[f32; 16]>,
        shape: ArrayShape,
        tag: MathTag,
    },
    Ptr(RnaPtr),
    Coll(RnaPtr),
    Tuple(Vec<Value>),
}

impl Value {
    pub fn float3(v: [f32; 3], tag: MathTag) -> Value {
        Value::FloatArray {
            data: SmallVec::from_slice(&v),
            shape: ArrayShape::Fixed(3),
            tag,
        }
    }
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Value::FloatArray { data, .. } => Some(data),
            _ => None,
        }
    }
}

/// Where a value is being assigned; selects Blender's message prefix (RFC 0001 F-VAL-2:
/// `CoerceCx` is referenced by the contract but not defined).
#[derive(Clone, Debug)]
pub enum CoerceCx {
    /// `bpy_struct: item.attr = val: …` — `owner` is the struct identifier (`Object`).
    RnaSet { owner: &'static str },
    /// `Converting py args to operator properties: …` — `owner` is the bl_idname.
    OpArgs { owner: &'static str },
}

impl CoerceCx {
    fn prefix(&self) -> &'static str {
        match self {
            CoerceCx::RnaSet { .. } => "bpy_struct: item.attr = val: ",
            CoerceCx::OpArgs { .. } => "Converting py args to operator properties: ",
        }
    }
    fn owner(&self) -> &'static str {
        match self {
            CoerceCx::RnaSet { owner } | CoerceCx::OpArgs { owner } => owner,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Coerced {
    pub value: Value,
    pub clamped: bool,
}

/// Default math wrapper by subtype ([RNA §0 #14]: 4-component colours are plain arrays).
pub fn math_tag(def: &PropDef) -> MathTag {
    let n = def.array.flat_len().unwrap_or(0);
    match def.subtype {
        Subtype::Translation
        | Subtype::Direction
        | Subtype::Velocity
        | Subtype::Acceleration
        | Subtype::Xyz
        | Subtype::XyzLength
        | Subtype::Coordinates => MathTag::Vector,
        Subtype::Euler => MathTag::Euler(EulerOrder::XYZ),
        Subtype::Quaternion => MathTag::Quaternion,
        Subtype::Axisangle => MathTag::AxisAngle,
        Subtype::Matrix => MathTag::Matrix,
        Subtype::Color | Subtype::ColorGamma if n == 3 => MathTag::Color,
        _ => MathTag::None,
    }
}

fn py_type_name(i: &Input) -> &'static str {
    match i {
        Input::None => "NoneType",
        Input::Bool(_) => "bool",
        Input::Int(_) => "int",
        Input::Float(_) => "float",
        Input::Str(_) | Input::Bare(_) => "str",
        Input::Tuple(_) => "tuple",
        Input::List(_) => "list",
        Input::Set(_) => "set",
        Input::Dict(_) => "dict",
        Input::Math(_) => "Vector",
        Input::Ptr(p) => p.srna.identifier,
    }
}

fn type_err(cx: &CoerceCx, msg: String) -> RnaError {
    RnaError::new(PyExc::TypeError, codes::RNA_TYPE, format!("{}{}", cx.prefix(), msg))
}

fn seq_items(i: &Input) -> Option<Vec<Input>> {
    match i {
        Input::Tuple(v) | Input::List(v) => Some(v.clone()),
        // Python quirk (verified): a str is a sequence, `location = 'x'` reports "not 1".
        Input::Str(s) => Some(s.chars().map(|c| Input::Str(c.to_string())).collect()),
        Input::Math(MathValue::Vector(v) | MathValue::Color(v) | MathValue::Euler(v, _)) => {
            Some(v.iter().map(|x| Input::Float(*x)).collect())
        }
        Input::Math(MathValue::Quaternion(q)) => Some(q.iter().map(|x| Input::Float(*x)).collect()),
        _ => None,
    }
}

fn as_f64(i: &Input) -> Option<f64> {
    match i {
        Input::Int(v) => Some(*v as f64),
        Input::Float(v) => Some(*v),
        Input::Bool(b) => Some(*b as i64 as f64),
        _ => None,
    }
}

/// The only place where Blender's assignment rules live (§9.2). Reduced to the kinds the probe
/// exercises; enough to check that `PropDef` carries every input `coerce` needs.
pub fn coerce(def: &'static PropDef, input: Input, cx: &CoerceCx) -> Result<Coerced, RnaError> {
    if def.flags.contains(PropFlags::READONLY) {
        let msg = if def.flags.contains(PropFlags::CONTEXT_MEMBER) {
            format!("bpy_struct: Context property \"{}\" is read-only", def.identifier)
        } else {
            format!("bpy_struct: attribute \"{}\" from \"{}\" is read-only", def.identifier, cx.owner())
        };
        let code = if def.flags.contains(PropFlags::CONTEXT_MEMBER) {
            codes::CONTEXT_READONLY
        } else {
            codes::RNA_READONLY
        };
        return Err(RnaError::new(PyExc::AttributeError, code, msg));
    }
    let owner = cx.owner();
    match (&def.kind, def.array) {
        (PropKind::Float { hard, .. }, ArrayShape::Scalar) => {
            let v = as_f64(&input).filter(|_| !matches!(input, Input::Bool(_))).ok_or_else(|| {
                type_err(
                    cx,
                    format!("{owner}.{} expected a float type, not {}", def.identifier, py_type_name(&input)),
                )
            })?;
            let c = v.clamp(hard.0, hard.1);
            Ok(Coerced {
                value: Value::Float(c as f32),
                clamped: c != v,
            })
        }
        (PropKind::Float { hard, .. }, shape) => {
            let n = shape.flat_len().unwrap_or(0);
            let items = seq_items(&input).ok_or_else(|| type_err(cx, String::new()))?;
            if items.len() != n {
                return Err(RnaError::new(
                    PyExc::ValueError,
                    codes::RNA_LENGTH,
                    format!(
                        "{}sequences of dimension 0 should contain {} items, not {}",
                        cx.prefix(),
                        n,
                        items.len()
                    ),
                ));
            }
            let mut data = SmallVec::new();
            let mut clamped = false;
            for it in &items {
                let v =
                    as_f64(it).ok_or_else(|| type_err(cx, format!("expected sequence items of type float, not {}", py_type_name(it))))?;
                let c = v.clamp(hard.0, hard.1);
                clamped |= c != v;
                data.push(c as f32);
            }
            Ok(Coerced {
                value: Value::FloatArray {
                    data,
                    shape,
                    tag: math_tag(def),
                },
                clamped,
            })
        }
        (PropKind::Int { hard, .. }, ArrayShape::Scalar) => match input {
            Input::Int(v) => {
                let c = v.clamp(hard.0, hard.1);
                Ok(Coerced {
                    value: Value::Int(c),
                    clamped: c != v,
                })
            }
            Input::Bool(b) => Ok(Coerced {
                value: Value::Int(b as i64),
                clamped: false,
            }),
            ref other => Err(type_err(
                cx,
                format!("{owner}.{} expected an int type, not {}", def.identifier, py_type_name(other)),
            )),
        },
        (PropKind::Bool { .. }, ArrayShape::Scalar) => match input {
            Input::Bool(b) => Ok(Coerced {
                value: Value::Bool(b),
                clamped: false,
            }),
            Input::Int(v @ (0 | 1)) => Ok(Coerced {
                value: Value::Bool(v == 1),
                clamped: false,
            }),
            ref other => Err(type_err(
                cx,
                format!("{owner}.{} expected True/False or 0/1, not {}", def.identifier, py_type_name(other)),
            )),
        },
        (PropKind::String { max_bytes, .. }, _) => match input {
            Input::Str(s) => {
                let mut s = s.as_str();
                let max = *max_bytes as usize;
                if max > 0 && s.len() > max {
                    let mut cut = max;
                    while !s.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    s = &s[..cut];
                }
                Ok(Coerced {
                    value: Value::Str(s.into()),
                    clamped: false,
                })
            }
            Input::Bare(s) => Ok(Coerced {
                value: Value::Str(s),
                clamped: false,
            }),
            ref other => Err(type_err(
                cx,
                format!("{owner}.{} expected a string type, not {}", def.identifier, py_type_name(other)),
            )),
        },
        (PropKind::Enum { items, .. }, _) if !def.flags.contains(PropFlags::ENUM_FLAG) => {
            let s: SmolStr = match &input {
                Input::Str(s) => s.as_str().into(),
                Input::Bare(s) => s.clone(),
                other => {
                    return Err(type_err(
                        cx,
                        format!("{owner}.{} expected a string enum, not {}", def.identifier, py_type_name(other)),
                    ));
                }
            };
            if items.iter().any(|i| i.identifier == s) {
                Ok(Coerced {
                    value: Value::Enum(s),
                    clamped: false,
                })
            } else {
                let list = items.iter().map(|i| format!("'{}'", i.identifier)).collect::<Vec<_>>().join(", ");
                Err(RnaError::new(
                    PyExc::TypeError,
                    codes::RNA_ENUM,
                    format!("{}enum \"{}\" not found in ({})", cx.prefix(), s, list),
                ))
            }
        }
        (PropKind::Pointer { fixed_type, .. }, _) => match input {
            Input::None if !def.flags.contains(PropFlags::NEVER_NULL) => Ok(Coerced {
                value: Value::None,
                clamped: false,
            }),
            Input::Ptr(p) if p.srna.is_a(fixed_type) => Ok(Coerced {
                value: Value::Ptr(p),
                clamped: false,
            }),
            ref other => Err(type_err(
                cx,
                format!(
                    "{owner}.{} expected a {} type, not {}",
                    def.identifier,
                    fixed_type,
                    py_type_name(other)
                ),
            )),
        },
        _ => Err(RnaError::new(
            PyExc::NotImplementedError,
            codes::NOT_IMPLEMENTED,
            format!("probe coerce: {}", def.identifier),
        )),
    }
}

/// Default value of a property as a `Value` (used to fill unset operator props).
pub fn default_value(def: &'static PropDef) -> Value {
    match &def.kind {
        PropKind::Bool {
            default: Defaults::Scalar(b),
        } => Value::Bool(*b),
        PropKind::Bool {
            default: Defaults::Array(a),
        } => Value::BoolArray(a.iter().copied().collect()),
        PropKind::Int {
            default: Defaults::Scalar(v),
            ..
        } => Value::Int(*v),
        PropKind::Int {
            default: Defaults::Array(a),
            ..
        } => Value::IntArray(a.iter().copied().collect()),
        PropKind::Float {
            default: Defaults::Scalar(v),
            ..
        } => Value::Float(*v as f32),
        PropKind::Float {
            default: Defaults::Array(a),
            ..
        } => Value::FloatArray {
            data: a.iter().map(|x| *x as f32).collect(),
            shape: def.array,
            tag: math_tag(def),
        },
        PropKind::String { default, .. } => Value::Str((*default).into()),
        PropKind::Enum {
            default: EnumDefault::Single(s),
            ..
        } => Value::Enum((*s).into()),
        PropKind::Enum {
            default: EnumDefault::Flags(f),
            ..
        } => Value::EnumSet(f.iter().map(|s| SmolStr::from(*s)).collect()),
        PropKind::Pointer { .. } | PropKind::Collection { .. } => Value::None,
    }
}
