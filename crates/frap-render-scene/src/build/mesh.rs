// SPDX-License-Identifier: GPL-3.0-or-later
//! Evaluated `Mesh` → [`RMesh`]: corner-split vertices ([`frap_mesh::split::corner_split`], the
//! same split as the glTF exporter, §6.1), loop triangles grouped by material slot, wireframe
//! edges.
//!
//! Layout of the result:
//! - vertex `i < split.len()` is split vertex `i` of `corner_split` (position of
//!   `vert_source[i]`, corner normal of `vert_corner[i]`, UVs / colours of that corner);
//! - vertices after them are the loose vertices (used by no face) that edges need, in vertex
//!   order, with their vertex normal and their POINT-domain colours (corner-domain colours and
//!   UVs have no value on a loose vertex: `[0, 0, 0, 1]` / `[0, 0]`);
//! - `uvs[n]` is the n-th UV map in `Mesh.uv_layers` order; `colors[n]` the n-th colour
//!   attribute (FLOAT_COLOR / BYTE_COLOR on POINT or CORNER, attribute order, BYTE_COLOR
//!   decoded from sRGB to linear as Blender does);
//! - triangles are Blender's loop triangles, bucketed by material slot (`material_index`
//!   clamped to `[0, max(slots, 1) - 1]` as the draw cache does), slots ascending, loop-triangle
//!   order inside a slot; `submeshes` lists the non-empty slots;
//! - `uv_active` / `color_active` / `color_default` index the UV map and colour attributes the
//!   engines read by default (WP 5.5): `active_or_default_uv_map_name()`, the colour Workbench
//!   VERTEX draws, the default (render) colour;
//! - `tangents` (only when asked for, [`extract_with_tangents`]): MikkTSpace tangents of one UV
//!   map, as EEVEE's draw cache computes them for a Normal Map node (`extract_tangent`,
//!   `bke::mesh::calc_uv_tangents`: triangles and quads are MikkTSpace faces, larger faces their
//!   loop triangles), one per split vertex (the tangent of its representative corner).

use std::sync::Arc;

use frap_color::spaces::srgb_to_linear;
use frap_mesh::split::corner_split;
use frap_mesh::{AttrData, Domain, Mesh};

use crate::types::RMesh;

/// Blender's per-face material slot attribute.
pub const MATERIAL_INDEX: &str = "material_index";

/// Per-face material slot, clamped like `mesh_render_mat_len` / the draw cache do.
fn face_slots(mesh: &Mesh, slot_count: usize) -> Vec<u32> {
    let last = slot_count.max(1) as i32 - 1;
    let n = mesh.faces_num();
    match mesh.attributes().get(MATERIAL_INDEX) {
        Some(a) if a.domain == Domain::Face => match &a.data {
            AttrData::Int(v) if v.len() == n => {
                v.iter().map(|&i| i.clamp(0, last) as u32).collect()
            }
            _ => vec![0; n],
        },
        _ => vec![0; n],
    }
}

enum ColorCol<'a> {
    Float(&'a [[f32; 4]]),
    Byte(&'a [[u8; 4]]),
}

fn byte_to_linear(c: [u8; 4]) -> [f32; 4] {
    let f = |x: u8| f32::from(x) / 255.0;
    [
        srgb_to_linear(f(c[0])),
        srgb_to_linear(f(c[1])),
        srgb_to_linear(f(c[2])),
        f(c[3]),
    ]
}

/// Extracts the render mesh of `mesh`, whose object has `slot_count` material slots (no
/// tangents).
pub fn extract(mesh: &Mesh, slot_count: usize) -> RMesh {
    extract_with_tangents(mesh, slot_count, None)
}

/// [`extract`], with the tangents of the UV map named `tangent_uv` when it names one
/// (`None`, or a name that is no UV map: no tangents).
pub fn extract_with_tangents(mesh: &Mesh, slot_count: usize, tangent_uv: Option<&str>) -> RMesh {
    let split = corner_split(mesh);
    let positions_src = mesh.positions();
    let corner_normals = mesh.corner_normals();

    let mut out = RMesh {
        positions: split
            .vert_source
            .iter()
            .map(|&v| positions_src[v as usize])
            .collect(),
        normals: split
            .vert_corner
            .iter()
            .map(|&c| {
                corner_normals
                    .get(c as usize)
                    .copied()
                    .unwrap_or([0.0, 0.0, 1.0])
            })
            .collect(),
        ..RMesh::default()
    };

    let uv_maps = mesh.uv_maps();
    out.uv_names = uv_maps
        .iter()
        .map(|(n, _)| smol_str::SmolStr::new(n))
        .collect();
    out.uv_active_render = mesh
        .uv_active_render
        .as_deref()
        .and_then(|a| out.uv_names.iter().position(|n| n == a))
        .or_else(|| (!out.uv_names.is_empty()).then_some(0));
    out.uv_active = mesh
        .uv_active
        .as_deref()
        .and_then(|a| out.uv_names.iter().position(|n| n == a))
        .or(out.uv_active_render);
    out.tangents = tangent_uv
        .and_then(|name| uv_maps.iter().find(|(n, _)| *n == name))
        .map(|(_, uv)| {
            let t = corner_tangents(mesh, uv, &corner_normals);
            split
                .vert_corner
                .iter()
                .map(|&c| t.get(c as usize).copied().unwrap_or(NO_TANGENT))
                .collect()
        });
    out.uvs = uv_maps
        .into_iter()
        .map(|(_, uv)| split.vert_corner.iter().map(|&c| uv[c as usize]).collect())
        .collect();

    // Per colour layer: its POINT-domain values (decoded), so loose vertices can take theirs.
    let mut point_colors: Vec<Option<Vec<[f32; 4]>>> = Vec::new();
    for a in mesh.attributes().iter() {
        if a.name.starts_with('.') || !matches!(a.domain, Domain::Point | Domain::Corner) {
            continue;
        }
        let col = match &a.data {
            AttrData::FloatColor(v) => ColorCol::Float(v.as_slice()),
            AttrData::ByteColor(v) => ColorCol::Byte(v.as_slice()),
            _ => continue,
        };
        let value = |i: u32| match &col {
            ColorCol::Float(v) => v[i as usize],
            ColorCol::Byte(v) => byte_to_linear(v[i as usize]),
        };
        out.color_names.push(a.name.clone());
        if a.domain == Domain::Corner {
            out.colors
                .push(split.vert_corner.iter().map(|&c| value(c)).collect());
            point_colors.push(None);
        } else {
            let per_vert: Vec<[f32; 4]> = (0..mesh.verts_num() as u32).map(value).collect();
            out.colors.push(
                split
                    .vert_source
                    .iter()
                    .map(|&v| per_vert[v as usize])
                    .collect(),
            );
            point_colors.push(Some(per_vert));
        }
    }

    let color_index = |name: Option<&str>| {
        name.filter(|n| !n.is_empty())
            .and_then(|n| out.color_names.iter().position(|c| c == n))
    };
    // `mesh_has_color_attribute`: the active colour, else the default one (an active name that
    // is no colour attribute gives none).
    out.color_active = match mesh.color_active.as_deref().filter(|n| !n.is_empty()) {
        Some(active) => color_index(Some(active)),
        None => color_index(mesh.color_default.as_deref()),
    };
    out.color_default = color_index(mesh.color_default.as_deref());

    // Triangles, bucketed by slot.
    let looptris = mesh.looptris();
    let tri_faces = mesh.looptri_faces();
    let slots = face_slots(mesh, slot_count);
    let n_buckets = slot_count.max(1);
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); n_buckets];
    for (t, tri) in looptris.iter().enumerate() {
        let slot = tri_faces
            .get(t)
            .and_then(|&f| slots.get(f as usize))
            .copied()
            .unwrap_or(0) as usize;
        buckets[slot].extend(tri.iter().map(|&c| split.corner_vert[c as usize]));
    }
    for (slot, b) in buckets.into_iter().enumerate() {
        if b.is_empty() {
            continue;
        }
        let start = out.indices.len() as u32;
        out.indices.extend(b);
        out.submeshes
            .push((slot as u32, start..out.indices.len() as u32));
    }

    // Edges: each mesh vertex maps to its first split vertex; loose vertices are appended.
    let edges = mesh.edges();
    if !edges.is_empty() {
        let mut first: Vec<u32> = vec![u32::MAX; mesh.verts_num()];
        for (i, &v) in split.vert_source.iter().enumerate() {
            let slot = &mut first[v as usize];
            if *slot == u32::MAX {
                *slot = i as u32;
            }
        }
        let mut loose: Vec<usize> = edges
            .iter()
            .flat_map(|e| [e[0] as usize, e[1] as usize])
            .filter(|&v| first.get(v) == Some(&u32::MAX))
            .collect();
        // Loose vertices in vertex order (deterministic whatever the edge order).
        loose.sort_unstable();
        loose.dedup();
        if !loose.is_empty() {
            let vn: Arc<Vec<[f32; 3]>> = mesh.vert_normals();
            for v in loose {
                first[v] = out.positions.len() as u32;
                out.positions.push(positions_src[v]);
                out.normals
                    .push(vn.get(v).copied().unwrap_or([0.0, 0.0, 1.0]));
                for uv in &mut out.uvs {
                    uv.push([0.0, 0.0]);
                }
                if let Some(t) = &mut out.tangents {
                    t.push(NO_TANGENT);
                }
                for (c, point) in out.colors.iter_mut().zip(&point_colors) {
                    c.push(match point {
                        Some(per_vert) => per_vert[v],
                        None => [0.0, 0.0, 0.0, 1.0],
                    });
                }
            }
        }
        out.edges = edges
            .iter()
            .filter(|e| e.iter().all(|&v| (v as usize) < first.len()))
            .map(|e| [first[e[0] as usize], first[e[1] as usize]])
            .collect();
    }
    out
}

/// The tangent of a corner MikkTSpace gives none (a corner of no face, a loose vertex).
const NO_TANGENT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// MikkTSpace tangent (xyz) and bitangent sign (w) of every corner of `mesh` for the per-corner
/// UV map `uv` (`bke::mesh::calc_uv_tangents` as the draw cache calls it, Blender 5.2.4,
/// GPL-2.0-or-later; `USE_TRI_DETECT_QUADS`: faces of three or four corners are MikkTSpace
/// faces, larger faces enter as their loop triangles).
pub fn corner_tangents(mesh: &Mesh, uv: &[[f32; 2]], corner_normals: &[[f32; 3]]) -> Vec<[f32; 4]> {
    let positions = mesh.positions();
    let corner_verts = mesh.corner_verts();
    let corner_pos: Vec<[f32; 3]> = corner_verts
        .iter()
        .map(|&v| positions[v as usize])
        .collect();
    let corners: Vec<u32> = (0..corner_verts.len() as u32).collect();
    let offsets = mesh.face_offsets();
    let mut faces: Vec<&[u32]> = offsets
        .windows(2)
        .filter(|w| w[1] - w[0] <= 4)
        .map(|w| &corners[w[0] as usize..w[1] as usize])
        .collect();
    // Larger faces: one MikkTSpace face per loop triangle (corner triples).
    let looptris = mesh.looptris();
    let tri_faces = mesh.looptri_faces();
    let big: Vec<[u32; 3]> = tri_faces
        .iter()
        .zip(looptris.iter())
        .filter(|(f, _)| {
            let f = **f as usize;
            offsets
                .get(f + 1)
                .zip(offsets.get(f))
                .is_some_and(|(b, a)| b - a > 4)
        })
        .map(|(_, tri)| *tri)
        .collect();
    faces.extend(big.iter().map(|t| &t[..]));
    if uv.len() != corner_pos.len() || corner_normals.len() != corner_pos.len() {
        return vec![NO_TANGENT; corner_pos.len()];
    }
    let mut out = frap_geom::mikktspace::corner_tangents(&corner_pos, corner_normals, uv, &faces);
    // Corners MikkTSpace never visited keep Blender's zeroed layer, read as (0, 0, 0, 1)
    // ("no tangent", which the Normal Map node skips).
    for t in &mut out {
        if *t == [0.0; 4] {
            *t = NO_TANGENT;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use frap_mesh::{AttrData, Attribute, Domain, Mesh};

    use super::*;

    fn cube() -> Mesh {
        frap_mesh::primitives::cube(2.0, true, [1.0; 3])
    }

    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    #[test]
    fn active_uv_and_colour_indices() {
        let mut m = cube();
        let r = extract(&m, 1);
        assert_eq!(r.uv_names, ["UVMap"]);
        assert_eq!(r.uv_active_render, Some(0));
        assert!(r.tangents.is_none());
        assert_eq!((r.color_active, r.color_default), (None, None));
        let corners = m.corners_num();
        let verts = m.verts_num();
        m.attributes_mut().insert(Attribute {
            name: "Paint".into(),
            domain: Domain::Corner,
            data: AttrData::ByteColor(vec![[255, 0, 0, 255]; corners].into()),
        });
        m.attributes_mut().insert(Attribute {
            name: "Height".into(),
            domain: Domain::Point,
            data: AttrData::FloatColor(vec![[0.0, 1.0, 0.0, 1.0]; verts].into()),
        });
        // `mesh_has_color_attribute`: the active colour, else the default one.
        m.color_active = None;
        m.color_default = Some("Height".into());
        let r = extract(&m, 1);
        assert_eq!(r.color_names, ["Paint", "Height"]);
        assert_eq!((r.color_active, r.color_default), (Some(1), Some(1)));
        m.color_active = Some("Paint".into());
        let r = extract(&m, 1);
        assert_eq!((r.color_active, r.color_default), (Some(0), Some(1)));
        // Byte colours are decoded from sRGB, float colours kept.
        assert_eq!(r.colors[0][0], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(r.colors[1][0], [0.0, 1.0, 0.0, 1.0]);
        // An active name that is no colour attribute: none (no fallback).
        m.color_active = Some("Missing".into());
        assert_eq!(extract(&m, 1).color_active, None);
        // A second UV map made active.
        let uv = m.uv_maps()[0].1.to_vec();
        m.attributes_mut().insert(Attribute {
            name: "Second".into(),
            domain: Domain::Corner,
            data: AttrData::Float2(uv.into()),
        });
        m.uv_active = Some("Second".into());
        m.uv_active_render = Some("UVMap".into());
        let r = extract(&m, 1);
        assert_eq!(r.uv_names, ["UVMap", "Second"]);
        assert_eq!((r.uv_active, r.uv_active_render), (Some(1), Some(0)));
        // No active UV map: the active-render one.
        m.uv_active = None;
        assert_eq!(extract(&m, 1).uv_active, Some(0));
    }

    #[test]
    fn tangents_are_mikktspace_of_the_named_uv_map() {
        let m = cube();
        let r = extract_with_tangents(&m, 1, Some("UVMap"));
        let t = r.tangents.as_ref().expect("tangents");
        assert_eq!(t.len(), r.positions.len());
        // Unit tangents orthogonal to the corner normal, bitangent sign ±1.
        for (i, tan) in t.iter().enumerate() {
            let n = r.normals[i];
            let xyz = [tan[0], tan[1], tan[2]];
            assert!((dot(xyz, xyz) - 1.0).abs() < 1e-4, "{tan:?}");
            assert!(dot(xyz, n).abs() < 1e-4, "{tan:?} {n:?}");
            assert_eq!(tan[3].abs(), 1.0);
        }
        // The same tangents as `Mesh.calc_tangents()` (quads are MikkTSpace faces).
        let uv = m.uv_maps()[0].1.to_vec();
        let reference = frap_geom::mikktspace::calc_uv_tangent_tris_quads(
            m.positions(),
            m.face_offsets(),
            m.corner_verts(),
            &m.corner_normals(),
            &uv,
        )
        .unwrap();
        let split = corner_split(&m);
        for (i, &c) in split.vert_corner.iter().enumerate() {
            assert_eq!(t[i], reference[c as usize]);
        }
        // An unknown UV map: none.
        assert!(
            extract_with_tangents(&m, 1, Some("Nope"))
                .tangents
                .is_none()
        );
        // The tangents are part of the mesh key.
        assert_ne!(
            super::super::keys::mesh_key(&r),
            super::super::keys::mesh_key(&extract(&m, 1))
        );
    }

    #[test]
    fn ngons_enter_mikktspace_as_loop_triangles() {
        let mut m = Mesh::new("");
        let p: Vec<[f32; 3]> = (0..5)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 5.0;
                [a.cos(), a.sin(), 0.0]
            })
            .collect();
        m.from_pydata(&p, &[], &[vec![0, 1, 2, 3, 4]], false);
        let uv: Vec<[f32; 2]> = p
            .iter()
            .map(|v| [v[0] * 0.5 + 0.5, v[1] * 0.5 + 0.5])
            .collect();
        let normals = m.corner_normals();
        let t = corner_tangents(&m, &uv, &normals);
        assert_eq!(t.len(), 5);
        // UVs aligned with X/Y: tangent +X, bitangent +Y (sign 1).
        for tan in t {
            assert!(
                (tan[0] - 1.0).abs() < 1e-4 && tan[1].abs() < 1e-4,
                "{tan:?}"
            );
            assert_eq!(tan[3], 1.0);
        }
        // A UV map of the wrong length: no tangent anywhere.
        assert_eq!(corner_tangents(&m, &uv[..2], &normals), vec![NO_TANGENT; 5]);
    }
}
