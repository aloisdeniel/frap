// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.split_faces()` (`ED_mesh_split_faces`, `editors/mesh/mesh_data.cc`, with
//! `geometry::split_edges`, `geometry/intern/mesh_split_edges.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! The edges to split are the `sharp_edge` edges plus every edge of a flat (`sharp_face`)
//! face; nothing happens without one. Around every vertex of a split edge, the corners are
//! grouped by the faces still connected across unsplit edges; each group but the last gets a
//! new vertex (appended, attributes and vertex-group weights copied from the original), the
//! last keeps the original. Each split edge keeps its index for its first corner's edge and
//! gets new edges (appended, attributes copied) for the others (identical ones merged); every
//! edge used by faces is rewritten as `(low, high)` vertex order, as Blender does. Loose split
//! edges get the new vertices left over. Refused in Edit Mode (`RNA_EDITMODE_WRITE`, D-06).

use std::sync::Arc;

use frap_mesh::{AttrData, Domain, Mesh};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of};

/// `bke::mesh::face_corner_prev`.
fn corner_prev(r: &std::ops::Range<usize>, c: usize) -> usize {
    if c == r.start { r.end - 1 } else { c - 1 }
}
/// `bke::mesh::face_corner_next`.
fn corner_next(r: &std::ops::Range<usize>, c: usize) -> usize {
    if c + 1 == r.end { r.start } else { c + 1 }
}

/// Groups of indices (`GroupedSpan`), each in ascending source order.
fn group_by(n: usize, keys: impl Iterator<Item = (usize, usize)>) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); n];
    for (k, v) in keys {
        out[k].push(v);
    }
    out
}

/// `OrderedEdge(a, b)` as `(low, high)`.
fn ordered(a: i32, b: i32) -> [i32; 2] {
    [a.min(b), a.max(b)]
}

// Ported from blender/source/blender/geometry/intern/mesh_split_edges.cc
// (calc_corner_groups_for_vertex).
#[allow(clippy::too_many_arguments)]
fn corner_groups_for_vertex(
    faces: &[std::ops::Range<usize>],
    corner_verts: &[i32],
    corner_edges: &[i32],
    edge_to_corner: &[Vec<usize>],
    corner_to_face: &[usize],
    split: &[bool],
    connected: &[usize],
    vert: usize,
) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut used = vec![false; connected.len()];
    for &start in connected {
        let mut group = Vec::new();
        let mut stack = vec![start];
        while let Some(corner) = stack.pop() {
            let i = connected
                .iter()
                .position(|&c| c == corner)
                .expect("corner of the vertex");
            if used[i] {
                continue;
            }
            used[i] = true;
            group.push(corner);
            let face = corner_to_face[corner];
            let prev = corner_prev(&faces[face], corner);
            for edge in [corner_edges[corner], corner_edges[prev]] {
                let edge = edge as usize;
                if split[edge] {
                    continue;
                }
                for &other in &edge_to_corner[edge] {
                    let other_face = corner_to_face[other];
                    if other_face == face {
                        continue;
                    }
                    let neighbor = if corner_verts[other] as usize == vert {
                        other
                    } else {
                        corner_next(&faces[other_face], other)
                    };
                    stack.push(neighbor);
                }
            }
        }
        if !group.is_empty() {
            groups.push(group);
        }
    }
    groups
}

/// Selected and unselected loose edges at a vertex (`calc_vert_loose_edges`).
fn vert_loose_edges(
    vert_to_edge: &[Vec<usize>],
    loose: &[bool],
    split: &[bool],
    vert: usize,
) -> (Vec<usize>, Vec<usize>) {
    let (mut sel, mut unsel) = (Vec::new(), Vec::new());
    for &e in &vert_to_edge[vert] {
        if loose[e] {
            if split[e] {
                sel.push(e);
            } else {
                unsel.push(e);
            }
        }
    }
    (sel, unsel)
}

/// Appends `map`-gathered copies of every attribute of `domain` (except `skip`).
fn propagate(mesh: &mut Mesh, domain: Domain, old_len: usize, map: &[usize], skip: &str) {
    let index: Vec<usize> = (0..old_len).chain(map.iter().copied()).collect();
    for a in mesh.attributes_mut().iter_mut() {
        if a.domain != domain || a.name == skip || matches!(a.data, AttrData::String(_)) {
            continue;
        }
        a.data = a.data.gather_with_default(&index, index.len());
    }
}

// Ported from blender/source/blender/geometry/intern/mesh_split_edges.cc (split_edges).
/// `geometry::split_edges(mesh, split)`.
pub(crate) fn split_edges(mesh: &mut Mesh, split: &[bool]) {
    let orig_verts = mesh.verts_num();
    let orig_edges: Vec<[i32; 2]> = mesh.edges().to_vec();
    let faces: Vec<std::ops::Range<usize>> = (0..mesh.faces_num()).map(|f| mesh.face(f)).collect();
    let mut corner_verts = mesh.corner_verts().to_vec();
    let mut corner_edges = mesh.corner_edges().to_vec();
    let corners = corner_verts.len();
    let mut corner_to_face = vec![0usize; corners];
    for (f, r) in faces.iter().enumerate() {
        corner_to_face[r.clone()].fill(f);
    }
    let vert_to_corner = group_by(
        orig_verts,
        corner_verts
            .iter()
            .enumerate()
            .map(|(c, &v)| (v as usize, c)),
    );
    let edge_to_corner = group_by(
        orig_edges.len(),
        corner_edges
            .iter()
            .enumerate()
            .map(|(c, &e)| (e as usize, c)),
    );
    let loose: Vec<bool> = edge_to_corner.iter().map(Vec::is_empty).collect();
    let any_loose = loose.iter().any(|&l| l);
    let vert_to_edge = group_by(
        orig_verts,
        orig_edges
            .iter()
            .enumerate()
            .flat_map(|(e, v)| [(v[0] as usize, e), (v[1] as usize, e)]),
    );
    // `vert_selection_from_edge`: the vertices of split edges, ascending.
    let mut affected = vec![false; orig_verts];
    for (e, v) in orig_edges.iter().enumerate() {
        if split[e] {
            affected[v[0] as usize] = true;
            affected[v[1] as usize] = true;
        }
    }
    let affected: Vec<usize> = (0..orig_verts).filter(|&v| affected[v]).collect();
    let groups: Vec<Vec<Vec<usize>>> = affected
        .iter()
        .map(|&v| {
            corner_groups_for_vertex(
                &faces,
                &corner_verts,
                &corner_edges,
                &edge_to_corner,
                &corner_to_face,
                split,
                &vert_to_corner[v],
                v,
            )
        })
        .collect();
    // `calc_vert_ranges_per_old_vert`.
    let mut counts: Vec<i64> = groups.iter().map(|g| g.len() as i64 - 1).collect();
    if any_loose {
        for (i, &v) in affected.iter().enumerate() {
            let (sel, unsel) = vert_loose_edges(&vert_to_edge, &loose, split, v);
            counts[i] += sel.len() as i64;
            if groups[i].is_empty() {
                counts[i] += i64::from(!unsel.is_empty());
            }
        }
    }
    let mut offsets = Vec::with_capacity(counts.len() + 1);
    offsets.push(0usize);
    for c in &counts {
        offsets.push(offsets.last().copied().unwrap_or(0) + (*c).max(0) as usize);
    }
    // `update_corner_verts`.
    for (i, gs) in groups.iter().enumerate() {
        for (g, group) in gs.iter().enumerate().take(gs.len().saturating_sub(1)) {
            let new_vert = (orig_verts + offsets[i] + g) as i32;
            for &c in group {
                corner_verts[c] = new_vert;
            }
        }
    }
    // `calc_new_edges`.
    let selected: Vec<usize> = (0..orig_edges.len()).filter(|&e| split[e]).collect();
    let mut edges = orig_edges.clone();
    let mut new_edges: Vec<[i32; 2]> = Vec::new();
    let mut edge_map: Vec<usize> = Vec::new();
    for &e in &selected {
        let cs = &edge_to_corner[e];
        if cs.is_empty() {
            continue;
        }
        let mut dedup: Vec<[i32; 2]> = Vec::new();
        for &c in cs {
            let f = corner_to_face[c];
            let next = corner_next(&faces[f], c);
            let oe = ordered(corner_verts[c], corner_verts[next]);
            let index = match dedup.iter().position(|d| *d == oe) {
                Some(i) => i,
                None => {
                    dedup.push(oe);
                    dedup.len() - 1
                }
            };
            if index != 0 {
                corner_edges[c] = (orig_edges.len() + new_edges.len() + index - 1) as i32;
            }
        }
        edges[e] = dedup[0];
        for d in &dedup[1..] {
            new_edges.push(*d);
            edge_map.push(e);
        }
    }
    // `update_unselected_edges`.
    for (e, cs) in edge_to_corner.iter().enumerate() {
        if split[e] || cs.is_empty() {
            continue;
        }
        let c = cs[0];
        let next = corner_next(&faces[corner_to_face[c]], c);
        edges[e] = ordered(corner_verts[c], corner_verts[next]);
    }
    // `reassign_loose_edge_verts`.
    if any_loose {
        for (i, &v) in affected.iter().enumerate() {
            let range_len = offsets[i + 1] - offsets[i];
            let mut k = groups[i].len().saturating_sub(1);
            if k == range_len {
                continue;
            }
            let (sel, unsel) = vert_loose_edges(&vert_to_edge, &loose, split, v);
            let swap = |edge: &mut [i32; 2], new_vert: i32| {
                if edge[0] == v as i32 {
                    edge[0] = new_vert;
                } else if edge[1] == v as i32 {
                    edge[1] = new_vert;
                }
            };
            let mut done = false;
            for e in sel {
                swap(&mut edges[e], (orig_verts + offsets[i] + k) as i32);
                k += 1;
                if k == range_len {
                    done = true;
                    break;
                }
            }
            if !done {
                let new_vert = (orig_verts + offsets[i] + k) as i32;
                for e in unsel {
                    swap(&mut edges[e], new_vert);
                }
            }
        }
    }
    // `propagate_edge_attributes` / `propagate_vert_attributes`.
    propagate(
        mesh,
        Domain::Edge,
        orig_edges.len(),
        &edge_map,
        ".edge_verts",
    );
    edges.extend(new_edges);
    let mut vert_map = Vec::new();
    for (i, &v) in affected.iter().enumerate() {
        vert_map.extend(std::iter::repeat_n(v, offsets[i + 1] - offsets[i]));
    }
    propagate(mesh, Domain::Point, orig_verts, &vert_map, "");
    if !mesh.vertex_group_names.is_empty() && mesh.deform_verts().is_some() {
        let dv = mesh.deform_verts_mut();
        for &v in &vert_map {
            let w = dv[v].clone();
            dv.push(w);
        }
    }
    let attrs = mesh.attributes_mut();
    for (name, data) in [
        (".edge_verts", AttrData::Int32x2(Arc::new(edges))),
        (".corner_vert", AttrData::Int(Arc::new(corner_verts))),
        (".corner_edge", AttrData::Int(Arc::new(corner_edges))),
    ] {
        if let Some(a) = attrs.get_mut(name) {
            a.data = data;
        }
    }
}

/// `ED_mesh_split_faces`.
pub(crate) fn split_faces(mesh: &mut Mesh) {
    let mut split: Vec<bool> = mesh
        .sharp_edge_flags()
        .map(|v| v.to_vec())
        .unwrap_or_else(|| vec![false; mesh.edges_num()]);
    split.resize(mesh.edges_num(), false);
    if let Some(sharp_faces) = mesh.sharp_face_flags() {
        let ce = mesh.corner_edges();
        for f in 0..mesh.faces_num() {
            if sharp_faces.get(f).copied().unwrap_or(false) {
                for c in mesh.face(f) {
                    split[ce[c] as usize] = true;
                }
            }
        }
    }
    if !split.iter().any(|&s| s) {
        return;
    }
    split_edges(mesh, &split);
}

#[rna_func(srna = "Mesh", func = "split_faces")]
fn mesh_split_faces(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "split_faces")?;
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("split_faces(): the mesh was removed"))?;
    split_faces(mesh);
    Ok(Value::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_cube_splits_into_six_quads() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        split_faces(&mut m);
        assert_eq!((m.verts_num(), m.edges_num(), m.faces_num()), (24, 24, 6));
        assert!(m.validate_report().is_none(), "{:?}", m.validate_report());
    }

    #[test]
    fn smooth_mesh_is_unchanged() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        m.shade_smooth();
        let before = m.corner_verts().to_vec();
        split_faces(&mut m);
        assert_eq!(m.corner_verts(), &before[..]);
        assert_eq!(m.verts_num(), 8);
    }
}
