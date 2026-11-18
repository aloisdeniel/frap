// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.subdivide_edgering` (Blender 5.2.4 `bmo_subdivide_edgering_exec`), implemented
//! by WP 2.6 (Subdivide and loops).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_subdivide_edgering.cc (Blender
//! 5.2.4, GPL-2.0-or-later); the edge-loop store from bmesh/intern/bmesh_edgeloop.cc.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.subdivide_edgering(bmesh, edges=[], interp_mode='LINEAR', smooth=0.0, cuts=0, profile_shape='SMOOTH', profile_shape_factor=0.0)`.
//!
//! The input edges form a ring between two (or more) edge loops, the *rims*: the triangles and
//! quads using two ring edges are the faces between them (`FACE_OUT`), their other edges the
//! rim edges. The rim loops are found in edge iteration order and paired (two rims: one pair;
//! more: the pairs connected by ring edges, in discovery order).
//!
//! **Create/kill order** per pair: the ring edges of the first rim's vertices (disk order) are
//! pushed on a stack, the faces between on another; the ring edges are popped (last first)
//! and each split `cuts` times (`BM_edge_split_n`: one vertex then one edge per cut, from its
//! second vertex); then the faces are popped and sliced `cuts` times from their first rim
//! corner (`BM_face_split`: one edge and one face per slice). Finally the new vertices are
//! moved along the interpolation (`LINEAR`, `PATH`: a Bezier path between the rim centres,
//! `SURFACE`: a Bezier per ring edge from the surface tangents), shaped by the profile.
//! Nothing is killed.
//!
//! Errors (`BMO_ERROR_CANCEL`, reported as INFO by the edit-mode operator): `No edge rings
//! found`, `Edge-ring pair isn't connected`, `Edge-rings are not connected`.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE` (plus
//! `UNTAN_MULTIRES`).

mod edgeloop;

use rustc_hash::FxHashMap;

use crate::ops::subdivide_edges::Falloff;
use crate::ops::subdivide_edges::math::{self, V3};
use crate::types::{ElemFlag, LoopId, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, VertId};
use edgeloop::{EdgeLoop, mesh_edgeloops_find};

/// Input slots of `subdivide_edgering` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubdivideEdgeringIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `interp_mode` (enum identifier).
    pub interp_mode: &'static str,
    /// `smooth`.
    pub smooth: f32,
    /// `cuts`.
    pub cuts: i32,
    /// `profile_shape` (enum identifier).
    pub profile_shape: &'static str,
    /// `profile_shape_factor`.
    pub profile_shape_factor: f32,
}

impl Default for SubdivideEdgeringIn {
    fn default() -> Self {
        SubdivideEdgeringIn {
            edges: Vec::new(),
            interp_mode: "LINEAR",
            smooth: 0.0,
            cuts: 0,
            profile_shape: "SMOOTH",
            profile_shape_factor: 0.0,
        }
    }
}

/// Output slots of `subdivide_edgering`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SubdivideEdgeringOut {
    /// `faces.out` as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the faces between the rims (and their slices), iteration order; empty
    /// when nothing changed.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_subdivide_edgering_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const VERT_SHARED: u16 = 1;

const EDGE_RING: u16 = 1;
const EDGE_RIM: u16 = 2;
const EDGE_IN_STACK: u16 = 4;

const FACE_OUT: u16 = 1;
const FACE_SHARED: u16 = 2;
const FACE_IN_STACK: u16 = 4;

/// `SUBD_RING_INTERP_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Interp {
    Linear,
    Path,
    Surface,
}

fn tag(bm: &BMesh, v: VertId) -> bool {
    bm.vert(v).flag.contains(ElemFlag::TAG)
}

fn set_tag(bm: &mut BMesh, el: &EdgeLoop, value: bool) {
    for &v in &el.verts {
        bm.vert_mut(v).flag.set(ElemFlag::TAG, value);
    }
}

/// `bezier_handle_calc_length_v3`.
fn bezier_handle_calc_length_v3(co_a: V3, no_a: V3, co_b: V3, no_b: V3) -> f32 {
    let dot = math::dot(no_a, no_b);
    // Gives the closest approximation of a circle with two parallel handles.
    let mut fac = 1.333333f32;
    if dot < 0.0 {
        // Scale down to 0.666 if they point directly at each other.
        let t = 1.0 + dot;
        fac = (fac * t) + (0.75 * (1.0 - t));
    }
    // 2D length projected on the plane of the normals.
    let mut co_a_ofs = math::cross(no_a, no_b);
    if math::len_squared_v3(co_a_ofs) > f32::EPSILON {
        co_a_ofs = math::add(co_a_ofs, co_a);
        co_a_ofs = math::closest_to_line_v3(co_a_ofs, co_b, co_a);
    } else {
        co_a_ofs = co_a;
    }
    let len = math::len_v3v3(co_a_ofs, co_b);
    (len * 0.5) * fac
}

/// `bm_vert_is_tag_edge_connect`: a ring edge of `v` leads to a tagged vertex.
fn vert_is_tag_edge_connect(bm: &BMesh, v: VertId) -> bool {
    bm.vert_edges(v)
        .into_iter()
        .any(|e| bm.oflag_test(e, EDGE_RING) && tag(bm, bm.edge_other_vert(e, v)))
}

/// `bm_edgeloop_check_overlap_all`: every vertex of each loop has a ring edge to the other.
fn edgeloop_check_overlap_all(bm: &mut BMesh, a: &EdgeLoop, b: &EdgeLoop) -> bool {
    let mut ok = true;
    set_tag(bm, a, false);
    set_tag(bm, b, true);
    if !a.verts.iter().all(|&v| vert_is_tag_edge_connect(bm, v)) {
        ok = false;
    } else {
        set_tag(bm, a, true);
        set_tag(bm, b, false);
        if !b.verts.iter().all(|&v| vert_is_tag_edge_connect(bm, v)) {
            ok = false;
        }
    }
    set_tag(bm, a, false);
    set_tag(bm, b, false);
    ok
}

/// `bm_edgering_pair_calc`: the pairs of rims connected by ring edges, in discovery order
/// (each pair once, ordered by rim index: Blender orders them by store address, which follows
/// the allocation order).
fn edgering_pair_calc(bm: &BMesh, rims: &[EdgeLoop]) -> Vec<(usize, usize)> {
    let mut vert_rim: FxHashMap<VertId, usize> = FxHashMap::default();
    for (i, el) in rims.iter().enumerate() {
        for &v in &el.verts {
            vert_rim.entry(v).or_insert(i);
        }
    }
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (i, el) in rims.iter().enumerate() {
        let v = el.verts[0];
        for e in bm.vert_edges(v) {
            if bm.oflag_test(e, EDGE_RING) {
                let v_other = bm.edge_other_vert(e, v);
                // A ring edge back into the same rim would pair the rim with itself (Blender
                // then subdivides a degenerate pair); frap skips it.
                if let Some(&j) = vert_rim.get(&v_other)
                    && j != i
                {
                    let p = (i.min(j), i.max(j));
                    if !pairs.contains(&p) {
                        pairs.push(p);
                    }
                }
            }
        }
    }
    pairs
}

/// `bm_edge_subdiv_as_loop`: cuts `e` and returns the ring loop from the other end to `v_a`.
fn edge_subdiv_as_loop(bm: &mut BMesh, e: EdgeId, v_a: VertId, cuts: usize) -> EdgeLoop {
    let v_b = bm.edge_other_vert(e, v_a);
    let splits = bm.edge_split_n(e, cuts);
    let v1 = bm.edge(e).v[0];
    let mut verts = Vec::with_capacity(cuts + 2);
    if v_a == v1 {
        verts.push(v_a);
        verts.extend(splits);
        verts.push(v_b);
    } else {
        verts.push(v_b);
        verts.extend(splits);
        verts.push(v_a);
    }
    let mut el = EdgeLoop::from_verts(verts, false);
    if v_a == v1 {
        el.flip();
    }
    el
}

/// `BM_edge_calc_face_tangent(e, l)`.
fn edge_calc_face_tangent(bm: &BMesh, l: LoopId) -> V3 {
    let ll = bm.lp(l);
    let v1 = bm.vert(ll.v).co;
    let v2 = bm.vert(bm.lp(ll.next).v).co;
    let tvec = math::sub(v1, v2);
    math::normalize_v3(math::cross(tvec, bm.face(ll.f).no)).0
}

/// `bm_vert_calc_surface_tangent`: the spline handle direction at `v`, pointing towards the
/// faces between the loops (`FACE_SHARED`).
fn vert_calc_surface_tangent(bm: &BMesh, v: VertId) -> V3 {
    let (mut found_outer, mut found_inner, mut found_outer_tag) = (false, false, false);
    let mut no_outer = [0.0f32; 3];
    let mut no_inner = [0.0f32; 3];
    for e in bm.vert_edges(v) {
        if bm.edge_is_wire(e) {
            // Pass: this may confuse things.
        } else if bm.oflag_test(e, EDGE_RIM) {
            for l in bm.edge_loops(e) {
                let no = edge_calc_face_tangent(bm, l);
                let f = bm.lp(l).f;
                if bm.oflag_test(f, FACE_SHARED) {
                    no_inner = math::add(no_inner, no);
                    found_inner = true;
                } else {
                    no_outer = math::add(no_outer, no);
                    found_outer = true;
                    if bm.oflag_test(f, FACE_OUT) {
                        found_outer_tag = true;
                    }
                }
            }
        }
    }
    if found_inner && found_outer_tag {
        // Blend between the two.
        let no_outer = math::normalize_v3(math::negate(no_outer)).0;
        let no_inner = math::normalize_v3(no_inner).0;
        math::normalize_v3(math::add(no_outer, no_inner)).0
    } else if found_outer {
        math::normalize_v3(math::negate(no_outer)).0
    } else {
        math::normalize_v3(no_inner).0
    }
}

/// `LoopPairStore`: the spline handle directions of both loops (`SURFACE` mode only).
#[derive(Default)]
struct LoopPairStore {
    nors_a: FxHashMap<VertId, V3>,
    nors_b: FxHashMap<VertId, V3>,
}

/// `bm_faces_share_tag_flush`: faces of the loop edges whose vertices are all shared.
fn faces_share_tag_flush(bm: &mut BMesh, edges: &[EdgeId]) {
    for &e in edges {
        for l in bm.edge_loops(e) {
            let f = bm.lp(l).f;
            if !bm.oflag_test(f, FACE_SHARED)
                && bm
                    .face_verts(f)
                    .into_iter()
                    .all(|v| bm.oflag_test(v, VERT_SHARED))
            {
                bm.oflag_enable(f, FACE_SHARED);
            }
        }
    }
}

/// `bm_faces_share_tag_clear`.
fn faces_share_tag_clear(bm: &mut BMesh, edges: &[EdgeId]) {
    for &e in edges {
        for l in bm.edge_loops(e) {
            let f = bm.lp(l).f;
            bm.oflag_disable(f, FACE_SHARED);
        }
    }
}

/// `bm_edgering_pair_store_create`.
fn edgering_pair_store_create(bm: &mut BMesh, a: &EdgeLoop, b: &EdgeLoop, interp: Interp) -> LoopPairStore {
    let mut lpair = LoopPairStore::default();
    if interp != Interp::Surface {
        return lpair;
    }
    let e_arr_a = a.edges(bm);
    let e_arr_b = b.edges(bm);
    for el in [a, b] {
        for &v in &el.verts {
            bm.oflag_enable(v, VERT_SHARED);
        }
    }
    faces_share_tag_flush(bm, &e_arr_a);
    faces_share_tag_flush(bm, &e_arr_b);
    for &v in &a.verts {
        let no = vert_calc_surface_tangent(bm, v);
        lpair.nors_a.insert(v, no);
    }
    for &v in &b.verts {
        let no = vert_calc_surface_tangent(bm, v);
        lpair.nors_b.insert(v, no);
    }
    for el in [a, b] {
        for &v in &el.verts {
            bm.oflag_disable(v, VERT_SHARED);
        }
    }
    faces_share_tag_clear(bm, &e_arr_a);
    faces_share_tag_clear(bm, &e_arr_b);
    lpair
}

/// `bm_edgering_pair_interpolate`: places the new vertices of the ring loops.
#[allow(clippy::too_many_arguments)]
fn edgering_pair_interpolate(
    bm: &mut BMesh,
    lpair: &LoopPairStore,
    a: &mut EdgeLoop,
    b: &mut EdgeLoop,
    rings: &[EdgeLoop],
    interp: Interp,
    cuts: usize,
    smooth: f32,
    falloff_cache: Option<&[f32]>,
) {
    let resolu = cuts + 2;
    a.calc_center(bm);
    b.calc_center(bm);
    let mut is_a_no_valid = a.calc_normal(bm);
    let mut is_b_no_valid = b.calc_normal(bm);
    let a_co = a.co;
    let b_co = b.co;

    // The normals need to face each other; both point the same way, so one is flipped.
    let el_dir = math::sub(a_co, b_co);
    let no = math::normalize_v3(el_dir).0;
    if !is_a_no_valid {
        is_a_no_valid = a.calc_normal_aligned(bm, no);
    }
    if !is_b_no_valid {
        is_b_no_valid = b.calc_normal_aligned(bm, no);
    }
    let _ = (is_a_no_valid, is_b_no_valid);
    let mut a_no = a.no;
    let mut b_no = b.no;
    if math::dot(a_no, el_dir) > 0.0 {
        a_no = math::negate(a_no);
    }
    if math::dot(b_no, el_dir) < 0.0 {
        b_no = math::negate(b_no);
    }

    let bezier = |q0: V3, q1: V3, q2: V3, q3: V3| -> Vec<V3> {
        let xs: Vec<Vec<f32>> = (0..3)
            .map(|i| math::forward_diff_bezier(q0[i], q1[i], q2[i], q3[i], resolu - 1))
            .collect();
        (0..resolu).map(|k| [xs[0][k], xs[1][k], xs[2][k]]).collect()
    };

    // The centre spline.
    let coord_array_main: Option<Vec<V3>> = if interp == Interp::Path || falloff_cache.is_some() {
        let handle_len = bezier_handle_calc_length_v3(a_co, a_no, b_co, b_no) * smooth;
        let handle_a = math::add(math::scale(a_no, handle_len), a_co);
        let handle_b = math::add(math::scale(b_no, handle_len), b_co);
        Some(bezier(a_co, handle_a, handle_b, b_co))
    } else {
        None
    };

    match interp {
        Interp::Linear => {
            if let Some(fc) = falloff_cache {
                let coord_array: Vec<V3> = (0..resolu)
                    .map(|i| math::interp_v3_v3v3(a_co, b_co, i as f32 / (resolu - 1) as f32))
                    .collect();
                for ring in rings {
                    for (i, &v) in ring.verts.iter().enumerate() {
                        if i > 0 && i < resolu - 1 {
                            let co = bm.vert(v).co;
                            bm.vert_mut(v).co = math::interp_v3_v3v3(coord_array[i], co, fc[i]);
                        }
                    }
                }
            }
        }
        Interp::Path => {
            let main = coord_array_main.as_ref().expect("path has a centre spline");
            // Very similar to `make_bevel_list_3D_minimum_twist`.
            let mut direction_array = vec![[0.0f32; 3]; resolu];
            direction_array[0] = a_no;
            direction_array[resolu - 1] = math::negate(b_no);
            for i in 1..resolu - 1 {
                direction_array[i] = math::bisect_v3_v3v3v3(main[i - 1], main[i], main[i + 1]);
            }
            let mut quat_array = vec![[1.0f32, 0.0, 0.0, 0.0]; resolu];
            quat_array[0] = math::normalize_qt(math::vec_to_quat(direction_array[0], 5, 1));
            for i in 1..resolu {
                let angle = math::angle_normalized_v3v3(direction_array[i - 1], direction_array[i]);
                if angle > 0.0 {
                    let cross_tmp = math::cross(direction_array[i - 1], direction_array[i]);
                    let q = math::axis_angle_to_quat(cross_tmp, angle);
                    quat_array[i] = math::normalize_qt(math::mul_qt_qtqt(q, quat_array[i - 1]));
                } else {
                    quat_array[i] = quat_array[i - 1];
                }
            }
            // The base triangles.
            let tri_array: Vec<[V3; 3]> = (0..resolu)
                .map(|i| {
                    let shape_size = falloff_cache.map_or(1.0, |fc| fc[i]);
                    let mut tri = [[0.0f32; 3]; 3];
                    tri[1][0] = shape_size;
                    tri[2][1] = shape_size;
                    for t in &mut tri {
                        *t = math::add(math::mul_qt_v3(quat_array[i], *t), main[i]);
                    }
                    tri
                })
                .collect();
            let tri_sta = tri_array[0];
            let tri_end = tri_array[resolu - 1];
            for ring in rings {
                let v_a = ring.verts[0];
                let v_b = *ring.verts.last().expect("ring has vertices");
                let (a_pos, b_pos) = (bm.vert(v_a).co, bm.vert(v_b).co);
                // Skip the first and last.
                for i in 1..ring.verts.len().saturating_sub(1) {
                    let tri_tmp = tri_array[i.min(resolu - 1)];
                    let co_a = math::transform_point_by_tri_v3(a_pos, tri_tmp, tri_sta);
                    let co_b = math::transform_point_by_tri_v3(b_pos, tri_tmp, tri_end);
                    bm.vert_mut(ring.verts[i]).co =
                        math::interp_v3_v3v3(co_a, co_b, i as f32 / (resolu - 1) as f32);
                }
            }
        }
        Interp::Surface => {
            // A Bezier handle per edge ring.
            for ring in rings {
                let v_a = ring.verts[0];
                let v_b = *ring.verts.last().expect("ring has vertices");
                let co_a = bm.vert(v_a).co;
                let co_b = bm.vert(v_b).co;
                // Normals are not recalculated here (feedback with 2+ connected rings).
                let no_a = lpair.nors_a.get(&v_a).copied().unwrap_or([0.0; 3]);
                let no_b = lpair.nors_b.get(&v_b).copied().unwrap_or([0.0; 3]);
                let handle_len = bezier_handle_calc_length_v3(co_a, no_a, co_b, no_b) * smooth;
                let handle_a = math::add(math::scale(no_a, handle_len), co_a);
                let handle_b = math::add(math::scale(no_b, handle_len), co_b);
                let coord_array = bezier(co_a, handle_a, handle_b, co_b);
                for i in 1..ring.verts.len().saturating_sub(1) {
                    if i < resolu - 1 {
                        let v = ring.verts[i];
                        bm.vert_mut(v).co = coord_array[i];
                        if let (Some(fc), Some(main)) = (falloff_cache, coord_array_main.as_ref()) {
                            let co = bm.vert(v).co;
                            bm.vert_mut(v).co = math::interp_v3_v3v3(main[i], co, fc[i]);
                        }
                    }
                }
            }
        }
    }
}

/// `bm_face_slice`: cuts the face of `l` into `cuts + 1` slices.
fn face_slice(bm: &mut BMesh, l: LoopId, cuts: usize) {
    let mut l_new = l;
    for _ in 0..cuts {
        let ln = bm.lp(l_new).clone();
        let next_next = bm.lp(ln.next).next;
        match bm.face_split(ln.f, ln.prev, next_next, None, false) {
            Some((_, lnn)) => {
                l_new = lnn;
            }
            // `l_prev` and `l_next->next` adjacent: cannot continue.
            None => break,
        }
        let rn = bm.lp(l_new).radial_next;
        if bm.face(bm.lp(l_new).f).len < bm.face(bm.lp(rn).f).len {
            l_new = rn;
        }
        let f = bm.lp(l_new).f;
        bm.oflag_enable(f, FACE_OUT);
        let rf = bm.lp(bm.lp(l_new).radial_next).f;
        bm.oflag_enable(rf, FACE_OUT);
    }
}

/// `bm_edgering_pair_order_is_flipped`.
fn edgering_pair_order_is_flipped(bm: &BMesh, a: &EdgeLoop, b: &EdgeLoop) -> bool {
    let ex = |x: VertId, y: VertId| bm.edge_exists(x, y).is_some();
    let (a0, b0) = (a.verts[0], b.verts[0]);
    let a1 = a.verts.get(1).copied().unwrap_or(a0);
    let b1 = b.verts.get(1).copied().unwrap_or(b0);
    // Step around any fan faces on both sides.
    let mut ia = 0usize;
    loop {
        ia += 1;
        if ia >= a.verts.len() || !(ex(a.verts[ia], b0) || ex(a.verts[ia], b1)) {
            break;
        }
    }
    let mut ib = 0usize;
    loop {
        ib += 1;
        if ib >= b.verts.len() || !(ex(b.verts[ib], a0) || ex(b.verts[ib], a1)) {
            break;
        }
    }
    // `v_iter_step ? v_iter_step->prev : lb->last`.
    let va = if ia < a.verts.len() {
        a.verts[ia - 1]
    } else {
        *a.verts.last().expect("loop has vertices")
    };
    let vb = if ib < b.verts.len() {
        b.verts[ib - 1]
    } else {
        *b.verts.last().expect("loop has vertices")
    };
    !(ex(va, vb) || ex(a1, vb) || ex(b1, va))
}

/// `bm_edgering_pair_order`: aligns the two loops and gives them the same winding.
fn edgering_pair_order(bm: &mut BMesh, a: &mut EdgeLoop, b: &mut EdgeLoop) {
    set_tag(bm, a, false);
    set_tag(bm, b, true);
    if a.closed && b.closed {
        let node = a.verts[0];
        let v_other = bm
            .vert_edges(node)
            .into_iter()
            .filter(|&e| bm.oflag_test(e, EDGE_RING))
            .map(|e| bm.edge_other_vert(e, node))
            .find(|&v| tag(bm, v));
        if let Some(v_other) = v_other {
            b.rotate_first(v_other);
            if edgering_pair_order_is_flipped(bm, a, b) {
                b.flip();
                // Re-ensure the first node.
                b.rotate_first(v_other);
            }
        }
    } else {
        // If they don't share an edge: flip.
        let e = bm.edge_exists(a.verts[0], b.verts[0]);
        if e.is_none_or(|e| !bm.oflag_test(e, EDGE_RING)) {
            b.flip();
        }
    }
    // For cases with multiple loops.
    set_tag(bm, b, false);
}

/// `bm_edgering_pair_subdiv`: cuts the ring edges between the loops and slices the faces.
fn edgering_pair_subdiv(bm: &mut BMesh, a: &EdgeLoop, b: &EdgeLoop, cuts: usize) -> Vec<EdgeLoop> {
    let mut rings = Vec::new();
    let mut edges_ring: Vec<EdgeId> = Vec::new();
    let mut faces_ring: Vec<FaceId> = Vec::new();
    set_tag(bm, a, false);
    set_tag(bm, b, true);
    for &v in &a.verts {
        for e in bm.vert_edges(v) {
            if !bm.oflag_test(e, EDGE_IN_STACK) {
                let v_other = bm.edge_other_vert(e, v);
                if tag(bm, v_other) {
                    bm.oflag_enable(e, EDGE_IN_STACK);
                    edges_ring.push(e);
                    // Add the faces to the stack.
                    for l in bm.edge_loops(e) {
                        let f = bm.lp(l).f;
                        if bm.oflag_test(f, FACE_OUT) && !bm.oflag_test(f, FACE_IN_STACK) {
                            bm.oflag_enable(f, FACE_IN_STACK);
                            faces_ring.push(f);
                        }
                    }
                }
            }
        }
    }
    while let Some(e) = edges_ring.pop() {
        frap_math::budget::check();
        bm.oflag_disable(e, EDGE_IN_STACK);
        // Unrelated to subdivision, but multiple rings fail without clearing it.
        bm.oflag_disable(e, EDGE_RING);
        let [v1, v2] = bm.edge(e).v;
        let v_other = if tag(bm, v1) { v1 } else { v2 };
        rings.push(edge_subdiv_as_loop(bm, e, v_other, cuts));
    }
    while let Some(f) = faces_ring.pop() {
        frap_math::budget::check();
        bm.oflag_disable(f, FACE_IN_STACK);
        // Check each edge of the face.
        for l in bm.face_loops(f) {
            if bm.oflag_test(bm.lp(l).e, EDGE_RIM) {
                face_slice(bm, l, cuts);
                break;
            }
        }
    }
    // Clear the tags so the new vertices are not tagged too.
    for r in &rings {
        set_tag(bm, r, false);
    }
    set_tag(bm, b, false);
    rings
}

/// Two distinct elements of `v`, mutably.
fn pair_mut(v: &mut [EdgeLoop], i: usize, j: usize) -> (&mut EdgeLoop, &mut EdgeLoop) {
    debug_assert_ne!(i, j);
    if i < j {
        let (lo, hi) = v.split_at_mut(j);
        (&mut lo[i], &mut hi[0])
    } else {
        let (lo, hi) = v.split_at_mut(i);
        (&mut hi[0], &mut lo[j])
    }
}

/// `bm_edgering_pair_ringsubd`.
#[allow(clippy::too_many_arguments)]
fn edgering_pair_ringsubd(
    bm: &mut BMesh,
    lpair: &LoopPairStore,
    a: &mut EdgeLoop,
    b: &mut EdgeLoop,
    interp: Interp,
    cuts: usize,
    smooth: f32,
    falloff_cache: Option<&[f32]>,
) {
    edgering_pair_order(bm, a, b);
    let rings = edgering_pair_subdiv(bm, a, b, cuts);
    edgering_pair_interpolate(bm, lpair, a, b, &rings, interp, cuts, smooth, falloff_cache);
}

/// Runs `subdivide_edgering` on `bm`.
pub fn subdivide_edgering(bm: &mut BMesh, i: &SubdivideEdgeringIn) -> Result<SubdivideEdgeringOut, BmError> {
    let interp = match i.interp_mode {
        "LINEAR" => Interp::Linear,
        "PATH" => Interp::Path,
        "SURFACE" => Interp::Surface,
        s => return Err(BmError::Invalid(format!("interp_mode: unknown '{s}'"))),
    };
    let profile_shape = Falloff::from_identifier(i.profile_shape)
        .ok_or_else(|| BmError::Invalid(format!("profile_shape: unknown '{}'", i.profile_shape)))?;
    for &e in &i.edges {
        if !bm.edges.contains(e.0) {
            return Err(BmError::Invalid(format!("edges: no edge {}", e.0)));
        }
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i, interp, profile_shape))
}

fn exec(
    bm: &mut BMesh,
    i: &SubdivideEdgeringIn,
    interp: Interp,
    profile_shape: Falloff,
) -> Result<SubdivideEdgeringOut, BmError> {
    let cuts = i.cuts.max(0) as usize;
    let resolu = cuts + 2;
    let smooth = i.smooth;

    for &e in &i.edges {
        bm.oflag_enable(e, EDGE_RING);
    }
    for v in bm.vert_ids() {
        bm.vert_mut(v).flag.remove(ElemFlag::TAG);
    }

    // Flag the outer edges (the loops on the bounds of the edge ring).
    for &e in &i.edges {
        for l in bm.edge_loops(e) {
            let f = bm.lp(l).f;
            // N-gons could be supported, other areas would need updating too (#48926).
            if bm.face(f).len <= 4 && !bm.oflag_test(f, FACE_OUT) {
                let loops = bm.face_loops(f);
                let ok = loops.iter().any(|&lf| {
                    let le = bm.lp(lf).e;
                    bm.oflag_test(le, EDGE_RING) && e != le
                });
                if ok {
                    bm.oflag_enable(f, FACE_OUT);
                    for &lf in &loops {
                        let le = bm.lp(lf).e;
                        if !bm.oflag_test(le, EDGE_RING) {
                            bm.oflag_enable(le, EDGE_RIM);
                        }
                    }
                }
            }
        }
    }

    // Cache the falloff of each step (symmetrical).
    let falloff_cache: Option<Vec<f32>> = (i.profile_shape_factor != 0.0).then(|| {
        (0..resolu)
            .map(|k| {
                let mut fac = k as f32 / (resolu - 1) as f32;
                fac = (1.0 - 2.0 * (0.5 - fac).abs()).abs();
                fac = profile_shape.calc(fac);
                1.0 + fac * i.profile_shape_factor
            })
            .collect()
    });
    let fc = falloff_cache.as_deref();

    // Execute the subdivision on all ring pairs.
    let mut rims = mesh_edgeloops_find(bm, &|bm, e| bm.oflag_test(e, EDGE_RIM));
    let count = rims.len();
    let changed;
    if count < 2 {
        return Err(BmError::Cancel("No edge rings found".into()));
    } else if count == 2 {
        let (a, b) = rims.split_at_mut(1);
        let (a, b) = (&mut a[0], &mut b[0]);
        if edgeloop_check_overlap_all(bm, a, b) {
            let lpair = edgering_pair_store_create(bm, a, b, interp);
            edgering_pair_ringsubd(bm, &lpair, a, b, interp, cuts, smooth, fc);
            changed = true;
        } else {
            return Err(BmError::Cancel("Edge-ring pair isn't connected".into()));
        }
    } else {
        let pairs = edgering_pair_calc(bm, &rims);
        if pairs.is_empty() {
            return Err(BmError::Cancel("Edge-rings are not connected".into()));
        }
        // First cache the pairs.
        let mut lpairs: Vec<Option<LoopPairStore>> = Vec::with_capacity(pairs.len());
        for &(ia, ib) in &pairs {
            let (a, b) = (rims[ia].clone(), rims[ib].clone());
            lpairs.push(edgeloop_check_overlap_all(bm, &a, &b).then(|| edgering_pair_store_create(bm, &a, &b, interp)));
        }
        let mut any = false;
        for (k, &(ia, ib)) in pairs.iter().enumerate() {
            if let Some(lpair) = &lpairs[k] {
                let (a, b) = pair_mut(&mut rims, ia, ib);
                edgering_pair_ringsubd(bm, lpair, a, b, interp, cuts, smooth, fc);
                any = true;
            }
        }
        changed = any;
    }
    let faces: Vec<FaceId> = if changed {
        bm.face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, FACE_OUT))
            .collect()
    } else {
        Vec::new()
    };
    Ok(SubdivideEdgeringOut {
        geom: faces.iter().map(|&f| BMElem::Face(f)).collect(),
        faces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::subdivide_edges::tests::{assert_valid, counts, cube, cyl};

    fn ring(
        bm: &mut BMesh,
        edges: &[u32],
        cuts: i32,
        interp_mode: &'static str,
        factor: f32,
    ) -> Result<SubdivideEdgeringOut, BmError> {
        subdivide_edgering(
            bm,
            &SubdivideEdgeringIn {
                edges: edges.iter().map(|&i| EdgeId(i)).collect(),
                interp_mode,
                smooth: 1.0,
                cuts,
                profile_shape: "SMOOTH",
                profile_shape_factor: factor,
            },
        )
    }

    #[test]
    fn cube_ring_every_mode() {
        for mode in ["LINEAR", "PATH", "SURFACE"] {
            for factor in [0.0, 0.5] {
                let mut bm = cube();
                let out = ring(&mut bm, &[0, 2, 7, 8], 3, mode, factor).unwrap();
                // 4 ring edges cut 3 times; the 4 faces between become 16.
                assert_eq!(counts(&bm), (20, 36, 18), "{mode} {factor}");
                assert_eq!(out.faces.len(), 16);
                assert_valid(&bm);
                for v in bm.vert_ids() {
                    assert!(bm.vert(v).co.iter().all(|c| c.is_finite()));
                }
            }
        }
    }

    #[test]
    fn cylinder_closed_rims() {
        let mut bm = cyl(8);
        let out = ring(&mut bm, &[2, 3, 6, 9, 12, 15, 18, 21], 2, "PATH", 0.0).unwrap();
        assert_eq!(out.faces.len(), 24);
        assert_eq!(counts(&bm), (32, 56, 26));
        assert_valid(&bm);
    }

    #[test]
    fn errors_change_nothing() {
        let mut bm = cube();
        let before = counts(&bm);
        assert_eq!(
            ring(&mut bm, &[0], 2, "PATH", 0.0),
            Err(BmError::Cancel("No edge rings found".into()))
        );
        assert_eq!(counts(&bm), before);
        assert!(matches!(
            ring(&mut bm, &[0], 2, "CUBIC", 0.0),
            Err(BmError::Invalid(_))
        ));
    }

    #[test]
    fn edge_loop_store() {
        let bm = cube();
        let mut el = EdgeLoop::from_verts(vec![VertId(0), VertId(1), VertId(3), VertId(2)], true);
        assert_eq!(el.edges(&bm).len(), 4);
        el.calc_center(&bm);
        assert_eq!(el.co, [-1.0, 0.0, 0.0]);
        assert!(el.calc_normal(&bm));
        let no = el.no;
        el.flip();
        assert_eq!(el.no, math::negate(no));
        assert_eq!(el.verts, vec![VertId(2), VertId(3), VertId(1), VertId(0)]);
        el.rotate_first(VertId(1));
        assert_eq!(el.verts[0], VertId(1));
    }
}
