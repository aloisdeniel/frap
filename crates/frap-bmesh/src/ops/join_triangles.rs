// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.join_triangles` (Blender 5.2.4 `bmo_join_triangles_exec`,
//! `operators/bmo_join_triangles.cc`), implemented by WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.join_triangles(bmesh, faces=[], cmp_seam=False, cmp_sharp=False, cmp_uvs=False, cmp_vcols=False, cmp_materials=False, angle_face_threshold=0.0, angle_shape_threshold=0.0, topology_influence=0.0, deselect_joined=False)`.
//!
//! **Algorithm.** Every manifold edge between two input triangles that is not delimited
//! (seam, sharp, material, face angle, shape angle, UV/colour discontinuity) goes into a
//! min-heap (`BLI_heap`, exact port) keyed by the error of the quad the join would make
//! (non-flatness + corner angles off 90° + concavity), in edge order. With
//! `topology_influence` the candidates next to existing input quads, and next to every new
//! quad, get their error lowered by how well they would align with that quad. The heap is
//! then emptied: an edge whose two faces are still triangles, and whose quad does not
//! already exist, is joined with `BM_faces_join_pair` (the second triangle reversed first if
//! the two are wound the same way).
//!
//! **Create/kill order** (`BM_faces_join`): the quad is created first (taking the most
//! recently freed face slot, else appended), then the shared edge and the two triangles are
//! killed, so the next join's quad takes the slot of the second triangle of this join.
//! Blender's own results therefore differ from the original face order (a re-joined cube
//! has another face order and loop start, [MESH §4.13]); goldens compare at T2.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use super::triangulate::heap::{Heap, NodeId};
use super::triangulate::math;
use crate::query::geom::V3;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use frap_mesh::AttrData;

/// Input slots of `join_triangles` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JoinTrianglesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `cmp_seam`.
    pub cmp_seam: bool,
    /// `cmp_sharp`.
    pub cmp_sharp: bool,
    /// `cmp_uvs`.
    pub cmp_uvs: bool,
    /// `cmp_vcols`.
    pub cmp_vcols: bool,
    /// `cmp_materials`.
    pub cmp_materials: bool,
    /// `angle_face_threshold`.
    pub angle_face_threshold: f32,
    /// `angle_shape_threshold`.
    pub angle_shape_threshold: f32,
    /// `topology_influence`.
    pub topology_influence: f32,
    /// `deselect_joined`.
    pub deselect_joined: bool,
}

/// Output slots of `join_triangles`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct JoinTrianglesOut {
    /// `faces.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the joined quads (or, with `deselect_joined`, the triangles left alone),
    /// face order.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_join_triangles_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const FACE_OUT: u16 = 1 << 0;
const FACE_INPUT: u16 = 1 << 2;

/// Improvement never exceeds 99%: keeps the relative order of improved edges.
const MAXIMUM_IMPROVEMENT: f32 = 0.99;

const PI_2: f32 = std::f32::consts::FRAC_PI_2;

// Ported from blender/source/blender/bmesh/operators/bmo_join_triangles.cc (quad_calc_error).
/// The error of a proposed quad: lower is better.
fn quad_calc_error(v1: V3, v2: V3, v3: V3, v4: V3) -> f32 {
    let mut error = 0.0f32;
    // Normal difference.
    {
        let n1 = math::normal_tri_v3(v1, v2, v3).0;
        let n2 = math::normal_tri_v3(v1, v3, v4).0;
        let angle_a = if math::compare_v3v3(n1, n2, f32::EPSILON) {
            0.0
        } else {
            math::angle_normalized_v3v3(n1, n2)
        };
        let n1 = math::normal_tri_v3(v2, v3, v4).0;
        let n2 = math::normal_tri_v3(v4, v1, v2).0;
        let angle_b = if math::compare_v3v3(n1, n2, f32::EPSILON) {
            0.0
        } else {
            math::angle_normalized_v3v3(n1, n2)
        };
        error += (angle_a + angle_b) / (std::f64::consts::PI * 2.0) as f32;
    }
    // Co-linearity.
    {
        let e = [
            math::normalize_v3(math::sub(v1, v2)).0,
            math::normalize_v3(math::sub(v2, v3)).0,
            math::normalize_v3(math::sub(v3, v4)).0,
            math::normalize_v3(math::sub(v4, v1)).0,
        ];
        let diff = ((math::angle_normalized_v3v3(e[0], e[1]) - PI_2).abs()
            + (math::angle_normalized_v3v3(e[1], e[2]) - PI_2).abs()
            + (math::angle_normalized_v3v3(e[2], e[3]) - PI_2).abs()
            + (math::angle_normalized_v3v3(e[3], e[0]) - PI_2).abs())
            / (std::f64::consts::PI * 2.0) as f32;
        error += diff;
    }
    // Concavity.
    {
        let area_a = math::area_tri_v3(v1, v2, v3) + math::area_tri_v3(v1, v3, v4);
        let area_b = math::area_tri_v3(v2, v3, v4) + math::area_tri_v3(v4, v1, v2);
        let area_min = area_a.min(area_b);
        let area_max = area_a.max(area_b);
        error += if area_max != 0.0 {
            1.0 - (area_min / area_max)
        } else {
            1.0
        };
    }
    error
}

/// `bm_edge_to_quad_verts`.
fn edge_to_quad_verts(bm: &BMesh, e: EdgeId) -> [VertId; 4] {
    let l = bm.edge(e).l.expect("invariant: manifold edge");
    let ll = bm.lp(l);
    let lr = bm.lp(ll.radial_next);
    [ll.v, bm.lp(lr.prev).v, bm.lp(ll.next).v, bm.lp(ll.prev).v]
}

struct DelimitData {
    do_seam: bool,
    do_sharp: bool,
    do_mat: bool,
    do_angle_face: bool,
    angle_face_cos: f32,
    do_angle_shape: bool,
    angle_shape: f32,
    /// Loop layers compared for contiguity (`CD_PROP_FLOAT2`, `CD_PROP_BYTE_COLOR`,
    /// `CD_PROP_COLOR`).
    cdata: Vec<usize>,
}

fn delimit_from_op(bm: &BMesh, i: &JoinTrianglesIn) -> DelimitData {
    let mut d = DelimitData {
        do_seam: i.cmp_seam,
        do_sharp: i.cmp_sharp,
        do_mat: i.cmp_materials,
        do_angle_face: false,
        angle_face_cos: 0.0,
        do_angle_shape: false,
        angle_shape: 0.0,
        cdata: Vec::new(),
    };
    let deg180 = 180.0f32 * (std::f32::consts::PI / 180.0);
    if i.angle_face_threshold < deg180 {
        d.angle_face_cos = libm::cosf(i.angle_face_threshold);
        d.do_angle_face = true;
    }
    if i.angle_shape_threshold < deg180 {
        d.angle_shape = i.angle_shape_threshold;
        d.do_angle_shape = true;
    }
    let layers_of = |pred: fn(&AttrData) -> bool| -> Vec<usize> {
        bm.ldata
            .layers
            .iter()
            .enumerate()
            .filter(|(_, l)| pred(&l.data))
            .map(|(k, _)| k)
            .collect()
    };
    if i.cmp_uvs {
        d.cdata
            .extend(layers_of(|a| matches!(a, AttrData::Float2(_))));
    }
    if i.cmp_vcols {
        d.cdata
            .extend(layers_of(|a| matches!(a, AttrData::ByteColor(_))));
        d.cdata
            .extend(layers_of(|a| matches!(a, AttrData::FloatColor(_))));
    }
    d
}

/// `CustomData_data_equals` on one loop layer (`layerEqual_propfloat2`,
/// `layerEqual_mloopcol`, `layerEqual_propcol`).
fn loop_cd_equal(bm: &BMesh, layer: usize, a: LoopId, b: LoopId) -> bool {
    let (sa, sb) = (a.slot(), b.slot());
    match &bm.ldata.layers[layer].data {
        AttrData::Float2(c) => {
            let (x, y) = (c[sa], c[sb]);
            let d = [x[0] - y[0], x[1] - y[1]];
            d[0] * d[0] + d[1] * d[1] < 0.00001
        }
        AttrData::ByteColor(c) => {
            let (x, y) = (c[sa], c[sb]);
            let d: Vec<f32> = (0..4).map(|k| (x[k] as i32 - y[k] as i32) as f32).collect();
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3] < 0.001
        }
        AttrData::FloatColor(c) => {
            let (x, y) = (c[sa], c[sb]);
            let mut tot = 0.0f32;
            for k in 0..4 {
                let d = x[k] - y[k];
                tot += d * d;
            }
            tot < 0.001
        }
        _ => true,
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_edge_is_contiguous_loop_cd).
/// `BM_edge_is_contiguous_loop_cd`: the loop data matches across the edge at both ends.
fn edge_is_contiguous_loop_cd(bm: &BMesh, e: EdgeId, layer: usize) -> bool {
    let Some(l_base_v1) = bm.edge(e).l else {
        return true;
    };
    if bm.lp(l_base_v1).radial_next == l_base_v1 {
        return true;
    }
    let l_base_v2 = bm.lp(l_base_v1).next;
    let mut l_iter = bm.lp(l_base_v1).radial_next;
    loop {
        let (iter_v1, iter_v2) = if bm.lp(l_iter).v == bm.lp(l_base_v1).v {
            (l_iter, bm.lp(l_iter).next)
        } else {
            (bm.lp(l_iter).next, l_iter)
        };
        if !loop_cd_equal(bm, layer, l_base_v1, iter_v1)
            || !loop_cd_equal(bm, layer, l_base_v2, iter_v2)
        {
            return false;
        }
        l_iter = bm.lp(l_iter).radial_next;
        if l_iter == l_base_v1 {
            return true;
        }
    }
}

/// `bm_edge_is_delimit`.
fn edge_is_delimit(bm: &BMesh, e: EdgeId, d: &DelimitData) -> bool {
    let l = bm.edge(e).l.expect("invariant: manifold edge");
    let f_a = bm.lp(l).f;
    let f_b = bm.lp(bm.lp(l).radial_next).f;
    let ef = bm.edge(e).flag;
    if d.do_seam && ef.contains(ElemFlag::SEAM) {
        return true;
    }
    if d.do_sharp && !ef.contains(ElemFlag::SMOOTH) {
        return true;
    }
    if d.do_mat && bm.face(f_a).mat_nr != bm.face(f_b).mat_nr {
        return true;
    }
    if d.do_angle_face && math::dot(bm.face(f_a).no, bm.face(f_b).no) < d.angle_face_cos {
        return true;
    }
    if d.do_angle_shape {
        let vs = edge_to_quad_verts(bm, e);
        let c = vs.map(|v| bm.vert(v).co);
        if math::is_quad_flip_v3(c[0], c[1], c[2], c[3]) != 0 {
            return true;
        }
        let ev = [
            math::normalize_v3(math::sub(c[0], c[1])).0,
            math::normalize_v3(math::sub(c[1], c[2])).0,
            math::normalize_v3(math::sub(c[2], c[3])).0,
            math::normalize_v3(math::sub(c[3], c[0])).0,
        ];
        for k in 0..4 {
            if (math::angle_normalized_v3v3(ev[k], ev[(k + 1) % 4]) - PI_2).abs() > d.angle_shape {
                return true;
            }
        }
    }
    for &layer in &d.cdata {
        if !edge_is_contiguous_loop_cd(bm, e, layer) {
            return true;
        }
    }
    false
}

struct State {
    heap: Heap<EdgeId>,
    /// Heap node of every edge (by edge index), when `use_topo_influence`.
    nodes: Vec<Option<NodeId>>,
    use_topo_influence: bool,
    topo_influence: f32,
}

/// `add_neighbors`: Blender's duplicate check compares against the not-yet-written slot, so
/// it never matches and every candidate is added.
fn add_neighbors(bm: &BMesh, items: &mut Vec<(EdgeId, LoopId)>, l_in_quad: LoopId) {
    let e = bm.lp(l_in_quad).e;
    if !bm.edge_is_manifold(e) {
        return;
    }
    let l_in_neighbor = bm.lp(l_in_quad).radial_next;
    if bm.face(bm.lp(l_in_neighbor).f).len != 3 {
        return;
    }
    let ln = bm.lp(l_in_neighbor);
    for l_other in [ln.prev, ln.next] {
        let lo = bm.lp(l_other);
        if bm.edge_is_manifold(lo.e) && bm.face(bm.lp(lo.radial_next).f).len == 3 && items.len() < 8
        {
            items.push((lo.e, l_in_neighbor));
        }
    }
}

/// `rotate_to_plane`.
fn rotate_to_plane(
    bm: &BMesh,
    quad_verts: &[VertId; 4],
    l_shared: LoopId,
    plane_normal: V3,
) -> [V3; 4] {
    let ls = bm.lp(l_shared);
    let (s_v, s_next_v) = (ls.v, bm.lp(ls.next).v);
    let rotation_axis = math::normalize_v3(math::sub(bm.vert(s_v).co, bm.vert(s_next_v).co)).0;
    let c = quad_verts.map(|v| bm.vert(v).co);
    let quad_normal = math::normal_quad_v3(c[0], c[1], c[2], c[3]).0;
    let angle = math::angle_signed_on_axis_v3v3_v3(plane_normal, quad_normal, rotation_axis);
    let mut r = [[0.0f32; 3]; 4];
    for k in 0..4 {
        if quad_verts[k] == s_v || quad_verts[k] == s_next_v {
            r[k] = c[k];
        } else {
            let local = math::sub(c[k], bm.vert(s_v).co);
            r[k] = math::add(
                math::rotate_normalized_v3_v3v3fl(local, rotation_axis, angle),
                bm.vert(s_v).co,
            );
        }
    }
    r
}

/// `compute_alignment`.
fn compute_alignment(
    bm: &BMesh,
    quad_a_vecs: &[V3; 4],
    quad_b_verts: &[VertId; 4],
    l_shared: LoopId,
    plane_normal: V3,
) -> f32 {
    let b = rotate_to_plane(bm, quad_b_verts, l_shared, plane_normal);
    let quad_b_vecs = [
        math::normalize_v3(math::sub(b[0], b[1])).0,
        math::normalize_v3(math::sub(b[1], b[2])).0,
        math::normalize_v3(math::sub(b[2], b[3])).0,
        math::normalize_v3(math::sub(b[3], b[0])).0,
    ];
    let mut error = [0.0f32; 4];
    for k in 0..4 {
        let angle_a = math::angle_normalized_v3v3(quad_a_vecs[k], quad_b_vecs[k]).abs();
        let angle_b = math::angle_normalized_v3v3(quad_a_vecs[k], quad_b_vecs[(k + 1) % 4]).abs();
        error[0] += angle_a;
        error[1] += angle_b;
        // `M_PI - angle` in double, accumulated into a float.
        error[2] = (error[2] as f64 + (std::f64::consts::PI - angle_a as f64)) as f32;
        error[3] = (error[3] as f64 + (std::f64::consts::PI - angle_b as f64)) as f32;
    }
    let best_error = error[0].min(error[1]).min(error[2]).min(error[3]) / 4.0;
    let alignment = (1.0 - (best_error as f64 / (std::f64::consts::PI / 4.0))) as f32;
    alignment.max(0.0)
}

/// `reprioritize_join`.
fn reprioritize_join(
    bm: &BMesh,
    s: &mut State,
    e_merge: EdgeId,
    l_shared: LoopId,
    neighbor_quad_vecs: &[V3; 4],
    neighbor_quad_error: f32,
    neighbor_quad_normal: V3,
) {
    let idx = bm.index_get(BMElem::Edge(e_merge));
    let Some(node) = usize::try_from(idx)
        .ok()
        .and_then(|i| s.nodes.get(i).copied().flatten())
    else {
        return;
    };
    let join_error_curr = s.heap.node_value(node);
    if neighbor_quad_error > join_error_curr {
        return;
    }
    let quad_verts_merge = edge_to_quad_verts(bm, e_merge);
    let alignment = compute_alignment(
        bm,
        neighbor_quad_vecs,
        &quad_verts_merge,
        l_shared,
        neighbor_quad_normal,
    );
    let improvement = neighbor_quad_error - join_error_curr;
    let multiplier = (s.topo_influence * alignment).min(MAXIMUM_IMPROVEMENT);
    let join_error_next = join_error_curr + (improvement * multiplier);
    s.heap.node_value_update(node, join_error_next);
}

/// `reprioritize_face_neighbors` on a quad.
fn reprioritize_face_neighbors(bm: &BMesh, s: &mut State, f: FaceId, f_error: f32) {
    let l_quad = bm.face_loops(f);
    if l_quad.len() != 4 {
        return;
    }
    let mut items: Vec<(EdgeId, LoopId)> = Vec::with_capacity(8);
    for &l in &l_quad {
        add_neighbors(bm, &mut items, l);
    }
    if items.is_empty() {
        return;
    }
    let mut quad_vecs = [[0.0f32; 3]; 4];
    let mut i = 3usize;
    for i_next in 0..4usize {
        quad_vecs[i] = math::normalize_v3(math::sub(
            bm.vert(bm.lp(l_quad[i]).v).co,
            bm.vert(bm.lp(l_quad[i_next]).v).co,
        ))
        .0;
        i = i_next;
    }
    let fno = bm.face(f).no;
    for (e, l) in items {
        reprioritize_join(bm, s, e, l, &quad_vecs, f_error, fno);
    }
}

/// `bm_faces_join_pair_by_edge`.
fn faces_join_pair_by_edge(bm: &mut BMesh, e: EdgeId) -> Option<FaceId> {
    let l_a = bm.edge(e).l?;
    let l_b = bm.lp(l_a).radial_next;
    let (fa, fb) = (bm.lp(l_a).f, bm.lp(l_b).f);
    if bm.face(fa).len != 3 || bm.face(fb).len != 3 {
        return None;
    }
    let v_quad = [
        bm.lp(l_a).v,
        bm.lp(bm.lp(l_b).prev).v,
        bm.lp(bm.lp(l_a).next).v,
        bm.lp(bm.lp(l_a).prev).v,
    ];
    if bm.face_exists(&v_quad).is_some() {
        return None;
    }
    // `BM_faces_join_pair`.
    if bm.lp(l_a).v == bm.lp(l_b).v {
        bm.loop_reverse(fb);
    }
    bm.faces_join(&[fa, fb], true, false).0
}

/// Runs `join_triangles` on `bm`.
pub fn join_triangles(bm: &mut BMesh, i: &JoinTrianglesIn) -> Result<JoinTrianglesOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "join_triangles: dead face {dead:?} in faces"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| {
        let delimit = delimit_from_op(bm, i);
        let mut s = State {
            heap: Heap::new(),
            nodes: Vec::new(),
            use_topo_influence: i.topology_influence != 0.0,
            topo_influence: i.topology_influence,
        };
        let select_tris_only = i.deselect_joined;
        for &f in &i.faces {
            if bm.face(f).len == 3 {
                bm.oflag_enable(f, FACE_INPUT);
                if select_tris_only {
                    bm.oflag_enable(f, FACE_OUT);
                }
            }
        }
        // Every edge of the mesh: candidates go into the heap.
        let edges = bm.edge_ids();
        if s.use_topo_influence {
            s.nodes = vec![None; edges.len()];
        }
        for (idx, &e) in edges.iter().enumerate() {
            bm.index_set(BMElem::Edge(e), idx as i32);
            let pair = bm.edge(e).l.and_then(|la| {
                let lb = bm.lp(la).radial_next;
                (la != lb && bm.lp(lb).radial_next == la).then(|| (bm.lp(la).f, bm.lp(lb).f))
            });
            if let Some((f_a, f_b)) = pair
                && bm.oflag_test(f_a, FACE_INPUT)
                && bm.oflag_test(f_b, FACE_INPUT)
                && !edge_is_delimit(bm, e, &delimit)
            {
                let vs = edge_to_quad_verts(bm, e);
                let c = vs.map(|v| bm.vert(v).co);
                let merge_error = quad_calc_error(c[0], c[1], c[2], c[3]);
                let node = s.heap.insert(merge_error, e);
                if s.use_topo_influence {
                    s.nodes[idx] = Some(node);
                }
            }
        }
        // Improve the candidates around the existing input quads.
        if s.use_topo_influence && !s.heap.is_empty() {
            for &f in &i.faces {
                if bm.face(f).len == 4 {
                    let c: Vec<V3> = bm.face_verts(f).iter().map(|&v| bm.vert(v).co).collect();
                    let mut f_error = quad_calc_error(c[0], c[1], c[2], c[3]);
                    f_error *= 2.0 - (s.topo_influence * MAXIMUM_IMPROVEMENT);
                    reprioritize_face_neighbors(bm, &mut s, f, f_error);
                }
            }
        }
        // Process all possible merges.
        while !s.heap.is_empty() {
            frap_math::budget::check();
            let f_error = s.heap.top_value();
            let e = s.heap.pop_min();
            if !bm.edges.contains(e.0) {
                continue;
            }
            if let Some(f_new) = faces_join_pair_by_edge(bm, e) {
                if !select_tris_only {
                    bm.oflag_enable(f_new, FACE_OUT);
                }
                if s.use_topo_influence {
                    reprioritize_face_neighbors(bm, &mut s, f_new, f_error);
                }
            }
        }
        let mut out = JoinTrianglesOut::default();
        for f in bm.face_ids() {
            if bm.oflag_test(f, FACE_OUT) {
                out.faces.push(f);
                out.geom.push(BMElem::Face(f));
            }
        }
        out
    }))
}
