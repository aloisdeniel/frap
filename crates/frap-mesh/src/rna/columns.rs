// SPDX-License-Identifier: GPL-3.0-or-later
//! Column access shared by the element bindings: flag and index columns that RNA writes create
//! on demand, robust per-element reads, and the small geometry functions RNA exposes
//! (`MeshPolygon.center/area`, `MeshLoopTriangle.normal/area`).
//!
//! Write semantics (verified on Blender 5.2.4): every RNA write of a flag or index column
//! (`select`, `hide`, `use_smooth`, `use_seam`, `use_edge_sharp`, `material_index`) creates the
//! attribute when it is missing, even when the written value equals the default, and keeps it
//! when every value becomes the default again (decision log W1-9).

use std::borrow::Cow;
use std::sync::Arc;

use frap_geom::normals::{newell_cross, normalize_v3};
use frap_rna::{PyExc, RnaError, RnaErrorKind};

use crate::mesh::{AttrData, Attribute, Domain, Mesh};

/// `.edge_verts`
pub(crate) const EDGE_VERTS: &str = ".edge_verts";
/// `.corner_vert`
pub(crate) const CORNER_VERT: &str = ".corner_vert";
/// `.corner_edge`
pub(crate) const CORNER_EDGE: &str = ".corner_edge";
/// `position`
pub(crate) const POSITION: &str = "position";
/// `.hide_vert`
pub(crate) const HIDE_VERT: &str = ".hide_vert";
/// `.hide_edge`
pub(crate) const HIDE_EDGE: &str = ".hide_edge";
/// `.hide_poly`
pub(crate) const HIDE_POLY: &str = ".hide_poly";
/// `material_index`
pub(crate) const MATERIAL_INDEX: &str = "material_index";

/// Element count of a domain.
pub(crate) fn domain_len(m: &Mesh, d: Domain) -> usize {
    match d {
        Domain::Point => m.verts_num(),
        Domain::Edge => m.edges_num(),
        Domain::Face => m.faces_num(),
        Domain::Corner => m.corners_num(),
    }
}

/// A boolean column as Blender reads it (`lookup_or_default<bool>`): converted from any type
/// and domain, `false` where missing.
pub(crate) fn flags<'a>(m: &'a Mesh, name: &str, d: Domain) -> Cow<'a, [bool]> {
    let n = domain_len(m, d);
    match m.lookup_bool(name, d) {
        Some(c) if c.len() == n => c,
        Some(c) => {
            let mut v = c.into_owned();
            v.resize(n, false);
            Cow::Owned(v)
        }
        None => Cow::Owned(vec![false; n]),
    }
}

/// One flag (`flags(m, name, d)[i]` without converting the whole column when it is stored as
/// BOOLEAN on `d`).
pub(crate) fn flag(m: &Mesh, name: &str, d: Domain, i: usize) -> bool {
    match m.bool_attribute(name, d) {
        Some(c) => c.get(i).copied().unwrap_or(false),
        None => flags(m, name, d).get(i).copied().unwrap_or(false),
    }
}

/// The BOOLEAN column `name` on `d`, created (converted from any existing attribute of that
/// name, else all false) when it is not stored as BOOLEAN on `d` with the domain's length.
/// Clears the mesh caches.
pub(crate) fn flags_mut<'a>(m: &'a mut Mesh, name: &str, d: Domain) -> &'a mut Vec<bool> {
    let n = domain_len(m, d);
    if m.bool_attribute(name, d).map(<[bool]>::len) != Some(n) {
        let v = flags(m, name, d).into_owned();
        m.set_bool_attribute(name, d, v);
    }
    match m.attributes_mut().get_mut(name).map(|a| &mut a.data) {
        Some(AttrData::Bool(v)) => Arc::make_mut(v),
        _ => unreachable!("invariant: {name} was just stored as BOOLEAN"),
    }
}

/// An INT column on `d` (`material_index`), `0` where missing.
pub(crate) fn ints<'a>(m: &'a Mesh, name: &str, d: Domain) -> Cow<'a, [i32]> {
    let n = domain_len(m, d);
    match m.attributes().get(name) {
        Some(Attribute {
            domain,
            data: AttrData::Int(v),
            ..
        }) if *domain == d && v.len() == n => Cow::Borrowed(v.as_slice()),
        _ => Cow::Owned(vec![0; n]),
    }
}

/// The INT column `name` on `d`, created (zeros) when it is not stored as INT on `d` with the
/// domain's length. Clears the mesh caches.
pub(crate) fn ints_mut<'a>(m: &'a mut Mesh, name: &str, d: Domain) -> &'a mut Vec<i32> {
    let n = domain_len(m, d);
    let stored = matches!(m.attributes().get(name), Some(Attribute {
        domain,
        data: AttrData::Int(v),
        ..
    }) if *domain == d && v.len() == n);
    if !stored {
        m.attributes_mut().insert(Attribute {
            name: name.into(),
            domain: d,
            data: AttrData::Int(Arc::new(vec![0; n])),
        });
    }
    match m.attributes_mut().get_mut(name).map(|a| &mut a.data) {
        Some(AttrData::Int(v)) => Arc::make_mut(v),
        _ => unreachable!("invariant: {name} was just stored as INT"),
    }
}

/// An existing INT column (`.corner_vert`, `.corner_edge`), mutably. Clears the mesh caches.
pub(crate) fn int_col_mut<'a>(m: &'a mut Mesh, name: &str) -> Option<&'a mut Vec<i32>> {
    match m.attributes_mut().get_mut(name).map(|a| &mut a.data) {
        Some(AttrData::Int(v)) => Some(Arc::make_mut(v)),
        _ => None,
    }
}

/// The `.edge_verts` column, mutably. Clears the mesh caches.
pub(crate) fn edges_mut(m: &mut Mesh) -> Option<&mut Vec<[i32; 2]>> {
    match m.attributes_mut().get_mut(EDGE_VERTS).map(|a| &mut a.data) {
        Some(AttrData::Int32x2(v)) => Some(Arc::make_mut(v)),
        _ => None,
    }
}

/// Corner range of face `f`, empty when the offsets are out of order or out of bounds (RNA
/// reads never panic on an inconsistent mesh).
pub(crate) fn face_range(m: &Mesh, f: usize) -> std::ops::Range<usize> {
    let offs = m.face_offsets();
    let (Some(&s), Some(&e)) = (offs.get(f), offs.get(f + 1)) else {
        return 0..0;
    };
    let (s, e) = (s.max(0) as usize, e.max(0) as usize);
    if s <= e && e <= m.corners_num() {
        s..e
    } else {
        0..0
    }
}

/// Vertex indices of face `f`.
pub(crate) fn face_verts(m: &Mesh, f: usize) -> &[i32] {
    &m.corner_verts()[face_range(m, f)]
}

/// Position of vertex `v`, `(0, 0, 0)` when out of range.
pub(crate) fn position(m: &Mesh, v: i32) -> [f32; 3] {
    usize::try_from(v)
        .ok()
        .and_then(|v| m.positions().get(v))
        .copied()
        .unwrap_or([0.0; 3])
}

/// Positions of the given vertices (out-of-range indices read the origin).
fn gather(m: &Mesh, verts: &[i32]) -> Vec<[f32; 3]> {
    verts.iter().map(|&v| position(m, v)).collect()
}

// Ported from blender/source/blender/blenkernel/intern/mesh_evaluate.cc (face_center_calc).
/// `MeshPolygon.center`: the mean of the face's vertex positions, accumulated as Blender does
/// (`center += position * (1 / n)`).
pub(crate) fn face_center(m: &Mesh, f: usize) -> [f32; 3] {
    let verts = face_verts(m, f);
    if verts.is_empty() {
        return [0.0; 3];
    }
    let w = 1.0 / verts.len() as f32;
    let mut c = [0.0f32; 3];
    for &v in verts {
        let p = position(m, v);
        for k in 0..3 {
            c[k] += p[k] * w;
        }
    }
    c
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (cross_tri_v3).
fn cross_tri(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> [f32; 3] {
    let n1 = [v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]];
    let n2 = [v2[0] - v3[0], v2[1] - v3[1], v2[2] - v3[2]];
    [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ]
}

fn len_v3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (area_tri_v3).
/// Area of a triangle.
pub(crate) fn area_tri(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> f32 {
    0.5 * len_v3(cross_tri(v1, v2, v3))
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (normal_tri_v3).
/// Unit normal of a triangle (zero when degenerate).
pub(crate) fn normal_tri(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> [f32; 3] {
    normalize_v3(cross_tri(v1, v2, v3)).0
}

// Ported from blender/source/blender/blenkernel/intern/mesh_evaluate.cc (face_area_calc) and
// blender/source/blender/blenlib/intern/math_geom.cc (area_poly_v3: half the Newell normal).
/// `MeshPolygon.area`.
pub(crate) fn face_area(m: &Mesh, f: usize) -> f32 {
    let verts = face_verts(m, f);
    match verts.len() {
        0..=2 => 0.0,
        3 => area_tri(
            position(m, verts[0]),
            position(m, verts[1]),
            position(m, verts[2]),
        ),
        n => {
            let pts = gather(m, verts);
            let idx: Vec<i32> = (0..n as i32).collect();
            len_v3(newell_cross(&pts, &idx)) * 0.5
        }
    }
}

/// `ValueError` raised by an RNA setter (the caller adds `bpy_struct: item.attr = val: `).
pub(crate) fn value_error(msg: impl Into<String>) -> RnaError {
    RnaError::new(RnaErrorKind::Other, PyExc::ValueError, msg)
}

/// Checks element indices written to a topology column (`what` names the property, `of` the
/// indexed domain) against `len` and returns them as `i32`s.
///
/// frap deviation (listed in `docs/deviations.md` by the integrator): Blender 5.2.4 stores any
/// value and leaves an invalid mesh for `Mesh.validate()` to repair; frap refuses
/// out-of-range indices with `ValueError`, so normals, loop triangles and every later read
/// stay well defined (§21: no panics on user-controlled data).
pub(crate) fn check_indices(
    what: &str,
    values: impl IntoIterator<Item = i64>,
    len: usize,
    of: &str,
) -> Result<Vec<i32>, RnaError> {
    values
        .into_iter()
        .map(|v| match usize::try_from(v) {
            Ok(u) if u < len => Ok(v as i32),
            _ => Err(value_error(format!(
                "frap: {what}: index {v} is out of range for {len} {of} (Blender stores it and \
                 leaves an invalid mesh for Mesh.validate(); frap refuses it)"
            ))),
        })
        .collect()
}
