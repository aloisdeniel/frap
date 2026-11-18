// SPDX-License-Identifier: GPL-3.0-or-later
//! Attribute values across types and domains: the implicit type conversions
//! (`blenkernel/intern/type_conversions.cc`) and the mesh domain interpolation
//! (`blenkernel/intern/mesh_attributes.cc`, `adapt_mesh_domain_*`, with the default mixers of
//! `BKE_attribute_math.hh`), Blender 5.2.4, GPL-2.0-or-later. Used by `mesh.attribute_set`
//! (a value given for one type written to an attribute of another) and
//! `geometry.attribute_convert` / `geometry.color_attribute_convert`
//! (`lookup_or_default(name, domain, type)`: the domain first, in the source type, then the
//! type).
//!
//! QUATERNION, FLOAT4X4 and STRING take part in no conversion here (frap refuses to convert
//! them; Blender converts quaternions and matrices to and from a few types).

use std::sync::Arc;

use frap_mesh::{AttrData, Domain, Mesh};

use super::attrs::encode_byte_color;

/// One attribute value.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Val {
    /// FLOAT
    F(f32),
    /// FLOAT2
    F2([f32; 2]),
    /// FLOAT_VECTOR
    F3([f32; 3]),
    /// FLOAT4
    F4([f32; 4]),
    /// FLOAT_COLOR (scene linear)
    C([f32; 4]),
    /// BYTE_COLOR (sRGB encoded)
    Bc([u8; 4]),
    /// INT
    I(i32),
    /// INT8
    I8(i8),
    /// INT16_2D
    S2([i16; 2]),
    /// INT32_2D
    I2([i32; 2]),
    /// BOOLEAN
    B(bool),
}

/// `srgb_to_linearrgb`.
fn srgb_to_linear(c: f32) -> f32 {
    if c < 0.04045 {
        if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) }
    } else {
        ((c + 0.055) * (1.0 / 1.055)).powf(2.4)
    }
}

/// `color::decode`: a byte colour as scene-linear floats.
pub(crate) fn decode_byte_color(b: [u8; 4]) -> [f32; 4] {
    let f = |x: u8| f32::from(x) * (1.0 / 255.0);
    [
        srgb_to_linear(f(b[0])),
        srgb_to_linear(f(b[1])),
        srgb_to_linear(f(b[2])),
        f(b[3]),
    ]
}

/// `IMB_colormanagement_get_luminance` with the default config's Rec.709 coefficients.
fn luminance(c: [f32; 4]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// `std::clamp` into `int8_t` (`float_to_int8`).
fn f_to_i8(a: f32) -> i8 {
    a.clamp(f32::from(i8::MIN), f32::from(i8::MAX)) as i8
}
/// `int_to_int8`.
fn i_to_i8(a: i32) -> i8 {
    a.clamp(i32::from(i8::MIN), i32::from(i8::MAX)) as i8
}
/// `math::midpoint` of two integers (rounds towards `a`).
fn midpoint(a: i64, b: i64) -> i64 {
    let (sign, small, large) = if a > b { (-1, b, a) } else { (1, a, b) };
    a + sign * ((large - small) / 2)
}

/// The data type identifier of a value.
pub(crate) fn type_of(v: Val) -> &'static str {
    match v {
        Val::F(_) => "FLOAT",
        Val::F2(_) => "FLOAT2",
        Val::F3(_) => "FLOAT_VECTOR",
        Val::F4(_) => "FLOAT4",
        Val::C(_) => "FLOAT_COLOR",
        Val::Bc(_) => "BYTE_COLOR",
        Val::I(_) => "INT",
        Val::I8(_) => "INT8",
        Val::S2(_) => "INT16_2D",
        Val::I2(_) => "INT32_2D",
        Val::B(_) => "BOOLEAN",
    }
}

// Ported from blender/source/blender/blenkernel/intern/type_conversions.cc.
/// The implicit conversion of `v` to `to` (identity for the same type); `None` for a type
/// without conversions.
pub(crate) fn convert(v: Val, to: &str) -> Option<Val> {
    if type_of(v) == to {
        return Some(v);
    }
    use Val::*;
    Some(match (v, to) {
        // From FLOAT.
        (F(a), "FLOAT2") => F2([a; 2]),
        (F(a), "FLOAT_VECTOR") => F3([a; 3]),
        (F(a), "FLOAT4") => F4([a; 4]),
        (F(a), "INT") => I(a as i32),
        (F(a), "INT16_2D") => S2([a as i16; 2]),
        (F(a), "INT32_2D") => I2([a as i32; 2]),
        (F(a), "BOOLEAN") => B(a > 0.0),
        (F(a), "INT8") => I8(f_to_i8(a)),
        (F(a), "FLOAT_COLOR") => C([a, a, a, 1.0]),
        (F(a), "BYTE_COLOR") => Bc(encode_byte_color([a, a, a, 1.0])),
        // From FLOAT2.
        (F2(a), "FLOAT_VECTOR") => F3([a[0], a[1], 0.0]),
        (F2(a), "FLOAT4") => F4([a[0], a[1], 0.0, 0.0]),
        (F2(a), "FLOAT") => F((a[0] + a[1]) / 2.0),
        (F2(a), "INT") => I(((a[0] + a[1]) / 2.0) as i32),
        (F2(a), "INT16_2D") => S2([a[0] as i16, a[1] as i16]),
        (F2(a), "INT32_2D") => I2([a[0] as i32, a[1] as i32]),
        (F2(a), "BOOLEAN") => B(a != [0.0; 2]),
        (F2(a), "INT8") => I8(f_to_i8((a[0] + a[1]) / 2.0)),
        (F2(a), "FLOAT_COLOR") => C([a[0], a[1], 0.0, 1.0]),
        (F2(a), "BYTE_COLOR") => Bc(encode_byte_color([a[0], a[1], 0.0, 1.0])),
        // From FLOAT_VECTOR.
        (F3(a), "BOOLEAN") => B(a != [0.0; 3]),
        (F3(a), "INT8") => I8(f_to_i8((a[0] + a[1] + a[2]) / 3.0)),
        (F3(a), "FLOAT") => F((a[0] + a[1] + a[2]) / 3.0),
        (F3(a), "INT") => I(((a[0] + a[1] + a[2]) / 3.0) as i32),
        (F3(a), "INT16_2D") => S2([a[0] as i16, a[1] as i16]),
        (F3(a), "INT32_2D") => I2([a[0] as i32, a[1] as i32]),
        (F3(a), "FLOAT2") => F2([a[0], a[1]]),
        (F3(a), "FLOAT4") => F4([a[0], a[1], a[2], 0.0]),
        (F3(a), "FLOAT_COLOR") => C([a[0], a[1], a[2], 1.0]),
        (F3(a), "BYTE_COLOR") => Bc(encode_byte_color([a[0], a[1], a[2], 1.0])),
        // From FLOAT4.
        (F4(a), "BOOLEAN") => B(a != [0.0; 4]),
        (F4(a), "INT8") => I8(f_to_i8((a[0] + a[1] + a[2] + a[3]) / 4.0)),
        (F4(a), "FLOAT") => F((a[0] + a[1] + a[2] + a[3]) / 4.0),
        (F4(a), "INT") => I(((a[0] + a[1] + a[2] + a[3]) / 4.0) as i32),
        (F4(a), "INT16_2D") => S2([a[0] as i16, a[1] as i16]),
        (F4(a), "INT32_2D") => I2([a[0] as i32, a[1] as i32]),
        (F4(a), "FLOAT2") => F2([a[0], a[1]]),
        (F4(a), "FLOAT_VECTOR") => F3([a[0], a[1], a[2]]),
        (F4(a), "FLOAT_COLOR") => C(a),
        (F4(a), "BYTE_COLOR") => Bc(encode_byte_color(a)),
        // From INT.
        (I(a), "BOOLEAN") => B(a > 0),
        (I(a), "INT8") => I8(i_to_i8(a)),
        (I(a), "INT16_2D") => S2([a as i16; 2]),
        (I(a), "INT32_2D") => I2([a; 2]),
        (I(a), "FLOAT") => F(a as f32),
        (I(a), "FLOAT2") => F2([a as f32; 2]),
        (I(a), "FLOAT_VECTOR") => F3([a as f32; 3]),
        (I(a), "FLOAT4") => F4([a as f32; 4]),
        (I(a), "FLOAT_COLOR") => C([a as f32, a as f32, a as f32, 1.0]),
        (I(a), "BYTE_COLOR") => Bc(encode_byte_color([a as f32, a as f32, a as f32, 1.0])),
        // From INT16_2D.
        (S2(a), "BOOLEAN") => B(a != [0; 2]),
        (S2(a), "FLOAT2") => F2([f32::from(a[0]), f32::from(a[1])]),
        (S2(a), "INT") => I(midpoint(i64::from(a[0]), i64::from(a[1])) as i32),
        (S2(a), "INT32_2D") => I2([i32::from(a[0]), i32::from(a[1])]),
        (S2(a), "INT8") => I8(i_to_i8(midpoint(i64::from(a[0]), i64::from(a[1])) as i32)),
        (S2(a), "FLOAT") => F((f32::from(a[0]) + f32::from(a[1])) / 2.0),
        (S2(a), "FLOAT_VECTOR") => F3([f32::from(a[0]), f32::from(a[1]), 0.0]),
        (S2(a), "FLOAT4") => F4([f32::from(a[0]), f32::from(a[1]), 0.0, 0.0]),
        (S2(a), "FLOAT_COLOR") => C([f32::from(a[0]), f32::from(a[1]), 0.0, 1.0]),
        (S2(a), "BYTE_COLOR") => {
            Bc(encode_byte_color([f32::from(a[0]), f32::from(a[1]), 0.0, 1.0]))
        }
        // From INT32_2D.
        (I2(a), "BOOLEAN") => B(a != [0; 2]),
        (I2(a), "FLOAT2") => F2([a[0] as f32, a[1] as f32]),
        (I2(a), "INT") => I(midpoint(i64::from(a[0]), i64::from(a[1])) as i32),
        (I2(a), "INT16_2D") => S2([a[0] as i16, a[1] as i16]),
        (I2(a), "INT8") => I8(i_to_i8(midpoint(i64::from(a[0]), i64::from(a[1])) as i32)),
        (I2(a), "FLOAT") => F((a[0] as f32 + a[1] as f32) / 2.0),
        (I2(a), "FLOAT_VECTOR") => F3([a[0] as f32, a[1] as f32, 0.0]),
        (I2(a), "FLOAT4") => F4([a[0] as f32, a[1] as f32, 0.0, 0.0]),
        (I2(a), "FLOAT_COLOR") => C([a[0] as f32, a[1] as f32, 0.0, 1.0]),
        (I2(a), "BYTE_COLOR") => Bc(encode_byte_color([a[0] as f32, a[1] as f32, 0.0, 1.0])),
        // From INT8.
        (I8(a), "BOOLEAN") => B(a > 0),
        (I8(a), "INT") => I(i32::from(a)),
        (I8(a), "INT16_2D") => S2([i16::from(a); 2]),
        (I8(a), "INT32_2D") => I2([i32::from(a); 2]),
        (I8(a), "FLOAT") => F(f32::from(a)),
        (I8(a), "FLOAT2") => F2([f32::from(a); 2]),
        (I8(a), "FLOAT_VECTOR") => F3([f32::from(a); 3]),
        (I8(a), "FLOAT4") => F4([f32::from(a); 4]),
        (I8(a), "FLOAT_COLOR") => C([f32::from(a), f32::from(a), f32::from(a), 1.0]),
        (I8(a), "BYTE_COLOR") => {
            let f = f32::from(a);
            Bc(encode_byte_color([f, f, f, 1.0]))
        }
        // From BOOLEAN.
        (B(a), "FLOAT") => F(if a { 1.0 } else { 0.0 }),
        (B(a), "INT8") => I8(i8::from(a)),
        (B(a), "INT") => I(i32::from(a)),
        (B(a), "INT16_2D") => S2([i16::from(a); 2]),
        (B(a), "INT32_2D") => I2([i32::from(a); 2]),
        (B(a), "FLOAT2") => F2([if a { 1.0 } else { 0.0 }; 2]),
        (B(a), "FLOAT_VECTOR") => F3([if a { 1.0 } else { 0.0 }; 3]),
        (B(a), "FLOAT4") => F4([if a { 1.0 } else { 0.0 }; 4]),
        (B(a), "FLOAT_COLOR") => C(if a { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }),
        (B(a), "BYTE_COLOR") => {
            Bc(encode_byte_color(if a { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }))
        }
        // From FLOAT_COLOR.
        (C(a), "BOOLEAN") => B(luminance(a) > 0.0),
        (C(a), "FLOAT") => F(luminance(a)),
        (C(a), "INT") => I(luminance(a) as i32),
        (C(a), "INT16_2D") => S2([a[0] as i16, a[1] as i16]),
        (C(a), "INT32_2D") => I2([a[0] as i32, a[1] as i32]),
        (C(a), "INT8") => I8(i_to_i8(luminance(a) as i32)),
        (C(a), "FLOAT2") => F2([a[0], a[1]]),
        (C(a), "FLOAT_VECTOR") => F3([a[0], a[1], a[2]]),
        (C(a), "FLOAT4") => F4(a),
        (C(a), "BYTE_COLOR") => Bc(encode_byte_color(a)),
        // From BYTE_COLOR.
        (Bc(a), "BOOLEAN") => B(a[0] > 0 || a[1] > 0 || a[2] > 0),
        (Bc(a), "INT16_2D") => S2([i16::from(a[0]), i16::from(a[1])]),
        (Bc(a), "INT32_2D") => I2([i32::from(a[0]), i32::from(a[1])]),
        (Bc(a), t) => return convert(C(decode_byte_color(a)), t),
        _ => return None,
    })
}

/// The value of element `i` of a column (`None` for the types without conversions).
pub(crate) fn get(data: &AttrData, i: usize) -> Option<Val> {
    Some(match data {
        AttrData::Float(v) => Val::F(v[i]),
        AttrData::Float2(v) => Val::F2(v[i]),
        AttrData::Float3(v) => Val::F3(v[i]),
        AttrData::Float4(v) => Val::F4(v[i]),
        AttrData::FloatColor(v) => Val::C(v[i]),
        AttrData::ByteColor(v) => Val::Bc(v[i]),
        AttrData::Int(v) => Val::I(v[i]),
        AttrData::Int8(v) => Val::I8(v[i]),
        AttrData::Int16x2(v) => Val::S2(v[i]),
        AttrData::Int32x2(v) => Val::I2(v[i]),
        AttrData::Bool(v) => Val::B(v[i]),
        _ => return None,
    })
}

/// Writes `val` (already of the column's type) to element `i`; `false` on a type mismatch.
pub(crate) fn put(data: &mut AttrData, i: usize, val: Val) -> bool {
    match (data, val) {
        (AttrData::Float(v), Val::F(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Float2(v), Val::F2(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Float3(v), Val::F3(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Float4(v), Val::F4(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::FloatColor(v), Val::C(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::ByteColor(v), Val::Bc(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Int(v), Val::I(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Int8(v), Val::I8(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Int16x2(v), Val::S2(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Int32x2(v), Val::I2(x)) => Arc::make_mut(v)[i] = x,
        (AttrData::Bool(v), Val::B(x)) => Arc::make_mut(v)[i] = x,
        _ => return false,
    }
    true
}

/// A column of `vals` of type `data_type` (`None` for an unsupported type).
fn column(data_type: &str, vals: &[Val]) -> Option<AttrData> {
    let mut out = super::attrs::default_column(data_type, vals.len())?;
    for (i, &v) in vals.iter().enumerate() {
        if !put(&mut out, i, v) {
            return None;
        }
    }
    Some(out)
}

/// Components of a value for mixing: floats (byte colours decoded) or integers.
fn comps(v: Val) -> ([f64; 4], usize, bool) {
    let f = |a: &[f32]| {
        let mut o = [0.0; 4];
        for (k, x) in a.iter().enumerate() {
            o[k] = f64::from(*x);
        }
        o
    };
    match v {
        Val::F(a) => (f(&[a]), 1, false),
        Val::F2(a) => (f(&a), 2, false),
        Val::F3(a) => (f(&a), 3, false),
        Val::F4(a) | Val::C(a) => (f(&a), 4, false),
        Val::Bc(a) => (f(&decode_byte_color(a)), 4, false),
        Val::I(a) => ([f64::from(a), 0.0, 0.0, 0.0], 1, true),
        Val::I8(a) => ([f64::from(a), 0.0, 0.0, 0.0], 1, true),
        Val::S2(a) => ([f64::from(a[0]), f64::from(a[1]), 0.0, 0.0], 2, true),
        Val::I2(a) => ([f64::from(a[0]), f64::from(a[1]), 0.0, 0.0], 2, true),
        Val::B(a) => ([if a { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0], 1, true),
    }
}

/// Rebuilds a value of the type of `like` from mixed components.
fn from_comps(like: Val, c: [f64; 4]) -> Val {
    let f = |k: usize| c[k] as f32;
    match like {
        Val::F(_) => Val::F(f(0)),
        Val::F2(_) => Val::F2([f(0), f(1)]),
        Val::F3(_) => Val::F3([f(0), f(1), f(2)]),
        Val::F4(_) => Val::F4([f(0), f(1), f(2), f(3)]),
        Val::C(_) => Val::C([f(0), f(1), f(2), f(3)]),
        Val::Bc(_) => Val::Bc(encode_byte_color([f(0), f(1), f(2), f(3)])),
        Val::I(_) => Val::I(c[0] as i32),
        Val::I8(_) => Val::I8(c[0] as i8),
        Val::S2(_) => Val::S2([c[0] as i16, c[1] as i16]),
        Val::I2(_) => Val::I2([c[0] as i32, c[1] as i32]),
        Val::B(_) => Val::B(c[0] != 0.0),
    }
}

fn domain_len(m: &Mesh, d: Domain) -> usize {
    m.domain_size(d)
}

/// The corner after `c` in face `f`.
fn next_corner(m: &Mesh, f: usize, c: usize) -> usize {
    let r = m.face(f);
    if c + 1 == r.end { r.start } else { c + 1 }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_attributes.cc (the iteration order
// of adapt_mesh_domain_*_impl).
/// Calls `mix(dst, src)` for every contribution of the interpolation from `from` to `to`.
fn for_each_contribution(m: &Mesh, from: Domain, to: Domain, mut mix: impl FnMut(usize, usize)) {
    let cv = m.corner_verts();
    let ce = m.corner_edges();
    match (from, to) {
        (Domain::Point, Domain::Face) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    mix(f, cv[c] as usize);
                }
            }
        }
        (Domain::Edge, Domain::Face) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    mix(f, ce[c] as usize);
                }
            }
        }
        (Domain::Corner, Domain::Face) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    mix(f, c);
                }
            }
        }
        (Domain::Point, Domain::Edge) => {
            for (e, [a, b]) in m.edges().iter().enumerate() {
                mix(e, *a as usize);
                mix(e, *b as usize);
            }
        }
        (Domain::Face, Domain::Edge) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    mix(ce[c] as usize, f);
                }
            }
        }
        (Domain::Corner, Domain::Edge) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    let e = ce[c] as usize;
                    mix(e, c);
                    mix(e, next_corner(m, f, c));
                }
            }
        }
        (Domain::Point, Domain::Corner) => {
            for (c, v) in cv.iter().enumerate() {
                mix(c, *v as usize);
            }
        }
        (Domain::Face, Domain::Corner) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    mix(c, f);
                }
            }
        }
        (Domain::Corner, Domain::Point) => {
            for (c, v) in cv.iter().enumerate() {
                mix(*v as usize, c);
            }
        }
        (Domain::Face, Domain::Point) => {
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    mix(cv[c] as usize, f);
                }
            }
        }
        (Domain::Edge, Domain::Point) => {
            for (e, [a, b]) in m.edges().iter().enumerate() {
                mix(*a as usize, e);
                mix(*b as usize, e);
            }
        }
        (Domain::Edge, Domain::Corner) => {
            for f in 0..m.faces_num() {
                let r = m.face(f);
                for c in r.clone() {
                    let prev = if c == r.start { r.end - 1 } else { c - 1 };
                    mix(c, ce[c] as usize);
                    mix(c, ce[prev] as usize);
                }
            }
        }
        _ => {}
    }
}

/// Values of `data` (on `from`) interpolated to `to` in their own type: booleans with
/// Blender's any-of / all-of rules, other types averaged (floats accumulated in f32, integers
/// in f64 and rounded, byte colours in linear float); elements without contributions get the
/// type's zero.
fn adapt(m: &Mesh, data: &AttrData, from: Domain, to: Domain) -> Option<Vec<Val>> {
    let n = data.len();
    let src: Vec<Val> = (0..n).map(|i| get(data, i)).collect::<Option<_>>()?;
    if from == to {
        return Some(src);
    }
    let len = domain_len(m, to);
    if let AttrData::Bool(v) = data {
        let any_of = matches!(
            (from, to),
            (Domain::Face, Domain::Edge)
                | (Domain::Face, Domain::Point)
                | (Domain::Edge, Domain::Point)
        );
        let mut seen = vec![false; len];
        let mut out = vec![!any_of; len];
        for_each_contribution(m, from, to, |d, s| {
            seen[d] = true;
            if any_of {
                out[d] |= v[s];
            } else {
                out[d] &= v[s];
            }
        });
        return Some(
            out.into_iter()
                .zip(seen)
                .map(|(o, s)| Val::B(o && s))
                .collect(),
        );
    }
    let like = *src.first()?;
    let (_, width, integer) = comps(like);
    let mut sum = vec![[0.0f64; 4]; len];
    let mut weight = vec![0u32; len];
    for_each_contribution(m, from, to, |d, s| {
        let (c, _, _) = comps(src[s]);
        for k in 0..width {
            sum[d][k] = if integer {
                sum[d][k] + c[k]
            } else {
                f64::from(sum[d][k] as f32 + c[k] as f32)
            };
        }
        weight[d] += 1;
    });
    Some(
        sum.into_iter()
            .zip(weight)
            .map(|(mut s, w)| {
                for x in s.iter_mut().take(width) {
                    *x = if w == 0 {
                        0.0
                    } else if integer {
                        (*x / f64::from(w)).round()
                    } else {
                        f64::from(*x as f32 * (1.0f32 / w as f32))
                    };
                }
                if w == 0 && matches!(like, Val::Bc(_)) {
                    return Val::Bc([0; 4]);
                }
                from_comps(like, s)
            })
            .collect(),
    )
}

/// `attributes.lookup_or_default(name, domain, type)` for an attribute of `m`: its values
/// interpolated to `to`, then converted to `data_type`. `None` when its type has no
/// conversion.
pub(crate) fn lookup_converted(
    m: &Mesh,
    data: &AttrData,
    from: Domain,
    to: Domain,
    data_type: &str,
) -> Option<AttrData> {
    let vals = adapt(m, data, from, to)?;
    let conv: Vec<Val> = vals
        .into_iter()
        .map(|v| convert(v, data_type))
        .collect::<Option<_>>()?;
    if conv.is_empty() {
        return super::attrs::default_column(data_type, 0);
    }
    column(data_type, &conv)
}

// Ported from blender/source/blender/editors/geometry/geometry_attributes.cc (convert_attribute).
/// `convert_attribute` on a mesh in Object Mode: the attribute `name` is replaced by one on
/// `domain` of `data_type` (values interpolated and converted; the type's default when its
/// type has no conversion), **appended at the end of the attribute list**. The active
/// attribute, active/default colour and active/render UV map that named it keep naming it
/// (when it still is a colour / UV map). `Err` carries Blender's report.
pub(crate) fn convert_attribute(
    m: &mut Mesh,
    names: &mut super::attrs::Names,
    name: &str,
    domain: Domain,
    data_type: &str,
) -> Result<(), String> {
    use super::attrs;
    if data_type == "STRING" {
        return Err("Cannot convert to the selected type".into());
    }
    let Some(src) = m.attributes().get(name).cloned() else {
        return Err(format!("Attribute '{name}' does not exist"));
    };
    let was_active = names.active.as_deref() == Some(name);
    let was_active_color = names.color_active.as_deref() == Some(name);
    let was_default_color = names.color_default.as_deref() == Some(name);
    let was_active_uv = names.uv_active.as_deref() == Some(name);
    let was_default_uv = names.uv_render.as_deref() == Some(name);
    let len = m.domain_size(domain);
    let data = lookup_converted(m, &src.data, src.domain, domain, data_type)
        .or_else(|| attrs::default_column(data_type, len))
        .ok_or_else(|| "Cannot convert to the selected type".to_string())?;
    let index = attrs::index_of(&attrs::Store::layers(m), names.active.as_deref());
    attrs::remove(m, names, name)?;
    if index >= 0 && !was_active {
        names.active = attrs::name_at(&attrs::Store::layers(m), index, false);
    }
    attrs::Store::insert(m, name.into(), domain, data);
    if was_active {
        names.active = Some(name.into());
    }
    if attrs::is_color(domain, data_type) {
        if was_active_color {
            names.color_active = Some(name.into());
        }
        if was_default_color {
            names.color_default = Some(name.into());
        }
    } else if attrs::is_uv(name, domain, data_type) {
        if was_active_uv {
            names.uv_active = Some(name.into());
        }
        if was_default_uv {
            names.uv_render = Some(name.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_conversions() {
        assert_eq!(convert(Val::F(2.7), "INT"), Some(Val::I(2)));
        assert_eq!(convert(Val::F(-300.0), "INT8"), Some(Val::I8(-128)));
        assert_eq!(convert(Val::F2([1.0, 2.0]), "FLOAT"), Some(Val::F(1.5)));
        assert_eq!(convert(Val::I(-3), "BOOLEAN"), Some(Val::B(false)));
        assert_eq!(convert(Val::B(true), "FLOAT_COLOR"), Some(Val::C([1.0; 4])));
        assert_eq!(convert(Val::S2([3, 8]), "INT"), Some(Val::I(5)));
        assert_eq!(convert(Val::I2([8, 3]), "INT"), Some(Val::I(6)));
        assert_eq!(
            convert(Val::Bc([255, 0, 0, 255]), "FLOAT_VECTOR"),
            Some(Val::F3([1.0, 0.0, 0.0]))
        );
        assert_eq!(convert(Val::C([0.0, 0.0, 0.0, 1.0]), "BOOLEAN"), Some(Val::B(false)));
    }

    #[test]
    fn midpoint_matches_std() {
        assert_eq!(midpoint(3, 8), 5);
        assert_eq!(midpoint(8, 3), 6);
        assert_eq!(midpoint(-3, 4), 0);
    }
}
