// SPDX-License-Identifier: GPL-3.0-or-later
//! Element mapping between a source and a destination mesh: for every destination vertex,
//! edge, face corner or face, the source elements (and interpolation weights) its data comes
//! from, under one of the `vert_mapping` / `edge_mapping` / `loop_mapping` / `poly_mapping`
//! modes.
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_remap.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `mesh_remap_bvhtree_query_nearest` (with its "local proximity"
//! reuse of the previous hit), `mesh_remap_bvhtree_query_raycast` (both directions),
//! `mesh_remap_interp_face_data_get`, `BKE_mesh_remap_calc_{verts,edges,loops,faces}_from_mesh`
//! (islands, the A* inner-cut correction, the approximate sphere rays with decreasing weight,
//! the seeded random face sampling of `POLYINTERP_PNORPROJ`) and
//! `mesh_island_to_astar_graph(_edge_process)`.

use frap_geom::bvh::{Nearest, RayHit};
use frap_math::rng::BliRng;
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use smallvec::SmallVec;

use super::astar::{self, Graph, Solution};
use super::bvhutils::MeshTree;
use super::islands::{self, IslandStore};
use super::math::{
    V3, area_poly_v2, area_tri_v2, axis_dominant_v3_to_m3, closest_on_tri_to_point, dot,
    face_center, interp, interp_slerp_safe, interp_weights_poly, invert_m3, len_sq_v3v3, len_v3v3,
    line_point_factor, mul_m3_v3, mul_v2_m3v3, negate,
};
use super::modes::*;
use super::space::{SpaceTransform, apply, apply_normal};

/// `MREMAP_RAYCAST_APPROXIMATE_NR`
const RAYCAST_APPROXIMATE_NR: i32 = 3;
/// `MREMAP_RAYCAST_APPROXIMATE_FAC`
const RAYCAST_APPROXIMATE_FAC: f32 = 5.0;
/// `MREMAP_RAYCAST_TRI_SAMPLES_MIN`
const RAYCAST_TRI_SAMPLES_MIN: i32 = 4;
/// `MREMAP_RAYCAST_TRI_SAMPLES_MAX`
const RAYCAST_TRI_SAMPLES_MAX: i32 = 20;
/// `ASTAR_STEPS_MAX`
const ASTAR_STEPS_MAX: f32 = 64.0;

/// `MeshPairRemapItem`: the sources of one destination element (none: left untouched).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RemapItem {
    /// `indices_src` with `weights_src`.
    pub sources: SmallVec<[(usize, f32); 4]>,
    /// `island`
    pub island: i32,
}

/// `MeshPairRemap`
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Remap {
    /// One item per destination element.
    pub items: Vec<RemapItem>,
}

impl Remap {
    fn new(n: usize) -> Remap {
        Remap {
            items: vec![RemapItem::default(); n],
        }
    }

    /// `mesh_remap_item_define`
    fn define(&mut self, i: usize, island: i32, indices: &[usize], weights: &[f32]) {
        let it = &mut self.items[i];
        it.island = island;
        it.sources = indices
            .iter()
            .copied()
            .zip(weights.iter().copied())
            .collect();
    }

    /// `BKE_mesh_remap_item_define_invalid`
    fn invalid(&mut self, i: usize) {
        self.define(i, 0, &[], &[]);
    }
}

/// `mesh_remap_bvhtree_query_nearest`: the hit distance, reusing the previous hit
/// (`nearest.index != -1`) to bound the search.
pub(crate) fn query_nearest(
    tree: &MeshTree<'_>,
    nearest: &mut Nearest,
    co: V3,
    max_dist_sq: f32,
) -> Option<f32> {
    if nearest.index.is_some() {
        nearest.dist_sq = len_sq_v3v3(co, nearest.co);
        if nearest.dist_sq > max_dist_sq {
            nearest.dist_sq = max_dist_sq;
            nearest.index = None;
        }
    } else {
        nearest.dist_sq = max_dist_sq;
    }
    tree.find_nearest(co, nearest);
    if nearest.index.is_some() && nearest.dist_sq <= max_dist_sq {
        Some(nearest.dist_sq.sqrt())
    } else {
        None
    }
}

/// `mesh_remap_bvhtree_query_raycast`: the nearer hit of the rays along `no` and `-no`.
fn query_raycast(
    tree: &MeshTree<'_>,
    rayhit: &mut RayHit,
    co: V3,
    no: V3,
    radius: f32,
    max_dist: f32,
) -> Option<f32> {
    rayhit.index = None;
    rayhit.dist = max_dist;
    tree.ray_cast(co, no, radius, rayhit);
    let mut tmp = *rayhit;
    tree.ray_cast(co, negate(no), radius, &mut tmp);
    if tmp.dist < rayhit.dist {
        *rayhit = tmp;
    }
    if rayhit.index.is_some() && rayhit.dist <= max_dist {
        Some(rayhit.dist)
    } else {
        None
    }
}

/// The source mesh data the mappings read.
pub(crate) struct SrcMesh<'a> {
    pub mesh: &'a Mesh,
    pub positions: &'a [V3],
    pub edges: &'a [[i32; 2]],
    pub face_offsets: &'a [i32],
    pub corner_verts: &'a [i32],
    pub corner_edges: &'a [i32],
    pub tris: std::sync::Arc<Vec<[u32; 3]>>,
    pub tri_faces: Vec<u32>,
}

impl<'a> SrcMesh<'a> {
    pub(crate) fn new(mesh: &'a Mesh) -> SrcMesh<'a> {
        SrcMesh {
            mesh,
            positions: mesh.positions(),
            edges: mesh.edges(),
            face_offsets: mesh.face_offsets(),
            corner_verts: mesh.corner_verts(),
            corner_edges: mesh.corner_edges(),
            tris: mesh.looptris(),
            tri_faces: mesh.looptri_faces(),
        }
    }

    fn faces_num(&self) -> usize {
        self.face_offsets.len().saturating_sub(1)
    }

    fn face(&self, f: usize) -> std::ops::Range<usize> {
        self.face_offsets[f] as usize..self.face_offsets[f + 1] as usize
    }

    fn face_verts(&self, f: usize) -> &'a [i32] {
        &self.corner_verts[self.face(f)]
    }

    fn tri_tree(&self, faces_mask: Option<&[bool]>) -> MeshTree<'_> {
        MeshTree::tris(
            self.positions,
            self.corner_verts,
            &self.tris,
            self.face_offsets,
            faces_mask,
        )
    }
}

/// `mesh_remap_interp_face_data_get`: the face's vertex (or corner) indices, their mean-value
/// weights at `point` (`do_weights`) and the index nearest to `point`.
fn interp_face_data_get(
    src: &SrcMesh<'_>,
    face: usize,
    point: V3,
    use_loops: bool,
    do_weights: bool,
) -> (SmallVec<[usize; 8]>, Vec<f32>, Option<usize>) {
    let range = src.face(face);
    let mut indices = SmallVec::new();
    let mut vcos: SmallVec<[V3; 8]> = SmallVec::new();
    let mut ref_dist_sq = f32::MAX;
    let mut closest = None;
    for c in range {
        let vert = src.corner_verts[c] as usize;
        let index = if use_loops { c } else { vert };
        indices.push(index);
        let co = src.positions[vert];
        vcos.push(co);
        let d = len_sq_v3v3(point, co);
        if d < ref_dist_sq {
            ref_dist_sq = d;
            closest = Some(index);
        }
    }
    let weights = if do_weights {
        interp_weights_poly(&vcos, point)
    } else {
        Vec::new()
    };
    (indices, weights, closest)
}

/// `BKE_mesh_remap_calc_verts_from_mesh`
pub(crate) fn calc_verts(
    mode: u32,
    st: Option<&SpaceTransform>,
    max_dist: f32,
    ray_radius: f32,
    positions_dst: &[V3],
    src: &SrcMesh<'_>,
    dst: &Mesh,
) -> Remap {
    let max_dist_sq = max_dist * max_dist;
    let mut map = Remap::new(positions_dst.len());
    if mode == MREMAP_MODE_TOPOLOGY {
        for i in 0..positions_dst.len() {
            map.define(i, 0, &[i], &[1.0]);
        }
        return map;
    }
    let mut nearest = Nearest::within(0.0);
    let mut rayhit = RayHit::within(0.0);
    if mode == MREMAP_MODE_VERT_NEAREST {
        let tree = MeshTree::verts(src.positions, None);
        for (i, &p) in positions_dst.iter().enumerate() {
            let co = apply(st, p);
            match query_nearest(&tree, &mut nearest, co, max_dist_sq) {
                Some(_) => map.define(i, 0, &[nearest.index.unwrap_or(0)], &[1.0]),
                None => map.invalid(i),
            }
        }
    } else if mode == MREMAP_MODE_VERT_EDGE_NEAREST || mode == MREMAP_MODE_VERT_EDGEINTERP_NEAREST {
        let tree = MeshTree::edges(src.positions, src.edges);
        for (i, &p) in positions_dst.iter().enumerate() {
            let co = apply(st, p);
            if query_nearest(&tree, &mut nearest, co, max_dist_sq).is_some() {
                let e = src.edges[nearest.index.unwrap_or(0)];
                let v1 = src.positions[e[0] as usize];
                let v2 = src.positions[e[1] as usize];
                if mode == MREMAP_MODE_VERT_EDGE_NEAREST {
                    let d1 = len_sq_v3v3(co, v1);
                    let d2 = len_sq_v3v3(co, v2);
                    let index = if d1 > d2 { e[1] } else { e[0] } as usize;
                    map.define(i, 0, &[index], &[1.0]);
                } else {
                    let w0 = line_point_factor(co, v2, v1).clamp(0.0, 1.0);
                    map.define(i, 0, &[e[0] as usize, e[1] as usize], &[w0, 1.0 - w0]);
                }
            } else {
                map.invalid(i);
            }
        }
    } else if mode == MREMAP_MODE_VERT_FACE_NEAREST
        || mode == MREMAP_MODE_VERT_POLYINTERP_NEAREST
        || mode == MREMAP_MODE_VERT_POLYINTERP_VNORPROJ
    {
        let vert_normals_dst = dst.vert_normals();
        let tree = src.tri_tree(None);
        if mode == MREMAP_MODE_VERT_POLYINTERP_VNORPROJ {
            for (i, &p) in positions_dst.iter().enumerate() {
                let co = apply(st, p);
                let no = apply_normal(st, vert_normals_dst[i]);
                if query_raycast(&tree, &mut rayhit, co, no, ray_radius, max_dist).is_some() {
                    let face = src.tri_faces[rayhit.index.unwrap_or(0)] as usize;
                    let (idx, w, _) = interp_face_data_get(src, face, rayhit.co, false, true);
                    map.define(i, 0, &idx, &w);
                } else {
                    map.invalid(i);
                }
            }
        } else {
            for (i, &p) in positions_dst.iter().enumerate() {
                let co = apply(st, p);
                if query_nearest(&tree, &mut nearest, co, max_dist_sq).is_some() {
                    let face = src.tri_faces[nearest.index.unwrap_or(0)] as usize;
                    if mode == MREMAP_MODE_VERT_FACE_NEAREST {
                        let (_, _, closest) =
                            interp_face_data_get(src, face, nearest.co, false, false);
                        map.define(i, 0, &[closest.unwrap_or(0)], &[1.0]);
                    } else {
                        let (idx, w, _) = interp_face_data_get(src, face, nearest.co, false, true);
                        map.define(i, 0, &idx, &w);
                    }
                } else {
                    map.invalid(i);
                }
            }
        }
    }
    map
}

/// `bke::mesh::build_vert_to_edge_map`: the edges of each vertex, in edge order.
fn vert_to_edge_map(edges: &[[i32; 2]], verts_num: usize) -> Vec<Vec<usize>> {
    let mut map = vec![Vec::new(); verts_num];
    for (i, e) in edges.iter().enumerate() {
        map[e[0] as usize].push(i);
        map[e[1] as usize].push(i);
    }
    map
}

/// `bke::mesh::edge_other_vert`
fn edge_other_vert(e: [i32; 2], v: i32) -> i32 {
    if e[0] == v { e[1] } else { e[0] }
}

/// `BKE_mesh_remap_calc_edges_from_mesh`
#[allow(clippy::too_many_arguments)]
pub(crate) fn calc_edges(
    mode: u32,
    st: Option<&SpaceTransform>,
    max_dist: f32,
    ray_radius: f32,
    positions_dst: &[V3],
    edges_dst: &[[i32; 2]],
    src: &SrcMesh<'_>,
    dst: &Mesh,
) -> Remap {
    let max_dist_sq = max_dist * max_dist;
    let mut map = Remap::new(edges_dst.len());
    if mode == MREMAP_MODE_TOPOLOGY {
        for i in 0..edges_dst.len() {
            map.define(i, 0, &[i], &[1.0]);
        }
        return map;
    }
    let mut nearest = Nearest::within(0.0);
    let mut rayhit = RayHit::within(0.0);
    let mid = |e: [i32; 2]| {
        interp(
            positions_dst[e[0] as usize],
            positions_dst[e[1] as usize],
            0.5,
        )
    };
    if mode == MREMAP_MODE_EDGE_VERT_NEAREST {
        // (hit_dist, index), hit_dist -1 = not computed yet.
        let mut v_dst_to_src: Vec<(f32, i32)> = vec![(-1.0, 0); positions_dst.len()];
        let vert_to_edge = vert_to_edge_map(src.edges, src.positions.len());
        let tree = MeshTree::verts(src.positions, None);
        for (i, &e_dst) in edges_dst.iter().enumerate() {
            let mut best_totdist = f32::MAX;
            let mut best_eidx_src: i32 = -1;
            for vidx_dst in [e_dst[0], e_dst[1]] {
                let v = vidx_dst as usize;
                if v_dst_to_src[v].0 == -1.0 {
                    let co = apply(st, positions_dst[v]);
                    match query_nearest(&tree, &mut nearest, co, max_dist_sq) {
                        Some(hit) => {
                            v_dst_to_src[v] = (hit, nearest.index.map_or(-1, |x| x as i32));
                        }
                        None => v_dst_to_src[v].0 = f32::MAX,
                    }
                }
            }
            for vidx_dst in [e_dst[0], e_dst[1]] {
                let (first_dist, vidx_src) = v_dst_to_src[vidx_dst as usize];
                if vidx_src < 0 {
                    continue;
                }
                for &eidx_src in &vert_to_edge[vidx_src as usize] {
                    let edge_src = src.edges[eidx_src];
                    let other_co_src = src.positions[edge_other_vert(edge_src, vidx_src) as usize];
                    let other_co_dst = positions_dst[edge_other_vert(e_dst, vidx_dst) as usize];
                    let totdist = first_dist + len_v3v3(other_co_src, other_co_dst);
                    if totdist < best_totdist {
                        best_totdist = totdist;
                        best_eidx_src = eidx_src as i32;
                    }
                }
            }
            if best_eidx_src >= 0 {
                // Blender then measures the hit distance (`isect_line_line_v3` of the two
                // edges), which `mesh_remap_item_define` does not store.
                map.define(i, 0, &[best_eidx_src as usize], &[1.0]);
            } else {
                map.invalid(i);
            }
        }
    } else if mode == MREMAP_MODE_EDGE_NEAREST {
        let tree = MeshTree::edges(src.positions, src.edges);
        for (i, &e) in edges_dst.iter().enumerate() {
            let co = apply(st, mid(e));
            match query_nearest(&tree, &mut nearest, co, max_dist_sq) {
                Some(_) => map.define(i, 0, &[nearest.index.unwrap_or(0)], &[1.0]),
                None => map.invalid(i),
            }
        }
    } else if mode == MREMAP_MODE_EDGE_POLY_NEAREST {
        let tree = src.tri_tree(None);
        for (i, &e) in edges_dst.iter().enumerate() {
            let co = apply(st, mid(e));
            if query_nearest(&tree, &mut nearest, co, max_dist_sq).is_some() {
                let face = src.tri_faces[nearest.index.unwrap_or(0)] as usize;
                let mut best_dist_sq = f32::MAX;
                let mut best: Option<usize> = None;
                for c in src.face(face) {
                    let ei = src.corner_edges[c] as usize;
                    let es = src.edges[ei];
                    let co_src = interp(
                        src.positions[es[0] as usize],
                        src.positions[es[1] as usize],
                        0.5,
                    );
                    let d = len_sq_v3v3(co, co_src);
                    if d < best_dist_sq {
                        best_dist_sq = d;
                        best = Some(ei);
                    }
                }
                if let Some(b) = best {
                    map.define(i, 0, &[b], &[1.0]);
                }
            } else {
                map.invalid(i);
            }
        }
    } else if mode == MREMAP_MODE_EDGE_EDGEINTERP_VNORPROJ {
        let (num_rays_min, num_rays_max) = (5, 100);
        let numedges_src = src.edges.len();
        let mut weights = vec![0.0f32; numedges_src];
        let tree = MeshTree::edges(src.positions, src.edges);
        let vert_normals_dst = dst.vert_normals();
        for (i, &e) in edges_dst.iter().enumerate() {
            let v1_co = apply(st, positions_dst[e[0] as usize]);
            let v2_co = apply(st, positions_dst[e[1] as usize]);
            let v1_no = apply_normal(st, vert_normals_dst[e[0] as usize]);
            let v2_no = apply_normal(st, vert_normals_dst[e[1] as usize]);
            weights.fill(0.0);
            let edge_dst_len = len_v3v3(v1_co, v2_co);
            let grid_size =
                ((edge_dst_len / ray_radius + 0.5) as i32).clamp(num_rays_min, num_rays_max);
            let grid_step = 1.0 / grid_size as f32;
            let mut totweights = 0.0f32;
            for j in 0..grid_size {
                let fac = grid_step * j as f32;
                let mut n = if ray_radius > 0.0 {
                    RAYCAST_APPROXIMATE_NR
                } else {
                    1
                };
                let mut w = 1.0f32;
                let co = interp(v1_co, v2_co, fac);
                let no = interp_slerp_safe(v1_no, v2_no, fac);
                while n > 0 {
                    n -= 1;
                    if query_raycast(&tree, &mut rayhit, co, no, ray_radius / w, max_dist).is_some()
                    {
                        weights[rayhit.index.unwrap_or(0)] += w;
                        totweights += w;
                        break;
                    }
                    w /= RAYCAST_APPROXIMATE_FAC;
                }
            }
            if totweights > grid_size as f32 / 2.0 {
                let mut idx = Vec::new();
                let mut ws = Vec::new();
                for (j, &wj) in weights.iter().enumerate() {
                    if wj == 0.0 {
                        continue;
                    }
                    ws.push(wj / totweights);
                    idx.push(j);
                }
                map.define(i, 0, &idx, &ws);
            } else {
                map.invalid(i);
            }
        }
    }
    map
}

/// `POLY_UNSET` / `POLY_CENTER_INIT` / `POLY_COMPLETE`
#[derive(Copy, Clone, PartialEq, Eq)]
enum FaceStatus {
    Unset,
    CenterInit,
    Complete,
}

/// The state of [`island_to_astar_graph`].
struct GraphBuild<'g> {
    islands: Option<&'g IslandStore>,
    island_index: i32,
    graph: Graph,
    done_edges: Vec<bool>,
    face_status: Vec<FaceStatus>,
    face_centers: Vec<V3>,
}

/// `mesh_island_to_astar_graph_edge_process`
fn edge_process(
    b: &mut GraphBuild<'_>,
    src: &SrcMesh<'_>,
    edge_to_face: &[Vec<i32>],
    edge: usize,
    is_innercut: bool,
) {
    let faces = &edge_to_face[edge];
    let mut fii = vec![-1i32; faces.len()];
    for i in 0..faces.len() {
        let pidx = faces[i] as usize;
        let pidx_isld = match &b.graph.face_island_index_map {
            Some(m) if b.islands.is_some() => m[pidx] as usize,
            _ => pidx,
        };
        let custom = if is_innercut { edge as i32 } else { -1 };
        if let Some(is) = b.islands
            && is.items_to_islands[src.face_offsets[pidx] as usize] != b.island_index
        {
            fii[i] = -1;
            continue;
        }
        if b.face_status[pidx_isld] == FaceStatus::Complete {
            fii[i] = pidx_isld as i32;
            continue;
        }
        if b.face_status[pidx_isld] == FaceStatus::Unset {
            let c = face_center(src.positions, src.face_verts(pidx));
            b.face_centers[pidx_isld] = c;
            b.graph.node_co[pidx_isld] = Some(c);
            b.face_status[pidx_isld] = FaceStatus::CenterInit;
        }
        for j in (0..i).rev() {
            let other = fii[j];
            if other == -1 || b.face_status[other as usize] == FaceStatus::Complete {
                continue;
            }
            let cost = len_v3v3(b.face_centers[other as usize], b.face_centers[pidx_isld]);
            b.graph.link_add(other as usize, pidx_isld, cost, custom);
        }
        fii[i] = pidx_isld as i32;
    }
    b.done_edges[edge] = true;
}

/// `mesh_island_to_astar_graph`
fn island_to_astar_graph(
    islands: Option<&IslandStore>,
    island_index: usize,
    src: &SrcMesh<'_>,
    edge_to_face: &[Vec<i32>],
) -> Graph {
    let faces_num = src.faces_num();
    let island_faces = islands.map(|s| &s.islands[island_index]);
    let node_num = island_faces.map_or(faces_num, Vec::len);
    let mut b = GraphBuild {
        islands,
        island_index: island_index as i32,
        graph: Graph::new(node_num),
        done_edges: vec![false; src.edges.len()],
        face_status: vec![FaceStatus::Unset; node_num],
        face_centers: vec![[0.0; 3]; node_num],
    };
    if let (Some(is), Some(faces)) = (islands, island_faces) {
        let mut map = vec![0i32; faces_num];
        for i in (0..faces.len()).rev() {
            map[faces[i] as usize] = i as i32;
        }
        b.graph.face_island_index_map = Some(map);
        let cuts = &is.innercuts[island_index];
        for i in (0..cuts.len()).rev() {
            edge_process(&mut b, src, edge_to_face, cuts[i] as usize, true);
        }
    }
    for pidx_isld in (0..node_num).rev() {
        let pidx = island_faces.map_or(pidx_isld, |f| f[pidx_isld] as usize);
        if b.face_status[pidx_isld] == FaceStatus::Complete {
            continue;
        }
        for c in src.face(pidx) {
            let edge = src.corner_edges[c] as usize;
            if b.done_edges[edge] {
                continue;
            }
            edge_process(&mut b, src, edge_to_face, edge, false);
        }
        b.face_status[pidx_isld] = FaceStatus::Complete;
    }
    b.graph
}

/// `IslandResult`
#[derive(Copy, Clone, Debug, Default)]
struct IslandResult {
    factor: f32,
    index_src: i32,
    hit_dist: f32,
    hit_point: V3,
}

/// The destination mesh data the face-corner and face mappings read.
pub(crate) struct DstFaces<'a> {
    pub positions: &'a [V3],
    pub corner_verts: &'a [i32],
    pub face_offsets: &'a [i32],
}

/// `bke::mesh::build_vert_to_corner_map` / the vertex → face map: per vertex, its corners (or
/// their faces), in corner order.
fn vert_to_corner_map(corner_verts: &[i32], verts_num: usize) -> Vec<Vec<usize>> {
    let mut map = vec![Vec::new(); verts_num];
    for (c, &v) in corner_verts.iter().enumerate() {
        map[v as usize].push(c);
    }
    map
}

/// `uv_seam` of the source, when it is a boolean edge attribute.
fn uv_seams(mesh: &Mesh) -> Option<&[bool]> {
    match mesh.attributes().get("uv_seam") {
        Some(Attribute {
            domain: Domain::Edge,
            data: AttrData::Bool(v),
            ..
        }) => Some(v.as_slice()),
        _ => None,
    }
}

/// `BKE_mesh_remap_calc_loops_from_mesh`; `gen_islands` is the UV island generator
/// (`BKE_mesh_calc_islands_loop_face_edgeseam`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn calc_loops(
    mode: u32,
    st: Option<&SpaceTransform>,
    max_dist: f32,
    ray_radius: f32,
    mesh_dst: &Mesh,
    dst: &DstFaces<'_>,
    src: &SrcMesh<'_>,
    gen_islands: bool,
    islands_precision: f32,
) -> Remap {
    let max_dist_sq = max_dist * max_dist;
    let corners_num = dst.corner_verts.len();
    let mut map = Remap::new(corners_num);
    if mode == MREMAP_MODE_TOPOLOGY {
        for i in 0..corners_num {
            map.define(i, 0, &[i], &[1.0]);
        }
        return map;
    }
    let use_from_vert = mode & MREMAP_USE_VERT != 0;
    let isld_steps_src = if islands_precision != 0.0 {
        ((ASTAR_STEPS_MAX * islands_precision + 0.499) as i32).max(1)
    } else {
        0
    };
    let need_lnors_src = mode & MREMAP_USE_LOOP != 0 && mode & MREMAP_USE_NORMAL != 0;
    let need_lnors_dst = need_lnors_src || mode & MREMAP_USE_NORPROJ != 0;
    let need_pnors_src =
        need_lnors_src || (mode & MREMAP_USE_POLY != 0 && mode & MREMAP_USE_NORMAL != 0);
    let need_pnors_dst = need_lnors_dst || need_pnors_src;
    let face_normals_dst = need_pnors_dst.then(|| mesh_dst.face_normals());
    let loop_normals_dst = need_lnors_dst.then(|| mesh_dst.corner_normals());
    let face_normals_src = need_pnors_src.then(|| src.mesh.face_normals());
    let loop_normals_src = need_lnors_src.then(|| src.mesh.corner_normals());

    let vert_to_corner_src = if use_from_vert {
        vert_to_corner_map(src.corner_verts, src.positions.len())
    } else {
        Vec::new()
    };
    let mut loop_to_face_src = Vec::new();
    let mut face_cents_src = Vec::new();
    let mut vert_to_face_src: Vec<Vec<usize>> = Vec::new();
    if use_from_vert {
        loop_to_face_src = vec![0usize; src.corner_verts.len()];
        for f in 0..src.faces_num() {
            for c in src.face(f) {
                loop_to_face_src[c] = f;
            }
            face_cents_src.push(face_center(src.positions, src.face_verts(f)));
        }
        if mode & MREMAP_USE_POLY != 0 {
            vert_to_face_src = vert_to_corner_src
                .iter()
                .map(|cs| cs.iter().map(|&c| loop_to_face_src[c]).collect())
                .collect();
        }
    }
    let edge_to_face_src =
        islands::edge_to_face_map(src.face_offsets, src.corner_edges, src.edges.len());

    let island_store = if gen_islands {
        islands::calc_islands_loop_face_edgeseam(
            src.edges.len(),
            uv_seams(src.mesh),
            src.face_offsets,
            src.corner_edges,
        )
    } else {
        None
    };
    let num_trees = island_store.as_ref().map_or(1, |s| s.islands.len());
    let graphs: Vec<Graph> = if isld_steps_src != 0 {
        (0..num_trees)
            .map(|t| island_to_astar_graph(island_store.as_ref(), t, src, &edge_to_face_src))
            .collect()
    } else {
        Vec::new()
    };

    // The BVH trees, from vertices or from triangles, one per island.
    let mut trees: Vec<MeshTree<'_>> = Vec::with_capacity(num_trees);
    if use_from_vert {
        if let Some(is) = &island_store {
            for isld in &is.islands {
                let mut active = vec![false; src.positions.len()];
                for &f in isld {
                    for &v in src.face_verts(f as usize) {
                        active[v as usize] = true;
                    }
                }
                trees.push(MeshTree::verts(src.positions, Some(&active)));
            }
        } else {
            trees.push(MeshTree::verts(src.positions, None));
        }
    } else if let Some(is) = &island_store {
        for t in 0..num_trees {
            let active: Vec<bool> = (0..src.faces_num())
                .map(|f| is.items_to_islands[src.face_offsets[f] as usize] == t as i32)
                .collect();
            trees.push(src.tri_tree(Some(&active)));
        }
    } else {
        trees.push(src.tri_tree(None));
    }

    let mut nearest = Nearest::within(0.0);
    let mut rayhit = RayHit::within(0.0);
    let dst_faces_num = dst.face_offsets.len().saturating_sub(1);
    let mut islands_res: Vec<Vec<IslandResult>> = vec![Vec::new(); num_trees];
    for pidx_dst in 0..dst_faces_num {
        let fstart = dst.face_offsets[pidx_dst] as usize;
        let fend = dst.face_offsets[pidx_dst + 1] as usize;
        let fsize = fend - fstart;
        let face_verts_dst = &dst.corner_verts[fstart..fend];
        let mut pcent_dst: Option<V3> = None;
        let pnor_dst = if mode == MREMAP_MODE_LOOP_NEAREST_POLYNOR {
            apply_normal(
                st,
                face_normals_dst.as_ref().map_or([0.0; 3], |n| n[pidx_dst]),
            )
        } else {
            [0.0; 3]
        };
        for (tindex, tdata) in trees.iter().enumerate() {
            let res = &mut islands_res[tindex];
            res.clear();
            res.resize(fsize, IslandResult::default());
            for plidx_dst in 0..fsize {
                let vert_dst = dst.corner_verts[fstart + plidx_dst] as usize;
                let r = &mut res[plidx_dst];
                if use_from_vert {
                    let co = apply(st, dst.positions[vert_dst]);
                    nearest.index = None;
                    if let Some(mut hit_dist) = query_nearest(tdata, &mut nearest, co, max_dist_sq)
                    {
                        let nidx = nearest.index.unwrap_or(0);
                        let loopnor = mode == MREMAP_MODE_LOOP_NEAREST_LOOPNOR;
                        let (nor_dst, refs): (V3, &[usize]) = if loopnor {
                            let n = loop_normals_dst
                                .as_ref()
                                .map_or([0.0; 3], |n| n[fstart + plidx_dst]);
                            (apply_normal(st, n), &vert_to_corner_src[nidx])
                        } else {
                            (pnor_dst, &vert_to_face_src[nidx])
                        };
                        let mut best_nor_dot = -2.0f32;
                        let mut best_sqdist_fallback = f32::MAX;
                        let mut best_index_src: i32 = -1;
                        for &index_src in refs {
                            let nsrc = if loopnor {
                                loop_normals_src.as_ref().map_or([0.0; 3], |n| n[index_src])
                            } else {
                                face_normals_src.as_ref().map_or([0.0; 3], |n| n[index_src])
                            };
                            let d = dot(nsrc, nor_dst);
                            let pidx_src = if loopnor {
                                loop_to_face_src[index_src]
                            } else {
                                index_src
                            };
                            let lidx_src = if loopnor {
                                index_src
                            } else {
                                src.face_offsets[pidx_src] as usize
                            };
                            if let Some(is) = &island_store
                                && is.items_to_islands[lidx_src] != tindex as i32
                            {
                                continue;
                            }
                            if d > best_nor_dot - 1e-6 {
                                let pc = *pcent_dst.get_or_insert_with(|| {
                                    face_center(dst.positions, face_verts_dst)
                                });
                                let sqdist = len_sq_v3v3(pc, face_cents_src[pidx_src]);
                                if d > best_nor_dot + 1e-6 || sqdist < best_sqdist_fallback {
                                    best_nor_dot = d;
                                    best_sqdist_fallback = sqdist;
                                    best_index_src = index_src as i32;
                                }
                            }
                        }
                        if best_index_src == -1 {
                            best_nor_dot = -1.0;
                            hit_dist = f32::MAX;
                        } else if mode == MREMAP_MODE_LOOP_NEAREST_POLYNOR {
                            let f = best_index_src as usize;
                            for c in src.face(f) {
                                if src.corner_verts[c] as usize == nidx {
                                    best_index_src = c as i32;
                                    break;
                                }
                            }
                        }
                        best_nor_dot = (best_nor_dot + 1.0) * 0.5;
                        r.factor = if hit_dist != 0.0 {
                            best_nor_dot / hit_dist
                        } else {
                            1e18
                        };
                        r.hit_dist = hit_dist;
                        r.index_src = best_index_src;
                    } else {
                        r.factor = 0.0;
                        r.hit_dist = f32::MAX;
                        r.index_src = -1;
                    }
                } else if mode & MREMAP_USE_NORPROJ != 0 {
                    let mut n = if ray_radius > 0.0 {
                        RAYCAST_APPROXIMATE_NR
                    } else {
                        1
                    };
                    let mut w = 1.0f32;
                    let co = apply(st, dst.positions[vert_dst]);
                    let no = apply_normal(
                        st,
                        loop_normals_dst
                            .as_ref()
                            .map_or([0.0; 3], |n| n[fstart + plidx_dst]),
                    );
                    let mut hit = false;
                    while n > 0 {
                        n -= 1;
                        if let Some(hit_dist) =
                            query_raycast(tdata, &mut rayhit, co, no, ray_radius / w, max_dist)
                        {
                            r.factor = (if hit_dist != 0.0 {
                                1.0 / hit_dist
                            } else {
                                1e18
                            }) * w;
                            r.hit_dist = hit_dist;
                            r.index_src = src.tri_faces[rayhit.index.unwrap_or(0)] as i32;
                            r.hit_point = rayhit.co;
                            hit = true;
                            break;
                        }
                        w /= RAYCAST_APPROXIMATE_FAC;
                    }
                    if !hit {
                        let co = apply(st, dst.positions[vert_dst]);
                        nearest.index = None;
                        r.factor = 0.0;
                        if let Some(hit_dist) = query_nearest(tdata, &mut nearest, co, max_dist_sq)
                        {
                            r.hit_dist = hit_dist;
                            r.index_src = src.tri_faces[nearest.index.unwrap_or(0)] as i32;
                            r.hit_point = nearest.co;
                        } else {
                            r.hit_dist = f32::MAX;
                            r.index_src = -1;
                        }
                    }
                } else {
                    let co = apply(st, dst.positions[vert_dst]);
                    nearest.index = None;
                    if let Some(hit_dist) = query_nearest(tdata, &mut nearest, co, max_dist_sq) {
                        r.factor = if hit_dist != 0.0 {
                            1.0 / hit_dist
                        } else {
                            1e18
                        };
                        r.hit_dist = hit_dist;
                        r.index_src = src.tri_faces[nearest.index.unwrap_or(0)] as i32;
                        r.hit_point = nearest.co;
                    } else {
                        r.factor = 0.0;
                        r.hit_dist = f32::MAX;
                        r.index_src = -1;
                    }
                }
            }
        }

        // The best source island for this face.
        let mut best_island_fac = 0.0f32;
        let mut best_island_index: i32 = -1;
        for (tindex, res) in islands_res.iter().enumerate() {
            let mut island_fac = 0.0f32;
            for r in res.iter().take(fsize) {
                island_fac += r.factor;
            }
            island_fac /= fsize as f32;
            if island_fac > best_island_fac {
                best_island_fac = island_fac;
                best_island_index = tindex as i32;
            }
        }
        let mut as_solution: Option<Solution> = None;
        let mut as_graph: Option<&Graph> = None;
        let mut best_island: Option<&Vec<i32>> = None;
        if best_island_index != -1 && isld_steps_src != 0 {
            best_island = island_store
                .as_ref()
                .map(|s| &s.islands[best_island_index as usize]);
            let g = &graphs[best_island_index as usize];
            as_graph = Some(g);
            as_solution = Some(Solution::new(g));
        }
        let face_island_index_map = as_graph.and_then(|g| g.face_island_index_map.as_ref());
        let mut pidx_src_prev: i32 = -1;
        #[allow(clippy::needless_range_loop)] // `plidx_dst` indexes the best island's results
        for plidx_dst in 0..fsize {
            let lidx_dst = fstart + plidx_dst;
            if best_island_index == -1 {
                map.invalid(lidx_dst);
                continue;
            }
            if let Some(s) = as_solution.as_mut() {
                s.crossed_innercut = false;
            }
            let isld_res = &mut islands_res[best_island_index as usize][plidx_dst];
            // The A* correction: the face just before the first inner cut on the shortest face
            // path from the previous corner's source face, if any.
            let astar_last_valid =
                |pidx_src: i32, pidx_src_prev: i32, sol: &mut Option<Solution>| -> Option<usize> {
                    let (Some(g), Some(sol)) = (as_graph, sol.as_mut()) else {
                        return None;
                    };
                    if pidx_src_prev == -1 || pidx_src_prev == pidx_src {
                        return None;
                    }
                    let (mut cur, prev) = match face_island_index_map {
                        Some(m) => (
                            m[pidx_src as usize] as usize,
                            m[pidx_src_prev as usize] as usize,
                        ),
                        None => (pidx_src as usize, pidx_src_prev as usize),
                    };
                    astar::solve(g, prev, cur, sol, isld_steps_src);
                    if !(sol.crossed_innercut && sol.steps > 0) {
                        return None;
                    }
                    let mut last_valid: Option<usize> = None;
                    let mut i = sol.steps - 1;
                    while i > 0 {
                        i -= 1;
                        let link = sol.prev_links[cur];
                        let eidx = link.map_or(-1, |l| g.links[l].edge);
                        cur = sol.prev_nodes[cur].max(0) as usize;
                        if eidx != -1 {
                            last_valid = Some(cur);
                        }
                    }
                    last_valid
                };
            if use_from_vert {
                let mut lidx_src = isld_res.index_src;
                if lidx_src >= 0 {
                    let mut pidx_src = loop_to_face_src[lidx_src as usize] as i32;
                    if let Some(lv) = astar_last_valid(pidx_src, pidx_src_prev, &mut as_solution) {
                        let mut best_dist_sq = f32::MAX;
                        let co = apply(st, dst.positions[dst.corner_verts[lidx_dst] as usize]);
                        pidx_src = best_island.map_or(lv as i32, |b| b[lv]);
                        for c in src.face(pidx_src as usize) {
                            let vs = src.corner_verts[c] as usize;
                            let d = len_sq_v3v3(src.positions[vs], co);
                            if d < best_dist_sq {
                                best_dist_sq = d;
                                lidx_src = c as i32;
                            }
                        }
                    }
                    map.define(lidx_dst, best_island_index, &[lidx_src as usize], &[1.0]);
                    pidx_src_prev = pidx_src;
                } else {
                    map.define(lidx_dst, best_island_index, &[], &[]);
                }
            } else {
                let mut pidx_src = isld_res.index_src;
                if pidx_src >= 0 {
                    let face_src = pidx_src as usize;
                    if let Some(lv) = astar_last_valid(pidx_src, pidx_src_prev, &mut as_solution) {
                        let mut best_dist_sq = f32::MAX;
                        let co = apply(st, dst.positions[dst.corner_verts[lidx_dst] as usize]);
                        pidx_src = best_island.map_or(lv as i32, |b| b[lv]);
                        let p = pidx_src as usize;
                        let start = src.face_offsets[p] as usize - 2 * p;
                        let count = (src.face_offsets[p + 1] - src.face_offsets[p]) as usize - 2;
                        for j in (0..count).rev() {
                            let tri = src.tris[start + j];
                            let h = closest_on_tri_to_point(
                                co,
                                src.positions[src.corner_verts[tri[0] as usize] as usize],
                                src.positions[src.corner_verts[tri[1] as usize] as usize],
                                src.positions[src.corner_verts[tri[2] as usize] as usize],
                            );
                            let d = len_sq_v3v3(co, h);
                            if d < best_dist_sq {
                                isld_res.hit_point = h;
                                best_dist_sq = d;
                            }
                        }
                    }
                    let hit_co = isld_res.hit_point;
                    if mode == MREMAP_MODE_LOOP_POLY_NEAREST {
                        let (_, _, closest) =
                            interp_face_data_get(src, face_src, hit_co, true, false);
                        map.define(lidx_dst, best_island_index, &[closest.unwrap_or(0)], &[1.0]);
                    } else {
                        let (idx, w, _) = interp_face_data_get(src, face_src, hit_co, true, true);
                        map.define(lidx_dst, best_island_index, &idx, &w);
                    }
                    pidx_src_prev = pidx_src;
                } else {
                    map.define(lidx_dst, best_island_index, &[], &[]);
                }
            }
        }
    }
    map
}

/// `BKE_mesh_remap_calc_faces_from_mesh`
pub(crate) fn calc_faces(
    mode: u32,
    st: Option<&SpaceTransform>,
    max_dist: f32,
    ray_radius: f32,
    mesh_dst: &Mesh,
    dst: &DstFaces<'_>,
    src: &SrcMesh<'_>,
) -> Remap {
    let max_dist_sq = max_dist * max_dist;
    let faces_num = dst.face_offsets.len().saturating_sub(1);
    let face_normals_dst =
        (mode & (MREMAP_USE_NORMAL | MREMAP_USE_NORPROJ) != 0).then(|| mesh_dst.face_normals());
    let mut map = Remap::new(faces_num);
    if mode == MREMAP_MODE_TOPOLOGY {
        for i in 0..faces_num {
            map.define(i, 0, &[i], &[1.0]);
        }
        return map;
    }
    let face_verts = |i: usize| {
        &dst.corner_verts[dst.face_offsets[i] as usize..dst.face_offsets[i + 1] as usize]
    };
    let mut nearest = Nearest::within(0.0);
    let mut rayhit = RayHit::within(0.0);
    let tree = src.tri_tree(None);
    let fnor = |i: usize| face_normals_dst.as_ref().map_or([0.0; 3], |n| n[i]);
    if mode == MREMAP_MODE_POLY_NEAREST {
        for i in 0..faces_num {
            let co = apply(st, face_center(dst.positions, face_verts(i)));
            match query_nearest(&tree, &mut nearest, co, max_dist_sq) {
                Some(_) => {
                    let face = src.tri_faces[nearest.index.unwrap_or(0)] as usize;
                    map.define(i, 0, &[face], &[1.0]);
                }
                None => map.invalid(i),
            }
        }
    } else if mode == MREMAP_MODE_POLY_NOR {
        for i in 0..faces_num {
            let co = apply(st, face_center(dst.positions, face_verts(i)));
            let no = apply_normal(st, fnor(i));
            if query_raycast(&tree, &mut rayhit, co, no, ray_radius, max_dist).is_some() {
                let face = src.tri_faces[rayhit.index.unwrap_or(0)] as usize;
                map.define(i, 0, &[face], &[1.0]);
            } else {
                map.invalid(i);
            }
        }
    } else if mode == MREMAP_MODE_POLY_POLYINTERP_PNORPROJ {
        let mut rng = BliRng::new(0);
        let numfaces_src = src.faces_num();
        let mut weights = vec![0.0f32; numfaces_src];
        for i in 0..faces_num {
            let fv = face_verts(i);
            let size = fv.len();
            let tris_num = size as i32 - 2;
            let mut pcent_dst = apply(st, face_center(dst.positions, fv));
            let tmp_no = apply_normal(st, fnor(i));
            weights.fill(0.0);
            let to_2d = axis_dominant_v3_to_m3(tmp_no);
            let from_2d = invert_m3(&to_2d);
            pcent_dst = mul_m3_v3(&to_2d, pcent_dst);
            let z = pcent_dst[2];
            let mut vcos_2d: Vec<[f32; 2]> = Vec::with_capacity(size);
            let mut min = [f32::MAX; 2];
            let mut max = [-f32::MAX; 2];
            for &v in fv {
                let co = apply(st, dst.positions[v as usize]);
                let p = mul_v2_m3v3(&to_2d, co);
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
                vcos_2d.push(p);
            }
            let size_2d = [max[0] - min[0], max[1] - min[1]];
            let mut tot_rays = if ray_radius != 0.0 {
                ((size_2d[0].max(size_2d[1]) / ray_radius + 0.5) as i32)
                    .clamp(RAYCAST_TRI_SAMPLES_MIN, RAYCAST_TRI_SAMPLES_MAX)
            } else {
                RAYCAST_TRI_SAMPLES_MIN
            };
            tot_rays *= tot_rays;
            let face_area_2d_inv = 1.0 / area_poly_v2(&vcos_2d).max(1e-9);
            let tris: Vec<[u32; 3]> = if size == 4 {
                vec![[0, 1, 2], [0, 2, 3]]
            } else {
                frap_geom::polyfill::polyfill_calc(&vcos_2d, -1)
            };
            let mut done_area = 0.0f32;
            let mut done_rays = 0i32;
            let mut totweights = 0.0f32;
            for j in 0..tris_num.max(0) as usize {
                let Some(t) = tris.get(j) else { break };
                let v1 = vcos_2d[t[0] as usize];
                let v2 = vcos_2d[t[1] as usize];
                let v3 = vcos_2d[t[2] as usize];
                done_area += area_tri_v2(v1, v2, v3);
                let rays_num = ((tot_rays as f32 * done_area * face_area_2d_inv + 0.5) as i32
                    - done_rays)
                    .max(0);
                done_rays += rays_num;
                for _ in 0..rays_num {
                    let mut n = if ray_radius > 0.0 {
                        RAYCAST_APPROXIMATE_NR
                    } else {
                        1
                    };
                    let mut w = 1.0f32;
                    let s = rng.get_triangle_sample(v1, v2, v3);
                    let co = mul_m3_v3(&from_2d, [s[0], s[1], z]);
                    while n > 0 {
                        n -= 1;
                        if query_raycast(&tree, &mut rayhit, co, tmp_no, ray_radius / w, max_dist)
                            .is_some()
                        {
                            let face = src.tri_faces[rayhit.index.unwrap_or(0)] as usize;
                            weights[face] += w;
                            totweights += w;
                            break;
                        }
                        w /= RAYCAST_APPROXIMATE_FAC;
                    }
                }
            }
            if totweights > 0.0 {
                let mut idx = Vec::new();
                let mut ws = Vec::new();
                for (j, &wj) in weights.iter().enumerate() {
                    if wj == 0.0 {
                        continue;
                    }
                    ws.push(wj / totweights);
                    idx.push(j);
                }
                map.define(i, 0, &idx, &ws);
            } else {
                map.invalid(i);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×2 grid of quads in z = 0 (9 vertices), as `Mesh::from_pydata` builds it.
    fn grid(z: f32, offset: f32) -> Mesh {
        let mut verts = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                verts.push([x as f32 + offset, y as f32, z]);
            }
        }
        let faces: Vec<Vec<i32>> = vec![
            vec![0, 1, 4, 3],
            vec![1, 2, 5, 4],
            vec![3, 4, 7, 6],
            vec![4, 5, 8, 7],
        ];
        let mut m = Mesh::new("g");
        m.from_pydata(&verts, &[], &faces, false);
        m
    }

    #[test]
    fn nearest_vertex_and_face_maps() {
        let src_mesh = grid(0.0, 0.0);
        let dst_mesh = grid(0.1, 0.05);
        let src = SrcMesh::new(&src_mesh);
        let m = calc_verts(
            MREMAP_MODE_VERT_NEAREST,
            None,
            f32::MAX,
            0.0,
            dst_mesh.positions(),
            &src,
            &dst_mesh,
        );
        for (i, it) in m.items.iter().enumerate() {
            assert_eq!(it.sources.as_slice(), &[(i, 1.0)]);
        }
        let dst = DstFaces {
            positions: dst_mesh.positions(),
            corner_verts: dst_mesh.corner_verts(),
            face_offsets: dst_mesh.face_offsets(),
        };
        let f = calc_faces(
            MREMAP_MODE_POLY_NEAREST,
            None,
            f32::MAX,
            0.0,
            &dst_mesh,
            &dst,
            &src,
        );
        for (i, it) in f.items.iter().enumerate() {
            assert_eq!(it.sources.as_slice(), &[(i, 1.0)]);
        }
        // Too far: nothing maps.
        let m = calc_verts(
            MREMAP_MODE_VERT_NEAREST,
            None,
            0.01,
            0.0,
            dst_mesh.positions(),
            &src,
            &dst_mesh,
        );
        assert!(m.items.iter().all(|it| it.sources.is_empty()));
    }

    #[test]
    fn interpolated_face_corners_sum_to_one() {
        let src_mesh = grid(0.0, 0.0);
        let dst_mesh = grid(0.2, 0.3);
        let src = SrcMesh::new(&src_mesh);
        let dst = DstFaces {
            positions: dst_mesh.positions(),
            corner_verts: dst_mesh.corner_verts(),
            face_offsets: dst_mesh.face_offsets(),
        };
        for mode in [
            MREMAP_MODE_LOOP_POLYINTERP_NEAREST,
            MREMAP_MODE_LOOP_POLYINTERP_LNORPROJ,
            MREMAP_MODE_LOOP_NEAREST_POLYNOR,
            MREMAP_MODE_LOOP_NEAREST_LOOPNOR,
            MREMAP_MODE_LOOP_POLY_NEAREST,
        ] {
            let m = calc_loops(mode, None, f32::MAX, 0.0, &dst_mesh, &dst, &src, true, 0.1);
            for it in &m.items {
                let s: f32 = it.sources.iter().map(|x| x.1).sum();
                assert!((s - 1.0).abs() < 1e-5, "mode {mode:#x}: {it:?}");
            }
        }
        let f = calc_faces(
            MREMAP_MODE_POLY_POLYINTERP_PNORPROJ,
            None,
            f32::MAX,
            0.0,
            &dst_mesh,
            &dst,
            &src,
        );
        for it in &f.items {
            let s: f32 = it.sources.iter().map(|x| x.1).sum();
            assert!((s - 1.0).abs() < 1e-5, "{it:?}");
        }
    }
}
