// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_polyfill_2d`: ear-clipping triangulation of a simple 2D polygon, ported from
//! `blender/source/blender/blenlib/intern/polyfill_2d.cc` (Blender 5.2) with the same compile
//! options (`USE_CLIP_EVEN`, `USE_CONVEX_SKIP`, `USE_CLIP_SWEEP`, `USE_KDTREE`,
//! `USE_KDTREE_INDEX_CACHE`, `USE_PRECOMPUTED_ISECT`), so the triangles and their order are
//! Blender's. Mesh loop triangles use it **without** beautify (ARCHITECTURE §6.1); BMesh
//! triangulation adds `polyfill_2d_beautify` (M2).
//!
//! [`polyfill_mesh_face`] is the n-gon branch of `mesh_calc_tessellation_for_face_impl`
//! (`mesh_tessellate.cc`): project with `axis_dominant_v3_to_m3_negate` of the Newell normal,
//! then fill with `coords_sign = 1`.

use crate::normals::{V3, newell_cross, normalize_v3};

type Sign = i8;
const CONCAVE: Sign = -1;
const TANGENTIAL: Sign = 0;
const CONVEX: Sign = 1;

const UNSET: u32 = u32::MAX;
const KDNODE_FLAG_REMOVED: u16 = 1;

#[derive(Copy, Clone, Debug)]
struct KdNode {
    neg: u32,
    pos: u32,
    index: u32,
    axis: u8,
    flag: u16,
    parent: u32,
}

struct KdTree2D<'a> {
    nodes: Vec<KdNode>,
    coords: &'a [[f32; 2]],
    root: u32,
    node_num: u32,
    nodes_map: Vec<u32>,
}

#[derive(Copy, Clone, Debug)]
struct PolyIndex {
    next: usize,
    prev: usize,
    index: u32,
    sign: Sign,
    cache_dirty: bool,
    index_last_hit: u32,
}

struct PolyFill<'a> {
    indices: Vec<PolyIndex>,
    /// Head of the circular list (`pf->indices`).
    head: usize,
    coords: &'a [[f32; 2]],
    coords_num: u32,
    coords_num_concave: u32,
    tris: Vec<[u32; 3]>,
    kdtree: KdTree2D<'a>,
}

#[inline]
fn signum(a: f32) -> Sign {
    if a > 0.0 {
        CONVEX
    } else if a == 0.0 {
        TANGENTIAL
    } else {
        CONCAVE
    }
}

#[inline]
fn area_tri_signed_v2_alt_2x(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    let d2 = [v2[0] - v1[0], v2[1] - v1[1]];
    let d3 = [v3[0] - v1[0], v3[1] - v1[1]];
    (d2[0] * d3[1]) - (d3[0] * d2[1])
}

#[inline]
fn span_tri_v2_sign(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> Sign {
    signum(area_tri_signed_v2_alt_2x(v3, v2, v1))
}

#[derive(Copy, Clone)]
struct TriIsect {
    edge: [[f32; 2]; 3],
    c: [f32; 3],
}

fn tri_isect_init(vs: [[f32; 2]; 3]) -> TriIsect {
    let edge = [
        [vs[1][0] - vs[0][0], vs[1][1] - vs[0][1]],
        [vs[2][0] - vs[1][0], vs[2][1] - vs[1][1]],
        [vs[0][0] - vs[2][0], vs[0][1] - vs[2][1]],
    ];
    let c = [
        (edge[0][0] * vs[0][1]) - (vs[0][0] * edge[0][1]),
        (edge[1][0] * vs[1][1]) - (vs[1][0] * edge[1][1]),
        (edge[2][0] * vs[2][1]) - (vs[2][0] * edge[2][1]),
    ];
    TriIsect { edge, c }
}

#[inline]
fn tri_isect_test(t: &TriIsect, co: [f32; 2]) -> bool {
    ((t.edge[1][1] * co[0]) - (t.edge[1][0] * co[1]) + t.c[1] >= 0.0)
        && ((t.edge[2][1] * co[0]) - (t.edge[2][0] * co[1]) + t.c[2] >= 0.0)
        && ((t.edge[0][1] * co[0]) - (t.edge[0][0] * co[1]) + t.c[0] >= 0.0)
}

impl<'a> KdTree2D<'a> {
    fn balance_recursive(&mut self, base: usize, node_num: u32, mut axis: u8, ofs: u32) -> u32 {
        if node_num == 0 {
            return UNSET;
        }
        if node_num == 1 {
            return ofs;
        }
        let coords = self.coords;
        let nodes = &mut self.nodes[base..base + node_num as usize];
        let ax = axis as usize;
        let mut neg: i64 = 0;
        let mut pos: i64 = node_num as i64 - 1;
        let median = (node_num / 2) as i64;
        while pos > neg {
            let co = coords[nodes[pos as usize].index as usize][ax];
            let mut i = neg - 1;
            let mut j = pos;
            loop {
                loop {
                    i += 1;
                    // `while (coords[nodes[++i].index][axis] < co) {}`: kept as a negated
                    // `<` (not `>=`) so a NaN pivot or coordinate stops the scan like
                    // Blender instead of running past the slice.
                    #[allow(clippy::neg_cmp_op_on_partial_ord)]
                    if !(coords[nodes[i as usize].index as usize][ax] < co) {
                        break;
                    }
                }
                loop {
                    j -= 1;
                    if !(coords[nodes[j as usize].index as usize][ax] > co && j > neg) {
                        break;
                    }
                }
                if i >= j {
                    break;
                }
                swap_head(nodes, i as usize, j as usize);
            }
            swap_head(nodes, i as usize, pos as usize);
            if i >= median {
                pos = i - 1;
            }
            if i <= median {
                neg = i + 1;
            }
        }
        let median = median as usize;
        nodes[median].axis = axis;
        axis = if axis == 0 { 1 } else { 0 };
        let n = self.balance_recursive(base, median as u32, axis, ofs);
        let p = self.balance_recursive(
            base + median + 1,
            node_num - (median as u32 + 1),
            axis,
            median as u32 + 1 + ofs,
        );
        self.nodes[base + median].neg = n;
        self.nodes[base + median].pos = p;
        median as u32 + ofs
    }

    fn init_mapping(&mut self) {
        for i in 0..self.node_num as usize {
            let node = self.nodes[i];
            if node.neg != UNSET {
                self.nodes[node.neg as usize].parent = i as u32;
            }
            if node.pos != UNSET {
                self.nodes[node.pos as usize].parent = i as u32;
            }
            self.nodes_map[node.index as usize] = i as u32;
        }
        let root = self.root as usize;
        self.nodes[root].parent = UNSET;
    }

    fn node_remove(&mut self, index: u32) {
        let mut node_index = self.nodes_map[index as usize];
        if node_index == UNSET {
            return;
        }
        self.nodes_map[index as usize] = UNSET;
        self.node_num -= 1;
        self.nodes[node_index as usize].flag |= KDNODE_FLAG_REMOVED;
        loop {
            let node = self.nodes[node_index as usize];
            if node.parent == UNSET {
                break;
            }
            let node_child = if node.neg == UNSET {
                node.pos
            } else if node.pos == UNSET {
                node.neg
            } else {
                break;
            };
            let parent = node.parent as usize;
            if self.nodes[parent].neg == node_index {
                self.nodes[parent].neg = node_child;
            } else {
                self.nodes[parent].pos = node_child;
            }
            if node_child != UNSET {
                self.nodes[node_child as usize].parent = node.parent;
            }
            if self.nodes[parent].flag & KDNODE_FLAG_REMOVED == 0 {
                break;
            }
            node_index = node.parent;
        }
    }

    fn isect_tri_recursive(
        &self,
        tri_index: [u32; 3],
        tri: &TriIsect,
        center: [f32; 2],
        bounds: [[f32; 2]; 2],
        node_i: u32,
    ) -> u32 {
        let node = &self.nodes[node_i as usize];
        let co = self.coords[node.index as usize];
        if node.flag & KDNODE_FLAG_REMOVED == 0
            && tri_isect_test(tri, co)
            && !tri_index.contains(&node.index)
        {
            return node.index;
        }
        let ax = node.axis as usize;
        let recurse_neg = || {
            if node.neg != UNSET && co[ax] >= bounds[ax][0] {
                self.isect_tri_recursive(tri_index, tri, center, bounds, node.neg)
            } else {
                UNSET
            }
        };
        let recurse_pos = || {
            if node.pos != UNSET && co[ax] <= bounds[ax][1] {
                self.isect_tri_recursive(tri_index, tri, center, bounds, node.pos)
            } else {
                UNSET
            }
        };
        if center[ax] > co[ax] {
            let r = recurse_pos();
            if r == UNSET { recurse_neg() } else { r }
        } else {
            let r = recurse_neg();
            if r == UNSET { recurse_pos() } else { r }
        }
    }

    fn isect_tri_single(&self, tri_index: [u32; 3], test_index: u32) -> bool {
        if self.nodes_map[test_index as usize] != UNSET && !tri_index.contains(&test_index) {
            let vs = tri_index.map(|i| self.coords[i as usize]);
            let t = tri_isect_init(vs);
            return tri_isect_test(&t, self.coords[test_index as usize]);
        }
        false
    }

    fn isect_tri(&self, ind: [u32; 3]) -> u32 {
        let vs = ind.map(|i| self.coords[i as usize]);
        let bounds = [
            [
                std_min3(vs[0][0], vs[1][0], vs[2][0]),
                std_max3(vs[0][0], vs[1][0], vs[2][0]),
            ],
            [
                std_min3(vs[0][1], vs[1][1], vs[2][1]),
                std_max3(vs[0][1], vs[1][1], vs[2][1]),
            ],
        ];
        let center = [
            (vs[0][0] + vs[1][0] + vs[2][0]) * (1.0 / 3.0),
            (vs[0][1] + vs[1][1] + vs[2][1]) * (1.0 / 3.0),
        ];
        let t = tri_isect_init(vs);
        self.isect_tri_recursive(ind, &t, center, bounds, self.root)
    }
}

/// `std::min({a, b, c})`: keeps the first smallest by `<`, so NaN propagates exactly as
/// in C++ (unlike `f32::min`, which drops NaN).
#[inline]
fn std_min3(a: f32, b: f32, c: f32) -> f32 {
    let mut r = a;
    if b < r {
        r = b;
    }
    if c < r {
        r = c;
    }
    r
}

/// `std::max({a, b, c})`: keeps the first largest by `<` (C++ `max_element` order).
#[inline]
fn std_max3(a: f32, b: f32, c: f32) -> f32 {
    let mut r = a;
    if r < b {
        r = b;
    }
    if r < c {
        r = c;
    }
    r
}

/// `SWAP(KDTreeNode2D_head, …)`: swaps `neg`, `pos` and `index` only.
fn swap_head(nodes: &mut [KdNode], i: usize, j: usize) {
    let (a, b) = (nodes[i], nodes[j]);
    nodes[i].neg = b.neg;
    nodes[i].pos = b.pos;
    nodes[i].index = b.index;
    nodes[j].neg = a.neg;
    nodes[j].pos = a.pos;
    nodes[j].index = a.index;
}

impl<'a> PolyFill<'a> {
    fn coord_sign_calc(&mut self, pi: usize) {
        let p = self.indices[pi];
        let c = self.coords;
        self.indices[pi].sign = span_tri_v2_sign(
            c[self.indices[p.prev].index as usize],
            c[p.index as usize],
            c[self.indices[p.next].index as usize],
        );
    }

    fn coord_remove(&mut self, pi: usize) {
        if self.kdtree.node_num != 0 {
            self.kdtree.node_remove(self.indices[pi].index);
        }
        let PolyIndex { next, prev, .. } = self.indices[pi];
        self.indices[next].prev = prev;
        self.indices[prev].next = next;
        if self.head == pi {
            self.head = next;
        }
        self.coords_num -= 1;
    }

    fn ear_tip_check(&mut self, pi: usize, sign_accept: Sign) -> bool {
        if self.coords_num_concave == 0 {
            return true;
        }
        let p = self.indices[pi];
        if p.sign != sign_accept {
            return false;
        }
        let ind = [
            p.index,
            self.indices[p.next].index,
            self.indices[p.prev].index,
        ];
        let cached = p.index_last_hit;
        if cached != UNSET {
            if !p.cache_dirty {
                if self.kdtree.nodes_map[cached as usize] != UNSET {
                    return false;
                }
            } else if self.kdtree.isect_tri_single(ind, cached) {
                self.indices[pi].cache_dirty = false;
                return false;
            }
        }
        let hit = self.kdtree.isect_tri(ind);
        if hit != UNSET {
            self.indices[pi].index_last_hit = hit;
            self.indices[pi].cache_dirty = false;
            return false;
        }
        true
    }

    fn ear_tip_find(&mut self, ear_init: usize, reverse: bool) -> usize {
        let coords_num = self.coords_num;
        let mut sign_accept = CONVEX;
        while sign_accept >= TANGENTIAL {
            let mut pi = ear_init;
            for _ in 0..coords_num {
                if self.ear_tip_check(pi, sign_accept) {
                    return pi;
                }
                pi = if reverse {
                    self.indices[pi].prev
                } else {
                    self.indices[pi].next
                };
            }
            sign_accept -= 1;
        }
        // Desperate mode: any vertex that is not concave, else the last one visited.
        let mut pi = ear_init;
        for _ in 0..coords_num {
            if self.indices[pi].sign != CONCAVE {
                return pi;
            }
            pi = self.indices[pi].next;
        }
        pi
    }

    fn ear_tip_cut(&mut self, pi: usize) {
        let p = self.indices[pi];
        self.tris.push([
            self.indices[p.prev].index,
            p.index,
            self.indices[p.next].index,
        ]);
        self.coord_remove(pi);
    }

    fn triangulate(&mut self) {
        let mut ear_init = self.head;
        let mut reverse = false;
        while self.coords_num > 3 {
            // O(n²) worst case on huge n-gons: a `--stmt-timeout` cancellation point.
            frap_math::budget::check();
            let ear = self.ear_tip_find(ear_init, reverse);
            if self.indices[ear].sign != CONVEX {
                self.coords_num_concave -= 1;
            }
            let pi_prev = self.indices[ear].prev;
            let pi_next = self.indices[ear].next;
            self.ear_tip_cut(ear);
            self.indices[pi_prev].cache_dirty = true;
            self.indices[pi_next].cache_dirty = true;
            let sign_orig_prev = self.indices[pi_prev].sign;
            let sign_orig_next = self.indices[pi_next].sign;
            if sign_orig_prev != CONVEX {
                self.coord_sign_calc(pi_prev);
                if self.indices[pi_prev].sign == CONVEX {
                    self.coords_num_concave -= 1;
                    let idx = self.indices[pi_prev].index;
                    self.kdtree.node_remove(idx);
                }
            }
            if sign_orig_next != CONVEX {
                self.coord_sign_calc(pi_next);
                if self.indices[pi_next].sign == CONVEX {
                    self.coords_num_concave -= 1;
                    let idx = self.indices[pi_next].index;
                    self.kdtree.node_remove(idx);
                }
            }
            ear_init = if reverse {
                self.indices[pi_prev].prev
            } else {
                self.indices[pi_next].next
            };
            if self.indices[ear_init].sign != CONVEX {
                ear_init = if reverse {
                    self.indices[ear_init].prev
                } else {
                    self.indices[ear_init].next
                };
                reverse = !reverse;
            }
        }
        if self.coords_num == 3 {
            let a = self.head;
            let b = self.indices[a].next;
            let c = self.indices[b].next;
            self.tris.push([
                self.indices[a].index,
                self.indices[b].index,
                self.indices[c].index,
            ]);
        }
    }
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (cross_poly_v2).
/// Trapezium-rule signed area (×2, negative for counter-clockwise polygons).
pub fn cross_poly_v2(verts: &[[f32; 2]]) -> f32 {
    let n = verts.len();
    let mut cross = 0.0f32;
    let mut prev = verts[n - 1];
    for &cur in verts {
        cross += (prev[0] - cur[0]) * (cur[1] + prev[1]);
        prev = cur;
    }
    cross
}

/// `BLI_polyfill_calc(coords, coords_num, coords_sign, r_tris)`: `coords_sign` is 1 for
/// counter-clockwise input, -1 for clockwise, 0 to detect. Returns `coords.len() - 2`
/// triangles of indices into `coords` (fewer only for inputs with < 3 points).
pub fn polyfill_calc(coords: &[[f32; 2]], coords_sign: i32) -> Vec<[u32; 3]> {
    let n = coords.len();
    if n < 3 {
        return Vec::new();
    }
    let sign = if coords_sign == 0 {
        if cross_poly_v2(coords) <= 0.0 { 1 } else { -1 }
    } else {
        coords_sign
    };
    let mut indices: Vec<PolyIndex> = (0..n)
        .map(|i| PolyIndex {
            next: (i + 1) % n,
            prev: (i + n - 1) % n,
            index: if sign == 1 {
                i as u32
            } else {
                (n - 1 - i) as u32
            },
            sign: TANGENTIAL,
            cache_dirty: false,
            index_last_hit: UNSET,
        })
        .collect();
    let mut concave = 0u32;
    for i in 0..n {
        let p = indices[i];
        let s = span_tri_v2_sign(
            coords[indices[p.prev].index as usize],
            coords[p.index as usize],
            coords[indices[p.next].index as usize],
        );
        indices[i].sign = s;
        if s != CONVEX {
            concave += 1;
        }
    }
    let mut pf = PolyFill {
        indices,
        head: 0,
        coords,
        coords_num: n as u32,
        coords_num_concave: concave,
        tris: Vec::with_capacity(n - 2),
        kdtree: KdTree2D {
            nodes: Vec::new(),
            coords,
            root: UNSET,
            node_num: 0,
            nodes_map: Vec::new(),
        },
    };
    if concave != 0 {
        pf.kdtree.nodes = pf
            .indices
            .iter()
            .filter(|p| p.sign != CONVEX)
            .map(|p| KdNode {
                neg: UNSET,
                pos: UNSET,
                index: p.index,
                axis: 0,
                flag: 0,
                parent: UNSET,
            })
            .collect();
        pf.kdtree.node_num = concave;
        pf.kdtree.nodes_map = vec![UNSET; n];
        pf.kdtree.root = pf.kdtree.balance_recursive(0, concave, 0, 0);
        pf.kdtree.init_mapping();
    }
    pf.triangulate();
    pf.tris
}

// Ported from blender/source/blender/blenlib/intern/math_vector.cc (ortho_basis_v3v3_v3).
/// Two unit vectors orthogonal to the unit vector `n`.
pub fn ortho_basis_v3v3_v3(n: V3) -> (V3, V3) {
    let eps = f32::EPSILON;
    let f = n[0] * n[0] + n[1] * n[1];
    if f > eps {
        let d = 1.0 / f.sqrt();
        let r1 = [n[1] * d, -n[0] * d, 0.0];
        let r2 = [-n[2] * r1[1], n[2] * r1[0], n[0] * r1[1] - n[1] * r1[0]];
        (r1, r2)
    } else {
        (
            [if n[2] < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
    }
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (axis_dominant_v3_to_m3_negate).
/// Rows of the projection matrix (after Blender's transpose): `r[i]` dotted with a 3D point
/// gives 2D coordinate `i` (`mul_v2_m3v3`).
pub fn axis_dominant_v3_to_m3_negate(normal: V3) -> [V3; 3] {
    let z = [-normal[0], -normal[1], -normal[2]];
    let (x, y) = ortho_basis_v3v3_v3(z);
    // Columns x, y, z; transpose_m3 makes them rows.
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

/// `mul_v2_m3v3(r, m, a)` with Blender's column-major `m` (`m[col][row]`).
#[inline]
fn mul_v2_m3v3(m: &[V3; 3], a: V3) -> [f32; 2] {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
    ]
}

/// Triangulates one mesh face of 5+ corners as `mesh_calc_tessellation_for_face_impl` does:
/// returns triangles of face-local corner indices.
pub fn polyfill_mesh_face(positions: &[V3], face_verts: &[i32]) -> Vec<[u32; 3]> {
    let (mut normal, len) = normalize_v3(newell_cross(positions, face_verts));
    if len == 0.0 {
        normal[2] = 1.0;
    }
    let m = axis_dominant_v3_to_m3_negate(normal);
    let proj: Vec<[f32; 2]> = face_verts
        .iter()
        .map(|&v| mul_v2_m3v3(&m, positions[v as usize]))
        .collect();
    polyfill_calc(&proj, 1)
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc
// (is_quad_flip_v3_first_third_fast).
/// True when splitting the quad along its 0–2 diagonal is degenerate, so the tessellation uses
/// the 1–3 diagonal instead.
pub fn is_quad_flip_v3_first_third_fast(v1: V3, v2: V3, v3: V3, v4: V3) -> bool {
    use crate::normals::{cross, dot, sub};
    let d_12 = sub(v2, v1);
    let d_13 = sub(v3, v1);
    let d_14 = sub(v4, v1);
    let cross_a = cross(d_12, d_13);
    let cross_b = cross(d_14, d_13);
    dot(cross_a, cross_b) > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area2(c: &[[f32; 2]], t: [u32; 3]) -> f32 {
        area_tri_signed_v2_alt_2x(c[t[0] as usize], c[t[1] as usize], c[t[2] as usize])
    }

    #[test]
    fn convex_fills_n_minus_2() {
        let c: Vec<[f32; 2]> = (0..9)
            .map(|i| {
                let a = i as f32 * 0.7;
                [libm::cosf(a), libm::sinf(a)]
            })
            .collect();
        let t = polyfill_calc(&c, 0);
        assert_eq!(t.len(), 7);
    }

    #[test]
    fn concave_u_shape_covers_area() {
        // U shape (counter-clockwise), area 7.
        let c = [
            [0.0, 0.0],
            [3.0, 0.0],
            [3.0, 3.0],
            [2.0, 3.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 3.0],
            [0.0, 3.0],
        ];
        // Blender's polyfill works on clockwise polygons (`coords_sign = 1` means
        // `cross_poly_v2 <= 0`); this one is counter-clockwise, so let it detect.
        assert!(cross_poly_v2(&c) > 0.0);
        let t = polyfill_calc(&c, 0);
        assert_eq!(t.len(), 6);
        let total: f32 = t.iter().map(|&tri| area2(&c, tri).abs()).sum();
        assert!((total / 2.0 - 7.0).abs() < 1e-5, "{total}");
        for tri in t {
            assert!(
                area2(&c, tri) <= 0.0,
                "{tri:?} is not wound like the reversed input"
            );
        }
    }

    #[test]
    fn reversed_input_detects_sign() {
        let c = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.5, -0.5]];
        let t = polyfill_calc(&c, 0);
        assert_eq!(t.len(), 3);
    }

    /// Regression (M8 stabilization round 3): NaN / ±inf coordinates reached the k-d tree
    /// balance with an inverted comparison and indexed past the slice. Blender finishes
    /// on such input (with garbage triangles); so must we, still producing n - 2 tris.
    #[test]
    fn non_finite_coords_do_not_panic() {
        let nan = f32::NAN;
        let inf = f32::INFINITY;
        let base: Vec<[f32; 2]> = vec![
            [0.0, 0.0],
            [3.0, 0.0],
            [3.0, 3.0],
            [2.0, 3.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 3.0],
            [0.0, 3.0],
        ];
        for bad in [nan, inf, -inf, f32::MAX, -f32::MAX] {
            for k in 0..base.len() {
                for axis in 0..2 {
                    let mut c = base.clone();
                    c[k][axis] = bad;
                    for sign in [-1, 0, 1] {
                        let t = polyfill_calc(&c, sign);
                        assert_eq!(t.len(), c.len() - 2, "bad={bad} k={k} axis={axis}");
                    }
                }
            }
            // Every coordinate non-finite.
            let c = vec![[bad, bad]; 7];
            for sign in [-1, 0, 1] {
                assert_eq!(polyfill_calc(&c, sign).len(), 5);
            }
            // Mixed NaN and inf.
            let mut c = base.clone();
            c[1] = [nan, inf];
            c[4] = [-inf, nan];
            c[6] = [bad, 0.5];
            for sign in [-1, 0, 1] {
                assert_eq!(polyfill_calc(&c, sign).len(), 6);
            }
        }
    }
}
