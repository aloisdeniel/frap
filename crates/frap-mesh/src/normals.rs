// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh-level normals and loop triangles (ARCHITECTURE §6.1): the dispatch of Blender's
//! `Mesh::face_normals()`, `vert_normals()`, `corner_normals()` and `corner_tris()`
//! (`mesh_normals.cc`, `mesh_tessellate.cc`) over the slice algorithms in `frap-geom`.

use frap_geom::normals::{self as gn, CornerNormalsInput, V3, VertToFaceMap};
use frap_geom::polyfill;

use crate::mesh::{AttrData, Attribute, CUSTOM_NORMAL, Domain, Mesh, NormalsDomain};

fn custom(m: &Mesh) -> Option<&Attribute> {
    m.attributes().get(CUSTOM_NORMAL)
}

fn custom_float3(m: &Mesh) -> Option<(Domain, &[V3])> {
    match custom(m) {
        Some(Attribute {
            domain,
            data: AttrData::Float3(v),
            ..
        }) => Some((*domain, v.as_slice())),
        _ => None,
    }
}

fn custom_short2(m: &Mesh) -> Option<&[[i16; 2]]> {
    match custom(m) {
        Some(Attribute {
            domain: Domain::Corner,
            data: AttrData::Int16x2(v),
            ..
        }) => Some(v.as_slice()),
        _ => None,
    }
}

/// `Mesh::face_normals_true()`: Newell normal of every face.
pub(crate) fn face_normals_true(m: &Mesh) -> Vec<V3> {
    gn::normals_calc_faces(m.positions(), m.face_offsets(), m.corner_verts())
}

fn vert_map(m: &Mesh) -> VertToFaceMap {
    VertToFaceMap::new(m.verts_num(), m.face_offsets(), m.corner_verts())
}

/// `Mesh::face_normals()`.
pub(crate) fn face_normals(m: &Mesh) -> Vec<V3> {
    if m.faces_num() == 0 {
        return Vec::new();
    }
    if let Some((domain, data)) = custom_float3(m) {
        match domain {
            Domain::Face => return data.to_vec(),
            Domain::Point => {
                return gn::mix_normals_vert_to_face(m.face_offsets(), m.corner_verts(), data);
            }
            Domain::Corner => return gn::mix_normals_corner_to_face(m.face_offsets(), data),
            Domain::Edge => {}
        }
    } else if custom_short2(m).is_some() {
        return gn::mix_normals_corner_to_face(m.face_offsets(), &m.corner_normals());
    }
    face_normals_true(m)
}

/// The "true" face normals: the cached [`Mesh::face_normals`] when no custom normals change
/// them, else a fresh computation.
fn true_face_normals_shared(m: &Mesh) -> std::sync::Arc<Vec<V3>> {
    if custom(m).is_none() {
        m.face_normals()
    } else {
        std::sync::Arc::new(face_normals_true(m))
    }
}

/// `Mesh::vert_normals()`.
pub(crate) fn vert_normals(m: &Mesh) -> Vec<V3> {
    let map = vert_map(m);
    if let Some((domain, data)) = custom_float3(m) {
        match domain {
            Domain::Point => return data.to_vec(),
            Domain::Face => {
                return gn::normals_calc_verts(
                    m.positions(),
                    m.face_offsets(),
                    m.corner_verts(),
                    &map,
                    data,
                );
            }
            Domain::Corner => {
                return gn::mix_normals_corner_to_vert(
                    m.positions(),
                    m.face_offsets(),
                    m.corner_verts(),
                    &map,
                    data,
                );
            }
            Domain::Edge => {}
        }
    } else if custom_short2(m).is_some() {
        return gn::mix_normals_corner_to_vert(
            m.positions(),
            m.face_offsets(),
            m.corner_verts(),
            &map,
            &m.corner_normals(),
        );
    }
    let fnor = true_face_normals_shared(m);
    gn::normals_calc_verts(
        m.positions(),
        m.face_offsets(),
        m.corner_verts(),
        &map,
        &fnor,
    )
}

/// `Mesh::corner_normals()`.
pub(crate) fn corner_normals(m: &Mesh) -> Vec<V3> {
    if m.faces_num() == 0 {
        return Vec::new();
    }
    match m.normals_domain() {
        NormalsDomain::Point => {
            let vn = m.vert_normals();
            m.corner_verts().iter().map(|&v| vn[v as usize]).collect()
        }
        NormalsDomain::Face => {
            let fnor = m.face_normals();
            let mut out = vec![[0.0; 3]; m.corners_num()];
            for f in 0..m.faces_num() {
                for c in m.face(f) {
                    out[c] = fnor[f];
                }
            }
            out
        }
        NormalsDomain::Corner => {
            if let Some((domain, data)) = custom_float3(m) {
                return if domain == Domain::Corner {
                    data.to_vec()
                } else {
                    vec![[0.0; 3]; m.corners_num()]
                };
            }
            let map = vert_map(m);
            let fnor = true_face_normals_shared(m);
            let (sharp_edges, sharp_faces) = (m.sharp_edge_flags(), m.sharp_face_flags());
            gn::normals_calc_corners(&CornerNormalsInput {
                positions: m.positions(),
                face_offsets: m.face_offsets(),
                corner_verts: m.corner_verts(),
                corner_edges: m.corner_edges(),
                vert_to_face: &map,
                face_normals: &fnor,
                sharp_edges: sharp_edges.as_deref(),
                sharp_faces: sharp_faces.as_deref(),
                custom_normals: custom_short2(m),
            })
        }
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_tessellate.cc
// (mesh_calc_tessellation_for_face_impl).
/// Loop triangles: triangles as is; quads `(0,1,2),(0,2,3)`, or `(0,1,3),(1,2,3)` when the
/// 0–2 split is degenerate (`is_quad_flip_v3_first_third_fast`); n-gons by `BLI_polyfill_2d`
/// without beautify.
pub(crate) fn looptris(m: &Mesh) -> Vec<[u32; 3]> {
    let p = m.positions();
    let cv = m.corner_verts();
    let mut out = Vec::with_capacity(m.corners_num());
    for f in 0..m.faces_num() {
        frap_math::budget::check();
        let r = m.face(f);
        let s = r.start as u32;
        match r.len() {
            0..=2 => {}
            3 => out.push([s, s + 1, s + 2]),
            4 => {
                let v = |i: usize| p[cv[r.start + i] as usize];
                if polyfill::is_quad_flip_v3_first_third_fast(v(0), v(1), v(2), v(3)) {
                    out.push([s, s + 1, s + 3]);
                    out.push([s + 1, s + 2, s + 3]);
                } else {
                    out.push([s, s + 1, s + 2]);
                    out.push([s, s + 2, s + 3]);
                }
            }
            _ => {
                for t in polyfill::polyfill_mesh_face(p, &cv[r.clone()]) {
                    out.push([s + t[0], s + t[1], s + t[2]]);
                }
            }
        }
    }
    out
}
