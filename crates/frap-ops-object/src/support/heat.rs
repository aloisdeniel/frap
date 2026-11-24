// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from blender/source/blender/editors/armature/meshlaplacian.cc (Blender 5.2.4).
//! Bone heat weighting for `object.parent_set(type='ARMATURE_AUTO')`: a port of
//! `heat_bone_weighting` and the Laplacian system of
//! `blender/source/blender/editors/armature/meshlaplacian.cc` (Blender 5.2.4,
//! GPL-2.0-or-later), after Baran and Popović, "Automatic Rigging and Animation of 3D
//! Characters" (SIGGRAPH 2007). The caller (`support::deform::add_verts_to_dgroups`, the port
//! of `armature_skinning.cc`) gathers the bones, their groups and the world-space vertices.
//!
//! Per mesh:
//! - the triangles of `corner_tris_calc` (from the original positions) go into a
//!   `BLI_bvhtree_new(tris, 0.0, 4, 6)` ([`frap_geom::bvh`]) bounded by their world-space
//!   boxes, and every vertex remembers the last triangle it belongs to (`vltree`);
//! - vertex normals are the normalised sums of the unit triangle normals;
//! - a bone is a heat source of a vertex when its distance (to the bone segment, divided by
//!   `0.5 · (cos + 1.001)` with `cos` between the vertex normal and the direction away from
//!   the bone) is within `1 + 1e-4` of the vertex's smallest one and the bone is visible:
//!   the segment from the vertex to its closest point on the bone (shortened by `1e-5` at
//!   both ends) crosses no triangle facing it (`dot(n, dir) < -1e-5`, watertight test). A
//!   vertex in no triangle sees every bone;
//! - `H = sources / max(mindist, 1e-4)²` and `p = 1 / sources` per vertex;
//! - the system is `(L + H) w = H p` per bone, `L` the cotangent Laplacian (Meyer et al.,
//!   mixed Voronoi areas, cotangents divided by the number of triangles of their edge), built
//!   once and solved in the least-squares sense (`MᵀM`, see `heat/solver.rs`);
//! - weights below 0.025 are dropped, 0.025–0.05 ramp to 0–0.05 (`heat_limit_weight`), and
//!   `xmirror` writes each weight to the mirrored vertex's flipped group too. A failed solve
//!   stops at that bone with `Bone Heat Weighting: failed to find solution for one or more
//!   bones` (a WARNING report). A fully occluded mesh part (no vertex sees a bone) leaves the
//!   system singular; Blender then fails or not depending on Eigen's exact pivots, frap
//!   always solves (deviations §2, `heat/solver.rs`).
//!
//! The B-Bone branch (several sources per bone, `WEIGHT_ADD`, the final clean-up) is ported
//! but only reached by consecutive bones sharing a group (locked groups): frap hands one
//! source per bone (B-Bone segments are M9, `deform.rs` warns). Weight Paint masks
//! (`use_paint_mask*`) do not apply: frap parents in Object Mode.

mod math;
mod solver;
#[cfg(test)]
mod tests;

use frap_bmesh::shape::deform as dv;
use frap_geom::bvh::{BvhTree, RayHit, isect_ray_tri_watertight_v3};
use frap_mesh::DeformVert;

use math::V3;
use solver::LeastSquares;

/// The report of a failed solve (`N_("Bone Heat Weighting: …")`).
pub const HEAT_FAILED: &str = "Bone Heat Weighting: failed to find solution for one or more bones";

/// `C_WEIGHT`
const C_WEIGHT: f32 = 1.0;
/// `WEIGHT_LIMIT_START`
const WEIGHT_LIMIT_START: f32 = 0.05;
/// `WEIGHT_LIMIT_END`
const WEIGHT_LIMIT_END: f32 = 0.025;
/// `DISTANCE_EPSILON`
const DISTANCE_EPSILON: f32 = 1e-4;

/// What [`heat_bone_weighting`] reads besides the weights it writes.
pub struct HeatInput<'a> {
    /// The vertices in world space (`verts`: the limit surface with a SUBSURF modifier).
    pub verts: &'a [V3],
    /// The triangles of `corner_tris_calc`, as vertex indices.
    pub tris: &'a [[usize; 3]],
    /// World-space bone roots, one per source (`numbones`).
    pub roots: &'a [V3],
    /// World-space bone tips.
    pub tips: &'a [V3],
    /// The group of each source (`dgrouplist`; `None`: a locked group, left alone).
    pub groups: &'a [Option<u32>],
    /// The flipped-name group of each source (`dgroupflip`, with `xmirror`).
    pub flips: &'a [Option<u32>],
    /// `selected` (every source outside Weight Paint).
    pub selected: &'a [bool],
    /// `vertsflipped`: the X-mirror vertex of each vertex (`mesh_get_x_mirror_vert`), when
    /// some source has a flipped group.
    pub mirror: Option<&'a [Option<usize>]>,
}

/// `heat_limit_weight`
pub fn heat_limit_weight(weight: f32) -> f32 {
    if weight < WEIGHT_LIMIT_END {
        return 0.0;
    }
    if weight < WEIGHT_LIMIT_START {
        let t = (weight - WEIGHT_LIMIT_END) / (WEIGHT_LIMIT_START - WEIGHT_LIMIT_END);
        return t * WEIGHT_LIMIT_START;
    }
    weight
}

/// `LaplacianSystem` with its `HeatWeighting` part.
struct HeatSystem<'a> {
    verts: &'a [V3],
    tris: &'a [[usize; 3]],
    roots: &'a [V3],
    tips: &'a [V3],
    vert_normals: Vec<V3>,
    /// `H`: the diagonal heat matrix.
    h: Vec<f32>,
    /// `p`
    p: Vec<f32>,
    mindist: Vec<f32>,
    bvh: BvhTree,
    /// `vltree`: a triangle each vertex belongs to (the last one).
    vltree: Vec<Option<usize>>,
    solver: LeastSquares,
}

impl<'a> HeatSystem<'a> {
    /// `laplacian_system_construct_begin` + `heat_ray_tree_create` + `heat_laplacian_create`
    /// + `laplacian_system_construct_end`.
    fn new(inp: &HeatInput<'a>) -> HeatSystem<'a> {
        let n = inp.verts.len();
        let mut sys = HeatSystem {
            verts: inp.verts,
            tris: inp.tris,
            roots: inp.roots,
            tips: inp.tips,
            vert_normals: Vec::new(),
            h: vec![0.0; n],
            p: vec![0.0; n],
            mindist: vec![0.0; n],
            bvh: BvhTree::new(inp.tris.len(), 0.0, 4, 6),
            vltree: vec![None; n],
            solver: LeastSquares::new(n),
        };
        sys.ray_tree_create();
        sys.calc_vnormals();
        // `--stmt-timeout`: one ray cast per vertex and heat source.
        let sources = u32::try_from(inp.roots.len()).unwrap_or(u32::MAX);
        for a in 0..n {
            frap_math::budget::check_n(sources.saturating_mul(2));
            sys.set_h(a);
        }
        sys.construct_end();
        sys
    }

    /// `heat_ray_tree_create`
    fn ray_tree_create(&mut self) {
        for (a, tri) in self.tris.iter().enumerate() {
            frap_math::budget::check();
            // `INIT_MINMAX` + `minmax_v3v3_v3`
            let mut lo = [f32::MAX; 3];
            let mut hi = [-f32::MAX; 3];
            for &v in tri {
                let p = self.verts[v];
                for k in 0..3 {
                    if p[k] < lo[k] {
                        lo[k] = p[k];
                    }
                    if p[k] > hi[k] {
                        hi[k] = p[k];
                    }
                }
            }
            self.bvh.insert(a, &[lo, hi]);
            for &v in tri {
                self.vltree[v] = Some(a);
            }
        }
        self.bvh.balance();
    }

    /// `heat_calc_vnormals`
    fn calc_vnormals(&mut self) {
        let mut nor = vec![[0.0f32; 3]; self.verts.len()];
        for &[v1, v2, v3] in self.tris {
            let f = math::normal_tri(self.verts[v1], self.verts[v2], self.verts[v3]);
            nor[v1] = math::add(nor[v1], f);
            nor[v2] = math::add(nor[v2], f);
            nor[v3] = math::add(nor[v3], f);
        }
        for n in &mut nor {
            *n = math::normalize(*n).0;
        }
        self.vert_normals = nor;
    }

    /// `heat_ray_source_visible`
    fn source_visible(&self, vertex: usize, source: usize) -> bool {
        if self.vltree[vertex].is_none() {
            return true;
        }
        let mut start = self.verts[vertex];
        let end = math::closest_to_line_segment(start, self.roots[source], self.tips[source]);
        let mut vec = math::sub(end, start);
        start = math::madd(start, vec, 1e-5);
        vec = math::scale(vec, 1.0 - 2e-5);
        // Normalised direction plus distance.
        let (dir, dist) = math::normalize(vec);
        let mut hit = RayHit::within(dist);
        let verts = self.verts;
        let tris = self.tris;
        // `bvh_callback`: the watertight test from `start`, kept when nearer and facing the
        // ray (`dot(n, vec) < -1e-5`; `normalize_v3` normalised `vec` in place).
        let mut cb = |index: usize, ray: &frap_geom::bvh::BvhRay, hit: &mut RayHit| {
            let [a, b, c] = tris[index];
            let (va, vb, vc) = (verts[a], verts[b], verts[c]);
            let Some(pre) = ray.isect_precalc.as_ref() else {
                return;
            };
            if let Some((dist_test, _)) = isect_ray_tri_watertight_v3(start, pre, va, vb, vc)
                && dist_test < hit.dist
            {
                let n = math::normal_tri(va, vb, vc);
                if math::dot(n, dir) < -1e-5 {
                    hit.index = Some(index);
                    hit.dist = dist_test;
                }
            }
        };
        self.bvh
            .ray_cast_ex(start, dir, 0.0, &mut hit, Some(&mut cb), true)
            .is_none()
    }

    /// `heat_source_distance`
    fn source_distance(&self, vertex: usize, source: usize) -> f32 {
        let v = self.verts[vertex];
        let closest = math::closest_to_line_segment(v, self.roots[source], self.tips[source]);
        let (d, dist) = math::normalize(math::sub(v, closest));
        let cosine = math::dot(d, self.vert_normals[vertex]);
        dist / (0.5 * (cosine + 1.001))
    }

    /// `heat_source_closest`
    fn source_closest(&self, vertex: usize, source: usize) -> bool {
        let dist = self.source_distance(vertex, source);
        dist <= self.mindist[vertex] * (1.0 + DISTANCE_EPSILON)
            && self.source_visible(vertex, source)
    }

    /// `heat_set_H`
    fn set_h(&mut self, vertex: usize) {
        let numsource = self.roots.len();
        let mut mindist = 1e10f32;
        for j in 0..numsource {
            let dist = self.source_distance(vertex, j);
            // `std::min(dist, mindist)`
            mindist = if mindist < dist { mindist } else { dist };
        }
        self.mindist[vertex] = mindist;
        let numclosest = (0..numsource)
            .filter(|&j| self.source_closest(vertex, j))
            .count();
        self.p[vertex] = if numclosest > 0 {
            1.0 / numclosest as f32
        } else {
            0.0
        };
        self.h[vertex] = if numclosest > 0 {
            // `max_ff(mindist, 1e-4f)`
            let m = if mindist > 1e-4 { mindist } else { 1e-4 };
            numclosest as f32 * C_WEIGHT / (m * m)
        } else {
            0.0
        };
    }

    /// `laplacian_system_construct_end` (with `areaweights`, without `storeweights`).
    fn construct_end(&mut self) {
        let n = self.verts.len();
        let verts = self.verts;
        // `edgehash`: the number of triangles of each edge.
        let mut edges: std::collections::HashMap<(usize, usize), u32> =
            std::collections::HashMap::with_capacity(self.tris.len() * 2);
        let key = |a: usize, b: usize| (a.min(b), a.max(b));
        for &[a, b, c] in self.tris {
            for (x, y) in [(a, b), (b, c), (c, a)] {
                *edges.entry(key(x, y)).or_insert(0) += 1;
            }
        }
        // `laplacian_triangle_area`
        let mut varea = vec![0.0f32; n];
        let right = 90.0f32 * (std::f64::consts::PI / 180.0) as f32;
        for &[i1, i2, i3] in self.tris {
            frap_math::budget::check();
            let (v1, v2, v3) = (verts[i1], verts[i2], verts[i3]);
            let mut t1 = math::cotangent_tri_weight(v1, v2, v3);
            let mut t2 = math::cotangent_tri_weight(v2, v3, v1);
            let mut t3 = math::cotangent_tri_weight(v3, v1, v2);
            let obtuse = if math::angle_v3v3v3(v2, v1, v3) > right {
                1
            } else if math::angle_v3v3v3(v1, v2, v3) > right {
                2
            } else if math::angle_v3v3v3(v1, v3, v2) > right {
                3
            } else {
                0
            };
            if obtuse > 0 {
                let area = math::area_tri(v1, v2, v3);
                varea[i1] += if obtuse == 1 { area } else { area * 0.5 };
                varea[i2] += if obtuse == 2 { area } else { area * 0.5 };
                varea[i3] += if obtuse == 3 { area } else { area * 0.5 };
            } else {
                let len1 = math::len_v3v3(v2, v3);
                let len2 = math::len_v3v3(v1, v3);
                let len3 = math::len_v3v3(v1, v2);
                t1 *= len1 * len1;
                t2 *= len2 * len2;
                t3 *= len3 * len3;
                varea[i1] += (t2 + t3) * 0.25;
                varea[i2] += (t1 + t3) * 0.25;
                varea[i3] += (t1 + t2) * 0.25;
            }
        }
        for (a, va) in varea.iter_mut().enumerate() {
            if *va != 0.0 {
                *va = 0.5 / *va;
            }
            // For heat weighting.
            self.solver.matrix_add(a, a, f64::from(self.h[a]));
        }
        // `laplacian_triangle_weights`
        for &[i1, i2, i3] in self.tris {
            frap_math::budget::check();
            let (v1, v2, v3) = (verts[i1], verts[i2], verts[i3]);
            let count = |a: usize, b: usize| edges[&key(a, b)] as f32;
            let t1 = math::cotangent_tri_weight(v1, v2, v3) / count(i2, i3);
            let t2 = math::cotangent_tri_weight(v2, v3, v1) / count(i3, i1);
            let t3 = math::cotangent_tri_weight(v3, v1, v2) / count(i1, i2);
            let s = &mut self.solver;
            let add = |s: &mut LeastSquares, r: usize, c: usize, v: f32| {
                s.matrix_add(r, c, f64::from(v));
            };
            add(s, i1, i1, (t2 + t3) * varea[i1]);
            add(s, i2, i2, (t1 + t3) * varea[i2]);
            add(s, i3, i3, (t1 + t2) * varea[i3]);
            add(s, i1, i2, -t3 * varea[i1]);
            add(s, i2, i1, -t3 * varea[i2]);
            add(s, i2, i3, -t1 * varea[i2]);
            add(s, i3, i2, -t1 * varea[i3]);
            add(s, i3, i1, -t2 * varea[i3]);
            add(s, i1, i3, -t2 * varea[i1]);
        }
    }
}

/// `heat_bone_weighting`: writes the heat weights of every selected source into `dverts`
/// (one entry per vertex). Returns the failure report, if a solve failed.
pub fn heat_bone_weighting(dverts: &mut [DeformVert], inp: &HeatInput<'_>) -> Option<&'static str> {
    let numbones = inp.roots.len();
    let n = inp.verts.len();
    if numbones == 0 {
        return None;
    }
    let mut sys = HeatSystem::new(inp);
    let mirror = inp.mirror;
    let flip_of = |a: usize, j: usize| -> Option<(u32, usize)> {
        let g = inp.flips[j]?;
        let f = mirror?.get(a).copied().flatten()?;
        Some((g, f))
    };
    let add = |dverts: &mut [DeformVert], g: Option<u32>, v: usize, w: f32, m: dv::AssignMode| {
        if let (Some(g), Some(d)) = (g, dverts.get_mut(v)) {
            dv::vert_add(d, g, w, m);
        }
    };
    let remove = |dverts: &mut [DeformVert], g: Option<u32>, v: usize| {
        if let (Some(g), Some(d)) = (g, dverts.get_mut(v)) {
            dv::remove_group(d, g);
        }
    };
    let weight = |dverts: &[DeformVert], g: Option<u32>, v: usize| -> f32 {
        match (g, dverts.get(v)) {
            (Some(g), Some(d)) => dv::weight(d, g),
            _ => 0.0,
        }
    };
    let mut error = None;
    for j in 0..numbones {
        // `--stmt-timeout`: per bone (each one is a visibility pass and a solve).
        frap_math::budget::check_now();
        if !inp.selected[j] {
            continue;
        }
        let group = inp.groups[j];
        let firstsegment = j == 0 || inp.groups[j - 1] != group;
        let lastsegment = j == numbones - 1 || group != inp.groups[j + 1];
        let bbone = !(firstsegment && lastsegment);
        // Clear weights.
        if bbone && firstsegment {
            for a in 0..n {
                remove(dverts, group, a);
                if let Some((g, f)) = flip_of(a, j) {
                    remove(dverts, Some(g), f);
                }
            }
        }
        // Fill the right-hand side.
        for a in 0..n {
            frap_math::budget::check();
            if sys.source_closest(a, j) {
                sys.solver.rhs_add(a, f64::from(sys.h[a] * sys.p[a]));
            }
        }
        if sys.solver.solve() {
            for a in 0..n {
                let solution = sys.solver.get(a) as f32;
                let flip = flip_of(a, j);
                if bbone {
                    if solution > 0.0 {
                        add(dverts, group, a, solution, dv::AssignMode::Add);
                        if let Some((g, f)) = flip {
                            add(dverts, Some(g), f, solution, dv::AssignMode::Add);
                        }
                    }
                } else {
                    let w = heat_limit_weight(solution);
                    if w > 0.0 {
                        add(dverts, group, a, w, dv::AssignMode::Replace);
                    } else {
                        remove(dverts, group, a);
                    }
                    if let Some((g, f)) = flip {
                        if w > 0.0 {
                            add(dverts, Some(g), f, w, dv::AssignMode::Replace);
                        } else {
                            remove(dverts, Some(g), f);
                        }
                    }
                }
            }
        } else if error.is_none() {
            error = Some(HEAT_FAILED);
            break;
        }
        // Remove too small weights.
        if bbone && lastsegment {
            for a in 0..n {
                let w = heat_limit_weight(weight(dverts, group, a));
                if w <= 0.0 {
                    remove(dverts, group, a);
                }
                if let Some((g, f)) = flip_of(a, j) {
                    let w = heat_limit_weight(weight(dverts, Some(g), f));
                    if w <= 0.0 {
                        remove(dverts, Some(g), f);
                    }
                }
            }
        }
    }
    error
}
