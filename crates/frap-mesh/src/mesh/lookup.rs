// SPDX-License-Identifier: GPL-3.0-or-later
//! Boolean reads with Blender's implicit domain and type conversion
//! (`AttributeAccessor::lookup<bool>(name, domain)`, which `lookup_or_default<bool>` wraps):
//! the stored values are first interpolated to the requested domain in their own type, then
//! converted to `bool`. This is how Blender 5.2 reads `sharp_face` / `sharp_edge` for
//! `Mesh.normals_domain` and the normals, so a `sharp_face` stored as FLOAT on FACE or as BOOLEAN
//! on POINT still counts.
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_attributes.cc
//! (`adapt_mesh_domain_*_impl`, including the boolean specialisations),
//! blender/source/blender/blenkernel/BKE_attribute_math.hh (the default mixers) and
//! blender/source/blender/blenkernel/intern/type_conversions.cc (`*_to_bool`).
//!
//! Verified on Blender 5.2.4 (cube, `shade_smooth()`, then one attribute, reading
//! `normals_domain`): FLOAT/INT/INT8 convert with `> 0`; FLOAT2/FLOAT_VECTOR/FLOAT4/INT16_2D/
//! INT32_2D with "any component non-zero"; FLOAT_COLOR with `rgb_to_grayscale > 0`; BYTE_COLOR
//! with "any of r, g, b non-zero"; QUATERNION, FLOAT4X4 and STRING have no conversion (the
//! default, false). Booleans interpolate with all-of (POINT/EDGE/CORNER → FACE, POINT/CORNER →
//! EDGE, CORNER → POINT, EDGE → CORNER) or any-of (FACE → EDGE, FACE/EDGE → POINT) rules; other
//! types average (integers rounded half away from zero, byte colours mixed in linear float).
//! Elements without contributions (loose edges from FACE/CORNER, loose vertices) get the
//! default.

use std::borrow::Cow;

use super::{AttrData, Domain, Mesh};

impl Mesh {
    /// `attributes.lookup<bool>(name, domain)`: the attribute's values interpolated to `domain`
    /// and converted to `bool` the way Blender does; `None` when it is missing or its type has
    /// no conversion to `bool` (callers then use the default, `false`). Borrowed when the
    /// attribute is already BOOLEAN on `domain`.
    pub fn lookup_bool(&self, name: &str, domain: Domain) -> Option<Cow<'_, [bool]>> {
        let a = self.attributes.get(name)?;
        if let AttrData::Bool(v) = &a.data {
            if a.domain == domain {
                return Some(Cow::Borrowed(v.as_slice()));
            }
            return Some(Cow::Owned(adapt_bool(self, v, a.domain, domain)));
        }
        let col = Column::from_data(&a.data)?;
        let mixed = if a.domain == domain {
            col.values
        } else {
            adapt_mixed(self, &col, a.domain, domain)
        };
        let n = col.width;
        Some(Cow::Owned(
            mixed
                .chunks(n.max(1))
                .map(|v| col.kind.to_bool(v))
                .collect(),
        ))
    }

    /// `attributes.lookup<float>(name, domain)`: the attribute's values interpolated to `domain`
    /// in their own type (booleans with the all-of/any-of rules, integers rounded), then
    /// converted to `float` as `type_conversions.cc` does (`bool_to_float`, `int_to_float`,
    /// `float2/3/4_to_float` component averages, `color_to_float` luminance, byte colours
    /// decoded first). `None` when it is missing or its type has no conversion to `float`
    /// (QUATERNION, FLOAT4X4, STRING). Borrowed when the attribute is already FLOAT on `domain`.
    pub fn lookup_float(&self, name: &str, domain: Domain) -> Option<Cow<'_, [f32]>> {
        let a = self.attributes.get(name)?;
        match &a.data {
            AttrData::Float(v) if a.domain == domain => {
                return Some(Cow::Borrowed(v.as_slice()));
            }
            AttrData::Bool(v) => {
                let b: Cow<'_, [bool]> = if a.domain == domain {
                    Cow::Borrowed(v.as_slice())
                } else {
                    Cow::Owned(adapt_bool(self, v, a.domain, domain))
                };
                return Some(Cow::Owned(
                    b.iter().map(|&x| f32::from(u8::from(x))).collect(),
                ));
            }
            _ => {}
        }
        let col = match &a.data {
            // `ColorGeometry4bMixer` mixes the encoded bytes in float and stores the result
            // back as bytes (truncated); `byte_color_to_float` then decodes.
            AttrData::ByteColor(v) => Column {
                kind: Kind::ByteRaw,
                width: 4,
                integer: false,
                values: v.iter().flat_map(|c| c.map(f64::from)).collect(),
            },
            data => Column::from_data(data)?,
        };
        let mixed = if a.domain == domain {
            col.values.clone()
        } else {
            adapt_mixed(self, &col, a.domain, domain)
        };
        let n = col.width;
        Some(Cow::Owned(
            mixed
                .chunks(n.max(1))
                .map(|v| col.kind.to_float(v))
                .collect(),
        ))
    }
}

/// How a numeric column converts to `bool` and averages.
#[derive(Copy, Clone)]
enum Kind {
    /// FLOAT, INT, INT8: `x > 0`.
    Positive,
    /// FLOAT2/3/4, INT16_2D, INT32_2D: any component non-zero.
    NonZero,
    /// FLOAT_COLOR: `rgb_to_grayscale(c) > 0`.
    Color,
    /// BYTE_COLOR: any of r, g, b non-zero (values held as linear floats while mixing).
    ByteColor,
    /// BYTE_COLOR held as its encoded bytes (`lookup_float` only).
    ByteRaw,
}

impl Kind {
    fn to_bool(self, v: &[f64]) -> bool {
        match self {
            Kind::Positive => v[0] > 0.0,
            Kind::NonZero => v.iter().any(|c| *c != 0.0),
            Kind::Color => {
                // rgb_to_grayscale (Rec. 709 luma), in float.
                let g = 0.2126f32 * v[0] as f32 + 0.7152f32 * v[1] as f32 + 0.0722f32 * v[2] as f32;
                g > 0.0
            }
            Kind::ByteColor => v[..3].iter().any(|c| linear_to_srgb_byte(*c as f32) > 0),
            Kind::ByteRaw => v[..3].iter().any(|c| *c as f32 as u8 > 0),
        }
    }

    /// `*_to_float` of `type_conversions.cc`.
    fn to_float(self, v: &[f64]) -> f32 {
        match self {
            Kind::Positive => v[0] as f32,
            // float2/3/4_to_float: the component average, in float.
            Kind::NonZero => v.iter().fold(0.0f32, |s, c| s + *c as f32) / v.len() as f32,
            // color_to_float: IMB_colormanagement_get_luminance (scene-linear Rec. 709).
            Kind::Color => luminance(v[0] as f32, v[1] as f32, v[2] as f32),
            Kind::ByteColor => {
                let c = |x: f64| srgb_byte_to_linear(linear_to_srgb_byte(x as f32));
                luminance(c(v[0]), c(v[1]), c(v[2]))
            }
            // byte_color_to_float: the (mixed, truncated) bytes decoded.
            Kind::ByteRaw => {
                let c = |x: f64| srgb_byte_to_linear(x as f32 as u8);
                luminance(c(v[0]), c(v[1]), c(v[2]))
            }
        }
    }
}

/// A numeric attribute flattened to `width` components per element. Values are held as f64
/// but float types are accumulated in f32 (see [`Accum`]).
struct Column {
    kind: Kind,
    width: usize,
    /// Integers round the average (`DefaultMixer<int>` accumulates in double, `std::round`).
    integer: bool,
    values: Vec<f64>,
}

impl Column {
    fn from_data(data: &AttrData) -> Option<Column> {
        fn flat<T: Copy, const N: usize>(v: &[[T; N]], f: impl Fn(T) -> f64) -> Vec<f64> {
            v.iter().flat_map(|e| e.map(&f)).collect()
        }
        let (kind, width, integer, values) = match data {
            AttrData::Float(v) => (
                Kind::Positive,
                1,
                false,
                v.iter().map(|x| *x as f64).collect(),
            ),
            AttrData::Int(v) => (
                Kind::Positive,
                1,
                true,
                v.iter().map(|x| *x as f64).collect(),
            ),
            AttrData::Int8(v) => (
                Kind::Positive,
                1,
                true,
                v.iter().map(|x| *x as f64).collect(),
            ),
            AttrData::Float2(v) => (Kind::NonZero, 2, false, flat(v, f64::from)),
            AttrData::Float3(v) => (Kind::NonZero, 3, false, flat(v, f64::from)),
            AttrData::Float4(v) => (Kind::NonZero, 4, false, flat(v, f64::from)),
            AttrData::Int16x2(v) => (Kind::NonZero, 2, true, flat(v, f64::from)),
            AttrData::Int32x2(v) => (Kind::NonZero, 2, true, flat(v, f64::from)),
            AttrData::FloatColor(v) => (Kind::Color, 4, false, flat(v, f64::from)),
            AttrData::ByteColor(v) => (
                Kind::ByteColor,
                4,
                false,
                v.iter()
                    .flat_map(|c| {
                        [
                            srgb_byte_to_linear(c[0]) as f64,
                            srgb_byte_to_linear(c[1]) as f64,
                            srgb_byte_to_linear(c[2]) as f64,
                            c[3] as f64 / 255.0,
                        ]
                    })
                    .collect(),
            ),
            // QUATERNION, FLOAT4X4, STRING, BOOLEAN (handled by the caller): no conversion.
            _ => return None,
        };
        Some(Column {
            kind,
            width,
            integer,
            values,
        })
    }
}

/// `attribute_math::DefaultMixer`: sums contributions (f32 for float types, f64 for integers),
/// then multiplies by the reciprocal of the weight; elements without contributions get zero.
struct Accum<'a> {
    col: &'a Column,
    sum: Vec<f64>,
    weight: Vec<u32>,
}

impl<'a> Accum<'a> {
    fn new(col: &'a Column, len: usize) -> Self {
        Accum {
            col,
            sum: vec![0.0; len * col.width],
            weight: vec![0; len],
        }
    }
    fn mix_in(&mut self, dst: usize, src: usize) {
        let w = self.col.width;
        for k in 0..w {
            let v = self.col.values[src * w + k];
            let s = &mut self.sum[dst * w + k];
            *s = if self.col.integer {
                *s + v
            } else {
                (*s as f32 + v as f32) as f64
            };
        }
        self.weight[dst] += 1;
    }
    fn finish(mut self) -> Vec<f64> {
        let w = self.col.width;
        for (i, &n) in self.weight.iter().enumerate() {
            for s in &mut self.sum[i * w..(i + 1) * w] {
                *s = if n == 0 {
                    0.0
                } else if self.col.integer {
                    (*s / n as f64).round()
                } else {
                    (*s as f32 * (1.0f32 / n as f32)) as f64
                };
            }
        }
        self.sum
    }
}

fn domain_len(m: &Mesh, d: Domain) -> usize {
    match d {
        Domain::Point => m.verts_num(),
        Domain::Edge => m.edges_num(),
        Domain::Face => m.faces_num(),
        Domain::Corner => m.corners_num(),
    }
}

/// The corner after `c` in face `f`.
fn next_corner(m: &Mesh, f: usize, c: usize) -> usize {
    let r = m.face(f);
    if c + 1 == r.end { r.start } else { c + 1 }
}

/// Calls `mix(dst, src)` for every contribution Blender's `adapt_mesh_domain_*` makes from
/// `from` to `to`, in its iteration order.
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
            // The corner's own edge and the previous corner's edge (the two edges at its
            // vertex in this face).
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

fn adapt_mixed(m: &Mesh, col: &Column, from: Domain, to: Domain) -> Vec<f64> {
    let mut acc = Accum::new(col, domain_len(m, to));
    for_each_contribution(m, from, to, |d, s| acc.mix_in(d, s));
    acc.finish()
}

/// Boolean interpolation (`adapt_mesh_domain_*_impl<bool>`): an element is true when any
/// contribution is (FACE → EDGE, FACE → POINT, EDGE → POINT) or when all are (every other pair);
/// elements without contributions (loose vertices/edges) are false. POINT → CORNER and
/// FACE → CORNER copy. Verified on 5.2.4 with `geometry.attribute_convert`.
fn adapt_bool(m: &Mesh, v: &[bool], from: Domain, to: Domain) -> Vec<bool> {
    let n = domain_len(m, to);
    let any_of = matches!(
        (from, to),
        (Domain::Face, Domain::Edge)
            | (Domain::Face, Domain::Point)
            | (Domain::Edge, Domain::Point)
    );
    let mut seen = vec![false; n];
    let mut out = vec![!any_of; n];
    for_each_contribution(m, from, to, |d, s| {
        seen[d] = true;
        if any_of {
            out[d] |= v[s];
        } else {
            out[d] &= v[s];
        }
    });
    for (o, s) in out.iter_mut().zip(seen) {
        if !s {
            *o = false;
        }
    }
    out
}

/// `IMB_colormanagement_get_luminance` with the scene-linear Rec. 709 coefficients.
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126f32 * r + 0.7152f32 * g + 0.0722f32 * b
}

/// `srgb_to_linearrgb` of a byte channel.
fn srgb_byte_to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c < 0.04045 {
        if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) }
    } else {
        libm::powf((c + 0.055) * (1.0 / 1.055), 2.4)
    }
}

/// `linearrgb_to_srgb` then `unit_float_to_uchar_clamp`.
fn linear_to_srgb_byte(c: f32) -> u8 {
    let s = if c < 0.003_130_8 {
        if c < 0.0 { 0.0 } else { c * 12.92 }
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    };
    if s <= 0.0 {
        0
    } else if s > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * s + 0.5) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Attribute, SHARP_EDGE, SHARP_FACE};
    use std::sync::Arc;

    /// Smooth default cube (8 verts, 12 edges, 6 faces, 24 corners) with one attribute.
    fn cube_with(name: &str, domain: Domain, data: AttrData) -> Mesh {
        let mut m = crate::primitives::cube(2.0, false, [1.0; 3]);
        m.shade_smooth();
        m.attributes_mut().insert(Attribute {
            name: name.into(),
            domain,
            data,
        });
        m
    }

    fn a<T>(v: T) -> Arc<T> {
        Arc::new(v)
    }

    fn nd(m: &Mesh) -> &'static str {
        m.normals_domain().identifier()
    }

    // Every case below was read from Blender 5.2.4 (`normals_domain` of the same set-up).
    #[test]
    fn type_conversions_match_blender() {
        use AttrData as D;
        let face = |d| nd(&cube_with(SHARP_FACE, Domain::Face, d));
        assert_eq!(face(D::Float(a(vec![1.0; 6]))), "FACE");
        assert_eq!(face(D::Float(a(vec![0.5; 6]))), "FACE");
        assert_eq!(face(D::Float(a(vec![-1.0; 6]))), "POINT");
        assert_eq!(face(D::Int(a(vec![2; 6]))), "FACE");
        assert_eq!(face(D::Int(a(vec![-1; 6]))), "POINT");
        assert_eq!(face(D::Int8(a(vec![-1; 6]))), "POINT");
        assert_eq!(face(D::Int8(a(vec![1; 6]))), "FACE");
        assert_eq!(face(D::Float2(a(vec![[-1.0, 0.0]; 6]))), "FACE");
        assert_eq!(face(D::Float3(a(vec![[0.0, 0.0, -1.0]; 6]))), "FACE");
        assert_eq!(face(D::Float4(a(vec![[0.0, 0.0, 0.0, 1.0]; 6]))), "FACE");
        assert_eq!(
            face(D::FloatColor(a(vec![[0.0, 0.0, 0.0, 1.0]; 6]))),
            "POINT"
        );
        assert_eq!(
            face(D::FloatColor(a(vec![[1.0, 0.0, 0.0, 0.0]; 6]))),
            "FACE"
        );
        assert_eq!(
            face(D::FloatColor(a(vec![[-1.0, 0.0, 0.0, 0.0]; 6]))),
            "POINT"
        );
        assert_eq!(face(D::ByteColor(a(vec![[0, 0, 0, 255]; 6]))), "POINT");
        assert_eq!(face(D::ByteColor(a(vec![[3, 0, 0, 0]; 6]))), "FACE");
        assert_eq!(face(D::Int16x2(a(vec![[-1, 0]; 6]))), "FACE");
        assert_eq!(face(D::Int32x2(a(vec![[0, 1]; 6]))), "FACE");
        assert_eq!(
            face(D::Quaternion(a(vec![[1.0, 0.0, 0.0, 0.0]; 6]))),
            "POINT"
        );
        assert_eq!(face(D::Float4x4(a(vec![[[1.0; 4]; 4]; 6]))), "POINT");
    }

    #[test]
    fn domain_interpolation_matches_blender() {
        use AttrData as D;
        let one_hot =
            |n: usize, i: usize, t: bool| -> Vec<bool> { (0..n).map(|k| (k == i) == t).collect() };
        let c = |name, dom, d| nd(&cube_with(name, dom, d));
        // Booleans.
        assert_eq!(
            c(SHARP_FACE, Domain::Point, D::Bool(a(vec![true; 8]))),
            "FACE"
        );
        assert_eq!(
            c(SHARP_FACE, Domain::Point, D::Bool(a(one_hot(8, 0, false)))),
            "CORNER"
        );
        assert_eq!(
            c(SHARP_FACE, Domain::Corner, D::Bool(a(vec![true; 24]))),
            "FACE"
        );
        assert_eq!(
            c(
                SHARP_FACE,
                Domain::Corner,
                D::Bool(a(one_hot(24, 0, false)))
            ),
            "CORNER"
        );
        assert_eq!(
            c(SHARP_FACE, Domain::Edge, D::Bool(a(vec![true; 12]))),
            "FACE"
        );
        assert_eq!(
            c(SHARP_FACE, Domain::Edge, D::Bool(a(one_hot(12, 0, true)))),
            "POINT"
        );
        assert_eq!(
            c(SHARP_FACE, Domain::Edge, D::Bool(a(one_hot(12, 0, false)))),
            "CORNER"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Face, D::Bool(a(vec![true; 6]))),
            "FACE"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Face, D::Bool(a(one_hot(6, 0, true)))),
            "CORNER"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Face, D::Bool(a(one_hot(6, 0, false)))),
            "FACE"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Point, D::Bool(a(vec![true; 8]))),
            "FACE"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Point, D::Bool(a(one_hot(8, 0, true)))),
            "POINT"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Point, D::Bool(a(one_hot(8, 0, false)))),
            "CORNER"
        );
        assert_eq!(
            c(SHARP_EDGE, Domain::Corner, D::Bool(a(one_hot(24, 0, true)))),
            "POINT"
        );
        let mut face0 = vec![false; 24];
        face0[..4].fill(true);
        assert_eq!(c(SHARP_EDGE, Domain::Corner, D::Bool(a(face0))), "POINT");
        assert_eq!(
            c(
                SHARP_EDGE,
                Domain::Corner,
                D::Bool(a(one_hot(24, 0, false)))
            ),
            "CORNER"
        );
        // Averages, then the type conversion.
        let int_pt = |k: usize| D::Int(a((0..8).map(|i| (i < k) as i32).collect()));
        assert_eq!(
            c(SHARP_FACE, Domain::Point, int_pt(1)),
            "POINT",
            "0.25 rounds to 0"
        );
        assert_eq!(
            c(SHARP_FACE, Domain::Point, int_pt(2)),
            "CORNER",
            "0.5 rounds to 1"
        );
        assert_eq!(c(SHARP_FACE, Domain::Point, int_pt(3)), "CORNER");
        let i8_pt = D::Int8(a((0..8).map(|i| (i == 0) as i8).collect()));
        assert_eq!(c(SHARP_FACE, Domain::Point, i8_pt), "POINT");
        let i16_pt = |k: usize| D::Int16x2(a((0..8).map(|i| [(i < k) as i16, 0]).collect()));
        assert_eq!(c(SHARP_FACE, Domain::Point, i16_pt(1)), "POINT");
        assert_eq!(c(SHARP_FACE, Domain::Point, i16_pt(2)), "CORNER");
        let tiny = D::Float(a((0..8).map(|i| if i == 0 { 0.1 } else { 0.0 }).collect()));
        assert_eq!(c(SHARP_FACE, Domain::Point, tiny), "CORNER");
        let cancel = D::Float(a((0..8)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect()));
        assert_eq!(c(SHARP_FACE, Domain::Point, cancel), "CORNER");
        let v3 = D::Float3(a((0..8)
            .map(|i| [if i % 2 == 0 { 1.0 } else { -1.0 }, 0.0, 0.0])
            .collect()));
        assert_eq!(c(SHARP_FACE, Domain::Point, v3), "CORNER");
        let bc = D::ByteColor(a((0..8)
            .map(|i| [if i == 0 { 255 } else { 0 }, 0, 0, 0])
            .collect()));
        assert_eq!(c(SHARP_FACE, Domain::Point, bc), "CORNER");
        let corner_f = D::Float(a((0..24).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect()));
        assert_eq!(c(SHARP_EDGE, Domain::Corner, corner_f), "CORNER");
        assert_eq!(
            c(SHARP_EDGE, Domain::Face, D::Float(a(vec![1.0; 6]))),
            "FACE"
        );
        // sharp_face all false on FACE and sharp_edge as FLOAT on CORNER, all 1.
        let mut m = cube_with(SHARP_EDGE, Domain::Corner, D::Float(a(vec![1.0; 24])));
        m.set_bool_attribute(SHARP_FACE, Domain::Face, vec![false; 6]);
        assert_eq!(nd(&m), "FACE");
    }

    #[test]
    fn converted_flags_drive_corner_normals() {
        // sharp_face as FLOAT on FACE, all 1: flat shading, like a BOOLEAN one.
        let m = cube_with(
            SHARP_FACE,
            Domain::Face,
            AttrData::Float(Arc::new(vec![1.0; 6])),
        );
        let flat = crate::primitives::cube(2.0, false, [1.0; 3]);
        assert_eq!(*m.corner_normals(), *flat.corner_normals());
        // Mixed: one face sharp through a POINT-domain boolean (verts of face 0 true): only
        // face 0 counts as sharp, so its corners take the face normal.
        let fverts: Vec<usize> = flat.corner_verts()[flat.face(0)]
            .iter()
            .map(|v| *v as usize)
            .collect();
        let pt: Vec<bool> = (0..8).map(|v| fverts.contains(&v)).collect();
        let m = cube_with(SHARP_FACE, Domain::Point, AttrData::Bool(Arc::new(pt)));
        assert_eq!(nd(&m), "CORNER");
        assert_eq!(
            m.sharp_face_flags().unwrap().as_ref(),
            &[true, false, false, false, false, false]
        );
        let cn = m.corner_normals();
        let fnor = m.face_normals();
        for c in m.face(0) {
            assert_eq!(cn[c], fnor[0]);
        }
    }

    #[test]
    fn byte_color_round_trip() {
        for b in [0u8, 1, 3, 10, 128, 254, 255] {
            assert_eq!(linear_to_srgb_byte(srgb_byte_to_linear(b)), b);
        }
    }
}
