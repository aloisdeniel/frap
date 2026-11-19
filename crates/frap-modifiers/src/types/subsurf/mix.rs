// SPDX-License-Identifier: GPL-3.0-or-later
//! Attribute interpolation of the subdivided mesh, as `subdiv_mesh.cc` does it: a vertex or
//! corner inside a coarse face mixes four values bilinearly (`mix_attrs` with `float4`
//! weights); for a face other than a quad those four are the corner's value, the middles of
//! its two edges (`mix2` at 0.5) and the face centre (`mix_indices` with uniform weights),
//! each computed in the attribute's type first (`VerticesForInterpolation`,
//! `LoopsForInterpolation`); loose-edge points mix two values.
//!
//! Ported from blender/source/blender/blenkernel/BKE_attribute_math.hh and
//! intern/attribute_math.cc (`mix2`, `mix4`, `mix_indices`, Blender 5.2.4). Quaternions mix
//! through their exponential maps everywhere (Blender's `mix2` of quaternions is a slerp; the
//! two agree at the 0.5 midpoints except for antipodal pairs) and 4×4 matrices mix
//! component-wise (Blender decomposes them into location/rotation/scale): both only occur on
//! geometry-nodes data.

use std::sync::Arc;

use frap_mesh::{AttrData, DeformVert};
use frap_subdiv::DataSource;
use smallvec::SmallVec;

/// A type the subdivided mesh interpolates.
pub(super) trait Mix: Clone {
    /// The type's default (`CPPType::default_value`).
    fn zero() -> Self;
    /// `mix_attrs` with two indices: `mix2` (booleans: `mix_indices`, any true with a
    /// non-zero weight).
    fn mix2(f: f32, a: &Self, b: &Self) -> Self;
    /// `mix_attrs` with four indices: `mix4` (booleans: `mix_indices`).
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self;
    /// `mix_indices(src, indices, weights)`.
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self;
}

fn safe_rcp(x: f32) -> f32 {
    if x == 0.0 { 0.0 } else { 1.0 / x }
}

macro_rules! float_vec_mix {
    ($t:ty, $n:expr) => {
        impl Mix for $t {
            fn zero() -> Self {
                [0.0; $n]
            }
            fn mix2(f: f32, a: &Self, b: &Self) -> Self {
                std::array::from_fn(|k| (1.0 - f) * a[k] + f * b[k])
            }
            fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
                std::array::from_fn(|k| {
                    w[0] * v[0][k] + w[1] * v[1][k] + w[2] * v[2][k] + w[3] * v[3][k]
                })
            }
            fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
                let mut acc = [0.0f32; $n];
                let mut ws = 0.0f32;
                for (x, &wt) in v.iter().zip(w) {
                    for k in 0..$n {
                        acc[k] += x[k] * wt;
                    }
                    ws += wt;
                }
                let r = safe_rcp(ws);
                acc.map(|a| a * r)
            }
        }
    };
}

float_vec_mix!([f32; 2], 2);
float_vec_mix!([f32; 3], 3);
float_vec_mix!([f32; 4], 4);

impl Mix for f32 {
    fn zero() -> Self {
        0.0
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        (1.0 - f) * a + f * b
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        w[0] * v[0] + w[1] * v[1] + w[2] * v[2] + w[3] * v[3]
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let mut acc = 0.0f32;
        let mut ws = 0.0f32;
        for (x, &wt) in v.iter().zip(w) {
            acc += **x * wt;
            ws += wt;
        }
        acc * safe_rcp(ws)
    }
}

impl Mix for i32 {
    fn zero() -> Self {
        0
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        ((1.0 - f) * *a as f32 + f * *b as f32).round() as i32
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        (w[0] * *v[0] as f32 + w[1] * *v[1] as f32 + w[2] * *v[2] as f32 + w[3] * *v[3] as f32)
            .round() as i32
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let mut acc = 0.0f64;
        let mut ws = 0.0f32;
        for (x, &wt) in v.iter().zip(w) {
            acc += **x as f64 * wt as f64;
            ws += wt;
        }
        (acc * safe_rcp(ws) as f64).round() as i32
    }
}

impl Mix for i8 {
    fn zero() -> Self {
        0
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        ((1.0 - f) * *a as f32 + f * *b as f32).round() as i8
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        (w[0] * *v[0] as f32 + w[1] * *v[1] as f32 + w[2] * *v[2] as f32 + w[3] * *v[3] as f32)
            .round() as i8
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let mut acc = 0.0f32;
        let mut ws = 0.0f32;
        for (x, &wt) in v.iter().zip(w) {
            acc += **x as f32 * wt;
            ws += wt;
        }
        (acc * safe_rcp(ws)).round() as i8
    }
}

impl Mix for bool {
    fn zero() -> Self {
        false
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        (*a && 1.0 - f > 0.0) || (*b && f > 0.0)
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        (0..4).any(|k| *v[k] && w[k] > 0.0)
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        v.iter().zip(w).any(|(x, &wt)| **x && wt > 0.0)
    }
}

/// `INT16_2D`: `short2(float2 mix)` truncates; `mix_indices` rounds.
impl Mix for [i16; 2] {
    fn zero() -> Self {
        [0; 2]
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        std::array::from_fn(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * f) as i16)
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        std::array::from_fn(|k| {
            (w[0] * v[0][k] as f32
                + w[1] * v[1][k] as f32
                + w[2] * v[2][k] as f32
                + w[3] * v[3][k] as f32) as i16
        })
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let f: Vec<[f32; 2]> = v.iter().map(|x| [x[0] as f32, x[1] as f32]).collect();
        let r = <[f32; 2]>::mix_weighted(&f.iter().collect::<Vec<_>>(), w);
        r.map(|x| x.round() as i16)
    }
}

/// `INT32_2D`: `int2(float2 mix)` truncates; `mix_indices` rounds through doubles.
impl Mix for [i32; 2] {
    fn zero() -> Self {
        [0; 2]
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        std::array::from_fn(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * f) as i32)
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        std::array::from_fn(|k| {
            (w[0] * v[0][k] as f32
                + w[1] * v[1][k] as f32
                + w[2] * v[2][k] as f32
                + w[3] * v[3][k] as f32) as i32
        })
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let mut acc = [0.0f64; 2];
        let mut ws = 0.0f32;
        for (x, &wt) in v.iter().zip(w) {
            acc[0] += x[0] as f64 * wt as f64;
            acc[1] += x[1] as f64 * wt as f64;
            ws += wt;
        }
        let r = safe_rcp(ws) as f64;
        acc.map(|a| (a * r).round() as i32)
    }
}

/// `BYTE_COLOR` (`ColorGeometry4b`): float mixes converted back by truncation.
impl Mix for [u8; 4] {
    fn zero() -> Self {
        [0; 4]
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        std::array::from_fn(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * f) as u8)
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        std::array::from_fn(|k| {
            (v[0][k] as f32 * w[0]
                + v[1][k] as f32 * w[1]
                + v[2][k] as f32 * w[2]
                + v[3][k] as f32 * w[3]) as u8
        })
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let f: Vec<[f32; 4]> = v.iter().map(|x| x.map(|c| c as f32)).collect();
        <[f32; 4]>::mix_weighted(&f.iter().collect::<Vec<_>>(), w).map(|c| c as u8)
    }
}

/// A `QUATERNION` value (w, x, y, z), mixed through exponential maps.
#[derive(Clone, Copy)]
struct Quat([f32; 4]);

fn quat_to_expmap(q: [f32; 4]) -> [f32; 3] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let q = if len > 0.0 {
        q.map(|c| c / len)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    };
    let l = (q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l == 0.0 {
        return [0.0; 3];
    }
    let angle = 2.0 * libm::atan2f(l, q[0]);
    [q[1] / l * angle, q[2] / l * angle, q[3] / l * angle]
}

fn expmap_to_quat(e: [f32; 3]) -> [f32; 4] {
    let angle = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
    if angle == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let (s, c) = libm::sincosf(angle * 0.5);
    [c, e[0] / angle * s, e[1] / angle * s, e[2] / angle * s]
}

impl Mix for Quat {
    fn zero() -> Self {
        Quat([1.0, 0.0, 0.0, 0.0])
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        Quat(expmap_to_quat(<[f32; 3]>::mix2(
            f,
            &quat_to_expmap(a.0),
            &quat_to_expmap(b.0),
        )))
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        let e = v.map(|q| quat_to_expmap(q.0));
        Quat(expmap_to_quat(<[f32; 3]>::mix4(
            w,
            [&e[0], &e[1], &e[2], &e[3]],
        )))
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        let e: Vec<[f32; 3]> = v.iter().map(|q| quat_to_expmap(q.0)).collect();
        Quat(expmap_to_quat(<[f32; 3]>::mix_weighted(
            &e.iter().collect::<Vec<_>>(),
            w,
        )))
    }
}

/// A `FLOAT4X4` value, mixed component-wise.
#[derive(Clone, Copy)]
struct M4([[f32; 4]; 4]);

impl Mix for M4 {
    fn zero() -> Self {
        M4(std::array::from_fn(|i| {
            std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 })
        }))
    }
    fn mix2(f: f32, a: &Self, b: &Self) -> Self {
        M4(std::array::from_fn(|i| {
            <[f32; 4]>::mix2(f, &a.0[i], &b.0[i])
        }))
    }
    fn mix4(w: [f32; 4], v: [&Self; 4]) -> Self {
        M4(std::array::from_fn(|i| {
            <[f32; 4]>::mix4(w, [&v[0].0[i], &v[1].0[i], &v[2].0[i], &v[3].0[i]])
        }))
    }
    fn mix_weighted(v: &[&Self], w: &[f32]) -> Self {
        M4(std::array::from_fn(|i| {
            let rows: Vec<&[f32; 4]> = v.iter().map(|m| &m.0[i]).collect();
            <[f32; 4]>::mix_weighted(&rows, w)
        }))
    }
}

/// The coarse face layout the sources refer to.
pub(super) struct Faces<'a> {
    pub offsets: &'a [i32],
    pub corner_verts: &'a [i32],
    pub edges: &'a [[i32; 2]],
}

/// Bilinear weights of a ptex coordinate (`quad_weights_from_uv`).
fn quad_weights(u: f32, v: f32) -> [f32; 4] {
    [(1.0 - u) * (1.0 - v), u * (1.0 - v), u * v, (1.0 - u) * v]
}

/// Interpolates one vertex (`corners == false`) or corner column through `sources`.
fn interp<T: Mix>(src: &[T], sources: &[DataSource], faces: &Faces<'_>, corners: bool) -> Vec<T> {
    let elem = |c: usize| -> usize {
        if corners {
            c
        } else {
            faces.corner_verts[c] as usize
        }
    };
    // The four interpolation values of a face's ptex corner, cached per (face, corner).
    let mut cache: Option<((u32, u32), [T; 4])> = None;
    let mut out = Vec::with_capacity(sources.len());
    for (i, s) in sources.iter().enumerate() {
        let value = match *s {
            DataSource::Copy(c) => src[c as usize].clone(),
            DataSource::Keep => src.get(i).cloned().unwrap_or_else(T::zero),
            DataSource::Edge { edge, u } => {
                let e = faces.edges[edge as usize];
                T::mix2(u, &src[e[0] as usize], &src[e[1] as usize])
            }
            DataSource::Ptex { face, corner, u, v } => {
                let start = faces.offsets[face as usize] as usize;
                let end = faces.offsets[face as usize + 1] as usize;
                let n = end - start;
                let w = quad_weights(u, v);
                if n == 4 {
                    let q = [0, 1, 2, 3].map(|k| &src[elem(start + k)]);
                    T::mix4(w, q)
                } else {
                    let key = (face, corner);
                    if cache.as_ref().is_none_or(|(k, _)| *k != key) {
                        let k = corner as usize;
                        let c0 = start + k;
                        let c1 = start + (k + 1) % n;
                        let cl = start + (k + n - 1) % n;
                        let all: Vec<&T> = (start..end).map(|c| &src[elem(c)]).collect();
                        let uw = vec![1.0 / n as f32; n];
                        let first = T::mix2(0.5, &src[elem(c0)], &src[elem(c1)]);
                        let last = if corners {
                            T::mix2(0.5, &src[elem(cl)], &src[elem(c0)])
                        } else {
                            T::mix2(0.5, &src[elem(c0)], &src[elem(cl)])
                        };
                        let centre = T::mix_weighted(&all, &uw);
                        cache = Some((key, [src[elem(c0)].clone(), first, centre, last]));
                    }
                    let st = &cache.as_ref().expect("cached").1;
                    T::mix4(w, [&st[0], &st[1], &st[2], &st[3]])
                }
            }
        };
        out.push(value);
    }
    out
}

/// Interpolates a vertex- or corner-domain column (`None`: a STRING column, which Blender
/// does not carry).
pub(super) fn interp_column(
    data: &AttrData,
    sources: &[DataSource],
    faces: &Faces<'_>,
    corners: bool,
) -> Option<AttrData> {
    Some(match data {
        AttrData::Float(v) => AttrData::Float(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Int(v) => AttrData::Int(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Int8(v) => AttrData::Int8(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Bool(v) => AttrData::Bool(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Float2(v) => AttrData::Float2(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Float3(v) => AttrData::Float3(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Float4(v) => AttrData::Float4(Arc::new(interp(v, sources, faces, corners))),
        AttrData::FloatColor(v) => {
            AttrData::FloatColor(Arc::new(interp(v, sources, faces, corners)))
        }
        AttrData::ByteColor(v) => AttrData::ByteColor(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Int16x2(v) => AttrData::Int16x2(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Int32x2(v) => AttrData::Int32x2(Arc::new(interp(v, sources, faces, corners))),
        AttrData::Quaternion(v) => {
            let q: Vec<Quat> = v.iter().map(|&x| Quat(x)).collect();
            let r = interp(&q, sources, faces, corners);
            AttrData::Quaternion(Arc::new(r.into_iter().map(|x| x.0).collect()))
        }
        AttrData::Float4x4(v) => {
            let m: Vec<M4> = v.iter().map(|&x| M4(x)).collect();
            let r = interp(&m, sources, faces, corners);
            AttrData::Float4x4(Arc::new(r.into_iter().map(|x| x.0).collect()))
        }
        AttrData::String(_) => return None,
        _ => return None,
    })
}

/// `mix_deform_verts` (BKE_deform.cc): weights summed per group (zero weights kept), sorted by
/// group; one source is copied.
fn mix_deform(src: &[&DeformVert], w: &[f32]) -> DeformVert {
    if src.len() == 1 {
        return src[0].clone();
    }
    let mut buf: SmallVec<[(u32, f32); 4]> = SmallVec::new();
    for (dv, &f) in src.iter().zip(w) {
        for &(g, x) in &dv.weights {
            match buf.iter_mut().find(|e| e.0 == g) {
                Some(e) => e.1 += x * f,
                None => buf.push((g, x * f)),
            }
        }
    }
    buf.sort_by_key(|e| e.0);
    DeformVert { weights: buf }
}

/// Vertex-group weights of the subdivided vertices.
pub(super) fn interp_deform(
    src: &[DeformVert],
    sources: &[DataSource],
    faces: &Faces<'_>,
) -> Vec<DeformVert> {
    sources
        .iter()
        .enumerate()
        .map(|(i, s)| match *s {
            DataSource::Copy(c) => src[c as usize].clone(),
            DataSource::Keep => src.get(i).cloned().unwrap_or_default(),
            DataSource::Edge { edge, u } => {
                let e = faces.edges[edge as usize];
                mix_deform(&[&src[e[0] as usize], &src[e[1] as usize]], &[1.0 - u, u])
            }
            DataSource::Ptex { face, corner, u, v } => {
                let start = faces.offsets[face as usize] as usize;
                let end = faces.offsets[face as usize + 1] as usize;
                let n = end - start;
                let w = quad_weights(u, v);
                let vert = |c: usize| &src[faces.corner_verts[c] as usize];
                if n == 4 {
                    mix_deform(
                        &[
                            vert(start),
                            vert(start + 1),
                            vert(start + 2),
                            vert(start + 3),
                        ],
                        &w,
                    )
                } else {
                    let k = corner as usize;
                    let c0 = start + k;
                    let c1 = start + (k + 1) % n;
                    let cl = start + (k + n - 1) % n;
                    let all: Vec<&DeformVert> = (start..end).map(vert).collect();
                    let centre = mix_deform(&all, &vec![1.0 / n as f32; n]);
                    let first = mix_deform(&[vert(c0), vert(c1)], &[0.5, 0.5]);
                    let last = mix_deform(&[vert(c0), vert(cl)], &[0.5, 0.5]);
                    mix_deform(&[vert(c0), &first, &centre, &last], &w)
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_mixing_follows_blender() {
        assert_eq!(i32::mix4([0.25; 4], [&1, &2, &2, &2]), 2);
        assert!(bool::mix4(
            [0.0, 0.0, 0.0, 1.0],
            [&false, &false, &false, &true]
        ));
        assert!(!bool::mix4(
            [1.0, 0.0, 0.0, 0.0],
            [&false, &true, &true, &true]
        ));
        assert_eq!(
            <[u8; 4]>::mix4([0.5, 0.5, 0.0, 0.0], [&[255; 4], &[0; 4], &[0; 4], &[0; 4]]),
            [127; 4]
        );
        assert_eq!(f32::mix_weighted(&[&1.0, &3.0], &[0.5, 0.5]), 2.0);
    }

    #[test]
    fn ngon_uses_corner_edge_middles_and_centre() {
        // A triangle with values 0, 3, 6 at its corners: the ptex corner 0 at (1, 1) is the
        // centre (3), at (1, 0) the middle of the first edge (1.5).
        let faces = Faces {
            offsets: &[0, 3],
            corner_verts: &[0, 1, 2],
            edges: &[[0, 1], [1, 2], [2, 0]],
        };
        let src = [0.0f32, 3.0, 6.0];
        let s = |u, v| DataSource::Ptex {
            face: 0,
            corner: 0,
            u,
            v,
        };
        let out = interp(
            &src,
            &[s(1.0, 1.0), s(1.0, 0.0), s(0.0, 1.0)],
            &faces,
            false,
        );
        assert_eq!(out, vec![3.0, 1.5, 3.0]);
    }
}
