// SPDX-License-Identifier: GPL-3.0-or-later
//! The part of `Mesh.validate()` an imported mesh can trip: faces that repeat a vertex
//! (degenerate strip triangles, welded triangles) and duplicate faces are removed, and so are
//! edges whose two vertices are the same. Other data (positions, loose edges) is kept.
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_validate.cc
//! (`BKE_mesh_validate_arrays`: "duplicated vert reference", duplicate faces, "same vertices"
//! edges), Blender 5.2, GPL-2.0-or-later. Of two identical faces the later one is removed.

use std::collections::HashSet;

use frap_mesh::{AttrData, Attribute, Domain, Mesh};

/// Removes invalid faces and edges; returns true when something was removed.
pub(crate) fn validate(mesh: &mut Mesh) -> bool {
    let faces_num = mesh.faces_num();
    let cv = mesh.corner_verts().to_vec();
    let mut keep_face = vec![true; faces_num];
    let mut seen: HashSet<Vec<i32>> = HashSet::new();
    for (f, keep) in keep_face.iter_mut().enumerate() {
        let r = mesh.face(f);
        let mut verts: Vec<i32> = cv[r].to_vec();
        verts.sort_unstable();
        if verts.windows(2).any(|w| w[0] == w[1]) || verts.len() < 3 {
            *keep = false;
            continue;
        }
        if !seen.insert(verts) {
            *keep = false;
        }
    }
    let edges = mesh.edges().to_vec();
    let keep_edge: Vec<bool> = edges.iter().map(|e| e[0] != e[1]).collect();
    if keep_face.iter().all(|k| *k) && keep_edge.iter().all(|k| *k) {
        return false;
    }

    let mut face_src = Vec::new();
    let mut corner_src = Vec::new();
    let mut offsets = vec![0i32];
    for (f, _) in keep_face.iter().enumerate().filter(|(_, k)| **k) {
        face_src.push(f);
        corner_src.extend(mesh.face(f));
        offsets.push(corner_src.len() as i32);
    }
    let edge_src: Vec<usize> = (0..edges.len()).filter(|&e| keep_edge[e]).collect();
    let new_cv: Vec<i32> = corner_src.iter().map(|&c| cv[c]).collect();
    let new_edges: Vec<[i32; 2]> = edge_src.iter().map(|&e| edges[e]).collect();

    let mut out = Mesh::new(mesh.header.name.clone());
    out.header = mesh.header.clone();
    out.materials = mesh.materials.clone();
    out.uv_active = mesh.uv_active.clone();
    out.uv_active_render = mesh.uv_active_render.clone();
    out.color_active = mesh.color_active.clone();
    out.color_default = mesh.color_default.clone();
    out.texspace = mesh.texspace;
    out.vertex_group_names = mesh.vertex_group_names.clone();
    out.shape_keys = mesh.shape_keys;
    for a in mesh.attributes().iter() {
        let data = match a.domain {
            Domain::Point => a.data.clone(),
            Domain::Face => a.data.gather_with_default(&face_src, face_src.len()),
            Domain::Corner => a.data.gather_with_default(&corner_src, corner_src.len()),
            Domain::Edge => a.data.gather_with_default(&edge_src, edge_src.len()),
        };
        out.attributes_mut().insert(Attribute {
            name: a.name.clone(),
            domain: a.domain,
            data,
        });
    }
    out.set_topology(offsets, new_cv, new_edges);
    // `.corner_edge` is recomputed from the kept edges by `set_topology`.
    if let Some(Attribute {
        data: AttrData::Int(ce),
        ..
    }) = out.attributes().get(".corner_edge")
        && ce.iter().any(|&e| e < 0)
    {
        out.calc_edges(true, true);
    }
    *mesh = out;
    true
}
