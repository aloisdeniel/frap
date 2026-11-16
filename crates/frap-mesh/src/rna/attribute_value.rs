// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed attribute values: `attributes[n].data[i]` (`FloatAttributeValue.value`,
//! `FloatVectorAttributeValue.vector`, `ByteColorAttributeValue.color`, …) and
//! `uv_layers[n].uv[i]` (`Float2AttributeValue`), over the attribute's column.
//!
//! - BYTE_COLOR `color` is scene linear (decoded from sRGB bytes), `color_srgb` the bytes
//!   / 255; FLOAT_COLOR `color` is stored, `color_srgb` encoded (see [`super::color`]).
//! - FLOAT4X4 is stored column-major (`m[col][row]`, Blender's `float4x4`): `value` reads the
//!   matrix rows (row-major `Value`, like every frap matrix), `foreach_get/set` the raw memory
//!   (verified: assigning rows `((1,2,3,4), …)` and `foreach_get` gives `1..16`).
//! - STRING values cannot be bulk-accessed (`RuntimeError`, as in Blender).

use std::ops::Range;
use std::sync::Arc;

use frap_rna::traits::{FromRnaValue, SmolStr, ToRnaValue};
use frap_rna::{Bulk, Field, MathTag, PropDef, RnaError, SetOutcome, StructDef, Value};
use frap_spec_rna as spec;

use super::attribute::{nth, nth_mut};
use super::bulk;
use super::color;
use super::soa::{self, SoaKind};
use crate::mesh::{AttrData, Mesh};

/// True when the data type has a dumped value struct (every type but FLOAT4).
pub(crate) fn has_values(data: &AttrData) -> bool {
    value_srna(data).is_some()
}

/// The value item struct of a data type.
fn value_srna(data: &AttrData) -> Option<&'static StructDef> {
    Some(match data {
        AttrData::Float(_) => &spec::FLOAT_ATTRIBUTE_VALUE,
        AttrData::Int(_) => &spec::INT_ATTRIBUTE_VALUE,
        AttrData::Int8(_) => &spec::BYTE_INT_ATTRIBUTE_VALUE,
        AttrData::Bool(_) => &spec::BOOL_ATTRIBUTE_VALUE,
        AttrData::Float2(_) => &spec::FLOAT2_ATTRIBUTE_VALUE,
        AttrData::Float3(_) => &spec::FLOAT_VECTOR_ATTRIBUTE_VALUE,
        AttrData::FloatColor(_) => &spec::FLOAT_COLOR_ATTRIBUTE_VALUE,
        AttrData::ByteColor(_) => &spec::BYTE_COLOR_ATTRIBUTE_VALUE,
        AttrData::Int16x2(_) => &spec::SHORT2_ATTRIBUTE_VALUE,
        AttrData::Int32x2(_) => &spec::INT2_ATTRIBUTE_VALUE,
        AttrData::Quaternion(_) => &spec::QUATERNION_ATTRIBUTE_VALUE,
        AttrData::Float4x4(_) => &spec::FLOAT4X4_ATTRIBUTE_VALUE,
        AttrData::String(_) => &spec::STRING_ATTRIBUTE_VALUE,
        _ => return None,
    })
}

/// The property a data type's value struct exposes its value under.
fn value_prop(data: &AttrData) -> &'static str {
    match data {
        AttrData::Float2(_) | AttrData::Float3(_) => "vector",
        AttrData::FloatColor(_) | AttrData::ByteColor(_) => "color",
        _ => "value",
    }
}

/// Column-major `m[col][row]` → row-major.
fn mat_rows(m: &[[f32; 4]; 4]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for r in 0..4 {
        for c in 0..4 {
            out[r * 4 + c] = m[c][r];
        }
    }
    out
}

/// Row-major → column-major `m[col][row]`.
fn mat_cols(rows: [f32; 16]) -> [[f32; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            m[c][r] = rows[r * 4 + c];
        }
    }
    m
}

/// Reads element `i` of a column for `prop` (`None`: the struct has no such property).
fn read(data: &AttrData, i: usize, prop: &'static PropDef) -> Option<Value> {
    let tag = MathTag::for_subtype(prop.subtype);
    let none = MathTag::None;
    let p = prop.identifier;
    Some(match data {
        AttrData::Float(v) if p == "value" => v[i].to_rna_value(prop, none),
        AttrData::Int(v) if p == "value" => v[i].to_rna_value(prop, none),
        AttrData::Int8(v) if p == "value" => v[i].to_rna_value(prop, none),
        AttrData::Bool(v) if p == "value" => Value::Bool(v[i]),
        AttrData::Float2(v) if p == "vector" => v[i].to_rna_value(prop, tag),
        AttrData::Float3(v) if p == "vector" => v[i].to_rna_value(prop, tag),
        AttrData::FloatColor(v) if p == "color" => v[i].to_rna_value(prop, tag),
        AttrData::FloatColor(v) if p == "color_srgb" => {
            color::linear4_to_srgb(v[i]).to_rna_value(prop, tag)
        }
        AttrData::ByteColor(v) if p == "color" => {
            color::byte_to_linear(v[i]).to_rna_value(prop, tag)
        }
        AttrData::ByteColor(v) if p == "color_srgb" => {
            color::byte_to_float(v[i]).to_rna_value(prop, tag)
        }
        AttrData::Int16x2(v) if p == "value" => v[i].map(i32::from).to_rna_value(prop, none),
        AttrData::Int32x2(v) if p == "value" => v[i].to_rna_value(prop, none),
        AttrData::Quaternion(v) if p == "value" => v[i].to_rna_value(prop, tag),
        AttrData::Float4x4(v) if p == "value" => mat_rows(&v[i]).to_rna_value(prop, tag),
        AttrData::String(v) if p == "value" => v[i].to_rna_value(prop, none),
        _ => return None,
    })
}

/// Writes element `i` (`Ok(false)`: the struct has no such property).
fn write(
    data: &mut AttrData,
    i: usize,
    prop: &'static PropDef,
    v: Value,
) -> Result<bool, RnaError> {
    let p = prop.identifier;
    match data {
        AttrData::Float(c) if p == "value" => Arc::make_mut(c)[i] = f32::from_rna_value(prop, v)?,
        AttrData::Int(c) if p == "value" => Arc::make_mut(c)[i] = i32::from_rna_value(prop, v)?,
        AttrData::Int8(c) if p == "value" => Arc::make_mut(c)[i] = i8::from_rna_value(prop, v)?,
        AttrData::Bool(c) if p == "value" => Arc::make_mut(c)[i] = bool::from_rna_value(prop, v)?,
        AttrData::Float2(c) if p == "vector" => {
            Arc::make_mut(c)[i] = <[f32; 2]>::from_rna_value(prop, v)?
        }
        AttrData::Float3(c) if p == "vector" => {
            Arc::make_mut(c)[i] = <[f32; 3]>::from_rna_value(prop, v)?
        }
        AttrData::FloatColor(c) if p == "color" => {
            Arc::make_mut(c)[i] = <[f32; 4]>::from_rna_value(prop, v)?
        }
        AttrData::FloatColor(c) if p == "color_srgb" => {
            Arc::make_mut(c)[i] = color::srgb4_to_linear(<[f32; 4]>::from_rna_value(prop, v)?)
        }
        AttrData::ByteColor(c) if p == "color" => {
            Arc::make_mut(c)[i] = color::linear_to_byte(<[f32; 4]>::from_rna_value(prop, v)?)
        }
        AttrData::ByteColor(c) if p == "color_srgb" => {
            Arc::make_mut(c)[i] = color::float_to_byte(<[f32; 4]>::from_rna_value(prop, v)?)
        }
        AttrData::Int16x2(c) if p == "value" => {
            let x = <[i32; 2]>::from_rna_value(prop, v)?;
            Arc::make_mut(c)[i] = x.map(|k| k.clamp(i16::MIN.into(), i16::MAX.into()) as i16)
        }
        AttrData::Int32x2(c) if p == "value" => {
            Arc::make_mut(c)[i] = <[i32; 2]>::from_rna_value(prop, v)?
        }
        AttrData::Quaternion(c) if p == "value" => {
            Arc::make_mut(c)[i] = <[f32; 4]>::from_rna_value(prop, v)?
        }
        AttrData::Float4x4(c) if p == "value" => {
            Arc::make_mut(c)[i] = mat_cols(<[f32; 16]>::from_rna_value(prop, v)?)
        }
        AttrData::String(c) if p == "value" => {
            Arc::make_mut(c)[i] = SmolStr::from_rna_value(prop, v)?
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `range` of a column as a bulk buffer (`None`: no fast path, read item by item).
fn read_bulk(data: &AttrData, prop: &PropDef, range: Range<usize>) -> Option<Bulk> {
    let p = prop.identifier;
    Some(match data {
        AttrData::Float(v) if p == "value" => Bulk::F32(v[range].to_vec()),
        AttrData::Int(v) if p == "value" => bulk::i32_slice(v, range),
        AttrData::Int8(v) if p == "value" => {
            Bulk::I32(v[range].iter().map(|x| i32::from(*x)).collect())
        }
        AttrData::Bool(v) if p == "value" => bulk::bool_slice(v, range),
        AttrData::Float2(v) if p == "vector" => bulk::f32_rows(v, range),
        AttrData::Float3(v) if p == "vector" => bulk::f32_rows(v, range),
        AttrData::FloatColor(v) if p == "color" => bulk::f32_rows(v, range),
        AttrData::Int16x2(v) if p == "value" => {
            Bulk::I32(v[range].iter().flat_map(|x| x.map(i32::from)).collect())
        }
        AttrData::Int32x2(v) if p == "value" => bulk::i32_rows(v, range),
        AttrData::Quaternion(v) if p == "value" => bulk::f32_rows(v, range),
        AttrData::Float4x4(v) if p == "value" => {
            Bulk::F32(v[range].iter().flatten().flatten().copied().collect())
        }
        _ => return None,
    })
}

/// Replaces a whole column from a bulk buffer (`false`: no fast path).
fn write_bulk(data: &mut AttrData, prop: &PropDef, buf: &Bulk) -> bool {
    let p = prop.identifier;
    match data {
        AttrData::Float(c) if p == "value" => *Arc::make_mut(c) = bulk::f32s(prop, buf),
        AttrData::Int(c) if p == "value" => *Arc::make_mut(c) = bulk::i32s(prop, buf),
        AttrData::Int8(c) if p == "value" => {
            *Arc::make_mut(c) = bulk::i32s(prop, buf).into_iter().map(|x| x as i8).collect()
        }
        AttrData::Bool(c) if p == "value" => *Arc::make_mut(c) = bulk::bools(buf),
        AttrData::Float2(c) if p == "vector" => {
            *Arc::make_mut(c) = bulk::rows(&bulk::f32s(prop, buf))
        }
        AttrData::Float3(c) if p == "vector" => {
            *Arc::make_mut(c) = bulk::rows(&bulk::f32s(prop, buf))
        }
        AttrData::FloatColor(c) if p == "color" => {
            *Arc::make_mut(c) = bulk::rows(&bulk::f32s(prop, buf))
        }
        AttrData::Int16x2(c) if p == "value" => {
            *Arc::make_mut(c) = bulk::rows::<i32, 2>(&bulk::i32s(prop, buf))
                .into_iter()
                .map(|r| r.map(|x| x as i16))
                .collect()
        }
        AttrData::Int32x2(c) if p == "value" => {
            *Arc::make_mut(c) = bulk::rows(&bulk::i32s(prop, buf))
        }
        AttrData::Quaternion(c) if p == "value" => {
            *Arc::make_mut(c) = bulk::rows(&bulk::f32s(prop, buf))
        }
        AttrData::Float4x4(c) if p == "value" => {
            *Arc::make_mut(c) = bulk::rows::<f32, 16>(&bulk::f32s(prop, buf))
                .into_iter()
                .map(|r| {
                    let mut m = [[0.0; 4]; 4];
                    for (k, x) in r.into_iter().enumerate() {
                        m[k / 4][k % 4] = x;
                    }
                    m
                })
                .collect()
        }
        _ => return false,
    }
    true
}

/// `attributes[n].data` and `uv_layers[n].uv` (context: the attribute's position in the set).
pub(crate) struct AttrValues;

impl SoaKind for AttrValues {
    type Ctx = usize;

    fn item_type(m: &Mesh, pos: usize) -> &'static StructDef {
        nth(m, pos)
            .and_then(|a| value_srna(&a.data))
            .unwrap_or(&spec::FLOAT_ATTRIBUTE_VALUE)
    }
    fn len(m: &Mesh, pos: usize) -> usize {
        nth(m, pos).map_or(0, |a| a.data.len())
    }
    fn get<'a>(
        m: &'a Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        nth(m, pos)
            .and_then(|a| read(&a.data, i, prop))
            .map(Field::Value)
            .ok_or_else(|| soa::no_attr::<Self>(m, pos, i, prop))
    }
    fn set(
        m: &mut Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        let known = nth(m, pos).is_some_and(|a| read(&a.data, i, prop).is_some());
        if !known {
            return Err(soa::no_attr::<Self>(m, pos, i, prop));
        }
        if let Some(a) = nth_mut(m, pos) {
            write(&mut a.data, i, prop, v)?;
        }
        Ok(SetOutcome::Done)
    }
    fn foreach_get(
        m: &Mesh,
        pos: usize,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, Self::len(m, pos))?;
        match nth(m, pos).and_then(|a| read_bulk(&a.data, prop, range.clone())) {
            Some(b) => Ok(b),
            None => bulk::generic_get::<Self>(m, pos, prop, range),
        }
    }
    fn foreach_set(
        m: &mut Mesh,
        pos: usize,
        prop: &'static PropDef,
        data: &Bulk,
    ) -> Result<(), RnaError> {
        let Some(a) = nth(m, pos) else {
            return bulk::generic_set::<Self>(m, pos, prop, data);
        };
        let (n, fast) = (
            a.data.len(),
            read_bulk(&a.data, prop, 0..0).is_some() && value_prop(&a.data) == prop.identifier,
        );
        if !fast {
            return bulk::generic_set::<Self>(m, pos, prop, data);
        }
        let data = &bulk::check_set(prop, data, n, prop.array_len())?;
        if let Some(a) = nth_mut(m, pos) {
            write_bulk(&mut a.data, prop, data);
        }
        Ok(())
    }
}
