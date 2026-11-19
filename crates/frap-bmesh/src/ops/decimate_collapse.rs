// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_decimate_collapse` (the DECIMATE modifier's `COLLAPSE` mode and `mesh.decimate`):
//! quadric edge collapse down to `factor` of the faces, optional vertex-group weights,
//! triangulation and symmetry (WP 4.5).
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_decimate_collapse.cc (Blender 5.2.4,
//! GPL-2.0-or-later), with every `#define` of the file on (`USE_SYMMETRY`, `USE_CUSTOMDATA`,
//! `USE_TRIANGULATE`, `USE_VERT_NORMAL_INTERP`, `USE_TOPOLOGY_FALLBACK`); `BLI_heap` and
//! `BLI_quadric` are ported in `decimate_collapse/{heap,quadric}.rs`, the symmetry search uses
//! the `BLI_kdtree` port of `frap-geom`.
//!
//! **Algorithm and order.** Faces of 4+ corners are first triangulated (`BM_face_triangulate`,
//! beauty quads and n-gons) in face order; the loops of every cut edge remember their original
//! face. Each vertex gets the sum of the plane quadrics of its faces (and of its boundary edges,
//! weighted 100). Every edge between triangles (two, or one on a boundary) goes on a min-heap
//! keyed by the quadric error of its optimal collapse point (flat areas, error below 1e-12,
//! fall back to a negative topology cost so they collapse first). The cheapest edge is popped
//! and its second vertex merged into the first at the optimal point, unless that would make
//! the topology degenerate or flip a face (the edge then goes back with an invalid cost), until
//! the face count reaches `factor` of the (triangulated) count. Finally, without
//! `do_triangulate`, triangle pairs that came from one face are joined back when the quad is
//! convex. Vertex, edge and UV/colour corner data are interpolated along each collapse.
//!
//! Indices: `vweights` are by vertex index in iteration order (`BM_elem_index_get` with clean
//! indices, as both callers set them). Afterwards every element index is dirty.

mod heap;
mod quadric;

use frap_geom::kdtree::KdTree;
use frap_mesh::AttrData;

use self::heap::{Heap, NodeId};
use self::quadric::Quadric;
use crate::ops::contextual_create::bli::is_quad_convex_v3;
use crate::ops::triangulate::{NgonMethod, QuadMethod, face_triangulate};
use crate::query::geom::{V3, cross, dot, len_squared_v3, normalize_v3, sub};
use crate::{BMesh, EdgeId, ElemFlag, ElemTypes, FaceId, LoopId, VertId};

/// `BM_mesh_decimate_collapse` arguments.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DecimateCollapseIn {
    /// `factor` (`ratio`): the fraction of faces to keep.
    pub factor: f32,
    /// Per-vertex weights (`vertex_group` with `vertex_group_factor`), or `None`, by vertex
    /// index in iteration order.
    pub vweights: Option<Vec<f32>>,
    /// `vertex_group_factor`.
    pub vweight_factor: f32,
    /// `use_collapse_triangulate`.
    pub do_triangulate: bool,
    /// `symmetry_axis` (`-1`: no symmetry).
    pub symmetry_axis: i32,
    /// Symmetry threshold (`BM_SYMMETRY_EPS`-scaled).
    pub symmetry_eps: f32,
}

impl Default for DecimateCollapseIn {
    fn default() -> Self {
        DecimateCollapseIn {
            factor: 1.0,
            vweights: None,
            vweight_factor: 1.0,
            do_triangulate: false,
            symmetry_axis: -1,
            symmetry_eps: 0.0,
        }
    }
}

impl DecimateCollapseIn {
    /// The arguments with `factor` (no weights, no triangulation, no symmetry): the modifier's
    /// and the operator's base (the struct is `#[non_exhaustive]`).
    pub fn with_factor(factor: f32) -> DecimateCollapseIn {
        DecimateCollapseIn {
            factor,
            ..DecimateCollapseIn::default()
        }
    }
}

/// `BOUNDARY_PRESERVE_WEIGHT`.
const BOUNDARY_PRESERVE_WEIGHT: f64 = 100.0;
/// `OPTIMIZE_EPS`.
const OPTIMIZE_EPS: f64 = 1e-8;
/// `COST_INVALID`.
const COST_INVALID: f32 = f32::MAX;
/// `TOPOLOGY_FALLBACK_EPS`.
const TOPOLOGY_FALLBACK_EPS: f32 = 1e-12;

/// `CD_UseFlag`.
#[derive(Copy, Clone, Debug, Default)]
struct CdUse {
    vert: bool,
    edge: bool,
    looop: bool,
}

/// The decimator's state (`eheap`, `eheap_table`, `vquadrics`, `vweights`, the symmetry map).
struct Decim {
    /// Dense vertex index by vertex slot (`BM_elem_index_get(v)`).
    vidx: Vec<u32>,
    /// Dense edge index by edge slot (`BM_elem_index_get(e)`, set after triangulation).
    eidx: Vec<u32>,
    vquadrics: Vec<Quadric>,
    vweights: Option<Vec<f32>>,
    vweight_factor: f32,
    eheap: Heap<EdgeId>,
    eheap_table: Vec<Option<NodeId>>,
    edge_symmetry_map: Option<Vec<i32>>,
    customdata: CdUse,
}

fn len_v3v3(a: V3, b: V3) -> f32 {
    len_squared_v3(sub(a, b)).sqrt()
}

fn len_squared_v3v3(a: V3, b: V3) -> f32 {
    len_squared_v3(sub(a, b))
}

/// `mid_v3_v3v3`.
fn mid(a: V3, b: V3) -> V3 {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}

/// `dot_v3db_v3fl`.
fn dot_db_fl(a: [f64; 3], b: V3) -> f64 {
    a[0] * f64::from(b[0]) + a[1] * f64::from(b[1]) + a[2] * f64::from(b[2])
}

/// `normalize_v3_db`.
fn normalize_db(n: &mut [f64; 4]) -> f64 {
    let mut d = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    if d > 1.0e-35 {
        d = d.sqrt();
        let f = 1.0 / d;
        n[0] *= f;
        n[1] *= f;
        n[2] *= f;
    } else {
        n[0] = 0.0;
        n[1] = 0.0;
        n[2] = 0.0;
        d = 0.0;
    }
    d
}

/// `compare_v3v3(a, b, limit)`.
fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit && (a[2] - b[2]).abs() <= limit
}

/// `line_point_factor_v3`.
fn line_point_factor_v3(p: V3, l1: V3, l2: V3) -> f32 {
    let u = sub(l2, l1);
    let h = sub(p, l1);
    let d = dot(u, u);
    if d > 0.0 { dot(u, h) / d } else { 0.0 }
}

/// `BM_edge_loop_pair`: the two loops of a manifold edge.
fn edge_loop_pair(bm: &BMesh, e: EdgeId) -> Option<(LoopId, LoopId)> {
    let la = bm.edge(e).l?;
    let lb = bm.lp(la).radial_next;
    (la != lb && bm.lp(lb).radial_next == la).then_some((la, lb))
}

/// `BM_loop_is_manifold`.
fn loop_is_manifold(bm: &BMesh, l: LoopId) -> bool {
    let r = bm.lp(l).radial_next;
    l != r && l == bm.lp(r).radial_next
}

/// Whether a custom-data column interpolates (`LayerTypeInfo::interp`).
fn has_interp(d: &AttrData) -> bool {
    !matches!(
        d,
        AttrData::Int8(_) | AttrData::Int16x2(_) | AttrData::Int32x2(_) | AttrData::String(_)
    ) && !matches!(d, AttrData::Float4x4(_))
}

/// Whether a corner column has custom-data math (`CustomData_layer_has_math`: `equal`, `add`,
/// `multiply`, `initminmax`, `dominmax`): UV maps and both colour types.
fn has_math(d: &AttrData) -> bool {
    matches!(
        d,
        AttrData::Float2(_) | AttrData::FloatColor(_) | AttrData::ByteColor(_)
    )
}

/// `CustomData_data_equals` of one corner column (types with math only).
fn data_equals(d: &AttrData, a: usize, b: usize) -> bool {
    match d {
        AttrData::Float2(c) => {
            let (x, y) = (c[a], c[b]);
            let (dx, dy) = (x[0] - y[0], x[1] - y[1]);
            dx * dx + dy * dy < 0.00001
        }
        AttrData::FloatColor(c) => {
            let mut tot = 0.0f32;
            for k in 0..4 {
                let d = c[a][k] - c[b][k];
                tot += d * d;
            }
            tot < 0.001
        }
        AttrData::ByteColor(c) => {
            let d: [f32; 4] = std::array::from_fn(|k| f32::from(c[a][k]) - f32::from(c[b][k]));
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3] < 0.001
        }
        _ => false,
    }
}

/// `round_fl_to_uchar_clamp`.
fn round_fl_to_uchar_clamp(f: f32) -> u8 {
    if f <= 0.0 {
        0
    } else if f > 255.0 - 0.5 {
        255
    } else {
        (f + 0.5) as u8
    }
}

/// `CustomData_bmesh_interp_n` of one corner column with math: `dst = Σ w[k] · src[k]`.
fn interp_n(d: &mut AttrData, src: [usize; 2], w: [f32; 2], dst: usize) {
    match d {
        AttrData::Float2(c) => {
            let mut r = [0.0f32; 2];
            for (&s, &wt) in src.iter().zip(&w) {
                r[0] += c[s][0] * wt;
                r[1] += c[s][1] * wt;
            }
            std::sync::Arc::make_mut(c)[dst] = r;
        }
        AttrData::FloatColor(c) => {
            let mut r = [0.0f32; 4];
            for (&s, &wt) in src.iter().zip(&w) {
                for k in 0..4 {
                    r[k] += c[s][k] * wt;
                }
            }
            std::sync::Arc::make_mut(c)[dst] = r;
        }
        AttrData::ByteColor(c) => {
            let mut r = [0.0f32; 4];
            for (&s, &wt) in src.iter().zip(&w) {
                for k in 0..4 {
                    r[k] += f32::from(c[s][k]) * wt;
                }
            }
            std::sync::Arc::make_mut(c)[dst] = r.map(round_fl_to_uchar_clamp);
        }
        _ => {}
    }
}

impl Decim {
    fn vi(&self, v: VertId) -> usize {
        self.vidx[v.slot()] as usize
    }
    fn ei(&self, e: EdgeId) -> usize {
        self.eidx[e.slot()] as usize
    }
    fn weight(&self, v: VertId) -> Option<f32> {
        self.vweights.as_ref().map(|w| w[self.vi(v)])
    }
}

/// `bm_decim_build_quadrics`.
fn build_quadrics(bm: &BMesh, d: &mut Decim) {
    for f in bm.face_ids() {
        let center = bm.face_calc_center_median(f);
        let no = bm.face(f).no;
        let mut plane = [f64::from(no[0]), f64::from(no[1]), f64::from(no[2]), 0.0];
        plane[3] = -dot_db_fl([plane[0], plane[1], plane[2]], center);
        let q = Quadric::from_plane(plane);
        for l in bm.face_loops(f) {
            let vi = d.vi(bm.lp(l).v);
            d.vquadrics[vi].add(&q);
        }
    }
    // Boundary edges.
    for e in bm.edge_ids() {
        if !bm.edge_is_boundary(e) {
            continue;
        }
        let [v1, v2] = bm.edge(e).v;
        let edge_vector = sub(bm.vert(v2).co, bm.vert(v1).co);
        let f = bm.lp(bm.edge(e).l.expect("boundary edge")).f;
        let edge_plane = cross(edge_vector, bm.face(f).no);
        let mut plane = [
            f64::from(edge_plane[0]),
            f64::from(edge_plane[1]),
            f64::from(edge_plane[2]),
            0.0,
        ];
        if normalize_db(&mut plane) > f64::from(f32::EPSILON) {
            let center = mid(bm.vert(v1).co, bm.vert(v2).co);
            plane[3] = -dot_db_fl([plane[0], plane[1], plane[2]], center);
            let mut q = Quadric::from_plane(plane);
            q.mul(BOUNDARY_PRESERVE_WEIGHT);
            let (i1, i2) = (d.vi(v1), d.vi(v2));
            d.vquadrics[i1].add(&q);
            d.vquadrics[i2].add(&q);
        }
    }
}

/// `bm_decim_calc_target_co_db`.
fn calc_target_co_db(bm: &BMesh, e: EdgeId, d: &Decim) -> [f64; 3] {
    let [v1, v2] = bm.edge(e).v;
    let q = Quadric::sum(&d.vquadrics[d.vi(v1)], &d.vquadrics[d.vi(v2)]);
    if let Some(co) = q.optimize(OPTIMIZE_EPS) {
        return co;
    }
    let (a, b) = (bm.vert(v1).co, bm.vert(v2).co);
    std::array::from_fn(|k| 0.5 * (f64::from(a[k]) + f64::from(b[k])))
}

/// `bm_decim_calc_target_co_fl`.
fn calc_target_co_fl(bm: &BMesh, e: EdgeId, d: &Decim) -> V3 {
    calc_target_co_db(bm, e, d).map(|x| x as f32)
}

/// `bm_edge_collapse_is_degenerate_flip`.
fn edge_collapse_is_degenerate_flip(bm: &BMesh, e: EdgeId, optimize_co: V3) -> bool {
    for v in bm.edge(e).v {
        for l in bm.vert_loops(v) {
            let ll = bm.lp(l);
            if ll.e != e && bm.lp(ll.prev).e != e {
                let co_prev = bm.vert(bm.lp(ll.prev).v).co;
                let co_next = bm.vert(bm.lp(ll.next).v).co;
                // Line between the two outer verts, re-used for both cross products.
                let vec_other = sub(co_prev, co_next);
                let vec_exist = sub(co_prev, bm.vert(v).co);
                let vec_optim = sub(co_prev, optimize_co);
                let cross_exist = cross(vec_other, vec_exist);
                let cross_optim = cross(vec_other, vec_optim);
                // Avoid normalize.
                if dot(cross_exist, cross_optim)
                    <= (len_squared_v3(cross_exist) + len_squared_v3(cross_optim)) * 0.01
                {
                    return true;
                }
            }
        }
    }
    false
}

/// `bm_decim_build_edge_cost_single__topology` (`squared`: the `_squared_` variant).
fn edge_cost_topology(bm: &BMesh, e: EdgeId, squared: bool) -> f32 {
    let [v1, v2] = bm.edge(e).v;
    let (a, b) = (bm.vert(v1), bm.vert(v2));
    let len = if squared {
        len_squared_v3v3(a.co, b.co)
    } else {
        len_v3v3(a.co, b.co)
    };
    dot(a.no, b.no).abs() / (-len).min(-f32::EPSILON)
}

/// `bm_decim_build_edge_cost_single`.
fn build_edge_cost_single(bm: &BMesh, e: EdgeId, d: &mut Decim) {
    let [v1, v2] = bm.edge(e).v;
    let ei = d.ei(e);
    let clear = |d: &mut Decim| {
        if let Some(n) = d.eheap_table[ei].take() {
            d.eheap.remove(n);
        }
    };
    if let (Some(w1), Some(w2)) = (d.weight(v1), d.weight(v2))
        && (w1 == 0.0 || w2 == 0.0)
    {
        return clear(d);
    }
    // Check we can collapse, some edges we better not touch.
    let l = bm.edge(e).l;
    if bm.edge_is_boundary(e) {
        // Only collapse triangles.
        if bm.face(bm.lp(l.expect("boundary")).f).len != 3 {
            return clear(d);
        }
    } else if bm.edge_is_manifold(e) {
        let la = l.expect("manifold");
        let lb = bm.lp(la).radial_next;
        if !(bm.face(bm.lp(la).f).len == 3 && bm.face(bm.lp(lb).f).len == 3) {
            return clear(d);
        }
    } else {
        return clear(d);
    }

    let mut cost = {
        let optimize_co = calc_target_co_db(bm, e, d);
        let q1 = &d.vquadrics[d.vi(v1)];
        let q2 = &d.vquadrics[d.vi(v2)];
        (q1.evaluate(optimize_co) + q2.evaluate(optimize_co)) as f32
    };
    // NOTE: 'cost' shouldn't be negative but happens sometimes with small values. This can
    // cause faces that make up a flat surface to over-collapse, see #37121.
    cost = cost.abs();

    if cost < TOPOLOGY_FALLBACK_EPS {
        // Subtract the existing cost to further differentiate edges from one another; keep
        // the topology cost below 0.0 so it doesn't interfere with the quadric cost (and these
        // edges are handled first).
        match (d.weight(v1), d.weight(v2)) {
            (Some(w1), Some(w2)) => {
                // With weights the real length is needed to scale them properly.
                let e_weight = w1 + w2;
                cost = edge_cost_topology(bm, e, false) - cost;
                // NOTE: this is rather arbitrary max weight is 2 here, allow for skipping
                // edges 4x the length, based on weights.
                if e_weight != 0.0 {
                    cost *= 1.0 + (e_weight * d.vweight_factor);
                }
            }
            _ => cost = edge_cost_topology(bm, e, true) - cost,
        }
    } else if let (Some(w1), Some(w2)) = (d.weight(v1), d.weight(v2)) {
        let e_weight = 2.0 - (w1 + w2);
        if e_weight != 0.0 {
            cost += len_v3v3(bm.vert(v1).co, bm.vert(v2).co) * (e_weight * d.vweight_factor);
        }
    }

    let mut node = d.eheap_table[ei];
    d.eheap.insert_or_update(&mut node, cost, e);
    d.eheap_table[ei] = node;
}

/// `bm_decim_invalid_edge_cost_single`: back on the heap with an invalid cost, so it may be
/// calculated again if the surrounding geometry changes.
fn invalid_edge_cost_single(e: EdgeId, d: &mut Decim) {
    let ei = d.ei(e);
    debug_assert!(d.eheap_table[ei].is_none());
    d.eheap_table[ei] = Some(d.eheap.insert(COST_INVALID, e));
}

/// `bm_edge_symmetry_map`: for every edge, the index of its mirror across `axis` (or -1).
fn edge_symmetry_map(bm: &BMesh, etable: &[EdgeId], axis: usize, limit: f32) -> Vec<i32> {
    let limit_sq = limit * limit;
    let mut tree = KdTree::new(etable.len());
    let mut map = vec![-1i32; etable.len()];
    let co = |v: VertId| bm.vert(v).co;
    for (i, &e) in etable.iter().enumerate() {
        let [v1, v2] = bm.edge(e).v;
        tree.insert(i as i32, mid(co(v1), co(v2)));
    }
    tree.balance();
    for (i, &e) in etable.iter().enumerate() {
        if map[i] != -1 {
            continue;
        }
        let [v1, v2] = bm.edge(e).v;
        let mut c = mid(co(v1), co(v2));
        c[axis] *= -1.0;
        let mut e_v1_co = co(v1);
        let mut e_v2_co = co(v2);
        e_v1_co[axis] *= -1.0;
        e_v2_co[axis] *= -1.0;
        let e_dir = sub(e_v2_co, e_v1_co);
        let mut found = -1i32;
        // `bm_edge_symmetry_check_cb`: exit on the first hit (the search range is very small).
        tree.range_search_cb(c, limit, |index, _, _| {
            let [o1, o2] = bm.edge(etable[index as usize]).v;
            let e_other_dir = sub(co(o2), co(o1));
            let (a, b) = if dot(e_other_dir, e_dir) > 0.0 {
                (o1, o2)
            } else {
                (o2, o1)
            };
            if len_squared_v3v3(e_v1_co, co(a)) > limit_sq
                || len_squared_v3v3(e_v2_co, co(b)) > limit_sq
            {
                return true;
            }
            found = index;
            false
        });
        if found != -1 {
            map[i] = found;
            map[found as usize] = i as i32;
        }
    }
    map
}

/// `bm_decim_triangulate_begin`: triangulates every face of 4+ corners; the loops of each new
/// edge get the index of the face it cut (every other loop -1). Returns whether anything was
/// cut and the number of new edges.
fn triangulate_begin(bm: &mut BMesh) -> (bool, usize) {
    let mut has_cut = false;
    let mut edges_tri_tot = 0usize;
    // First clear the loop index values.
    let caps = bm.loops.capacity();
    if bm.index.loops.len() < caps {
        bm.index.loops.resize(caps, -1);
    }
    for f in bm.face_ids() {
        for l in bm.face_loops(f) {
            bm.index.loops[l.slot()] = -1;
        }
    }
    bm.index.dirty |= ElemTypes::LOOP;

    let mut faces_double: Vec<FaceId> = Vec::new();
    // Adding new faces while looping over faces is normally best avoided, but any face
    // touched twice is already a triangle (`BM_ITER_MESH` visits the new ones too).
    let mut s = 0u32;
    while (s as usize) < bm.faces.capacity() {
        if bm.faces.contains(s) && bm.face(FaceId(s)).len > 3 {
            frap_math::budget::check();
            let f = FaceId(s);
            // `bm_face_triangulate`; the face's identity stands for `BM_elem_index_get(f)`.
            let f_index = s as i32;
            let t = face_triangulate(bm, f, QuadMethod::Beauty, NgonMethod::Beauty, false);
            let caps = bm.loops.capacity();
            if bm.index.loops.len() < caps {
                bm.index.loops.resize(caps, -1);
            }
            for &e in &t.edges_new {
                let Some(l_first) = bm.edge(e).l else {
                    continue;
                };
                let mut l = l_first;
                loop {
                    bm.index.loops[l.slot()] = f_index;
                    has_cut = true;
                    l = bm.lp(l).radial_next;
                    if l == l_first {
                        break;
                    }
                }
            }
            for &fnew in &t.faces_new {
                bm.face_normal_update(fnew);
            }
            edges_tri_tot += t.edges_new.len();
            // `r_faces_double` is a linked list: prepended, killed from its head.
            for &fd in t.faces_double.iter().rev() {
                faces_double.insert(0, fd);
            }
        }
        s += 1;
    }
    for f in faces_double {
        if bm.faces.contains(f.0) {
            bm.face_kill(f);
        }
    }
    if has_cut {
        bm.index.dirty |= ElemTypes::EDGE | ElemTypes::FACE;
        bm.elem_index_ensure(ElemTypes::EDGE | ElemTypes::FACE);
    }
    (has_cut, edges_tri_tot)
}

/// The loop index set by [`triangulate_begin`] (`BM_elem_index_get(l)`).
fn loop_index(bm: &BMesh, l: LoopId) -> i32 {
    bm.index.loops.get(l.slot()).copied().unwrap_or(-1)
}

/// `bm_decim_triangulate_end`: joins triangle pairs that came from one face back.
fn triangulate_end(bm: &mut BMesh, edges_tri_tot: usize) {
    // Collect before merging: the loop indices are lost for n-gons.
    let mut edges_tri: Vec<EdgeId> = Vec::with_capacity(edges_tri_tot.min(bm.edges_num()));
    let cap = edges_tri_tot.min(bm.edges_num());
    for e in bm.edge_ids() {
        let Some((l_a, l_b)) = edge_loop_pair(bm, e) else {
            continue;
        };
        let l_a_index = loop_index(bm, l_a);
        if l_a_index == -1 || l_a_index != loop_index(bm, l_b) {
            continue;
        }
        // If the vertices are equal, the faces have become flipped.
        if bm.lp(l_a).v == bm.lp(l_b).v {
            continue;
        }
        // Check we are not making a degenerate quad.
        let can_loop_merge = |l: LoopId| {
            loop_is_manifold(bm, l)
                && bm.lp(l).v != bm.lp(bm.lp(l).radial_next).v
                && l_a_index == loop_index(bm, l)
                && l_a_index == loop_index(bm, bm.lp(l).radial_next)
        };
        let (la, lb) = (bm.lp(l_a), bm.lp(l_b));
        if bm.face(la.f).len == 3
            && bm.face(lb.f).len == 3
            && !can_loop_merge(la.next)
            && !can_loop_merge(la.prev)
            && !can_loop_merge(lb.next)
            && !can_loop_merge(lb.prev)
        {
            let [e1, e2] = bm.edge(e).v;
            let other = |l: &crate::BMLoop| {
                let nv = bm.lp(l.next).v;
                if bm.vert_in_edge(e, nv) {
                    bm.lp(l.prev).v
                } else {
                    nv
                }
            };
            let vquad = [e1, other(la), e2, other(lb)];
            let c = vquad.map(|v| bm.vert(v).co);
            if !is_quad_convex_v3(c[0], c[1], c[2], c[3]) {
                continue;
            }
        }
        // Highly unlikely to fail, but prevents possible double-ups.
        if edges_tri.len() < cap {
            edges_tri.push(e);
        }
    }

    for e in edges_tri {
        if !bm.edges.contains(e.0) {
            continue;
        }
        let Some((l_a, l_b)) = edge_loop_pair(bm, e) else {
            continue;
        };
        let f_array = [bm.lp(l_a).f, bm.lp(l_b).f];
        let (f_join, f_double) = bm.faces_join(&f_array, false, true);
        // Duplicates are not allowed in the resulting mesh: remove them (#159549).
        if f_double.is_some()
            && let Some(fj) = f_join
        {
            bm.face_kill(fj);
        }
        if bm.edges.contains(e.0) && bm.edge(e).l.is_none() {
            bm.edge_kill(e);
        }
    }
}

/// `bm_edge_collapse_loop_customdata`: `l` defines the vertex to collapse into.
fn edge_collapse_loop_customdata(
    bm: &mut BMesh,
    l: LoopId,
    v_clear: VertId,
    v_other: VertId,
    customdata_fac: f32,
) {
    // These don't need to be updated: they are removed when the edge collapses.
    let is_manifold = bm.edge_is_manifold(bm.lp(l).e);
    let (l_clear, l_other) = if bm.lp(l).v == v_clear {
        (l, bm.lp(l).next)
    } else {
        (bm.lp(l).next, l)
    };
    debug_assert_eq!(bm.lp(l_clear).v, v_clear);
    debug_assert_eq!(bm.lp(l_other).v, v_other);
    let layers: Vec<usize> = (0..bm.ldata.layers.len())
        .filter(|&i| has_math(&bm.ldata.layers[i].data))
        .collect();

    // Now we have both corners of the face `l->f`.
    for side in 0..2 {
        let f_exit = is_manifold.then(|| bm.lp(bm.lp(l).radial_next).f);
        let mut e_prev = bm.lp(l).e;
        let (l_first, src, w) = if side == 0 {
            (
                l_clear,
                [l_clear.slot(), l_other.slot()],
                [customdata_fac, 1.0 - customdata_fac],
            )
        } else {
            (
                l_other,
                [l_other.slot(), l_clear.slot()],
                [1.0 - customdata_fac, customdata_fac],
            )
        };
        let mut l_iter = l_first;
        // Walk around the fan using `e_prev`.
        while let Some(li) = bm.vert_step_fan_loop(l_iter, &mut e_prev) {
            if li == l_first {
                break;
            }
            l_iter = li;
            // Quit once we hit the opposite face, if we have one.
            if f_exit == Some(bm.lp(l_iter).f) {
                break;
            }
            for &i in &layers {
                let data = &mut bm.ldata.layers[i].data;
                // Detect seams.
                if data_equals(data, src[0], l_iter.slot()) {
                    interp_n(data, src, w, l_iter.slot());
                }
            }
        }
    }
}

/// `bm_edge_tag_enable`.
fn edge_tag_set(bm: &mut BMesh, e: EdgeId, on: bool) {
    let [v1, v2] = bm.edge(e).v;
    bm.vert_mut(v1).flag.set(ElemFlag::TAG, on);
    bm.vert_mut(v2).flag.set(ElemFlag::TAG, on);
    if let Some(l) = bm.edge(e).l {
        let f = bm.lp(l).f;
        bm.face_mut(f).flag.set(ElemFlag::TAG, on);
        let r = bm.lp(l).radial_next;
        if r != l {
            let f = bm.lp(r).f;
            bm.face_mut(f).flag.set(ElemFlag::TAG, on);
        }
    }
}

/// `bm_edge_tag_test`: is the edge or one of its faces tagged?
fn edge_tag_test(bm: &BMesh, e: EdgeId) -> bool {
    let [v1, v2] = bm.edge(e).v;
    if bm.vert(v1).flag.contains(ElemFlag::TAG) || bm.vert(v2).flag.contains(ElemFlag::TAG) {
        return true;
    }
    let Some(l) = bm.edge(e).l else {
        return false;
    };
    if bm.face(bm.lp(l).f).flag.contains(ElemFlag::TAG) {
        return true;
    }
    let r = bm.lp(l).radial_next;
    r != l && bm.face(bm.lp(r).f).flag.contains(ElemFlag::TAG)
}

/// `bm_edge_is_manifold_or_boundary` (takes the edge's loop).
fn edge_is_manifold_or_boundary(bm: &BMesh, l: Option<LoopId>) -> bool {
    l.is_some_and(|l| bm.lp(bm.lp(l).radial_next).radial_next == l)
}

/// `bm_edge_collapse_is_degenerate_topology`: whether the faces or edges of the two vertices
/// overlap (excluding the two faces of `e_first` and `e_first` itself).
fn edge_collapse_is_degenerate_topology(bm: &mut BMesh, e_first: EdgeId) -> bool {
    let [v1, v2] = bm.edge(e_first).v;
    // Clear the flags on both disks.
    for v in [v1, v2] {
        let mut e_iter = e_first;
        loop {
            if !edge_is_manifold_or_boundary(bm, bm.edge(e_iter).l) {
                return true;
            }
            edge_tag_set(bm, e_iter, false);
            e_iter = bm.disk_edge_next(e_iter, v);
            if e_iter == e_first {
                break;
            }
        }
    }
    // Now enable one side...
    let mut e_iter = e_first;
    loop {
        edge_tag_set(bm, e_iter, true);
        e_iter = bm.disk_edge_next(e_iter, v1);
        if e_iter == e_first {
            break;
        }
    }
    // ... except for the edge we will collapse, we know that's shared (each face is a
    // triangle).
    let untag_tri = |bm: &mut BMesh, l_face: LoopId| {
        let f = bm.lp(l_face).f;
        debug_assert_eq!(bm.face(f).len, 3);
        bm.face_mut(f).flag.remove(ElemFlag::TAG);
        let b = bm.lp(l_face).next;
        let c = bm.lp(b).next;
        for l in [l_face, b, c] {
            let v = bm.lp(l).v;
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
    };
    let l_radial = bm.edge(e_first).l.expect("manifold or boundary");
    untag_tri(bm, l_radial);
    let l_face = bm.lp(l_radial).radial_next;
    if l_face != l_radial {
        untag_tri(bm, l_face);
    }
    // And check for overlap.
    let mut e_iter = e_first;
    loop {
        if edge_tag_test(bm, e_iter) {
            return true;
        }
        e_iter = bm.disk_edge_next(e_iter, v2);
        if e_iter == e_first {
            break;
        }
    }
    false
}

/// `bm_edge_collapse`: collapses an edge used by one or two triangles, killing `v_clear`;
/// returns the indices of the other edges removed (`r_e_clear_other`, -1 when absent).
fn edge_collapse(
    bm: &mut BMesh,
    d: &mut Decim,
    e_clear: EdgeId,
    v_clear: VertId,
    customdata_fac: f32,
) -> Option<[i32; 2]> {
    let v_other = bm.edge_other_vert(e_clear, v_clear);
    // `e_*_other[0]` uses `v_clear`.
    let others = |bm: &BMesh, l: LoopId| {
        let (p, n) = (bm.lp(bm.lp(l).prev).e, bm.lp(bm.lp(l).next).e);
        if bm.vert_in_edge(p, v_clear) {
            [p, n]
        } else {
            [n, p]
        }
    };
    let cd = d.customdata;
    if bm.edge_is_manifold(e_clear) {
        let (l_a, l_b) = edge_loop_pair(bm, e_clear).expect("manifold");
        debug_assert!(bm.face(bm.lp(l_a).f).len == 3 && bm.face(bm.lp(l_b).f).len == 3);
        let e_a_other = others(bm, l_a);
        let e_b_other = others(bm, l_b);
        // Not totally common but we want to avoid.
        if e_b_other.contains(&e_a_other[0]) || e_b_other.contains(&e_a_other[1]) {
            return None;
        }
        let r = [d.ei(e_a_other[0]) as i32, d.ei(e_b_other[0]) as i32];
        // Before killing, do the custom data.
        if cd.vert {
            bm.data_interp_from_verts(v_other, v_clear, v_other, customdata_fac);
        }
        if cd.edge {
            bm.data_interp_from_edges(e_a_other[1], e_a_other[0], e_a_other[1], customdata_fac);
            bm.data_interp_from_edges(e_b_other[1], e_b_other[0], e_b_other[1], customdata_fac);
        }
        if cd.looop {
            let l = bm.edge(e_clear).l.expect("manifold");
            edge_collapse_loop_customdata(bm, l, v_clear, v_other, customdata_fac);
            let l = bm.lp(l).radial_next;
            edge_collapse_loop_customdata(bm, l, v_clear, v_other, customdata_fac);
        }
        bm.edge_kill(e_clear);
        let fl = bm.vert(v_clear).flag;
        bm.vert_mut(v_other).flag |= fl;
        bm.vert_splice(v_other, v_clear);
        let fl = bm.edge(e_a_other[0]).flag;
        bm.edge_mut(e_a_other[1]).flag |= fl;
        let fl = bm.edge(e_b_other[0]).flag;
        bm.edge_mut(e_b_other[1]).flag |= fl;
        bm.edge_splice(e_a_other[1], e_a_other[0]);
        bm.edge_splice(e_b_other[1], e_b_other[0]);
        // Update the mirror map.
        if let Some(map) = &mut d.edge_symmetry_map {
            for (k, keep) in [(0, e_a_other[1]), (1, e_b_other[1])] {
                let m = map[r[k] as usize];
                if m != -1 {
                    map[m as usize] = d.eidx[keep.slot()] as i32;
                }
            }
        }
        return Some(r);
    }
    if bm.edge_is_boundary(e_clear) {
        // Same as above but only one triangle.
        let l_a = bm.edge(e_clear).l.expect("boundary");
        debug_assert_eq!(bm.face(bm.lp(l_a).f).len, 3);
        let e_a_other = others(bm, l_a);
        let r = [d.ei(e_a_other[0]) as i32, -1];
        if cd.vert {
            bm.data_interp_from_verts(v_other, v_clear, v_other, customdata_fac);
        }
        if cd.edge {
            bm.data_interp_from_edges(e_a_other[1], e_a_other[0], e_a_other[1], customdata_fac);
        }
        if cd.looop {
            edge_collapse_loop_customdata(bm, l_a, v_clear, v_other, customdata_fac);
        }
        bm.edge_kill(e_clear);
        let fl = bm.vert(v_clear).flag;
        bm.vert_mut(v_other).flag |= fl;
        bm.vert_splice(v_other, v_clear);
        let fl = bm.edge(e_a_other[0]).flag;
        bm.edge_mut(e_a_other[1]).flag |= fl;
        bm.edge_splice(e_a_other[1], e_a_other[0]);
        if let Some(map) = &mut d.edge_symmetry_map {
            let m = map[r[0] as usize];
            if m != -1 {
                map[m as usize] = d.eidx[e_a_other[1].slot()] as i32;
            }
        }
        return Some(r);
    }
    None
}

/// `bm_decim_edge_collapse`: collapses `e` into its first vertex (removing `e->v2`). With
/// `optimize_co_calc` the degenerate checks run and `optimize_co` is computed; otherwise the
/// caller did both. Returns whether the edge collapsed.
fn decim_edge_collapse(
    bm: &mut BMesh,
    d: &mut Decim,
    e: EdgeId,
    optimize_co: &mut V3,
    optimize_co_calc: bool,
) -> bool {
    let [v_other, v_clear] = bm.edge(e).v;
    let v_other_index = d.vi(v_other);
    // The vertex is removed, so only store the index.
    let v_clear_index = d.vi(v_clear);
    let v_clear_no = bm.vert(v_clear).no;

    if optimize_co_calc {
        // Disallow collapsing which results in degenerate cases.
        if edge_collapse_is_degenerate_topology(bm, e) {
            // Add back with a high cost.
            invalid_edge_cost_single(e, d);
            return false;
        }
        *optimize_co = calc_target_co_fl(bm, e, d);
        // Check if this would result in an overlapping face.
        if edge_collapse_is_degenerate_flip(bm, e, *optimize_co) {
            invalid_edge_cost_single(e, d);
            return false;
        }
    }

    // Used for the custom-data merging.
    let (c1, c2) = (bm.vert(v_other).co, bm.vert(v_clear).co);
    let customdata_fac = if !compare_v3v3(c1, c2, f32::EPSILON) {
        line_point_factor_v3(*optimize_co, c1, c2)
    } else {
        // Avoid divide by zero.
        0.5
    };

    let Some(e_clear_other) = edge_collapse(bm, d, e, v_clear, customdata_fac) else {
        // Add back with a high cost.
        invalid_edge_cost_single(e, d);
        return false;
    };

    // Update the collapse info.
    if let Some(w) = &mut d.vweights {
        // `interpf(target, origin, fac)`.
        let v_other_weight = (customdata_fac * w[v_other_index]
            + (1.0 - customdata_fac) * w[v_clear_index])
            .clamp(0.0, 1.0);
        w[v_other_index] = v_other_weight;
    }

    bm.vert_mut(v_other).co = *optimize_co;

    // Remove the killed edges from the heap.
    for ec in e_clear_other {
        if ec != -1
            && let Some(n) = d.eheap_table[ec as usize].take()
        {
            d.eheap.remove(n);
        }
    }

    // Update the vertex quadric: add the killed vertex's to the kept one.
    let q = d.vquadrics[v_clear_index];
    d.vquadrics[v_other_index].add(&q);

    // Face normals are not used by the progressive updates; the vertex normal is interpolated
    // (`USE_VERT_NORMAL_INTERP`, flipped faces don't mess it up).
    {
        let no = bm.vert(v_other).no;
        let t = customdata_fac;
        let s = 1.0 - t;
        let n = std::array::from_fn(|k| s * no[k] + t * v_clear_no[k]);
        bm.vert_mut(v_other).no = normalize_v3(n).0;
    }

    // Update the error costs and the heap.
    if let Some(e_first) = bm.vert(v_other).e {
        let mut e_iter = e_first;
        loop {
            build_edge_cost_single(bm, e_iter, d);
            e_iter = bm.disk_edge_next(e_iter, v_other);
            if e_iter == e_first {
                break;
            }
        }
    }

    // Update the edges around the vertex face fan: surrounding faces may have been set to
    // COST_INVALID because of a face overlap that no longer occurs.
    for l in bm.vert_loops(v_other) {
        let ll = bm.lp(l);
        if bm.face(ll.f).len == 3 {
            let e_outer = if bm.vert_in_edge(bm.lp(ll.prev).e, ll.v) {
                bm.lp(ll.next).e
            } else {
                bm.lp(ll.prev).e
            };
            debug_assert!(!bm.vert_in_edge(e_outer, ll.v));
            build_edge_cost_single(bm, e_outer, d);
        }
    }
    true
}

/// `BM_edge_share_vert_check`.
fn edge_share_vert(bm: &BMesh, a: EdgeId, b: EdgeId) -> bool {
    let [a1, a2] = bm.edge(a).v;
    let [b1, b2] = bm.edge(b).v;
    a1 == b1 || a1 == b2 || a2 == b1 || a2 == b2
}

/// `BM_mesh_decimate_collapse`.
pub fn decimate_collapse(bm: &mut BMesh, i: &DecimateCollapseIn) {
    // Vertex indices must be valid (`BLI_assert` in Blender): iteration order.
    let mut vidx = vec![u32::MAX; bm.verts.capacity()];
    for (k, v) in bm.vert_ids().into_iter().enumerate() {
        vidx[v.slot()] = k as u32;
    }

    // Temporarily convert quads and n-gons to triangles.
    let (use_triangulate, edges_tri_tot) = triangulate_begin(bm);

    // Edge indices index `eheap_table`: dense, in iteration order.
    let etable = bm.edge_ids();
    let mut eidx = vec![u32::MAX; bm.edges.capacity()];
    for (k, e) in etable.iter().enumerate() {
        eidx[e.slot()] = k as u32;
    }

    let mut d = Decim {
        vidx,
        eidx,
        vquadrics: vec![Quadric::default(); bm.verts_num()],
        vweights: i.vweights.clone(),
        vweight_factor: i.vweight_factor,
        eheap: Heap::with_capacity(etable.len()),
        eheap_table: vec![None; etable.len()],
        edge_symmetry_map: None,
        customdata: CdUse::default(),
    };
    if let Some(w) = &mut d.vweights {
        w.resize(d.vquadrics.len(), 0.0);
    }

    // Build the initial edge collapse cost data.
    build_quadrics(bm, &mut d);
    for &e in &etable {
        build_edge_cost_single(bm, e, &mut d);
    }

    let face_tot_target = (bm.faces_num() as f32 * i.factor) as i32;
    bm.index.dirty = ElemTypes::all();

    let use_symmetry = i.symmetry_axis != -1;
    if use_symmetry {
        let axis = i.symmetry_axis.clamp(0, 2) as usize;
        d.edge_symmetry_map = Some(edge_symmetry_map(bm, &etable, axis, i.symmetry_eps));
    }

    // Custom data: vertex and edge layers that interpolate, corner layers with math.
    d.customdata = CdUse {
        vert: bm.vdata.layers.iter().any(|l| has_interp(&l.data))
            || bm.vdata.deform.is_some()
            || !bm.shape_layers.is_empty(),
        edge: bm.edata.layers.iter().any(|l| has_interp(&l.data)),
        looop: bm.ldata.layers.iter().any(|l| has_math(&l.data)),
    };

    let go_on = |bm: &BMesh, d: &Decim| {
        (bm.faces_num() as i64 > i64::from(face_tot_target))
            && !d.eheap.is_empty()
            && d.eheap.top_value() != COST_INVALID
    };

    if !use_symmetry {
        // Simple non-mirror case.
        while go_on(bm, &d) {
            frap_math::budget::check();
            let e = d.eheap.pop_min();
            // Under normal conditions won't be accessed again.
            let ei = d.ei(e);
            d.eheap_table[ei] = None;
            let mut optimize_co = [0.0f32; 3];
            decim_edge_collapse(bm, &mut d, e, &mut optimize_co, true);
        }
    } else {
        let axis = i.symmetry_axis.clamp(0, 2) as usize;
        'pop: while go_on(bm, &d) {
            frap_math::budget::check();
            // - `eheap_table[e_index_mirr]` is only removed from the heap at the last moment
            //   since it's possible (in theory) for collapsing `e` to remove `e_mirr`.
            // - Edges sharing a vertex are ignored, so the pivot vertex isn't moved to one
            //   side.
            let e = d.eheap.pop_min();
            let e_index = d.ei(e);
            let e_index_mirr = d.edge_symmetry_map.as_ref().expect("symmetry map")[e_index];
            let mut e_mirr: Option<EdgeId> = None;
            let mut e_invalidate = 0u8;
            d.eheap_table[e_index] = None;

            'step: {
                if e_index_mirr != -1 {
                    let mi = e_index_mirr as usize;
                    if mi == e_index {
                        // Pass.
                    } else if let Some(n) = d.eheap_table[mi] {
                        let em = d.eheap.node_ptr(n);
                        e_mirr = Some(em);
                        // For now ignore edges with a shared vertex: permanently, otherwise
                        // they would be re-evaluated and collapsed again and again.
                        if edge_share_vert(bm, e, em) {
                            break 'step;
                        }
                    } else {
                        // The mirror edge can't be operated on (asymmetrical meshes).
                        e_invalidate |= 1;
                        break 'step;
                    }
                }

                // Run both before checking (they invalidate the surrounding geometry).
                let ok_a = !edge_collapse_is_degenerate_topology(bm, e);
                let ok_b = e_mirr.is_none_or(|em| !edge_collapse_is_degenerate_topology(bm, em));
                // Disallow collapsing which results in degenerate cases.
                if !ok_a || !ok_b {
                    e_invalidate |= 1 | if e_mirr.is_some() { 2 } else { 0 };
                    break 'step;
                }
                let mut optimize_co = calc_target_co_fl(bm, e, &d);
                if e_index_mirr == e_index as i32 {
                    optimize_co[axis] = 0.0;
                }
                // Check if this would result in an overlapping face.
                if edge_collapse_is_degenerate_flip(bm, e, optimize_co) {
                    e_invalidate |= 1 | if e_mirr.is_some() { 2 } else { 0 };
                    break 'step;
                }

                if decim_edge_collapse(bm, &mut d, e, &mut optimize_co, false) {
                    if let Some(em) = e_mirr {
                        let mi = e_index_mirr as usize;
                        if let Some(n) = d.eheap_table[mi].take() {
                            debug_assert_ne!(mi, e_index);
                            d.eheap.remove(n);
                            optimize_co[axis] *= -1.0;
                            decim_edge_collapse(bm, &mut d, em, &mut optimize_co, false);
                        }
                    }
                } else if e_mirr.is_some() && d.eheap_table[e_index_mirr as usize].is_some() {
                    e_invalidate |= 2;
                    break 'step;
                }
                debug_assert_eq!(e_invalidate, 0);
                continue 'pop;
            }

            // `invalidate:`
            if e_invalidate & 1 != 0 {
                invalid_edge_cost_single(e, &mut d);
            }
            if e_invalidate & 2 != 0 {
                let mi = e_index_mirr as usize;
                if let Some(n) = d.eheap_table[mi].take() {
                    d.eheap.remove(n);
                }
                invalid_edge_cost_single(e_mirr.expect("mirror edge"), &mut d);
            }
        }
    }

    if !i.do_triangulate && use_triangulate {
        // It's possible we only had triangles: skip this step in that case.
        triangulate_end(bm, edges_tri_tot);
    }
    bm.index.dirty = ElemTypes::all();
}

#[cfg(test)]
mod tests;
