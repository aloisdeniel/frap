// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh merge of `object.join` (port of the mesh part of `ED_mesh_join_objects_exec`,
//! `editors/mesh/meshtools.cc`, Blender 5.2.4, GPL-2.0-or-later), as verified on 5.2.4:
//!
//! - elements are concatenated part by part (the active object first, then the other selected
//!   meshes in base order); vertex, edge and corner indices are offset; positions are mapped
//!   into the active object's space (`inverse(active world) · part world`);
//! - attributes are merged by name: the result lists `position`, `.edge_verts`,
//!   `.corner_vert`, `.corner_edge`, then `material_index` (when written), then the active
//!   mesh's other attributes in its order, then the attributes only later parts have, in their
//!   order; a part without an attribute contributes zeros (`False`, `(0, 0)`, black);
//! - vertex groups are merged by name (the active mesh's first); the active mesh's UV map and
//!   colour-attribute choices are kept.
//!
//! An attribute that exists with different domains or types on two parts is converted by
//! Blender (highest-complexity type, highest-priority domain); frap refuses it with
//! `OP_NOT_IMPLEMENTED` (rename one of them first).

use std::sync::Arc;

use frap_math::glam::{Mat3, Mat4, Vec3};
use frap_mesh::{AttrData, Attribute, CUSTOM_NORMAL, DeformVert, Domain, Mesh};
use smol_str::SmolStr;

use crate::ops::material_slot_add::common::MATERIAL_INDEX;

/// One mesh joined into the result.
pub(crate) struct Part<'a> {
    /// The mesh (original data, not evaluated).
    pub mesh: &'a Mesh,
    /// Into the active object's space; `None` for the active object itself.
    pub xform: Option<Mat4>,
    /// New slot index of each of the part's slots (empty without material handling).
    pub matmap: Vec<i32>,
}

const POSITION: &str = "position";
const EDGE_VERTS: &str = ".edge_verts";
const CORNER_VERT: &str = ".corner_vert";
const CORNER_EDGE: &str = ".corner_edge";

fn is_topology(name: &str) -> bool {
    matches!(
        name,
        POSITION | EDGE_VERTS | CORNER_VERT | CORNER_EDGE | MATERIAL_INDEX
    )
}

fn domain_len(m: &Mesh, d: Domain) -> usize {
    match d {
        Domain::Point => m.verts_num(),
        Domain::Edge => m.edges_num(),
        Domain::Face => m.faces_num(),
        Domain::Corner => m.corners_num(),
    }
}

/// Concatenates columns of one type.
fn concat(cols: &[AttrData]) -> Option<AttrData> {
    macro_rules! go {
        ($($v:ident),*) => {
            match cols.first()? {
                $(AttrData::$v(_) => {
                    let mut out = Vec::new();
                    for c in cols {
                        match c {
                            AttrData::$v(x) => out.extend_from_slice(x.as_slice()),
                            _ => return None,
                        }
                    }
                    Some(AttrData::$v(Arc::new(out)))
                })*
                _ => None,
            }
        };
    }
    go!(
        Float, Int, Int8, Bool, Float2, Float3, Float4, FloatColor, ByteColor, Int16x2, Int32x2,
        Quaternion, Float4x4, String
    )
}

/// Normals in a part's space mapped into the active object's (inverse transpose, normalised).
fn transform_normals(data: &AttrData, xform: &Mat4) -> AttrData {
    match data {
        AttrData::Float3(v) => {
            let n = Mat3::from_mat4(*xform).inverse().transpose();
            AttrData::Float3(Arc::new(
                v.iter()
                    .map(|x| (n * Vec3::from(*x)).normalize_or_zero().to_array())
                    .collect(),
            ))
        }
        other => other.clone(),
    }
}

/// The face material indices of a part after remapping (`matmap`), clamped to its slots.
fn material_indices(p: &Part) -> Vec<i32> {
    let n = p.mesh.faces_num();
    let src: Option<&[i32]> = match p.mesh.attributes().get(MATERIAL_INDEX).map(|a| &a.data) {
        Some(AttrData::Int(v)) => Some(v.as_slice()),
        _ => None,
    };
    (0..n)
        .map(|f| {
            let mi = src.map_or(0, |s| s[f]);
            if p.matmap.is_empty() {
                return 0;
            }
            let i = mi.clamp(0, p.matmap.len() as i32 - 1) as usize;
            p.matmap[i]
        })
        .collect()
}

/// Joins `parts` (the active mesh first) into a new mesh named after `parts[0]` and carrying
/// its settings. `write_material_index` adds the remapped `material_index` attribute.
pub(crate) fn join(parts: &[Part], write_material_index: bool) -> Result<Mesh, String> {
    let base = parts.first().ok_or("nothing to join")?.mesh;
    let (mut positions, mut edges, mut offsets, mut corner_verts, mut corner_edges) =
        (Vec::new(), Vec::new(), vec![0i32], Vec::new(), Vec::new());
    for p in parts {
        let (v0, e0, c0) = (
            positions.len() as i32,
            edges.len() as i32,
            corner_verts.len() as i32,
        );
        match &p.xform {
            Some(m) => positions.extend(
                p.mesh
                    .positions()
                    .iter()
                    .map(|x| m.transform_point3(Vec3::from(*x)).to_array()),
            ),
            None => positions.extend_from_slice(p.mesh.positions()),
        }
        edges.extend(p.mesh.edges().iter().map(|e| [e[0] + v0, e[1] + v0]));
        offsets.extend(p.mesh.face_offsets().iter().skip(1).map(|o| o + c0));
        corner_verts.extend(p.mesh.corner_verts().iter().map(|v| v + v0));
        corner_edges.extend(p.mesh.corner_edges().iter().map(|e| e + e0));
    }
    let mut out = Mesh::new(base.header.name.clone());
    out.set_positions(positions);
    out.set_topology(offsets, corner_verts, edges);
    out.attributes_mut().insert(Attribute {
        name: CORNER_EDGE.into(),
        domain: Domain::Corner,
        data: AttrData::Int(Arc::new(corner_edges)),
    });
    if write_material_index {
        let mi: Vec<i32> = parts.iter().flat_map(material_indices).collect();
        out.attributes_mut().insert(Attribute {
            name: MATERIAL_INDEX.into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new(mi)),
        });
    }
    // Generic attributes, by name, in first-seen order.
    let mut names: Vec<(SmolStr, Domain, AttrData)> = Vec::new();
    for p in parts {
        for a in p.mesh.attributes().iter() {
            if is_topology(&a.name) {
                continue;
            }
            match names.iter().find(|(n, _, _)| *n == a.name) {
                Some((_, d, proto)) => {
                    if *d != a.domain || proto.data_type() != a.data.data_type() {
                        return Err(format!(
                            "frap: joining attribute '{}' stored as {} {} and {} {} is not \
                             implemented (Blender converts it); rename one of them first",
                            a.name,
                            d.identifier(),
                            proto.data_type(),
                            a.domain.identifier(),
                            a.data.data_type()
                        ));
                    }
                }
                None => names.push((a.name.clone(), a.domain, a.data.clone())),
            }
        }
    }
    for (name, domain, proto) in names {
        let cols: Vec<AttrData> = parts
            .iter()
            .map(|p| match p.mesh.attributes().get(&name) {
                Some(a) => match (&p.xform, name == CUSTOM_NORMAL) {
                    (Some(m), true) => transform_normals(&a.data, m),
                    _ => a.data.clone(),
                },
                None => proto.default_of_len(domain_len(p.mesh, domain)),
            })
            .collect();
        let data = concat(&cols).ok_or_else(|| format!("frap internal: attribute '{name}'"))?;
        out.attributes_mut()
            .insert(Attribute { name, domain, data });
    }
    // Vertex groups by name.
    let mut groups: Vec<SmolStr> = base.vertex_group_names.clone();
    for p in parts {
        for g in &p.mesh.vertex_group_names {
            if !groups.contains(g) {
                groups.push(g.clone());
            }
        }
    }
    if parts.iter().any(|p| p.mesh.deform_verts().is_some()) {
        let mut dv: Vec<DeformVert> = Vec::with_capacity(out.verts_num());
        for p in parts {
            match p.mesh.deform_verts() {
                Some(src) => dv.extend(src.iter().map(|d| {
                    let mut n = DeformVert::default();
                    for &(g, w) in &d.weights {
                        let name = p.mesh.vertex_group_names.get(g as usize);
                        let ng = name
                            .and_then(|nm| groups.iter().position(|x| x == nm))
                            .unwrap_or(g as usize);
                        n.weights.push((ng as u32, w));
                    }
                    n
                })),
                None => dv.extend((0..p.mesh.verts_num()).map(|_| DeformVert::default())),
            }
        }
        *out.deform_verts_mut() = dv;
    }
    out.header = base.header.clone();
    out.vertex_group_names = groups;
    out.uv_active = base.uv_active.clone();
    out.uv_active_render = base.uv_active_render.clone();
    out.color_active = base.color_active.clone();
    out.color_default = base.color_default.clone();
    out.texspace = base.texspace;
    out.materials = base.materials.clone();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(name: &str, at: f32) -> Mesh {
        let p = vec![
            [at, 0.0, 0.0],
            [at + 1.0, 0.0, 0.0],
            [at + 1.0, 1.0, 0.0],
            [at, 1.0, 0.0],
        ];
        Mesh::from_faces_bmesh_order(name, p, &[&[0, 1, 2, 3]])
    }

    #[test]
    fn concatenates_with_offsets_and_transform() {
        let a = quad("A", 0.0);
        let b = quad("B", 0.0);
        let parts = [
            Part {
                mesh: &a,
                xform: None,
                matmap: vec![0],
            },
            Part {
                mesh: &b,
                xform: Some(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))),
                matmap: vec![1],
            },
        ];
        let m = join(&parts, true).unwrap();
        assert_eq!(m.verts_num(), 8);
        assert_eq!(m.edges_num(), 8);
        assert_eq!(m.faces_num(), 2);
        assert_eq!(m.positions()[4], [5.0, 0.0, 0.0]);
        assert_eq!(m.edges()[4], [b.edges()[0][0] + 4, b.edges()[0][1] + 4]);
        assert_eq!(m.corner_verts()[4..], [4, 5, 6, 7]);
        assert_eq!(m.corner_edges()[4], b.corner_edges()[0] + 4);
        assert_eq!(m.face_offsets(), &[0, 4, 8]);
        let names: Vec<&str> = m.attributes().iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            [
                POSITION,
                EDGE_VERTS,
                CORNER_VERT,
                CORNER_EDGE,
                MATERIAL_INDEX
            ]
        );
        match &m.attributes().get(MATERIAL_INDEX).unwrap().data {
            AttrData::Int(v) => assert_eq!(v.as_slice(), &[0, 1]),
            _ => panic!(),
        }
        assert_eq!(m.header.name, "A");
    }

    fn parts<'a>(a: &'a Mesh, b: &'a Mesh) -> [Part<'a>; 2] {
        [
            Part {
                mesh: a,
                xform: None,
                matmap: vec![],
            },
            Part {
                mesh: b,
                xform: Some(Mat4::IDENTITY),
                matmap: vec![],
            },
        ]
    }

    #[test]
    fn missing_attributes_are_zero_filled_and_conflicts_refused() {
        let mut a = quad("A", 0.0);
        let mut b = quad("B", 0.0);
        b.attributes_mut().insert(Attribute {
            name: "w".into(),
            domain: Domain::Point,
            data: AttrData::Float(Arc::new(vec![1.0, 2.0, 3.0, 4.0])),
        });
        a.vertex_group_names = vec!["G0".into(), "G1".into()];
        b.vertex_group_names = vec!["G1".into()];
        b.deform_verts_mut()[0].weights.push((0, 0.5));
        let m = join(&parts(&a, &b), false).unwrap();
        match &m.attributes().get("w").unwrap().data {
            AttrData::Float(v) => {
                assert_eq!(v.as_slice(), &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0])
            }
            _ => panic!(),
        }
        assert!(m.attributes().get(MATERIAL_INDEX).is_none());
        assert_eq!(m.vertex_group_names, ["G0", "G1"]);
        let dv = m.deform_verts().unwrap();
        assert_eq!(dv.len(), 8);
        assert_eq!(dv[4].weights.as_slice(), &[(1, 0.5)]);
        a.attributes_mut().insert(Attribute {
            name: "w".into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new(vec![7])),
        });
        assert!(join(&parts(&a, &b), false).is_err());
    }
}
