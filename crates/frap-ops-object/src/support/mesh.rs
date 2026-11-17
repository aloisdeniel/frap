// SPDX-License-Identifier: GPL-3.0-or-later
//! Whole-mesh transforms of the object operators. Ports of `bke::mesh_transform`,
//! `bke::mesh_translate` (`blenkernel/intern/mesh.cc`) and `bke::mesh_flip_faces`
//! (`blenkernel/intern/mesh_flip_faces.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Shape keys are not transformed yet: frap's `Key` datablock arrives in M6.

use std::sync::Arc;

use frap_mesh::{AttrData, Domain, Mesh};
use glam::{Mat3, Mat4, Vec3};

/// Blender's internal corner → edge attribute (private in frap-mesh).
const CORNER_EDGE: &str = ".corner_edge";

/// `mesh_transform`: positions by `m`, float custom normals by its normal matrix.
pub fn transform(mesh: &mut Mesh, m: &Mat4) {
    for p in mesh.positions_mut().iter_mut() {
        *p = m.transform_point3(Vec3::from_array(*p)).to_array();
    }
    let nmat = Mat3::from_mat4(*m).inverse().transpose();
    if let Some(a) = mesh.attributes_mut().get_mut(frap_mesh::CUSTOM_NORMAL)
        && let AttrData::Float3(v) = &mut a.data
    {
        for n in Arc::make_mut(v).iter_mut() {
            let t = nmat * Vec3::from_array(*n);
            *n = frap_math::matrix::normalize_v3(t.to_array()).0;
        }
    }
}

/// `mesh_translate`: every position moved by `offset`.
pub fn translate(mesh: &mut Mesh, offset: Vec3) {
    if offset == Vec3::ZERO {
        return;
    }
    for p in mesh.positions_mut().iter_mut() {
        *p = (Vec3::from_array(*p) + offset).to_array();
    }
}

/// The corner swaps of a face flip: corners `j + 1` and `last - j` for `j < size / 2`.
fn face_swaps(start: usize, end: usize) -> impl Iterator<Item = (usize, usize)> {
    let size = end - start;
    (0..size / 2).map(move |j| (start + j + 1, end - 1 - j))
}

fn swap_column(data: &mut AttrData, swaps: &[(usize, usize)]) {
    macro_rules! go {
        ($v:expr) => {{
            let v = Arc::make_mut($v);
            for &(a, b) in swaps {
                if a != b {
                    v.swap(a, b);
                }
            }
        }};
    }
    match data {
        AttrData::Float(v) => go!(v),
        AttrData::Int(v) => go!(v),
        AttrData::Int8(v) => go!(v),
        AttrData::Bool(v) => go!(v),
        AttrData::Float2(v) => go!(v),
        AttrData::Float3(v) => go!(v),
        AttrData::Float4(v) => go!(v),
        AttrData::FloatColor(v) => go!(v),
        AttrData::ByteColor(v) => go!(v),
        AttrData::Int16x2(v) => go!(v),
        AttrData::Int32x2(v) => go!(v),
        AttrData::Quaternion(v) => go!(v),
        AttrData::Float4x4(v) => go!(v),
        // Blender skips string attributes (`CD_PROP_STRING`); `AttrData` is
        // `#[non_exhaustive]`, a later column type is decided when it is added.
        _ => {}
    }
}

/// `mesh_flip_faces` on every face: the winding is reversed keeping the first corner
/// (`[0, 4, 6, 2]` → `[0, 2, 6, 4]`, verified), corner edges follow, and every other corner
/// attribute is permuted like the corner vertices.
pub fn flip_faces(mesh: &mut Mesh) {
    let offsets = mesh.face_offsets().to_vec();
    let mut vert_swaps = Vec::new();
    let mut edge_swaps = Vec::new();
    for f in offsets.windows(2) {
        let (s, e) = (f[0] as usize, f[1] as usize);
        for (a, b) in face_swaps(s, e) {
            vert_swaps.push((a, b));
            edge_swaps.push((a - 1, b));
        }
    }
    let attrs = mesh.attributes_mut();
    let names: Vec<_> = attrs
        .iter()
        .filter(|a| a.domain == Domain::Corner)
        .map(|a| a.name.clone())
        .collect();
    for name in names {
        let swaps = if name == CORNER_EDGE {
            &edge_swaps
        } else {
            &vert_swaps
        };
        if let Some(a) = attrs.get_mut(&name) {
            swap_column(&mut a.data, swaps);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flipping_keeps_the_first_corner_and_edges_follow() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let before_edges = m.corner_edges().to_vec();
        flip_faces(&mut m);
        // Primitive face 0 is [0, 1, 3, 2].
        assert_eq!(&m.corner_verts()[0..4], &[0, 2, 3, 1]);
        // Every corner edge still joins its corner and the next one.
        let (cv, ce, edges) = (m.corner_verts(), m.corner_edges(), m.edges());
        for f in 0..m.faces_num() {
            let r = m.face(f);
            for c in r.clone() {
                let next = if c + 1 == r.end { r.start } else { c + 1 };
                let e = edges[ce[c] as usize];
                let (a, b) = (cv[c], cv[next]);
                assert!(
                    (e[0] == a && e[1] == b) || (e[0] == b && e[1] == a),
                    "face {f} corner {c}"
                );
            }
        }
        assert_ne!(ce, &before_edges[..]);
        flip_faces(&mut m);
        assert_eq!(m.corner_edges(), &before_edges[..]);
    }

    #[test]
    fn transform_and_translate() {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        transform(&mut m, &Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)));
        assert_eq!(m.positions()[0], [-2.0, -1.0, -1.0]);
        translate(&mut m, Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(m.positions()[0], [-3.0, -1.0, -1.0]);
    }
}
