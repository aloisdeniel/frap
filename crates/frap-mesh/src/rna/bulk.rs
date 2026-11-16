// SPDX-License-Identifier: GPL-3.0-or-later
//! `foreach_get` / `foreach_set` support (ARCHITECTURE §9.4, §13.3, RFC 0001 F-TR-7).
//!
//! Blender semantics, verified on 5.2.4 (`bpy_rna.cc` `foreach_getset` and
//! `RNA_property_collection_raw_*`):
//! - variable-length arrays (`MeshPolygon.vertices`) read and write the concatenation of
//!   the items' arrays;
//! - a buffer shorter than that, or a property that is not BOOLEAN/INT/FLOAT, raises
//!   `RuntimeError: internal error setting the array`; `foreach_set` ignores the rest of a
//!   longer buffer;
//! - `foreach_set` of a read-only property (`index`, `normal`) checks the length and then does
//!   nothing;
//! - Python numbers of any kind are accepted for any numeric property (`[1, 0]` sets `select`).

use std::ops::Range;

use frap_rna::traits::index_out_of_range;
use frap_rna::{
    ArrayShape, Bulk, Field, MathTag, PropDef, PropKind, PyExc, RnaError, RnaErrorKind, Value,
};

use super::soa::SoaKind;
use crate::mesh::Mesh;

/// `RuntimeError: internal error setting the array` (Blender's message for every bulk-access
/// failure: wrong length, unsupported type, variable item length).
pub(crate) fn array_error() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "internal error setting the array",
    )
}

/// An empty buffer of the type `prop` reads into.
pub(crate) fn empty_for(prop: &PropDef) -> Result<Bulk, RnaError> {
    match prop.kind {
        PropKind::Float { .. } => Ok(Bulk::F32(Vec::new())),
        PropKind::Int { .. } => Ok(Bulk::I32(Vec::new())),
        PropKind::Bool { .. } => Ok(Bulk::Bool(Vec::new())),
        _ => Err(array_error()),
    }
}

/// Checks `range` against a collection of `len` items.
pub(crate) fn check_range(range: &Range<usize>, len: usize) -> Result<(), RnaError> {
    if range.start > range.end {
        return Err(index_out_of_range(range.start, len));
    }
    if range.end > len {
        return Err(index_out_of_range(range.end - 1, len));
    }
    Ok(())
}

/// Appends a value to the buffer; returns the number of scalars appended.
fn push(out: &mut Bulk, v: &Value) -> Result<usize, RnaError> {
    let n = match (out, v) {
        (Bulk::F32(b), Value::Float(f)) => {
            b.push(*f);
            1
        }
        (Bulk::F32(b), Value::Double(f)) => {
            b.push(*f as f32);
            1
        }
        (Bulk::F32(b), Value::FloatArray { data, .. }) => {
            b.extend_from_slice(data);
            data.len()
        }
        (Bulk::I32(b), Value::Int(i)) => {
            b.push(*i as i32);
            1
        }
        (Bulk::I32(b), Value::IntArray(a)) => {
            b.extend(a.iter().map(|i| *i as i32));
            a.len()
        }
        (Bulk::Bool(b), Value::Bool(x)) => {
            b.push(*x);
            1
        }
        (Bulk::Bool(b), Value::BoolArray(a)) => {
            b.extend_from_slice(a);
            a.len()
        }
        _ => return Err(array_error()),
    };
    Ok(n)
}

/// Item-by-item `foreach_get` through [`SoaKind::get`] (computed properties).
pub(crate) fn generic_get<K: SoaKind + ?Sized>(
    m: &Mesh,
    ctx: K::Ctx,
    prop: &'static PropDef,
    range: Range<usize>,
) -> Result<Bulk, RnaError> {
    let mut out = empty_for(prop)?;
    check_range(&range, K::len(m, ctx))?;
    for i in range {
        let Field::Value(v) = K::get(m, ctx, i, prop)? else {
            return Err(array_error());
        };
        push(&mut out, &v)?;
    }
    Ok(out)
}

/// Scalars per item for a bulk write: the first item's array length (variable-length arrays
/// such as `MeshPolygon.vertices`), else the property's.
fn item_len<K: SoaKind + ?Sized>(
    m: &Mesh,
    ctx: K::Ctx,
    prop: &'static PropDef,
) -> Result<usize, RnaError> {
    if matches!(prop.array, ArrayShape::Scalar) || K::len(m, ctx) == 0 {
        return Ok(prop.array_len());
    }
    Ok(match K::get(m, ctx, 0, prop)? {
        Field::Value(Value::FloatArray { data, .. }) => data.len(),
        Field::Value(Value::IntArray(a)) => a.len(),
        Field::Value(Value::BoolArray(a)) => a.len(),
        _ => prop.array_len(),
    })
}

/// Checks a bulk write of `n` items of `per` scalars (numeric property, at least `n * per`
/// scalars) and returns the first `n * per` scalars: Blender ignores the rest of a longer
/// buffer (verified: `vertices.foreach_set('co', [0.0] * 25)` on 8 vertices succeeds).
pub(crate) fn check_set(
    prop: &PropDef,
    data: &Bulk,
    n: usize,
    per: usize,
) -> Result<Bulk, RnaError> {
    empty_for(prop)?;
    let need = n * per;
    if data.len() < need {
        return Err(array_error());
    }
    Ok(match data {
        Bulk::F32(v) => Bulk::F32(v[..need].to_vec()),
        Bulk::I32(v) => Bulk::I32(v[..need].to_vec()),
        Bulk::Bool(v) => Bulk::Bool(v[..need].to_vec()),
        other => other.clone(),
    })
}

/// Item-by-item `foreach_set` through [`SoaKind::set`].
pub(crate) fn generic_set<K: SoaKind + ?Sized>(
    m: &mut Mesh,
    ctx: K::Ctx,
    prop: &'static PropDef,
    data: &Bulk,
) -> Result<(), RnaError> {
    let n = K::len(m, ctx);
    let per = item_len::<K>(m, ctx, prop)?;
    let data = &check_set(prop, data, n, per)?;
    if prop.is_readonly() {
        return Ok(());
    }
    for i in 0..n {
        K::set(m, ctx, i, prop, value_at(prop, data, i, per))?;
    }
    Ok(())
}

/// Scalar `k` of the buffer as `f64`.
fn scalar(data: &Bulk, k: usize) -> f64 {
    match data {
        Bulk::F32(v) => f64::from(v[k]),
        Bulk::I32(v) => f64::from(v[k]),
        Bulk::Bool(v) => f64::from(u8::from(v[k])),
        _ => 0.0,
    }
}

/// Scalar `k` as an `i64` (floats truncate toward zero, like Python's `int()` in
/// `foreach_set`), clamped to the property's hard range.
fn int_at(prop: &PropDef, data: &Bulk, k: usize) -> i64 {
    let v = match data {
        Bulk::I32(v) => i64::from(v[k]),
        Bulk::Bool(v) => i64::from(v[k]),
        _ => scalar(data, k) as i64,
    };
    match prop.kind {
        PropKind::Int { hard: (lo, hi), .. } => v.clamp(lo, hi),
        _ => v,
    }
}

/// Scalar `k` as an `f32`, clamped to the property's hard range.
fn float_at(prop: &PropDef, data: &Bulk, k: usize) -> f32 {
    let v = scalar(data, k);
    let v = match prop.kind {
        PropKind::Float { hard: (lo, hi), .. } if !v.is_nan() => v.clamp(lo, hi),
        _ => v,
    };
    v as f32
}

/// Scalar `k` as a `bool` (any non-zero number is true).
fn bool_at(data: &Bulk, k: usize) -> bool {
    match data {
        Bulk::Bool(v) => v[k],
        _ => scalar(data, k) != 0.0,
    }
}

/// The value of item `i` (`per` scalars from `i * per`) as a coerced `Value` for `prop`.
pub(crate) fn value_at(prop: &PropDef, data: &Bulk, i: usize, per: usize) -> Value {
    let r = i * per..i * per + per;
    let scalar_prop = matches!(prop.array, ArrayShape::Scalar);
    match prop.kind {
        PropKind::Float { .. } if scalar_prop => Value::Float(float_at(prop, data, r.start)),
        PropKind::Float { .. } => Value::FloatArray {
            data: r.map(|k| float_at(prop, data, k)).collect(),
            shape: prop.array,
            tag: MathTag::for_subtype(prop.subtype),
        },
        PropKind::Int { .. } if scalar_prop => Value::Int(int_at(prop, data, r.start)),
        PropKind::Int { .. } => Value::IntArray(r.map(|k| int_at(prop, data, k)).collect()),
        PropKind::Bool { .. } if scalar_prop => Value::Bool(bool_at(data, r.start)),
        PropKind::Bool { .. } => Value::BoolArray(r.map(|k| bool_at(data, k)).collect()),
        _ => Value::None,
    }
}

/// The whole buffer as clamped `f32`s.
pub(crate) fn f32s(prop: &PropDef, data: &Bulk) -> Vec<f32> {
    (0..data.len()).map(|k| float_at(prop, data, k)).collect()
}

/// The whole buffer as clamped `i32`s.
pub(crate) fn i32s(prop: &PropDef, data: &Bulk) -> Vec<i32> {
    (0..data.len())
        .map(|k| int_at(prop, data, k) as i32)
        .collect()
}

/// The whole buffer as `bool`s.
pub(crate) fn bools(data: &Bulk) -> Vec<bool> {
    match data {
        Bulk::Bool(v) => v.clone(),
        _ => (0..data.len()).map(|k| bool_at(data, k)).collect(),
    }
}

/// Rows `range` of an `[f32; N]` column, flattened.
pub(crate) fn f32_rows<const N: usize>(rows: &[[f32; N]], range: Range<usize>) -> Bulk {
    Bulk::F32(rows[range].iter().flatten().copied().collect())
}

/// Rows `range` of an `[i32; N]` column, flattened.
pub(crate) fn i32_rows<const N: usize>(rows: &[[i32; N]], range: Range<usize>) -> Bulk {
    Bulk::I32(rows[range].iter().flatten().copied().collect())
}

/// `range` of an `i32` column.
pub(crate) fn i32_slice(col: &[i32], range: Range<usize>) -> Bulk {
    Bulk::I32(col[range].to_vec())
}

/// `range` of a `bool` column.
pub(crate) fn bool_slice(col: &[bool], range: Range<usize>) -> Bulk {
    Bulk::Bool(col[range].to_vec())
}

/// Item indices `range` (`index` properties).
pub(crate) fn indices(range: Range<usize>) -> Bulk {
    Bulk::I32(range.map(|i| i as i32).collect())
}

/// Splits a flat buffer into `[T; N]` rows.
pub(crate) fn rows<T: Copy + Default, const N: usize>(flat: &[T]) -> Vec<[T; N]> {
    flat.chunks_exact(N)
        .map(|c| {
            let mut r = [T::default(); N];
            r.copy_from_slice(c);
            r
        })
        .collect()
}
