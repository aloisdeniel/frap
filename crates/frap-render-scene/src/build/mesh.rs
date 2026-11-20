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
//!   order inside a slot; `submeshes` lists the non-empty slots.

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

/// Extracts the render mesh of `mesh`, whose object has `slot_count` material slots.
pub fn extract(mesh: &Mesh, slot_count: usize) -> RMesh {
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
