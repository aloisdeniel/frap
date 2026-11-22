// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_intersect`: cuts the faces of a BMesh along their intersections (the FLOAT solver
//! of the BOOLEAN modifier, `mesh.intersect` and `mesh.intersect_boolean`), optionally
//! dissolving the vertices that only the triangulation needed, separating the cut faces, and
//! (`boolean_mode`) removing or flipping the face islands a ray-parity test puts inside or
//! outside the other side.
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_intersect.cc (Blender 5.2.4,
//! GPL-2.0-or-later) with `USE_SPLICE`, `USE_NET`, `USE_SEPARATE`, `USE_DISSOLVE`,
//! `USE_NET_ISLAND_CONNECT` and `USE_BVH` as Blender builds it. The triangle pairs come from
//! the k-DOP BVH overlap (`BLI_bvhtree_overlap_ex(tree_b, tree_a)` with
//! `BVH_OVERLAP_USE_THREADING`, so above 1024 leaves the pairs come per root child, in a
//! release build's order) and are intersected in that order; the faces are split in the
//! order of Blender's integer-keyed `GHash` (`intersect/ghash.rs`). One order is not
//! Blender's, because Blender walks a pointer-keyed hash table there: the edges are spliced in
//! the order they received their first cut vertex (each with its own list in Blender's
//! order); that only matters when two edges' splices interact, which they do not in
//! non-degenerate input (the cases are compared order-independently, T2).
//!
//! Element indices: vertices and faces are indexed in iteration order at the start, as
//! `BM_mesh_elem_index_ensure` does; vertices made here have none (`-1`).

pub mod ghash;
pub mod math;
pub mod query;
pub mod tessellate;
#[cfg(test)]
mod tests;

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use self::math::{V3, dot, normal_tri_v3, sub};
use self::math::{
    interp_v3_v3v3, isect_line_line_epsilon_v3, isect_line_segment_tri_epsilon_v3,
    isect_point_tri_v3, isect_ray_tri_epsilon_v3, len_squared_v3v3, line_point_factor_v3,
    mid_v3_v3v3v3, normalize_v3,
};
use self::query::{
    face_calc_point_in_face, face_share_edge_check, vert_collapse_check_double_face,
    vert_pair_share_face_check, vert_splice_check_double_edge, vert_splice_check_double_face,
};
use crate::ops::contextual_create::connect_islands::face_split_edgenet_connect_islands;
use crate::ops::contextual_create::construct::mesh_calc_face_groups;
use crate::ops::contextual_create::kdopbvh::BvhTree;
use crate::ops::contextual_create::split_edgenet::face_split_edgenet;
use crate::ops::dissolve_verts::mods::{vert_collapse_edge, vert_is_edge_pair};
use crate::ops::split_edges::mesh_edgesplit;
use crate::types::{BMesh, BmError, CreateFlag, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// `boolean_mode` (`BMESH_ISECT_BOOLEAN_*`, aligned with `BooleanModifierOp`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum BooleanMode {
    /// `BMESH_ISECT_BOOLEAN_NONE` (-1): cut only.
    #[default]
    None,
    /// `BMESH_ISECT_BOOLEAN_ISECT` (0).
    Intersect,
    /// `BMESH_ISECT_BOOLEAN_UNION` (1).
    Union,
    /// `BMESH_ISECT_BOOLEAN_DIFFERENCE` (2).
    Difference,
}

/// The face-side callback: `-1` skip, `0` side A (`tree_a`), `1` side B (`use_self == false`).
pub type TestFn<'a> = &'a dyn Fn(&BMesh, FaceId) -> i32;

/// The arguments of `BM_mesh_intersect`.
pub struct IntersectIn<'a> {
    /// The triangulation of the faces (`looptris`).
    pub looptris: &'a [[LoopId; 3]],
    /// `test_fn`.
    pub test_fn: TestFn<'a>,
    /// Intersect side A with itself (no side B).
    pub use_self: bool,
    /// Split the cut edges apart (`BM_mesh_edgesplit`).
    pub use_separate: bool,
    /// Dissolve the vertices only the triangulation needed.
    pub use_dissolve: bool,
    /// Connect cut islands (holes) to the face boundary before splitting.
    pub use_island_connect: bool,
    /// Connect partial islands (`BM_face_split_edgenet_connect_islands`).
    pub use_partial_connect: bool,
    /// Tag the cut edges (`BM_ELEM_TAG`).
    pub use_edge_tag: bool,
    /// The boolean to apply after cutting.
    pub boolean_mode: BooleanMode,
    /// `eps` (`double_threshold` / `threshold`).
    pub eps: f32,
}

/// The result of `BM_mesh_intersect`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct IntersectOut {
    /// Whether the mesh changed (intersections cut or faces removed or flipped).
    pub changed: bool,
}

/// `enum ISectType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ISectType {
    EdgeTriEdge0 = 0,
    EdgeTriEdge1 = 1,
    EdgeTriEdge2 = 2,
    EdgeTri = 3,
}

impl ISectType {
    fn edge(i: usize) -> ISectType {
        [
            ISectType::EdgeTriEdge0,
            ISectType::EdgeTriEdge1,
            ISectType::EdgeTriEdge2,
        ][i]
    }
}

/// `ISectEpsilon`.
struct Epsilon {
    eps: f32,
    eps2x: f32,
    eps2x_sq: f32,
    eps_margin: f32,
    eps_margin_sq: f32,
}

/// A `GHash` of `LinkBase` lists: the lists keep Blender's order (`ghash_insert_link`
/// prepends, so the newest element comes first); the keys are visited in the order of their
/// first insertion.
struct LinkMap<K, V> {
    index: FxHashMap<K, usize>,
    /// Keys with their values in insertion order (the list is the reverse).
    entries: Vec<(K, Vec<V>)>,
}

impl<K: Copy + Eq + std::hash::Hash, V: Copy + PartialEq> LinkMap<K, V> {
    fn new() -> Self {
        LinkMap {
            index: FxHashMap::default(),
            entries: Vec::new(),
        }
    }

    /// `ghash_insert_link(gh, key, val, use_test)`: false when `use_test` finds `val` already
    /// in the list.
    fn insert_link(&mut self, key: K, val: V, use_test: bool) -> bool {
        match self.index.get(&key) {
            Some(&i) => {
                let list = &mut self.entries[i].1;
                if use_test && list.contains(&val) {
                    return false;
                }
                list.push(val);
            }
            None => {
                self.index.insert(key, self.entries.len());
                self.entries.push((key, vec![val]));
            }
        }
        true
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The list order (newest first) of insertion-ordered values.
fn list_order<V: Copy>(v: &[V]) -> Vec<V> {
    v.iter().rev().copied().collect()
}

/// `ISectState`.
struct State {
    /// `edgetri_cache` (`int[4]` → vertex).
    edgetri_cache: FxHashMap<[i32; 4], VertId>,
    /// `edge_verts`: the vertices to splice into each edge.
    edge_verts: LinkMap<EdgeId, VertId>,
    /// `face_edges`: the cut edges of each original face (by face index).
    face_edges: LinkMap<i32, EdgeId>,
    /// The iteration order of `face_edges`' keys (`BLI_ghash_int_new`).
    face_order: ghash::IntKeyOrder,
    /// `wire_edges`.
    wire_edges: FxHashSet<EdgeId>,
    /// `vert_dissolve` in insertion order (the list is the reverse).
    vert_dissolve: Vec<VertId>,
    epsilon: Epsilon,
    /// Vertex index by slot (`BM_elem_index_get` after `BM_mesh_elem_index_ensure`).
    vidx: Vec<i32>,
    /// Face index by slot.
    fidx: Vec<i32>,
}

impl State {
    fn vert_index(&self, v: VertId) -> i32 {
        self.vidx.get(v.slot()).copied().unwrap_or(-1)
    }
    fn face_index(&self, f: FaceId) -> i32 {
        self.fidx.get(f.slot()).copied().unwrap_or(-1)
    }
    /// `edge_verts_add`.
    fn edge_verts_add(&mut self, e: EdgeId, v: VertId, use_test: bool) {
        self.edge_verts.insert_link(e, v, use_test);
    }
    /// `face_edges_add`.
    fn face_edges_add(&mut self, f_index: i32, e: EdgeId, use_test: bool) {
        if !self.face_edges.index.contains_key(&f_index) {
            self.face_order.insert_new(f_index);
        }
        self.face_edges.insert_link(f_index, e, use_test);
    }
    /// `vert_dissolve_add`.
    fn vert_dissolve_add(&mut self, bm: &mut BMesh, v: VertId) {
        bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        self.vert_dissolve.push(v);
    }
}

/// `tri_v3_scale`: the triangle scaled by `t` about its centre.
fn tri_v3_scale(v: [V3; 3], t: f32) -> [V3; 3] {
    let p = mid_v3_v3v3v3(v[0], v[1], v[2]);
    [
        interp_v3_v3v3(p, v[0], t),
        interp_v3_v3v3(p, v[1], t),
        interp_v3_v3v3(p, v[2], t),
    ]
}

/// `bm_vert_other_edge`: the other edge of an edge-pair vertex.
fn vert_other_edge(bm: &BMesh, v: VertId, e: EdgeId) -> EdgeId {
    let ve = bm.vert(v).e.expect("edge-pair vertex");
    if ve != e {
        ve
    } else {
        bm.disk_edge_next(ve, v)
    }
}

/// `intersect_line_tri`: where the line `p0`-`p1` meets the triangle `t_cos` (an edge of it,
/// or its interior), and the point.
fn intersect_line_tri(
    p0: V3,
    p1: V3,
    t_cos: [V3; 3],
    t_nor: V3,
    e: &Epsilon,
) -> Option<(ISectType, V3)> {
    let mut p_dir = sub(p0, p1);
    normalize_v3(&mut p_dir);

    for i_t0 in 0..3 {
        let i_t1 = (i_t0 + 1) % 3;
        let mut te_dir = sub(t_cos[i_t0], t_cos[i_t1]);
        normalize_v3(&mut te_dir);
        if dot(p_dir, te_dir).abs() >= 1.0 - e.eps {
            // Co-linear.
            continue;
        }
        let (ix_pair_type, i0, i1) =
            isect_line_line_epsilon_v3(p0, p1, t_cos[i_t0], t_cos[i_t1], 0.0);
        if ix_pair_type == 0 {
            continue;
        }
        let ix_pair = if ix_pair_type == 1 {
            [i0, i0]
        } else {
            [i0, i1]
        };
        if ix_pair_type == 1 || len_squared_v3v3(ix_pair[0], ix_pair[1]) <= e.eps_margin_sq {
            let fac = line_point_factor_v3(ix_pair[1], t_cos[i_t0], t_cos[i_t1]);
            if fac >= e.eps_margin && fac <= 1.0 - e.eps_margin {
                let fac = line_point_factor_v3(ix_pair[0], p0, p1);
                if fac >= e.eps_margin && fac <= 1.0 - e.eps_margin {
                    return Some((ISectType::edge(i_t0), ix_pair[0]));
                }
            }
        }
    }

    // Check the ray isn't planar with the triangle.
    if dot(p_dir, t_nor).abs() >= e.eps
        && let Some(fac) =
            isect_line_segment_tri_epsilon_v3(p0, p1, t_cos[0], t_cos[1], t_cos[2], 0.0)
        && fac >= e.eps_margin
        && fac <= 1.0 - e.eps_margin
    {
        let r_ix = interp_v3_v3v3(p0, p1, fac);
        let d = len_squared_v3v3(t_cos[0], r_ix)
            .min(len_squared_v3v3(t_cos[1], r_ix))
            .min(len_squared_v3v3(t_cos[2], r_ix));
        if d >= e.eps_margin_sq {
            return Some((ISectType::EdgeTri, r_ix));
        }
    }
    None
}

/// `bm_isect_edge_tri`: the vertex where the edge `e_v0`-`e_v1` meets the triangle `t`
/// (cached per edge/triangle and edge/edge key), created and queued for splicing on first
/// sight.
#[allow(clippy::too_many_arguments)]
fn isect_edge_tri(
    s: &mut State,
    bm: &mut BMesh,
    mut e_v0: VertId,
    mut e_v1: VertId,
    t: [VertId; 3],
    t_index: i32,
    t_cos: [V3; 3],
    t_nor: V3,
) -> Option<VertId> {
    let ti = t.map(|v| s.vert_index(v));
    if s.vert_index(e_v0) > s.vert_index(e_v1) {
        std::mem::swap(&mut e_v0, &mut e_v1);
    }
    let (i0, i1) = (s.vert_index(e_v0), s.vert_index(e_v1));
    // Order the triangle, then the pairs (1-2, 2-3).
    let order = |mut k: [i32; 4]| {
        if k[2] > k[3] {
            k.swap(2, 3);
        }
        if k[0] > k[2] {
            k.swap(0, 2);
            k.swap(1, 3);
        }
        k
    };
    let k_arr: [[i32; 4]; 4] = [
        order([i0, i1, ti[0], ti[1]]),
        order([i0, i1, ti[1], ti[2]]),
        order([i0, i1, ti[2], ti[0]]),
        [i0, i1, t_index, -1],
    ];
    for k in &k_arr {
        if let Some(&iv) = s.edgetri_cache.get(k) {
            return Some(iv);
        }
    }

    let co0 = bm.vert(e_v0).co;
    let co1 = bm.vert(e_v1).co;
    let (side, ix) = intersect_line_tri(co0, co1, t_cos, t_nor, &s.epsilon)?;
    let iv = bm.vert_create(ix, None, CreateFlag::empty());
    match bm.edge_exists(e_v0, e_v1) {
        Some(e) => s.edge_verts_add(e, iv, false),
        None => s.vert_dissolve_add(bm, iv),
    }
    if side != ISectType::EdgeTri {
        let i = side as usize;
        if let Some(e) = bm.edge_exists(t[i], t[(i + 1) % 3]) {
            s.edge_verts_add(e, iv, false);
        }
    }
    s.edgetri_cache.insert(k_arr[side as usize], iv);
    Some(iv)
}

/// `bm_isect_tri_tri`: intersects the triangles `a` and `b` (looptri indices `a_index`,
/// `b_index`): records the vertices touching or cutting the other triangle, and the cut edge
/// on each face when there is one.
fn isect_tri_tri(
    s: &mut State,
    bm: &mut BMesh,
    a_index: i32,
    b_index: i32,
    a: [LoopId; 3],
    b: [LoopId; 3],
    no_shared: bool,
) {
    let f_a = bm.lp(a[0]).f;
    let f_b = bm.lp(b[0]).f;
    let fv_a = a.map(|l| bm.lp(l).v);
    let fv_b = b.map(|l| bm.lp(l).v);
    let f_a_cos = fv_a.map(|v| bm.vert(v).co);
    let f_b_cos = fv_b.map(|v| bm.vert(v).co);

    if no_shared {
        if fv_a.iter().any(|v| fv_b.contains(v)) {
            return;
        }
    } else if face_share_edge_check(bm, f_a, f_b) {
        return;
    }

    // `VERT_VISIT_A` / `VERT_VISIT_B`: membership of the two stacks.
    let mut iv_ls_a: SmallVec<[VertId; 8]> = SmallVec::new();
    let mut iv_ls_b: SmallVec<[VertId; 8]> = SmallVec::new();
    fn push_test(ls: &mut SmallVec<[VertId; 8]>, v: VertId) {
        if !ls.contains(&v) {
            ls.push(v);
        }
    }

    // Vert-vert: any vertices touching (no new vertex).
    for i_a in 0..3 {
        for i_b in 0..3 {
            if len_squared_v3v3(f_a_cos[i_a], f_b_cos[i_b]) <= s.epsilon.eps2x_sq {
                push_test(&mut iv_ls_a, fv_a[i_a]);
                push_test(&mut iv_ls_b, fv_b[i_b]);
            }
        }
    }

    // Vert-edge.
    for i_a in 0..3 {
        if iv_ls_a.contains(&fv_a[i_a]) {
            continue;
        }
        for i_b_e0 in 0..3 {
            let i_b_e1 = (i_b_e0 + 1) % 3;
            if iv_ls_b.contains(&fv_b[i_b_e0]) || iv_ls_b.contains(&fv_b[i_b_e1]) {
                continue;
            }
            let fac = line_point_factor_v3(f_a_cos[i_a], f_b_cos[i_b_e0], f_b_cos[i_b_e1]);
            if fac > 0.0 - s.epsilon.eps && fac < 1.0 + s.epsilon.eps {
                let ix = interp_v3_v3v3(f_b_cos[i_b_e0], f_b_cos[i_b_e1], fac);
                if len_squared_v3v3(ix, f_a_cos[i_a]) <= s.epsilon.eps2x_sq {
                    push_test(&mut iv_ls_b, fv_a[i_a]);
                    if let Some(e) = bm.edge_exists(fv_b[i_b_e0], fv_b[i_b_e1]) {
                        s.edge_verts_add(e, fv_a[i_a], true);
                    }
                    break;
                }
            }
        }
    }
    for i_b in 0..3 {
        if iv_ls_b.contains(&fv_b[i_b]) {
            continue;
        }
        for i_a_e0 in 0..3 {
            let i_a_e1 = (i_a_e0 + 1) % 3;
            if iv_ls_a.contains(&fv_a[i_a_e0]) || iv_ls_a.contains(&fv_a[i_a_e1]) {
                continue;
            }
            let fac = line_point_factor_v3(f_b_cos[i_b], f_a_cos[i_a_e0], f_a_cos[i_a_e1]);
            if fac > 0.0 - s.epsilon.eps && fac < 1.0 + s.epsilon.eps {
                let ix = interp_v3_v3v3(f_a_cos[i_a_e0], f_a_cos[i_a_e1], fac);
                if len_squared_v3v3(ix, f_b_cos[i_b]) <= s.epsilon.eps2x_sq {
                    push_test(&mut iv_ls_a, fv_b[i_b]);
                    if let Some(e) = bm.edge_exists(fv_a[i_a_e0], fv_a[i_a_e1]) {
                        s.edge_verts_add(e, fv_b[i_b], true);
                    }
                    break;
                }
            }
        }
    }

    // Vert-tri.
    {
        let t_scale = tri_v3_scale(f_b_cos, 1.0 - s.epsilon.eps2x);
        for i_a in 0..3 {
            if iv_ls_a.contains(&fv_a[i_a]) {
                continue;
            }
            if let Some(ix) = isect_point_tri_v3(f_a_cos[i_a], t_scale[0], t_scale[1], t_scale[2])
                && len_squared_v3v3(ix, f_a_cos[i_a]) <= s.epsilon.eps2x_sq
            {
                push_test(&mut iv_ls_a, fv_a[i_a]);
                push_test(&mut iv_ls_b, fv_a[i_a]);
            }
        }
    }
    {
        let t_scale = tri_v3_scale(f_a_cos, 1.0 - s.epsilon.eps2x);
        for i_b in 0..3 {
            if iv_ls_b.contains(&fv_b[i_b]) {
                continue;
            }
            if let Some(ix) = isect_point_tri_v3(f_b_cos[i_b], t_scale[0], t_scale[1], t_scale[2])
                && len_squared_v3v3(ix, f_b_cos[i_b]) <= s.epsilon.eps2x_sq
            {
                push_test(&mut iv_ls_a, fv_b[i_b]);
                push_test(&mut iv_ls_b, fv_b[i_b]);
            }
        }
    }

    if iv_ls_a.len() >= 3 && iv_ls_b.len() >= 3 {
        // Overlap.
        return;
    }

    let (f_a_nor, _) = normal_tri_v3(f_a_cos[0], f_a_cos[1], f_a_cos[2]);
    let (f_b_nor, _) = normal_tri_v3(f_b_cos[0], f_b_cos[1], f_b_cos[2]);

    // Edge-tri and edge-edge.
    for i_a_e0 in 0..3 {
        let i_a_e1 = (i_a_e0 + 1) % 3;
        if iv_ls_a.contains(&fv_a[i_a_e0]) || iv_ls_a.contains(&fv_a[i_a_e1]) {
            continue;
        }
        if let Some(iv) = isect_edge_tri(
            s,
            bm,
            fv_a[i_a_e0],
            fv_a[i_a_e1],
            fv_b,
            b_index,
            f_b_cos,
            f_b_nor,
        ) {
            push_test(&mut iv_ls_a, iv);
            push_test(&mut iv_ls_b, iv);
        }
    }
    for i_b_e0 in 0..3 {
        let i_b_e1 = (i_b_e0 + 1) % 3;
        if iv_ls_b.contains(&fv_b[i_b_e0]) || iv_ls_b.contains(&fv_b[i_b_e1]) {
            continue;
        }
        if let Some(iv) = isect_edge_tri(
            s,
            bm,
            fv_b[i_b_e0],
            fv_b[i_b_e1],
            fv_a,
            a_index,
            f_a_cos,
            f_a_nor,
        ) {
            push_test(&mut iv_ls_a, iv);
            push_test(&mut iv_ls_b, iv);
        }
    }

    for (ie_vs, f) in [(&iv_ls_a, f_a), (&iv_ls_b, f_b)] {
        if ie_vs.len() != 2 {
            continue;
        }
        // Possible but unlikely for an edge-edge intersection.
        let (ie, ie_exists) = match bm.edge_exists(ie_vs[0], ie_vs[1]) {
            None => {
                let ie = bm.edge_create(ie_vs[0], ie_vs[1], None, CreateFlag::empty());
                s.wire_edges.insert(ie);
                (ie, false)
            }
            Some(ie) => {
                // May already exist.
                s.wire_edges.insert(ie);
                if bm.edge_in_face(ie, f) {
                    continue;
                }
                (ie, true)
            }
        };
        let fi = s.face_index(f);
        s.face_edges_add(fi, ie, ie_exists);
    }
}

/// `isect_bvhtree_point_v3`: how many distinct depths (`FLT_EPSILON * 10` apart) the ray
/// from `co` along +X crosses the triangles of `tree`.
fn isect_bvhtree_point_v3(tree: &BvhTree, looptris: &[[V3; 3]], co: V3) -> i32 {
    let dir = [1.0f32, 0.0, 0.0];
    let mut z_buffer: Vec<f32> = Vec::new();
    // `raycast_callback`: never updates the hit, so every leaf the ray reaches is tested.
    tree.ray_cast(co, dir, f32::MAX, &mut |index, _dist| {
        let [v0, v1, v2] = looptris[index as usize];
        if let Some(dist) = isect_ray_tri_epsilon_v3(co, dir, v0, v1, v2, f32::EPSILON)
            && dist >= 0.0
        {
            z_buffer.push(dist);
        }
        false
    });
    match z_buffer.len() {
        0 => 0,
        1 => 1,
        _ => {
            let eps = f32::EPSILON * 10.0;
            let mut num_isect = 1;
            z_buffer.sort_by(|a, b| a.total_cmp(b));
            let mut depth_last = z_buffer[0];
            for &d in &z_buffer[1..] {
                if d - depth_last > eps {
                    depth_last = d;
                    num_isect += 1;
                }
            }
            num_isect
        }
    }
}

/// `bm_loop_filter_fn`: face groups step over an edge that is not tagged (a cut) and whose
/// faces are all on the same side (faces without a side are ignored).
fn loop_filter(bm: &BMesh, l: LoopId, test_fn: TestFn<'_>) -> bool {
    let lp = bm.lp(l);
    if bm.edge(lp.e).flag.contains(ElemFlag::TAG) {
        return false;
    }
    if lp.radial_next == l {
        return false;
    }
    let face_side = test_fn(bm, lp.f);
    let mut l_iter = lp.radial_next;
    while l_iter != l {
        let other = test_fn(bm, bm.lp(l_iter).f);
        if other != -1 && other != face_side {
            return false;
        }
        l_iter = bm.lp(l_iter).radial_next;
    }
    true
}

/// `BM_mesh_intersect`.
pub fn intersect(bm: &mut BMesh, i: &IntersectIn<'_>) -> Result<IntersectOut, BmError> {
    let looptris = i.looptris;
    let test_fn = i.test_fn;
    let eps = i.eps;
    let boolean = i.boolean_mode != BooleanMode::None;

    // `BM_mesh_elem_index_ensure(VERT | EDGE | FACE)` and the face table.
    let mut vidx = vec![-1i32; bm.verts.capacity()];
    for (n, v) in bm.vert_ids().into_iter().enumerate() {
        vidx[v.slot()] = n as i32;
    }
    let mut fidx = vec![-1i32; bm.faces.capacity()];
    let mut ftable: Vec<Option<FaceId>> = Vec::with_capacity(bm.faces_num());
    for (n, f) in bm.face_ids().into_iter().enumerate() {
        fidx[f.slot()] = n as i32;
        ftable.push(Some(f));
    }
    let eps2x = eps * 2.0;
    let eps_margin = eps2x * 10.0;
    let mut s = State {
        edgetri_cache: FxHashMap::default(),
        edge_verts: LinkMap::new(),
        face_edges: LinkMap::new(),
        face_order: ghash::IntKeyOrder::new(),
        wire_edges: FxHashSet::default(),
        vert_dissolve: Vec::new(),
        epsilon: Epsilon {
            eps,
            eps2x,
            eps2x_sq: eps2x * eps2x,
            eps_margin,
            eps_margin_sq: eps_margin * eps_margin,
        },
        vidx,
        fidx,
    };

    if i.use_dissolve {
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
    }

    // The triangles' original coordinates (the ray-cast callbacks read them after the cut).
    let tri_cos: Vec<[V3; 3]> = looptris
        .iter()
        .map(|t| t.map(|l| bm.vert(bm.lp(l).v).co))
        .collect();
    let tri_side: Vec<i32> = looptris
        .iter()
        .map(|t| test_fn(bm, bm.lp(t[0]).f))
        .collect();

    let build = |side: i32| {
        let mut tree = BvhTree::new(s.epsilon.eps_margin, 8, 8);
        for (n, t) in tri_cos.iter().enumerate() {
            frap_math::budget::check();
            if tri_side[n] == side {
                tree.insert(n as i32, t);
            }
        }
        tree.balance();
        tree
    };
    let tree_a = build(0);
    let tree_b_own = (!i.use_self).then(|| build(1));
    let tree_b = tree_b_own.as_ref().unwrap_or(&tree_a);

    // For self intersection allow shared vertices, not shared edges (#75946).
    let isect_tri_tri_no_shared = boolean;

    // `BLI_bvhtree_overlap_ex(tree_b, tree_a, ..., USE_THREADING | RETURN_PAIRS)`: every
    // "thread" pushes its pairs on a stack and `BLI_stack_pop_n` empties them newest first,
    // thread after thread.
    let mut per_thread: Vec<Vec<(i32, i32)>> = Vec::new();
    tree_b.overlap_ex(&tree_a, 0, true, &mut |a, b, thread| {
        frap_math::budget::check();
        if per_thread.len() <= thread {
            per_thread.resize_with(thread + 1, Vec::new);
        }
        per_thread[thread].push((a, b));
        true
    });
    let overlap: Vec<(i32, i32)> = per_thread
        .into_iter()
        .flat_map(|t| t.into_iter().rev())
        .collect();
    for &(ia, ib) in &overlap {
        frap_math::budget::check();
        isect_tri_tri(
            &mut s,
            bm,
            ia,
            ib,
            looptris[ia as usize],
            looptris[ib as usize],
            isect_tri_tri_no_shared,
        );
    }

    // ---- USE_SPLICE: splice the vertices into the edges.
    for (e, verts) in std::mem::take(&mut s.edge_verts.entries) {
        frap_math::budget::check_n(verts.len() as u32);
        let v_start = bm.edge(e).v[0];
        let mut vs = list_order(&verts);
        if vs.len() > 1 {
            // `edge_verts_sort`: by distance from the start.
            let co = bm.vert(v_start).co;
            let mut keyed: Vec<(f32, VertId)> = vs
                .iter()
                .map(|&v| (len_squared_v3v3(co, bm.vert(v).co), v))
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            vs = keyed.into_iter().map(|(_, v)| v).collect();
        }
        let is_wire = s.wire_edges.contains(&e);
        let mut v_prev = v_start;
        for vi in vs {
            let [ev1, ev2] = bm.edge(e).v;
            let fac = line_point_factor_v3(bm.vert(vi).co, bm.vert(ev1).co, bm.vert(ev2).co);
            if bm.vert_in_edge(e, v_prev) {
                let (v_new, e_split) = bm.edge_split(e, v_prev, fac.clamp(0.0, 1.0));
                v_prev = v_new;
                if bm.edge_exists(v_prev, vi).is_none()
                    && !vert_pair_share_face_check(bm, v_prev, vi)
                    && !vert_splice_check_double_edge(bm, v_prev, vi)
                    && !vert_splice_check_double_face(bm, v_prev, vi)
                {
                    bm.vert_splice(vi, v_prev);
                } else {
                    let co = bm.vert(vi).co;
                    bm.vert_mut(v_prev).co = co;
                }
                v_prev = vi;
                if is_wire {
                    s.wire_edges.insert(e_split);
                }
            }
        }
    }

    // ---- USE_DISSOLVE (important to handle before the edge-net).
    if i.use_dissolve && !boolean {
        dissolve_cut_chains(&mut s, bm, &mut ftable);
    }

    // ---- USE_NET: split the faces.
    let face_edges_len = s.face_edges.len();
    let mut face_lists = std::mem::take(&mut s.face_edges.entries);
    for f_index in s.face_order.iter_order() {
        let edges = std::mem::take(&mut face_lists[s.face_edges.index[&f_index]].1);
        frap_math::budget::check_n(edges.len() as u32);
        let Some(Some(f)) = ftable.get(f_index as usize).copied() else {
            continue;
        };
        if !bm.faces.contains(f.0) {
            continue;
        }
        let mut edge_arr = list_order(&edges);
        edge_arr.retain(|e| bm.edges.contains(e.0));
        if i.use_island_connect
            && let Some(net) =
                face_split_edgenet_connect_islands(bm, f, &edge_arr, i.use_partial_connect)
        {
            edge_arr = net;
        }
        face_split_edgenet(bm, f, &edge_arr);
    }

    // ---- USE_SEPARATE.
    let wire: Vec<EdgeId> = s
        .wire_edges
        .iter()
        .copied()
        .filter(|e| bm.edges.contains(e.0))
        .collect();
    if i.use_separate {
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        for &e in &wire {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
        mesh_edgesplit(bm, false, true, false);
    } else if boolean || i.use_edge_tag {
        // No need to clear for boolean.
        for &e in &wire {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
    }

    let mut has_edit_boolean = false;
    if boolean {
        let ftable2 = bm.face_ids();
        let filter = |bm: &BMesh, l: LoopId| loop_filter(bm, l, test_fn);
        let groups = mesh_calc_face_groups(bm, &filter);
        let tree_pair = [&tree_a, tree_b];
        for group in &groups {
            frap_math::budget::check_n(group.len() as u32);
            let Some(&f) = group.first() else {
                continue;
            };
            let side = test_fn(bm, f);
            if side == -1 {
                continue;
            }
            let side = usize::from(side == 0);
            let co = face_calc_point_in_face(bm, f);
            let hits = isect_bvhtree_point_v3(tree_pair[side], &tri_cos, co);
            let odd = (hits & 1) == 1;
            let (do_remove, do_flip) = match i.boolean_mode {
                BooleanMode::Intersect => (!odd, false),
                BooleanMode::Union => (odd, false),
                BooleanMode::Difference => (odd == (side == 1), side == 0),
                BooleanMode::None => unreachable!(),
            };
            if do_remove {
                // Postpone killing the faces (the ray casts read the mesh): mark them.
                for &g in group {
                    bm.face_mut(g).mat_nr = -1;
                }
            } else if do_flip {
                for &g in group {
                    bm.face_normal_flip(g);
                }
            }
            has_edit_boolean |= do_flip || do_remove;
        }

        if i.use_dissolve {
            // The dissolve for booleans, after the classification.
            for v in list_order(&s.vert_dissolve) {
                frap_math::budget::check();
                if !bm.verts.contains(v.0) || !vert_is_edge_pair(bm, v) {
                    continue;
                }
                let ve = bm.vert(v).e.expect("edge pair");
                // Would dissolving make a 2-sided face?
                let mut ok = !bm
                    .edge_loops(ve)
                    .into_iter()
                    .any(|l| bm.face(bm.lp(l).f).len == 3);
                // Would it make a duplicate face (degenerate input, #150360)?
                if ok && vert_collapse_check_double_face(bm, v) {
                    ok = false;
                }
                if ok {
                    vert_collapse_edge(bm, ve, v, true, false, false);
                }
            }
        }

        for f in ftable2 {
            if bm.faces.contains(f.0) && bm.face(f).mat_nr == -1 {
                bm.face_kill_loose(f);
            }
        }
    }

    let has_edit_isect = face_edges_len != 0;
    // The selection history is unlikely to be useful any more (#86799).
    bm.select_history.clear();
    bm.index.dirty = crate::ElemTypes::all();
    Ok(IntersectOut {
        changed: has_edit_isect || has_edit_boolean,
    })
}

/// The first `USE_DISSOLVE` pass (no boolean): removes the cut vertices only the
/// triangulation needed, walking each chain of wire edges between two kept vertices and
/// splicing its ends together instead.
fn dissolve_cut_chains(s: &mut State, bm: &mut BMesh, ftable: &mut [Option<FaceId>]) {
    let tag = |bm: &BMesh, v: VertId| bm.vert(v).flag.contains(ElemFlag::TAG);
    let etag = |bm: &BMesh, e: EdgeId| bm.edge(e).flag.contains(ElemFlag::TAG);
    let dissolve = list_order(&s.vert_dissolve);
    for &v in &dissolve {
        if bm.verts.contains(v.0) && tag(bm, v) && !vert_is_edge_pair(bm, v) {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
    }

    let mut splice_ls: Vec<[VertId; 2]> = Vec::with_capacity(s.wire_edges.len());
    for &v in &dissolve {
        frap_math::budget::check();
        if !bm.verts.contains(v.0) || !tag(bm, v) {
            continue;
        }
        // The chain.
        let e0 = bm.vert(v).e.expect("edge pair");
        let e1 = bm.disk_edge_next(e0, v);
        if etag(bm, e0) || etag(bm, e1) {
            continue;
        }
        // The vertex may be on an existing face edge that doesn't divide the face: those
        // edges are not wire and are not handled here (#63787).
        if !s.wire_edges.contains(&e0) || !s.wire_edges.contains(&e1) {
            continue;
        }
        let mut v_a = bm.edge_other_vert(e0, v);
        let mut v_b = bm.edge_other_vert(e1, v);
        if tag(bm, v_a) && tag(bm, v_b) {
            // Only start on an end.
        } else if !tag(bm, v_a) && !tag(bm, v_b) {
            // A single edge spans the face.
            bm.edge_mut(e1).flag.insert(ElemFlag::TAG);
            splice_ls.push([v, v_b]);
        } else {
            let e = if tag(bm, v_a) {
                e0
            } else {
                std::mem::swap(&mut v_a, &mut v_b);
                e1
            };
            let _ = (v_a, v_b);
            // Walk the chain.
            let mut v_step = v;
            let mut e_step = e;
            loop {
                let v_next = bm.edge_other_vert(e_step, v_step);
                bm.edge_mut(e_step).flag.insert(ElemFlag::TAG);
                if !tag(bm, v_next) {
                    splice_ls.push([v, v_next]);
                    break;
                }
                let e_next = vert_other_edge(bm, v_next, e_step);
                e_step = e_next;
                v_step = v_next;
                bm.edge_mut(e_step).flag.insert(ElemFlag::TAG);
            }
        }
    }

    // Remove the edges from the face lists.
    for (_, list) in &mut s.face_edges.entries {
        list.retain(|&e| !(bm.edges.contains(e.0) && etag(bm, e)));
    }

    for e in bm.edge_ids() {
        if !bm.edges.contains(e.0) || !etag(bm, e) {
            continue;
        }
        // In rare cases faces of `face_edges` are removed with the edges (degenerate input):
        // make sure they are not referenced again.
        for l in bm.edge_loops(e) {
            let fi = s.face_index(bm.lp(l).f);
            if fi >= 0
                && let Some(slot) = ftable.get_mut(fi as usize)
            {
                *slot = None;
            }
        }
        s.wire_edges.remove(&e);
        bm.edge_kill(e);
    }

    // Remove the vertices.
    let mut verts_invalid: FxHashSet<VertId> = FxHashSet::default();
    for &v in &dissolve {
        if bm.verts.contains(v.0) && tag(bm, v) && bm.vert(v).e.is_none() {
            verts_invalid.insert(v);
            bm.vert_kill(v);
        }
    }
    for [a, b] in splice_ls {
        if verts_invalid.contains(&a) || verts_invalid.contains(&b) {
            continue;
        }
        if !bm.verts.contains(a.0) || !bm.verts.contains(b.0) || a == b {
            continue;
        }
        if bm.edge_exists(a, b).is_none()
            && !vert_splice_check_double_edge(bm, a, b)
            && !vert_splice_check_double_face(bm, a, b)
        {
            bm.vert_splice(b, a);
        }
    }
}
