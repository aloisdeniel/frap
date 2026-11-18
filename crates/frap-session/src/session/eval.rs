// SPDX-License-Identifier: GPL-3.0-or-later
//! Expression values and arithmetic (ARCHITECTURE §11.3): Python numbers, strings and
//! sequences, `mathutils` values (single precision, as `mathutils` stores them; Euler matrices
//! through Blender's `eul_to_mat3`, never `glam::EulerRot`), and RNA targets. Messages are
//! CPython's and `mathutils`' (verified on Blender 5.2.4).

use frap_lang::{BinOp, Builtin};
use frap_math::glam::{Mat3, Mat4};
use frap_math::{EulerOrder, euler, quat};
use frap_rna::{FrapError, Input, MathTag, MathValue, PropDef, PyExc, Value};
use frap_scene::HostRead;
use indexmap::IndexMap;
use serde_json::Value as J;
use smol_str::SmolStr;

use crate::errors::*;
use crate::resolve::{self, ArrayKind, Caps, Elem, Read, Target};
use crate::response::f32_json;

/// A Python value during evaluation.
#[derive(Clone, Debug)]
pub enum Py {
    /// `None`
    None,
    /// `bool`
    Bool(bool),
    /// `int`
    Int(i64),
    /// `float`
    Float(f64),
    /// `str`
    Str(String),
    /// Short-form unquoted word.
    Bare(SmolStr),
    /// `tuple`
    Tuple(Vec<Py>),
    /// `list`
    List(Vec<Py>),
    /// `set`
    Set(Vec<Py>),
    /// `dict`
    Dict(IndexMap<String, Py>),
    /// `mathutils.Vector`
    Vector(Vec<f32>),
    /// `mathutils.Euler`
    Euler([f32; 3], EulerOrder),
    /// `mathutils.Quaternion` (w, x, y, z)
    Quaternion([f32; 4]),
    /// `mathutils.Matrix`, rows
    Matrix(Vec<Vec<f32>>),
    /// `mathutils.Color`
    Color([f32; 3]),
    /// `bpy_prop_array`
    PropArray(Vec<Py>),
    /// An RNA object (struct, ID, collection, list of IDs, function).
    Rna(Target),
    /// An already-encoded result (`foreach_get(attr)`'s `$array`).
    Json(J),
}

fn zero_div(msg: &str) -> FrapError {
    FrapError::new(PyExc::ValueError, ZERO_DIVISION, msg)
        .with_detail("python_exception", "ZeroDivisionError")
}

fn type_err(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::TypeError, RNA_TYPE, msg)
}

fn value_err(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::ValueError, RNA_TYPE, msg)
}

fn attr_err(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::AttributeError, RNA_NO_ATTRIBUTE, msg)
}

impl Py {
    /// A front-end input as a Python value (`Session::invoke/set/call`, §11.6).
    pub fn from_input(i: Input) -> Py {
        let seq = |v: Vec<Input>| v.into_iter().map(Py::from_input).collect();
        let f32s = |v: Vec<f64>| v.into_iter().map(|x| x as f32).collect::<Vec<f32>>();
        match i {
            Input::None => Py::None,
            Input::Bool(b) => Py::Bool(b),
            Input::Int(i) => Py::Int(i),
            Input::Float(f) => Py::Float(f),
            Input::Str(s) => Py::Str(s),
            Input::Bare(s) => Py::Bare(s),
            Input::Tuple(v) => Py::Tuple(seq(v)),
            Input::List(v) => Py::List(seq(v)),
            Input::Set(v) => Py::Set(seq(v)),
            Input::Dict(d) => {
                Py::Dict(d.into_iter().map(|(k, v)| (k, Py::from_input(v))).collect())
            }
            Input::Math(MathValue::Vector(v)) => Py::Vector(f32s(v)),
            Input::Math(MathValue::Euler(e, o)) => Py::Euler(e.map(|x| x as f32), o),
            Input::Math(MathValue::Quaternion(q)) => Py::Quaternion(q.map(|x| x as f32)),
            Input::Math(MathValue::Matrix(rows)) => {
                Py::Matrix(rows.into_iter().map(f32s).collect())
            }
            Input::Math(MathValue::Color(c)) => {
                let c = f32s(c);
                let at = |i: usize| c.get(i).copied().unwrap_or(0.0);
                Py::Color([at(0), at(1), at(2)])
            }
            Input::Ptr(p) => Py::Rna(Target::Struct(p)),
            _ => Py::None,
        }
    }

    /// Python type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Py::None => "NoneType",
            Py::Bool(_) => "bool",
            Py::Int(_) => "int",
            Py::Float(_) => "float",
            Py::Str(_) | Py::Bare(_) => "str",
            Py::Tuple(_) => "tuple",
            Py::List(_) => "list",
            Py::Set(_) => "set",
            Py::Dict(_) => "dict",
            Py::Vector(_) => "Vector",
            Py::Euler(..) => "Euler",
            Py::Quaternion(_) => "Quaternion",
            Py::Matrix(_) => "Matrix",
            Py::Color(_) => "Color",
            Py::PropArray(_) => "bpy_prop_array",
            Py::Rna(t) => resolve::py_type_name(t),
            Py::Json(_) => "list",
        }
    }

    /// The number, for `int`/`float`/`bool`.
    pub fn number(&self) -> Option<f64> {
        match self {
            Py::Bool(b) => Some(*b as i64 as f64),
            Py::Int(i) => Some(*i as f64),
            Py::Float(f) => Some(*f),
            _ => None,
        }
    }

    fn int(&self) -> Option<i64> {
        match self {
            Py::Bool(b) => Some(*b as i64),
            Py::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Items of a sequence (`PySequence_Fast`), flattening `mathutils` values.
    pub fn items(&self) -> Option<Vec<Py>> {
        Some(match self {
            Py::Tuple(v) | Py::List(v) | Py::Set(v) | Py::PropArray(v) => v.clone(),
            Py::Vector(v) => v.iter().map(|x| Py::Float(*x as f64)).collect(),
            Py::Euler(v, _) | Py::Color(v) => v.iter().map(|x| Py::Float(*x as f64)).collect(),
            Py::Quaternion(v) => v.iter().map(|x| Py::Float(*x as f64)).collect(),
            Py::Matrix(rows) => rows.iter().map(|r| Py::Vector(r.clone())).collect(),
            _ => return None,
        })
    }

    fn floats(&self) -> Option<Vec<f32>> {
        self.items()?
            .iter()
            .map(|x| x.number().map(|f| f as f32))
            .collect()
    }

    /// The untyped input `coerce` takes (§9.2).
    pub fn to_input(&self) -> Result<Input, FrapError> {
        let f = |v: &[f32]| v.iter().map(|x| *x as f64).collect::<Vec<_>>();
        Ok(match self {
            Py::None => Input::None,
            Py::Bool(b) => Input::Bool(*b),
            Py::Int(i) => Input::Int(*i),
            Py::Float(x) => Input::Float(*x),
            Py::Str(s) => Input::Str(s.clone()),
            Py::Bare(s) => Input::Bare(s.clone()),
            Py::Tuple(v) | Py::PropArray(v) => Input::Tuple(seq_input(v)?),
            Py::List(v) => Input::List(seq_input(v)?),
            Py::Set(v) => Input::Set(seq_input(v)?),
            Py::Dict(d) => Input::Dict(
                d.iter()
                    .map(|(k, v)| Ok((k.clone(), v.to_input()?)))
                    .collect::<Result<_, FrapError>>()?,
            ),
            Py::Vector(v) => Input::Math(MathValue::Vector(f(v))),
            Py::Euler(v, o) => Input::Math(MathValue::Euler(
                [v[0] as f64, v[1] as f64, v[2] as f64],
                *o,
            )),
            Py::Quaternion(q) => Input::Math(MathValue::Quaternion([
                q[0] as f64,
                q[1] as f64,
                q[2] as f64,
                q[3] as f64,
            ])),
            Py::Matrix(rows) => Input::Math(MathValue::Matrix(rows.iter().map(|r| f(r)).collect())),
            Py::Color(c) => Input::Math(MathValue::Color(f(c))),
            Py::Rna(Target::Struct(p)) => Input::Ptr(p.clone()),
            Py::Rna(Target::None) => Input::None,
            Py::Rna(Target::Value(v)) => value_input(v),
            Py::Rna(Target::List { items, .. }) => Input::List(
                items
                    .iter()
                    .map(|t| Py::Rna(t.clone()).to_input())
                    .collect::<Result<_, _>>()?,
            ),
            Py::Rna(t) => {
                return Err(type_err(format!(
                    "frap: a {} cannot be used as a value here",
                    resolve::py_type_name(t)
                )));
            }
            Py::Json(_) => {
                return Err(type_err(
                    "frap: a bulk array cannot be used as a value here",
                ));
            }
        })
    }

    /// JSON of a computed value (`print`, function results).
    pub fn to_json(&self, host: &dyn HostRead, caps: Caps) -> Result<J, FrapError> {
        let arr = |v: &[f32]| J::Array(v.iter().map(|x| f32_json(*x)).collect());
        Ok(match self {
            Py::None => J::Null,
            Py::Bool(b) => J::from(*b),
            Py::Int(i) => J::from(*i),
            Py::Float(x) => f64_json(*x),
            Py::Str(s) => J::from(s.as_str()),
            Py::Bare(s) => J::from(s.as_str()),
            Py::Tuple(v) | Py::List(v) | Py::PropArray(v) => J::Array(
                v.iter()
                    .map(|x| x.to_json(host, caps))
                    .collect::<Result<_, _>>()?,
            ),
            Py::Set(v) => {
                let mut items: Vec<J> = v
                    .iter()
                    .map(|x| x.to_json(host, caps))
                    .collect::<Result<_, _>>()?;
                items.sort_by_key(|a| a.to_string());
                J::Array(items)
            }
            Py::Dict(d) => J::Object(
                d.iter()
                    .map(|(k, v)| Ok((k.clone(), v.to_json(host, caps)?)))
                    .collect::<Result<_, FrapError>>()?,
            ),
            Py::Vector(v) => arr(v),
            Py::Euler(v, _) | Py::Color(v) => arr(v),
            Py::Quaternion(q) => arr(q),
            Py::Matrix(rows) => J::Array(rows.iter().map(|r| arr(r)).collect()),
            Py::Rna(t) => resolve::encode(host, t, caps)?.0,
            Py::Json(j) => j.clone(),
        })
    }
}

fn seq_input(v: &[Py]) -> Result<Vec<Input>, FrapError> {
    v.iter().map(Py::to_input).collect()
}

fn value_input(v: &Value) -> Input {
    match v {
        Value::None => Input::None,
        Value::Bool(b) => Input::Bool(*b),
        Value::Int(i) => Input::Int(*i),
        Value::Float(f) => Input::Float(*f as f64),
        Value::Double(f) => Input::Float(*f),
        Value::Str(s) | Value::Enum(s) => Input::Str(s.to_string()),
        Value::EnumSet(s) => Input::Set(s.iter().map(|x| Input::Str(x.to_string())).collect()),
        Value::Tuple(t) => Input::Tuple(t.iter().map(value_input).collect()),
        Value::FloatArray { data, .. } => {
            Input::Tuple(data.iter().map(|x| Input::Float(*x as f64)).collect())
        }
        Value::IntArray(a) => Input::Tuple(a.iter().map(|x| Input::Int(*x)).collect()),
        Value::BoolArray(a) => Input::Tuple(a.iter().map(|x| Input::Bool(*x)).collect()),
        _ => Input::None,
    }
}

pub use crate::response::f64_json;

/// A property value as Python sees it (`mathutils` wrappers, `bpy_prop_array`, `set`).
pub fn value_py(prop: Option<&PropDef>, v: Value) -> Py {
    match v {
        Value::None | Value::Id(None) => Py::None,
        Value::Bool(b) => Py::Bool(b),
        Value::Int(i) => Py::Int(i),
        Value::Float(f) => Py::Float(f as f64),
        Value::Double(f) => Py::Float(f),
        Value::Str(s) | Value::Enum(s) => Py::Str(s.to_string()),
        Value::EnumSet(s) => Py::Set(s.into_iter().map(|x| Py::Str(x.to_string())).collect()),
        Value::IntArray(a) => Py::PropArray(a.into_iter().map(Py::Int).collect()),
        Value::BoolArray(a) => Py::PropArray(a.into_iter().map(Py::Bool).collect()),
        Value::Tuple(t) => Py::Tuple(t.into_iter().map(|x| value_py(None, x)).collect()),
        Value::FloatArray { data, shape, tag } => {
            let kind = match prop {
                Some(p) => ArrayKind::of(p),
                None => ArrayKind::of_value(data.len(), shape, tag),
            };
            let d = data.to_vec();
            match kind {
                ArrayKind::Vector(_) => Py::Vector(d),
                ArrayKind::Euler => Py::Euler(
                    [d[0], d[1], d[2]],
                    match tag {
                        MathTag::Euler(o) => o,
                        _ => EulerOrder::XYZ,
                    },
                ),
                ArrayKind::Quaternion => Py::Quaternion([d[0], d[1], d[2], d[3]]),
                ArrayKind::Color => Py::Color([d[0], d[1], d[2]]),
                ArrayKind::Matrix(_, c) => Py::Matrix(d.chunks(c).map(|r| r.to_vec()).collect()),
                ArrayKind::PropArray => {
                    Py::PropArray(d.into_iter().map(|x| Py::Float(x as f64)).collect())
                }
            }
        }
        other => Py::Rna(Target::Value(other)),
    }
}

/// A resolved target as a Python value (properties are read).
pub fn target_py(host: &dyn HostRead, t: Target) -> Result<Py, FrapError> {
    Ok(match t {
        Target::None => Py::None,
        Target::Prop(p) => match resolve::read_prop(host, &p)? {
            Read::Target(inner) => target_py(host, inner)?,
            Read::Value(whole) => {
                let v = resolve::element(p.prop, whole, p.elem)?;
                match p.elem {
                    Elem::Whole => value_py(Some(p.prop), v),
                    Elem::Row(_) => value_py(None, v),
                    _ => value_py(None, v),
                }
            }
        },
        Target::Value(v) => value_py(None, v),
        other => Py::Rna(other),
    })
}

// ------------------------------------------------------------------ arithmetic

fn op_str(op: BinOp, inplace: bool) -> &'static str {
    match (op, inplace) {
        (BinOp::Add, false) => "+",
        (BinOp::Sub, false) => "-",
        (BinOp::Mul, false) => "*",
        (BinOp::Div, false) => "/",
        (BinOp::MatMul, false) => "@",
        (BinOp::Pow, false) => "** or pow()",
        (BinOp::Add, true) => "+=",
        (BinOp::Sub, true) => "-=",
        (BinOp::Mul, true) => "*=",
        (BinOp::Div, true) => "/=",
        (BinOp::MatMul, true) => "@=",
        (BinOp::Pow, true) => "**=",
        _ => "?",
    }
}

fn unsupported(op: BinOp, inplace: bool, a: &Py, b: &Py) -> FrapError {
    type_err(format!(
        "unsupported operand type(s) for {}: '{}' and '{}'",
        op_str(op, inplace),
        a.type_name(),
        b.type_name()
    ))
}

fn is_num(p: &Py) -> bool {
    p.number().is_some()
}

fn ints(a: &Py, b: &Py) -> Option<(i64, i64)> {
    Some((a.int()?, b.int()?))
}

fn float_op(op: BinOp, x: f64, y: f64) -> Result<Py, FrapError> {
    Ok(Py::Float(match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => {
            if y == 0.0 {
                return Err(zero_div("float division by zero"));
            }
            x / y
        }
        BinOp::Pow => {
            if x == 0.0 && y < 0.0 {
                return Err(zero_div("zero to a negative power"));
            }
            if x < 0.0 && y.fract() != 0.0 {
                return Err(FrapError::new(
                    PyExc::NotImplementedError,
                    NOT_IMPLEMENTED,
                    "frap: complex results are not supported",
                ));
            }
            x.powf(y)
        }
        _ => return Err(type_err("unsupported operand")),
    }))
}

fn int_op(op: BinOp, x: i64, y: i64) -> Result<Py, FrapError> {
    let big = |r: Option<i64>, f: f64| r.map_or(Py::Float(f), Py::Int);
    Ok(match op {
        BinOp::Add => big(x.checked_add(y), x as f64 + y as f64),
        BinOp::Sub => big(x.checked_sub(y), x as f64 - y as f64),
        BinOp::Mul => big(x.checked_mul(y), x as f64 * y as f64),
        BinOp::Div => {
            if y == 0 {
                return Err(zero_div("division by zero"));
            }
            Py::Float(x as f64 / y as f64)
        }
        BinOp::Pow if y >= 0 => big(
            u32::try_from(y).ok().and_then(|e| x.checked_pow(e)),
            (x as f64).powf(y as f64),
        ),
        BinOp::Pow => {
            if x == 0 {
                return Err(zero_div("zero to a negative power"));
            }
            Py::Float((x as f64).powf(y as f64))
        }
        _ => return Err(type_err("unsupported operand")),
    })
}

fn vec_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "Vector addition",
        _ => "Vector subtraction",
    }
}

fn mat3(rows: &[Vec<f32>]) -> Option<Mat3> {
    (rows.len() == 3 && rows.iter().all(|r| r.len() == 3)).then(|| {
        Mat3::from_cols_array_2d(&[
            [rows[0][0], rows[0][1], rows[0][2]],
            [rows[1][0], rows[1][1], rows[1][2]],
            [rows[2][0], rows[2][1], rows[2][2]],
        ])
        .transpose()
    })
}

fn mat4(rows: &[Vec<f32>]) -> Option<Mat4> {
    (rows.len() == 4 && rows.iter().all(|r| r.len() == 4)).then(|| {
        let mut a = [[0.0f32; 4]; 4];
        for (i, r) in rows.iter().enumerate() {
            a[i].copy_from_slice(r);
        }
        Mat4::from_cols_array_2d(&a).transpose()
    })
}

fn rows3(m: Mat3) -> Vec<Vec<f32>> {
    m.transpose()
        .to_cols_array_2d()
        .iter()
        .map(|r| r.to_vec())
        .collect()
}

fn rows4(m: Mat4) -> Vec<Vec<f32>> {
    m.transpose()
        .to_cols_array_2d()
        .iter()
        .map(|r| r.to_vec())
        .collect()
}

fn mat_mul(a: &[Vec<f32>], b: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, FrapError> {
    let (n, k) = (a.len(), a.first().map_or(0, |r| r.len()));
    if b.len() != k {
        return Err(value_err(
            "matrix1 * matrix2: matrix1 number of columns and the matrix2 number of rows must be the same",
        ));
    }
    let m = b.first().map_or(0, |r| r.len());
    Ok((0..n)
        .map(|i| {
            (0..m)
                .map(|j| (0..k).map(|t| a[i][t] * b[t][j]).sum())
                .collect()
        })
        .collect())
}

fn mat_vec(rows: &[Vec<f32>], v: &[f32]) -> Result<Vec<f32>, FrapError> {
    let cols = rows.first().map_or(0, |r| r.len());
    if v.len() == cols {
        return Ok(rows
            .iter()
            .map(|r| r.iter().zip(v).map(|(a, b)| a * b).sum())
            .collect());
    }
    if v.len() + 1 == cols {
        // `Matrix(4x4) @ Vector(3)`: w = 1, the result keeps 3 components (mathutils).
        let mut h = v.to_vec();
        h.push(1.0);
        let r: Vec<f32> = rows
            .iter()
            .map(|r| r.iter().zip(&h).map(|(a, b)| a * b).sum())
            .collect();
        return Ok(r[..v.len()].to_vec());
    }
    Err(value_err(
        "matrix * vector: len(matrix.col) and len(vector) must be the same, except for 4x4 matrix * 3D vector.",
    ))
}

/// `a <op> b` (or `a <op>= b` when `inplace`), with Python/mathutils semantics.
pub fn binop(op: BinOp, a: &Py, b: &Py, inplace: bool) -> Result<Py, FrapError> {
    // Numbers.
    if let Some((x, y)) = ints(a, b)
        && op != BinOp::MatMul
    {
        return int_op(op, x, y);
    }
    if let (Some(x), Some(y)) = (a.number(), b.number()) {
        if op == BinOp::MatMul {
            return Err(unsupported(op, inplace, a, b));
        }
        return float_op(op, x, y);
    }
    match (a, b) {
        // Sequences and strings.
        (Py::Str(x), Py::Str(y)) if op == BinOp::Add => return Ok(Py::Str(format!("{x}{y}"))),
        (Py::Str(s), n) | (n, Py::Str(s)) if op == BinOp::Mul && n.int().is_some() => {
            return Ok(Py::Str(s.repeat(n.int().unwrap_or(0).max(0) as usize)));
        }
        (Py::Tuple(x), Py::Tuple(y)) if op == BinOp::Add => {
            return Ok(Py::Tuple(x.iter().chain(y).cloned().collect()));
        }
        (Py::List(x), Py::List(y)) if op == BinOp::Add => {
            return Ok(Py::List(x.iter().chain(y).cloned().collect()));
        }
        (Py::Tuple(s), n) | (n, Py::Tuple(s)) if op == BinOp::Mul && n.int().is_some() => {
            let k = n.int().unwrap_or(0).max(0) as usize;
            return Ok(Py::Tuple((0..k).flat_map(|_| s.iter().cloned()).collect()));
        }
        (Py::List(s), n) | (n, Py::List(s)) if op == BinOp::Mul && n.int().is_some() => {
            let k = n.int().unwrap_or(0).max(0) as usize;
            return Ok(Py::List((0..k).flat_map(|_| s.iter().cloned()).collect()));
        }
        _ => {}
    }
    match (op, a, b) {
        // Vector ± Vector.
        (BinOp::Add | BinOp::Sub, Py::Vector(x), Py::Vector(y)) => {
            if x.len() != y.len() {
                return Err(attr_err(format!(
                    "{}: vectors must have the same dimensions for this operation",
                    vec_name(op)
                )));
            }
            let s = if op == BinOp::Add { 1.0 } else { -1.0 };
            Ok(Py::Vector(
                x.iter().zip(y).map(|(p, q)| p + s * q).collect(),
            ))
        }
        (BinOp::Add | BinOp::Sub, Py::Vector(_), other)
        | (BinOp::Add | BinOp::Sub, other, Py::Vector(_))
            if !matches!(other, Py::Vector(_)) =>
        {
            let sym = if op == BinOp::Add { "+" } else { "-" };
            Err(attr_err(format!(
                "{}: ({} {}{} {}) invalid type for this operation",
                vec_name(op),
                a.type_name(),
                sym,
                if inplace { "=" } else { "" },
                b.type_name()
            )))
        }
        // Vector * scalar, scalar * Vector, element-wise, division.
        (BinOp::Mul, Py::Vector(v), n) | (BinOp::Mul, n, Py::Vector(v)) if is_num(n) => {
            let f = n.number().unwrap_or(0.0) as f32;
            Ok(Py::Vector(v.iter().map(|x| x * f).collect()))
        }
        (BinOp::Mul, Py::Vector(x), Py::Vector(y)) => {
            if x.len() != y.len() {
                return Err(value_err(
                    "Vector multiplication: vectors must have the same dimensions for this operation",
                ));
            }
            Ok(Py::Vector(x.iter().zip(y).map(|(p, q)| p * q).collect()))
        }
        (BinOp::Mul, Py::Vector(_), other) | (BinOp::Mul, other, Py::Vector(_))
            if !matches!(other, Py::Matrix(_) | Py::Quaternion(_)) =>
        {
            Err(type_err(format!(
                "{}lement-wise multiplication: not supported between '{}' and '{}' types",
                if inplace { "In place e" } else { "E" },
                a.type_name(),
                b.type_name()
            )))
        }
        (BinOp::Div, Py::Vector(v), n) if is_num(n) => {
            let f = n.number().unwrap_or(0.0) as f32;
            if f == 0.0 {
                return Err(zero_div("Vector division: divide by zero error"));
            }
            Ok(Py::Vector(v.iter().map(|x| x / f).collect()))
        }
        (BinOp::MatMul, Py::Vector(x), Py::Vector(y)) => {
            if x.len() != y.len() {
                return Err(value_err(
                    "Vector multiplication: vectors must have the same dimensions for this operation",
                ));
            }
            Ok(Py::Float(
                x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>() as f64
            ))
        }
        // Color.
        (BinOp::Add | BinOp::Sub, Py::Color(x), Py::Color(y)) => {
            let s = if op == BinOp::Add { 1.0 } else { -1.0 };
            Ok(Py::Color([
                x[0] + s * y[0],
                x[1] + s * y[1],
                x[2] + s * y[2],
            ]))
        }
        (BinOp::Mul, Py::Color(c), n) | (BinOp::Mul, n, Py::Color(c)) if is_num(n) => {
            let f = n.number().unwrap_or(0.0) as f32;
            Ok(Py::Color([c[0] * f, c[1] * f, c[2] * f]))
        }
        // Matrix.
        (BinOp::MatMul, Py::Matrix(m), Py::Vector(v)) => Ok(Py::Vector(mat_vec(m, v)?)),
        (BinOp::MatMul, Py::Matrix(x), Py::Matrix(y)) => Ok(Py::Matrix(mat_mul(x, y)?)),
        (BinOp::Mul, Py::Matrix(m), n) | (BinOp::Mul, n, Py::Matrix(m)) if is_num(n) => {
            let f = n.number().unwrap_or(0.0) as f32;
            Ok(Py::Matrix(
                m.iter()
                    .map(|r| r.iter().map(|x| x * f).collect())
                    .collect(),
            ))
        }
        (BinOp::Add | BinOp::Sub, Py::Matrix(x), Py::Matrix(y)) if x.len() == y.len() => {
            let s = if op == BinOp::Add { 1.0 } else { -1.0 };
            Ok(Py::Matrix(
                x.iter()
                    .zip(y)
                    .map(|(r, q)| r.iter().zip(q).map(|(p, q)| p + s * q).collect())
                    .collect(),
            ))
        }
        // Quaternion.
        (BinOp::MatMul, Py::Quaternion(p), Py::Quaternion(q)) => {
            Ok(Py::Quaternion(quat::mul_qt_qtqt(*p, *q)))
        }
        (BinOp::MatMul, Py::Quaternion(q), Py::Vector(v)) if v.len() == 3 => {
            let r = quat::to_glam(*q) * frap_math::glam::Vec3::new(v[0], v[1], v[2]);
            Ok(Py::Vector(r.to_array().to_vec()))
        }
        (BinOp::Mul, Py::Quaternion(q), n) | (BinOp::Mul, n, Py::Quaternion(q)) if is_num(n) => Ok(
            Py::Quaternion(quat::mul_qt_fl(*q, n.number().unwrap_or(0.0) as f32)),
        ),
        _ => Err(unsupported(op, inplace, a, b)),
    }
}

/// `-x` / `+x`.
pub fn unary(neg: bool, v: Py) -> Result<Py, FrapError> {
    let s = if neg { -1.0f32 } else { 1.0 };
    Ok(match v {
        Py::Int(i) => Py::Int(if neg { -i } else { i }),
        Py::Bool(b) => Py::Int(if neg { -(b as i64) } else { b as i64 }),
        Py::Float(f) => Py::Float(if neg { -f } else { f }),
        Py::Vector(x) => Py::Vector(x.iter().map(|a| a * s).collect()),
        Py::Quaternion(q) => Py::Quaternion(q.map(|a| a * s)),
        Py::Matrix(m) => Py::Matrix(
            m.iter()
                .map(|r| r.iter().map(|a| a * s).collect())
                .collect(),
        ),
        Py::Color(c) if !neg => Py::Color(c),
        other => {
            return Err(type_err(format!(
                "bad operand type for unary {}: '{}'",
                if neg { '-' } else { '+' },
                other.type_name()
            )));
        }
    })
}

// ------------------------------------------------------------------ built-ins

/// One evaluated call argument.
pub struct ArgVal {
    /// Keyword.
    pub name: Option<SmolStr>,
    /// Value.
    pub value: Py,
}

fn take_arg<'a>(args: &'a [ArgVal], pos: usize, name: &str) -> Option<&'a Py> {
    args.iter()
        .find(|a| a.name.as_deref() == Some(name))
        .or_else(|| args.iter().filter(|a| a.name.is_none()).nth(pos))
        .map(|a| &a.value)
}

fn num_arg(fname: &str, args: &[ArgVal], pos: usize, name: &str) -> Result<f64, FrapError> {
    let v = take_arg(args, pos, name).ok_or_else(|| {
        type_err(format!(
            "{fname}() missing required argument '{name}' (pos {})",
            pos + 1
        ))
    })?;
    v.number()
        .ok_or_else(|| type_err(format!("must be real number, not {}", v.type_name())))
}

fn floats_arg(fname: &str, v: &Py) -> Result<Vec<f32>, FrapError> {
    v.floats().ok_or_else(|| {
        type_err(format!(
            "{fname}(): expected a sequence of numbers, not {}",
            v.type_name()
        ))
    })
}

fn order_arg(v: Option<&Py>) -> Result<EulerOrder, FrapError> {
    match v {
        None => Ok(EulerOrder::XYZ),
        Some(Py::Str(s)) => EulerOrder::from_blender(s).ok_or_else(|| {
            value_err(format!(
                "Euler(): invalid order '{s}', expected one of 'XYZ', 'XZY', 'YXZ', 'YZX', 'ZXY', 'ZYX'"
            ))
        }),
        Some(Py::Bare(s)) => order_arg(Some(&Py::Str(s.to_string()))),
        Some(other) => Err(type_err(format!(
            "Euler(): order must be a string, not {}",
            other.type_name()
        ))),
    }
}

fn identity(n: usize) -> Vec<Vec<f32>> {
    (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect()
}

fn resize(m: Vec<Vec<f32>>, n: usize) -> Vec<Vec<f32>> {
    let mut out = identity(n);
    for (i, r) in m.iter().enumerate().take(n) {
        for (j, x) in r.iter().enumerate().take(n) {
            out[i][j] = *x;
        }
    }
    out
}

fn size_arg(fname: &str, args: &[ArgVal], pos: usize) -> Result<usize, FrapError> {
    match take_arg(args, pos, "size") {
        None => Err(type_err(format!("{fname}(): expected a matrix size"))),
        Some(v) => match v.int() {
            Some(n @ 2..=4) => Ok(n as usize),
            _ => Err(value_err(format!(
                "{fname}(): can only return a 2x2 3x3 or 4x4 matrix"
            ))),
        },
    }
}

/// Evaluates a built-in call (`Vector(...)`, `math.radians(...)`, `float('inf')`, …).
pub fn builtin(b: Builtin, args: &[ArgVal]) -> Result<Py, FrapError> {
    let first = take_arg(args, 0, "");
    Ok(match b {
        Builtin::Pi => Py::Float(std::f64::consts::PI),
        Builtin::Radians => Py::Float(num_arg("radians", args, 0, "x")?.to_radians()),
        Builtin::Degrees => Py::Float(num_arg("degrees", args, 0, "x")?.to_degrees()),
        Builtin::Sqrt => {
            let x = num_arg("sqrt", args, 0, "x")?;
            if x < 0.0 {
                return Err(value_err("math domain error"));
            }
            Py::Float(x.sqrt())
        }
        Builtin::Sin => Py::Float(num_arg("sin", args, 0, "x")?.sin()),
        Builtin::Cos => Py::Float(num_arg("cos", args, 0, "x")?.cos()),
        Builtin::Tan => Py::Float(num_arg("tan", args, 0, "x")?.tan()),
        Builtin::Float => match first {
            None => Py::Float(0.0),
            Some(Py::Str(s)) => Py::Float(parse_py_float(s).ok_or_else(|| {
                value_err(format!("could not convert string to float: '{s}'"))
            })?),
            Some(v) => Py::Float(v.number().ok_or_else(|| {
                type_err(format!(
                    "float() argument must be a string or a real number, not '{}'",
                    v.type_name()
                ))
            })?),
        },
        Builtin::Int => match first {
            None => Py::Int(0),
            Some(Py::Str(s)) => Py::Int(s.trim().replace('_', "").parse().map_err(|_| {
                value_err(format!("invalid literal for int() with base 10: '{s}'"))
            })?),
            Some(Py::Float(f)) => {
                if !f.is_finite() {
                    return Err(value_err("cannot convert float NaN or infinity to integer"));
                }
                Py::Int(f.trunc() as i64)
            }
            Some(v) => Py::Int(v.int().ok_or_else(|| {
                type_err(format!(
                    "int() argument must be a string, a bytes-like object or a real number, not '{}'",
                    v.type_name()
                ))
            })?),
        },
        Builtin::Set => match first {
            None => Py::Set(vec![]),
            Some(v) => Py::Set(v.items().ok_or_else(|| {
                type_err(format!("'{}' object is not iterable", v.type_name()))
            })?),
        },
        Builtin::Vector => match take_arg(args, 0, "seq") {
            None => Py::Vector(vec![0.0; 3]),
            Some(v) => {
                let f = floats_arg("Vector", v)?;
                if f.len() < 2 {
                    return Err(value_err(
                        "Vector(): invalid size, vectors must have at least 2 dimensions",
                    ));
                }
                Py::Vector(f)
            }
        },
        Builtin::Color => match take_arg(args, 0, "rgb") {
            None => Py::Color([0.0; 3]),
            Some(v) => {
                let f = floats_arg("Color", v)?;
                if f.len() != 3 {
                    return Err(value_err(format!(
                        "Color(): sequence size is {}, expected 3",
                        f.len()
                    )));
                }
                Py::Color([f[0], f[1], f[2]])
            }
        },
        Builtin::Euler => {
            let order = order_arg(take_arg(args, 1, "order"))?;
            match take_arg(args, 0, "angles") {
                None => Py::Euler([0.0; 3], order),
                Some(v) => {
                    let f = floats_arg("Euler", v)?;
                    if f.len() != 3 {
                        return Err(value_err(format!(
                            "Euler(): sequence size is {}, expected 3",
                            f.len()
                        )));
                    }
                    Py::Euler([f[0], f[1], f[2]], order)
                }
            }
        }
        Builtin::Quaternion => match (take_arg(args, 0, "seq"), take_arg(args, 1, "angle")) {
            (None, _) => Py::Quaternion([1.0, 0.0, 0.0, 0.0]),
            (Some(v), None) => {
                let f = floats_arg("Quaternion", v)?;
                if f.len() != 4 {
                    return Err(value_err(format!(
                        "Quaternion(): sequence size is {}, expected 4",
                        f.len()
                    )));
                }
                Py::Quaternion([f[0], f[1], f[2], f[3]])
            }
            (Some(axis), Some(angle)) => {
                let a = floats_arg("Quaternion", axis)?;
                let ang = angle.number().ok_or_else(|| type_err("Quaternion(): angle must be a number"))?;
                if a.len() != 3 {
                    return Err(value_err("Quaternion(): axis must be a 3D vector"));
                }
                Py::Quaternion(quat::axis_angle_to_quat([a[0], a[1], a[2]], ang as f32))
            }
        },
        Builtin::Matrix => match take_arg(args, 0, "rows") {
            None => Py::Matrix(identity(4)),
            Some(v) => {
                let rows: Vec<Vec<f32>> = v
                    .items()
                    .ok_or_else(|| type_err("Matrix(): expected a sequence of rows"))?
                    .iter()
                    .map(|r| floats_arg("Matrix", r))
                    .collect::<Result<_, _>>()?;
                let n = rows.len();
                if !(2..=4).contains(&n) || rows.iter().any(|r| !(2..=4).contains(&r.len())) {
                    return Err(value_err(
                        "Matrix(): expects no args or a single arg containing 2-4 numeric sequences",
                    ));
                }
                Py::Matrix(rows)
            }
        },
        Builtin::MatrixIdentity => Py::Matrix(identity(size_arg("Matrix.Identity", args, 0)?)),
        Builtin::MatrixTranslation => {
            let v = take_arg(args, 0, "vector")
                .ok_or_else(|| type_err("Matrix.Translation(): expected a vector"))?;
            let f = floats_arg("Matrix.Translation", v)?;
            let mut m = identity(4);
            for (i, x) in f.iter().enumerate().take(3) {
                m[i][3] = *x;
            }
            Py::Matrix(m)
        }
        Builtin::MatrixRotation => {
            // `C_Matrix_Rotation`: the angle is wrapped to [-π, π) in single precision first.
            let angle = quat::angle_wrap_rad(num_arg("Matrix.Rotation", args, 0, "angle")? as f32);
            let n = size_arg("Matrix.Rotation", args, 1)?;
            let axis = take_arg(args, 2, "axis");
            let m3 = match axis {
                Some(Py::Str(s)) if matches!(s.as_str(), "X" | "Y" | "Z") => {
                    // `axis_angle_to_mat3_single` (single-precision trigonometry).
                    let (sn, c) = (angle.sin(), angle.cos());
                    let m = match s.as_str() {
                        "X" => [[1.0, 0.0, 0.0], [0.0, c, -sn], [0.0, sn, c]],
                        "Y" => [[c, 0.0, sn], [0.0, 1.0, 0.0], [-sn, 0.0, c]],
                        _ => [[c, -sn, 0.0], [sn, c, 0.0], [0.0, 0.0, 1.0]],
                    };
                    m.iter().map(|r| r.to_vec()).collect::<Vec<_>>()
                }
                Some(v @ (Py::Vector(_) | Py::Tuple(_) | Py::List(_))) => {
                    let a = floats_arg("Matrix.Rotation", v)?;
                    if a.len() != 3 {
                        return Err(value_err("Matrix.Rotation(): axis must be a 3D vector"));
                    }
                    rows3(quat::axis_angle_to_mat3([a[0], a[1], a[2]], angle))
                }
                None if n == 2 => {
                    let (sn, c) = (angle.sin(), angle.cos());
                    vec![vec![c, -sn], vec![sn, c]]
                }
                _ => {
                    return Err(value_err(
                        "Matrix.Rotation(): axis must be 'X', 'Y', 'Z' or a 3D vector",
                    ));
                }
            };
            Py::Matrix(resize(m3, n))
        }
        Builtin::MatrixScale => {
            let f = num_arg("Matrix.Scale", args, 0, "factor")? as f32;
            let n = size_arg("Matrix.Scale", args, 1)?;
            let mut m = identity(n);
            match take_arg(args, 2, "axis") {
                None => {
                    for (i, r) in m.iter_mut().enumerate().take(n.min(3)) {
                        r[i] = f;
                    }
                    if n == 2 {
                        m[1][1] = f;
                    }
                }
                Some(v) => {
                    let a = floats_arg("Matrix.Scale", v)?;
                    let len2: f32 = a.iter().map(|x| x * x).sum();
                    let k = f - 1.0;
                    let dim = n.min(3).min(a.len());
                    for i in 0..dim {
                        for j in 0..dim {
                            let e = if i == j { 1.0 } else { 0.0 };
                            m[i][j] = e + k * a[i] * a[j] / len2.max(f32::MIN_POSITIVE);
                        }
                    }
                }
            }
            Py::Matrix(m)
        }
        _ => {
            return Err(FrapError::new(
                PyExc::NotImplementedError,
                NOT_IMPLEMENTED,
                "frap: this built-in is not implemented",
            ));
        }
    })
}

fn parse_py_float(s: &str) -> Option<f64> {
    let t = s.trim().to_ascii_lowercase().replace('_', "");
    match t.as_str() {
        "inf" | "+inf" | "infinity" | "+infinity" => Some(f64::INFINITY),
        "-inf" | "-infinity" => Some(f64::NEG_INFINITY),
        "nan" | "+nan" | "-nan" => Some(f64::NAN),
        _ => t.parse().ok(),
    }
}

/// Evaluates a method call on a computed value (`Vector(...).normalized()`).
pub fn method(recv: Py, name: &str, args: &[ArgVal]) -> Result<Py, FrapError> {
    let no_attr = |r: &Py| {
        attr_err(format!(
            "'{}' object has no attribute '{name}'",
            r.type_name()
        ))
    };
    Ok(match (&recv, name) {
        (_, "copy") => recv.clone(),
        (Py::Vector(v), "normalized") => {
            let len = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            Py::Vector(if len > 0.0 {
                v.iter().map(|x| x / len).collect()
            } else {
                v.clone()
            })
        }
        (Py::Vector(v), "to_tuple") => Py::Tuple(v.iter().map(|x| Py::Float(*x as f64)).collect()),
        (Py::Vector(v), "to_2d") => {
            Py::Vector((0..2).map(|i| v.get(i).copied().unwrap_or(0.0)).collect())
        }
        (Py::Vector(v), "angle") => {
            // `Vector.angle(other, fallback)`: `saacos(dot / (|a| |b|))`, double precision.
            let o = take_arg(args, 0, "other")
                .ok_or_else(|| type_err("Vector.angle(other, fallback): expected a vector"))?;
            let w = floats_arg("Vector.angle", o)?;
            if w.len() != v.len() {
                return Err(value_err(
                    "Vector.angle(other): size mismatch, vector sizes must match",
                ));
            }
            let (mut dot, mut la, mut lb) = (0.0f64, 0.0f64, 0.0f64);
            for (a, b) in v.iter().zip(&w) {
                dot += (*a as f64) * (*b as f64);
                la += (*a as f64) * (*a as f64);
                lb += (*b as f64) * (*b as f64);
            }
            if la == 0.0 || lb == 0.0 {
                return match take_arg(args, 1, "fallback") {
                    Some(f) => Ok(f.clone()),
                    None => Err(value_err(
                        "Vector.angle(other): zero length vectors have no valid angle",
                    )),
                };
            }
            // `saacos` of the double-precision cosine, returned as a float (verified on 5.2.4:
            // `Vector((7.36, -6.93, 4.96)).angle((0, 0, 1))` is the f32 rounding of the double
            // result); libm (rule 2.2.12).
            let c = dot / (la.sqrt() * lb.sqrt());
            Py::Float(if c <= -1.0 {
                std::f32::consts::PI as f64
            } else if c >= 1.0 {
                0.0
            } else {
                libm::acos(c) as f32 as f64
            })
        }
        (Py::Vector(v), "lerp") => {
            let o = take_arg(args, 0, "other")
                .ok_or_else(|| type_err("Vector.lerp(): expected a vector"))?;
            let w = floats_arg("Vector.lerp", o)?;
            let f = num_arg("Vector.lerp", args, 1, "factor")? as f32;
            if w.len() != v.len() {
                return Err(value_err("Vector.lerp(): size mismatch"));
            }
            Py::Vector(v.iter().zip(&w).map(|(a, b)| a + (b - a) * f).collect())
        }
        (Py::Vector(v), "to_3d") => {
            Py::Vector((0..3).map(|i| v.get(i).copied().unwrap_or(0.0)).collect())
        }
        (Py::Vector(v), "to_4d") => Py::Vector(
            (0..4)
                .map(|i| v.get(i).copied().unwrap_or(if i == 3 { 1.0 } else { 0.0 }))
                .collect(),
        ),
        (Py::Vector(_), "dot") => {
            let o =
                take_arg(args, 0, "other").ok_or_else(|| type_err("dot(): expected a vector"))?;
            binop(
                BinOp::MatMul,
                &recv,
                &Py::Vector(floats_arg("Vector.dot", o)?),
                false,
            )?
        }
        (Py::Vector(v), "cross") => {
            let o =
                take_arg(args, 0, "other").ok_or_else(|| type_err("cross(): expected a vector"))?;
            let w = floats_arg("Vector.cross", o)?;
            if v.len() != 3 || w.len() != 3 {
                return Err(value_err(
                    "Vector.cross(other): expects both vectors to be 3D",
                ));
            }
            Py::Vector(vec![
                v[1] * w[2] - v[2] * w[1],
                v[2] * w[0] - v[0] * w[2],
                v[0] * w[1] - v[1] * w[0],
            ])
        }
        (Py::Euler(e, o), "to_matrix") => Py::Matrix(rows3(euler::eul_o_to_mat3(*e, *o))),
        (Py::Euler(e, o), "to_quaternion") => Py::Quaternion(euler::eul_o_to_quat(*e, *o)),
        (Py::Quaternion(q), "to_matrix") => Py::Matrix(rows3(quat::quat_to_mat3(*q))),
        (Py::Quaternion(q), "to_euler") => {
            let o = order_arg(take_arg(args, 0, "order"))?;
            Py::Euler(euler::quat_to_eul_o(*q, o), o)
        }
        (Py::Quaternion(q), "normalized") => Py::Quaternion(quat::normalize_qt(*q).0),
        (Py::Quaternion(q), "inverted") => Py::Quaternion(quat::invert_qt(*q)),
        (Py::Quaternion(q), "conjugated") => Py::Quaternion(quat::conjugate_qt(*q)),
        (Py::Quaternion(q), "to_axis_angle") => {
            let (axis, angle) = quat::quat_to_axis_angle(quat::normalize_qt(*q).0);
            Py::Tuple(vec![Py::Vector(axis.to_vec()), Py::Float(angle as f64)])
        }
        (Py::Matrix(m), "to_euler") => {
            let o = order_arg(take_arg(args, 0, "order"))?;
            let m3 = mat3(&resize(m.clone(), 3)).ok_or_else(|| {
                value_err(
                    "Matrix.to_euler(): inappropriate matrix size - expects 3x3 or 4x4 matrix",
                )
            })?;
            Py::Euler(euler::mat3_to_eul_o(&m3, o), o)
        }
        (Py::Matrix(m), "to_quaternion") => {
            let m3 = mat3(&resize(m.clone(), 3)).ok_or_else(|| {
                value_err("Matrix.to_quat(): inappropriate matrix size - expects 3x3 or 4x4 matrix")
            })?;
            Py::Quaternion(quat::mat3_to_quat(&m3))
        }
        (Py::Matrix(m), "to_3x3") => Py::Matrix(resize(m.clone(), 3)),
        (Py::Matrix(m), "to_4x4") => Py::Matrix(resize(m.clone(), 4)),
        (Py::Matrix(m), "transposed") => {
            let r = m.len();
            let c = m.first().map_or(0, |x| x.len());
            Py::Matrix((0..c).map(|j| (0..r).map(|i| m[i][j]).collect()).collect())
        }
        (Py::Matrix(m), "inverted") => {
            let inv = match m.len() {
                3 => mat3(m)
                    .filter(|x| x.determinant() != 0.0)
                    .map(|x| rows3(x.inverse())),
                4 => mat4(m)
                    .filter(|x| x.determinant() != 0.0)
                    .map(|x| rows4(x.inverse())),
                _ => None,
            };
            Py::Matrix(
                inv.ok_or_else(|| value_err("Matrix.inverted(): matrix does not have an inverse"))?,
            )
        }
        (Py::Matrix(m), "inverted_safe") => {
            let inv = match m.len() {
                3 => mat3(m)
                    .filter(|x| x.determinant() != 0.0)
                    .map(|x| rows3(x.inverse())),
                4 => mat4(m)
                    .filter(|x| x.determinant() != 0.0)
                    .map(|x| rows4(x.inverse())),
                _ => None,
            };
            // A singular matrix falls back to the identity (`invert_m4_m4_safe_ortho` is not
            // ported; Blender's result differs only for singular input).
            Py::Matrix(inv.unwrap_or_else(|| identity(m.len())))
        }
        (Py::Matrix(m), "decompose") => {
            let m4 = mat4(&resize(m.clone(), 4)).ok_or_else(|| {
                value_err("Matrix.decompose(): inappropriate matrix size - expects 4x4 matrix")
            })?;
            let (loc, q, size) = frap_math::matrix::decompose_wxyz(&m4);
            Py::Tuple(vec![
                Py::Vector(loc.to_array().to_vec()),
                Py::Quaternion(q),
                Py::Vector(size.to_array().to_vec()),
            ])
        }
        (Py::Matrix(m), "to_translation") if m.len() == 4 => {
            Py::Vector(vec![m[0][3], m[1][3], m[2][3]])
        }
        (Py::Matrix(m), "to_scale") => {
            let m3 = resize(m.clone(), 3);
            Py::Vector(
                (0..3)
                    .map(|j| (0..3).map(|i| m3[i][j] * m3[i][j]).sum::<f32>().sqrt())
                    .collect(),
            )
        }
        _ => return Err(no_attr(&recv)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: &[f32]) -> Py {
        Py::Vector(x.to_vec())
    }

    #[test]
    fn python_numbers() {
        assert!(
            matches!(binop(BinOp::Div, &Py::Int(7), &Py::Int(2), false).unwrap(), Py::Float(f) if f == 3.5)
        );
        assert!(matches!(
            binop(BinOp::Pow, &Py::Int(2), &Py::Int(3), false).unwrap(),
            Py::Int(8)
        ));
        assert!(matches!(
            binop(BinOp::Add, &Py::Bool(true), &Py::Int(1), false).unwrap(),
            Py::Int(2)
        ));
        let e = binop(BinOp::Div, &Py::Float(1.0), &Py::Int(0), false).unwrap_err();
        assert_eq!(e.message, "float division by zero");
        let e = binop(BinOp::Div, &Py::Int(1), &Py::Int(0), false).unwrap_err();
        assert_eq!(e.message, "division by zero");
        assert!(
            matches!(binop(BinOp::Add, &Py::Str("A".into()), &Py::Str("B".into()), false).unwrap(), Py::Str(s) if s == "AB")
        );
        assert!(
            matches!(binop(BinOp::Mul, &Py::Str("A".into()), &Py::Int(2), false).unwrap(), Py::Str(s) if s == "AA")
        );
    }

    #[test]
    fn mathutils_messages_match_blender() {
        // Verified on Blender 5.2.4.
        let e = binop(
            BinOp::Add,
            &v(&[0., 0., 0.]),
            &Py::Tuple(vec![Py::Int(1); 3]),
            true,
        )
        .unwrap_err();
        assert_eq!(e.exception, PyExc::AttributeError);
        assert_eq!(
            e.message,
            "Vector addition: (Vector += tuple) invalid type for this operation"
        );
        let e = binop(
            BinOp::Add,
            &v(&[0., 0., 0.]),
            &Py::Tuple(vec![Py::Int(1); 3]),
            false,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "Vector addition: (Vector + tuple) invalid type for this operation"
        );
        let e = binop(
            BinOp::Sub,
            &v(&[0., 0., 0.]),
            &Py::Tuple(vec![Py::Int(1); 3]),
            true,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "Vector subtraction: (Vector -= tuple) invalid type for this operation"
        );
        let e = binop(BinOp::Add, &v(&[0., 0., 0.]), &v(&[1., 2.]), true).unwrap_err();
        assert_eq!(
            e.message,
            "Vector addition: vectors must have the same dimensions for this operation"
        );
        let e = binop(
            BinOp::Mul,
            &v(&[0., 0., 0.]),
            &Py::Tuple(vec![Py::Int(1); 3]),
            true,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "In place element-wise multiplication: not supported between 'Vector' and 'tuple' types"
        );
        let e = binop(BinOp::Div, &v(&[1., 0., 0.]), &Py::Int(0), true).unwrap_err();
        assert_eq!(e.message, "Vector division: divide by zero error");
        let e = binop(
            BinOp::Mul,
            &Py::PropArray(vec![Py::Float(1.0)]),
            &Py::Int(2),
            true,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "unsupported operand type(s) for *=: 'bpy_prop_array' and 'int'"
        );
        let e = binop(
            BinOp::Add,
            &Py::Euler([0.; 3], EulerOrder::XYZ),
            &Py::Euler([0.; 3], EulerOrder::XYZ),
            true,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "unsupported operand type(s) for +=: 'Euler' and 'Euler'"
        );
        assert!(
            matches!(binop(BinOp::Mul, &v(&[1., 2., 3.]), &Py::Int(2), false).unwrap(), Py::Vector(x) if x == [2., 4., 6.])
        );
        assert!(
            matches!(binop(BinOp::MatMul, &v(&[1., 2., 3.]), &v(&[1., 1., 1.]), false).unwrap(), Py::Float(f) if f == 6.0)
        );
    }

    #[test]
    fn matrices_follow_mathutils() {
        let t = builtin(
            Builtin::MatrixTranslation,
            &[ArgVal {
                name: None,
                value: Py::Tuple(vec![Py::Int(1), Py::Int(2), Py::Int(3)]),
            }],
        )
        .unwrap();
        let r = binop(BinOp::MatMul, &t, &v(&[1., 1., 1.]), false).unwrap();
        assert!(matches!(r, Py::Vector(x) if x == [2., 3., 4.]));
        // Matrix.Rotation(radians(90), 4, 'Z')[0][0] is 7.549790126404332e-08 in Blender
        // (single-precision cos of the f32 angle).
        let m = builtin(
            Builtin::MatrixRotation,
            &[
                ArgVal {
                    name: None,
                    value: Py::Float(90f64.to_radians()),
                },
                ArgVal {
                    name: None,
                    value: Py::Int(4),
                },
                ArgVal {
                    name: None,
                    value: Py::Str("Z".into()),
                },
            ],
        )
        .unwrap();
        let Py::Matrix(rows) = m else { panic!() };
        assert_eq!(rows[0][0] as f64, 7.549790126404332e-08);
        assert_eq!(rows[0][1], -1.0);
        assert_eq!(rows[3][3], 1.0);
        // Euler((0.1, 0.2, 0.3)).to_matrix()[0][0] == 0.936293363571167 (eul_to_mat3).
        let e = method(
            Py::Euler([0.1, 0.2, 0.3], EulerOrder::XYZ),
            "to_matrix",
            &[],
        )
        .unwrap();
        let Py::Matrix(rows) = e else { panic!() };
        assert_eq!(rows[0][0] as f64, 0.936293363571167);
        assert_eq!(rows[2][0] as f64, -0.19866932928562164);
    }

    #[test]
    fn float_builtin_parses_python_spellings() {
        let f = |s: &str| {
            builtin(
                Builtin::Float,
                &[ArgVal {
                    name: None,
                    value: Py::Str(s.into()),
                }],
            )
        };
        assert!(matches!(f("inf").unwrap(), Py::Float(x) if x == f64::INFINITY));
        assert!(matches!(f("-Infinity").unwrap(), Py::Float(x) if x == f64::NEG_INFINITY));
        assert!(matches!(f("nan").unwrap(), Py::Float(x) if x.is_nan()));
        assert_eq!(
            f("x").unwrap_err().message,
            "could not convert string to float: 'x'"
        );
    }
}
