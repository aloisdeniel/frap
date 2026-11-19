// SPDX-License-Identifier: GPL-3.0-or-later
//! BUILD (`BuildModifier`): shows a growing part of the mesh over time. At scene time `t`, the
//! fraction `(t - frame_start) / frame_duration` (clamped to 0–1, reversed by `use_reverse`)
//! of the faces is kept (with their vertices and the edges between those), or of the edges
//! when that keeps no face, or of the vertices when it keeps no edge; elements are taken in
//! index order, or shuffled by `seed` with `use_random_order` (`BLI_array_randomize`).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_build.cc (Blender 5.2.4).

use frap_scene::generated::modifier_settings::BuildSettings;
use indexmap::IndexMap;

use super::smooth::common::{Sources, Src, Topology, rebuild};
use crate::prelude::*;

#[modifier(BUILD, kind = Nonconstructive, flags = ACCEPTS_MESH | ACCEPTS_CVS, krate = crate)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Build(s) = &md.settings else {
        return Ok(mesh);
    };
    Ok(build(s, &mesh, cx.frame()))
}

// Ported from blender/source/blender/modifiers/intern/MOD_build.cc (modify_mesh).
/// The BUILD result of `mesh` at scene time `ctime`.
pub(crate) fn build(s: &BuildSettings, mesh: &Mesh, ctime: f32) -> Mesh {
    let vert_src_num = mesh.verts_num();
    let edges_src = mesh.edges();
    let faces_src_num = mesh.faces_num();
    let corner_verts_src = mesh.corner_verts();
    let corner_edges_src = mesh.corner_edges();
    let mut vert_map: Vec<usize> = (0..vert_src_num).collect();
    let mut edge_map: Vec<usize> = (0..edges_src.len()).collect();
    let mut face_map: Vec<usize> = (0..faces_src_num).collect();

    let mut frac = (ctime - s.frame_start) / s.frame_duration;
    frac = frac.clamp(0.0, 1.0);
    // CLAMP keeps a NaN (0/0) as is; `int(NaN)` is undefined in C, 0 here.
    if s.use_reverse {
        frac = 1.0 - frac;
    }
    let faces_dst_num = (faces_src_num as f32 * frac) as usize;
    let edges_dst_num = (edges_src.len() as f32 * frac) as usize;
    let seed = s.seed as u32;

    // old vertex → new vertex, in insertion order.
    let mut vert_hash: IndexMap<usize, usize> = IndexMap::new();
    // new edge → old edge, and old edge → new edge.
    let mut edge_hash: Vec<usize> = Vec::new();
    let mut edge_hash2: IndexMap<usize, usize> = IndexMap::new();
    let mut loops_dst_num = 0usize;

    if faces_dst_num > 0 {
        if s.use_random_order {
            frap_math::rng::array_randomize(&mut face_map, seed);
        }
        for &f in &face_map[..faces_dst_num] {
            for c in mesh.face(f) {
                let v = corner_verts_src[c] as usize;
                let n = vert_hash.len();
                vert_hash.entry(v).or_insert(n);
            }
            loops_dst_num += mesh.face(f).len();
        }
        for (i, e) in edges_src.iter().enumerate() {
            if vert_hash.contains_key(&(e[0] as usize)) && vert_hash.contains_key(&(e[1] as usize))
            {
                edge_hash2.insert(i, edge_hash.len());
                edge_hash.push(i);
            }
        }
    } else if edges_dst_num > 0 {
        if s.use_random_order {
            frap_math::rng::array_randomize(&mut edge_map, seed);
        }
        for &e in &edge_map[..edges_dst_num] {
            for v in edges_src[e] {
                let n = vert_hash.len();
                vert_hash.entry(v as usize).or_insert(n);
            }
        }
        for &e in &edge_map[..edges_dst_num] {
            edge_hash2.insert(e, edge_hash.len());
            edge_hash.push(e);
        }
    } else {
        let verts_num = (vert_src_num as f32 * frac) as usize;
        if s.use_random_order {
            frap_math::rng::array_randomize(&mut vert_map, seed);
        }
        for (i, &v) in vert_map[..verts_num].iter().enumerate() {
            vert_hash.insert(v, i);
        }
    }

    let mut vert = vec![Src::Default; vert_hash.len()];
    for (&old, &new) in &vert_hash {
        vert[new] = Src::Copy(old);
    }
    let mut topo = Topology {
        edges: Vec::with_capacity(edge_hash.len()),
        face_offsets: Vec::with_capacity(faces_dst_num + 1),
        corner_verts: Vec::with_capacity(loops_dst_num),
        corner_edges: Vec::with_capacity(loops_dst_num),
    };
    let mut src = Sources {
        vert,
        edge: Vec::with_capacity(edge_hash.len()),
        face: Vec::with_capacity(faces_dst_num),
        corner: Vec::with_capacity(loops_dst_num),
    };
    for &old in &edge_hash {
        let e = edges_src[old];
        topo.edges
            .push([vert_hash[&(e[0] as usize)] as i32, vert_hash[&(e[1] as usize)] as i32]);
        src.edge.push(Src::Copy(old));
    }
    topo.face_offsets.push(0);
    for &f in &face_map[..faces_dst_num] {
        src.face.push(Src::Copy(f));
        for c in mesh.face(f) {
            src.corner.push(Src::Copy(c));
            topo.corner_verts
                .push(vert_hash[&(corner_verts_src[c] as usize)] as i32);
            topo.corner_edges
                .push(edge_hash2[&(corner_edges_src[c] as usize)] as i32);
        }
        topo.face_offsets.push(topo.corner_verts.len() as i32);
    }
    rebuild(mesh, topo, &src)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Mesh {
        frap_mesh::primitives::grid::grid(&frap_mesh::primitives::grid::Grid {
            x_subdivisions: 3,
            y_subdivisions: 3,
            ..Default::default()
        })
    }

    #[test]
    fn fractions_by_faces_edges_and_verts() {
        let m = grid();
        let s = BuildSettings {
            frame_start: 1.0,
            frame_duration: 100.0,
            ..BuildSettings::default()
        };
        let nf = m.faces_num();
        // At 25 % a quarter of the faces is kept (truncated), with their vertices and edges.
        let out = build(&s, &m, 26.0);
        assert_eq!(out.faces_num(), nf / 4);
        assert!(out.verts_num() < m.verts_num());
        assert_eq!(build(&s, &m, 101.0).faces_num(), nf);
        assert_eq!(build(&s, &m, 0.0).verts_num(), 0);
        let rev = BuildSettings {
            use_reverse: true,
            ..s.clone()
        };
        assert_eq!(build(&rev, &m, 0.0).faces_num(), nf);
    }
}
