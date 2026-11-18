// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_intersect_edges` (Auto Merge with split): finds the vertices closer than `dist` to
//! another vertex or to an edge, and the edges crossing other edges, where at least one side
//! is tagged (`select_only`: selected); splits the crossed edges at the crossings, fills
//! `targetmap` with the vertex pairs to weld (`weld_verts`' map: every key points at the last
//! vertex of its chain), and with `split_faces` splits the faces that the wire edges or edge
//! chains moved onto them cross (`BM_face_split_edgenet`).
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_intersect_edges.cc (Blender 5.2.4,
//! GPL-2.0-or-later), including the order Blender finds the pairs in: the k-DOP BVH overlap
//! traversal (`BLI_bvhtree_overlap_ex` with one interaction per leaf and per-root-child
//! "thread" stacks above 1024 leaves), the cuts of an edge sorted by factor with the C
//! `qsort`, and the `GHash` semantics of the target map (a later entry for a key shadows the
//! earlier one). Element indices are used as in Blender as scratch storage
//! (`BM_elem_index_set`), so the vertex and edge indices are dirty afterwards.

use crate::ops::contextual_create::bli::c_qsort;
use crate::ops::contextual_create::kdopbvh::BvhTree;
use crate::ops::contextual_create::split_edgenet::face_split_edgenet;
use crate::query::geom;
use crate::{BMElem, BMesh, CreateFlag, EdgeId, ElemFlag, FaceId, LoopId, VertId};

type V3 = [f32; 3];

/// `KDOP_TREE_TYPE`.
const KDOP_TREE_TYPE: usize = 4;
/// `KDOP_AXIS_LEN`.
const KDOP_AXIS_LEN: usize = 14;
/// `BLI_STACK_PAIR_LEN`.
const STACK_PAIR_LEN: usize = 2 * KDOP_TREE_TYPE;
/// `EDGE_ACT_TO_TEST`.
const EDGE_ACT_TO_TEST: i32 = 1;
/// `EDGE_REMAIN_TO_TEST`.
const EDGE_REMAIN_TO_TEST: i32 = 2;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len_squared(a: V3) -> f32 {
    dot(a, a)
}
fn madd(a: V3, d: V3, f: f32) -> V3 {
    [a[0] + d[0] * f, a[1] + d[1] * f, a[2] + d[2] * f]
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc.
/// `isect_ray_ray_epsilon_v3`: the factors of the closest points of two rays (`None` when
/// parallel within `epsilon`).
fn isect_ray_ray_epsilon_v3(
    co_a: V3,
    dir_a: V3,
    co_b: V3,
    dir_b: V3,
    eps: f32,
) -> Option<(f32, f32)> {
    let n = cross(dir_b, dir_a);
    let nlen = len_squared(n);
    if nlen < eps {
        return None;
    }
    let t = sub(co_b, co_a);
    let c = sub(n, t);
    let lambda_a = dot(cross(c, dir_b), n) / nlen;
    let lambda_b = dot(cross(c, dir_a), n) / nlen;
    Some((lambda_a, lambda_b))
}

/// `ray_point_factor_v3_ex`.
fn ray_point_factor_v3_ex(p: V3, co: V3, dir: V3, eps: f32, fallback: f32) -> f32 {
    let rel = sub(p, co);
    let d = len_squared(dir);
    if d > eps { dot(dir, rel) / d } else { fallback }
}

/// `isect_point_poly_v2`.
fn isect_point_poly_v2(pt: [f32; 2], verts: &[[f32; 2]]) -> bool {
    let nr = verts.len();
    let mut isect = false;
    let mut j = nr - 1;
    for i in 0..nr {
        if ((verts[i][1] > pt[1]) != (verts[j][1] > pt[1]))
            && (pt[0]
                < (verts[j][0] - verts[i][0]) * (pt[1] - verts[i][1]) / (verts[j][1] - verts[i][1])
                    + verts[i][0])
        {
            isect = !isect;
        }
        j = i;
    }
    isect
}

/// `BM_face_point_inside_test` (with the face's stored normal).
fn face_point_inside_test(bm: &BMesh, f: FaceId, co: V3) -> bool {
    let rows = geom::axis_dominant_v3_rows(bm.face(f).no);
    let co_2d = geom::project_v2(&rows, co);
    let proj: Vec<[f32; 2]> = bm
        .face_verts(f)
        .into_iter()
        .map(|v| geom::project_v2(&rows, bm.vert(v).co))
        .collect();
    isect_point_poly_v2(co_2d, &proj)
}

/// `BM_vert_pair_share_face_check`.
fn vert_pair_share_face_check(bm: &BMesh, a: VertId, b: VertId) -> bool {
    if bm.vert(a).e.is_none() || bm.vert(b).e.is_none() {
        return false;
    }
    bm.vert_faces(a)
        .into_iter()
        .any(|f| bm.face_vert_share_loop(f, b).is_some())
}

/// `BM_vert_pair_shared_face_cb`: the first face around `v_a` (loop order) holding `v_b`
/// for which `cb(f, l_a, l_b)` holds.
fn vert_pair_shared_face_cb(
    bm: &BMesh,
    v_a: VertId,
    v_b: VertId,
    allow_adjacent: bool,
    cb: &mut dyn FnMut(&BMesh, FaceId, LoopId, LoopId) -> bool,
) -> Option<FaceId> {
    if bm.vert(v_a).e.is_none() || bm.vert(v_b).e.is_none() {
        return None;
    }
    for l_a in bm.vert_loops(v_a) {
        let f = bm.lp(l_a).f;
        if let Some(l_b) = bm.face_vert_share_loop(f, v_b) {
            let adjacent = bm.lp(l_a).next == l_b || bm.lp(l_a).prev == l_b;
            if (allow_adjacent || !adjacent) && cb(bm, f, l_a, l_b) {
                return Some(f);
            }
        }
    }
    None
}

/// `bm_vert_pair_share_splittable_face_cb`: the segment `v_a`-`v_b` crosses the edges next
/// to `l_a` or `l_b`.
fn share_splittable_face(bm: &BMesh, l_a: LoopId, l_b: LoopId, v_a_co: V3, v_a_b_dir: V3) -> bool {
    let (range_min, range_max) = (-f32::EPSILON, 1.0 + f32::EPSILON);
    let co_of = |l: LoopId| bm.vert(bm.lp(l).v).co;
    let la = bm.lp(l_a);
    let co = co_of(la.prev);
    let dir = sub(co_of(la.next), co);
    // `IN_RANGE` is exclusive.
    if let Some((_, lambda_b)) =
        isect_ray_ray_epsilon_v3(v_a_co, v_a_b_dir, co, dir, f32::MIN_POSITIVE)
    {
        if lambda_b > range_min && lambda_b < range_max {
            return true;
        }
        let lb = bm.lp(l_b);
        let co = co_of(lb.prev);
        let dir = sub(co_of(lb.next), co);
        if let Some((_, lambda_b)) =
            isect_ray_ray_epsilon_v3(v_a_co, v_a_b_dir, co, dir, f32::MIN_POSITIVE)
        {
            return lambda_b > range_min && lambda_b < range_max;
        }
    }
    false
}

/// `bm_vert_pair_best_face_get`.
fn vert_pair_best_face_get(
    bm: &BMesh,
    v_a: VertId,
    v_b: VertId,
    edgenet: &[EdgeId],
    epsilon: f32,
) -> Option<FaceId> {
    debug_assert_ne!(v_a, v_b);
    if edgenet.len() == 1 {
        let co_b = bm.vert(v_b).co;
        let dir = sub(bm.vert(v_a).co, co_b);
        return vert_pair_shared_face_cb(bm, v_a, v_b, false, &mut |bm, _f, l_a, l_b| {
            share_splittable_face(bm, l_a, l_b, co_b, dir)
        });
    }
    // `bm_vert_pair_share_best_splittable_face_cb`: the face that contains every vertex of
    // the net and over whose normal the net spans the least.
    let mut best_range = f32::MAX;
    let mut best: Option<FaceId> = None;
    vert_pair_shared_face_cb(bm, v_a, v_b, true, &mut |bm, f, l_a, l_b| {
        let no = bm.face(f).no;
        let mut min = dot(bm.vert(bm.lp(l_a).v).co, no);
        let mut max = dot(bm.vert(bm.lp(l_b).v).co, no);
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        let e0 = bm.edge(edgenet[0]);
        let e1 = bm.edge(edgenet[1]);
        let mut v_test = if e0.v[0] == e1.v[0] || e0.v[0] == e1.v[1] {
            e0.v[1]
        } else {
            e0.v[0]
        };
        for &e in &edgenet[..edgenet.len() - 1] {
            v_test = bm.edge_other_vert(e, v_test);
            let co = bm.vert(v_test).co;
            if !face_point_inside_test(bm, f, co) {
                return false;
            }
            let d = dot(co, no);
            min = min.min(d);
            max = max.max(d);
        }
        let range = max - min;
        if range < best_range {
            best_range = range;
            best = Some(f);
        }
        false
    });
    if let Some(f) = best {
        // The net must span less than the face over the normal.
        let no = bm.face(f).no;
        let (mut min, mut max) = (f32::MAX, -f32::MAX);
        for v in bm.face_verts(f) {
            let d = dot(bm.vert(v).co, no);
            min = min.min(d);
            max = max.max(d);
        }
        let face_range = max - min + 2.0 * epsilon;
        if face_range < best_range {
            best = None;
        }
    }
    best
}

/// `EDBMSplitElem`: a vertex, or an edge and the factor along it (until the edge is cut and
/// the element becomes the cut's vertex).
#[derive(Copy, Clone, Debug)]
enum Elem {
    Vert(VertId),
    Edge(EdgeId, f32),
}

impl Elem {
    fn vert(self) -> VertId {
        match self {
            Elem::Vert(v) => v,
            Elem::Edge(..) => unreachable!("every edge element was cut into a vertex"),
        }
    }
}

/// Scratch state of the overlap callbacks (`EDBMSplitData`).
struct SplitData {
    verts: Vec<VertId>,
    edges: Vec<EdgeId>,
    stacks: [Vec<[Elem; 2]>; STACK_PAIR_LEN],
    dist_sq: f32,
    dist_sq_sq: f32,
}

/// `bm_edge_pair_elem_setup`: counts the edge's cuts in its index.
fn edge_elem(bm: &mut BMesh, e: EdgeId, lambda: f32) -> Elem {
    let n = bm.index_get(BMElem::Edge(e));
    bm.index_set(BMElem::Edge(e), n + 1);
    Elem::Edge(e, lambda)
}

/// `bm_edgexvert_isect_impl`.
fn edgexvert_isect(
    bm: &mut BMesh,
    v: VertId,
    e: EdgeId,
    co: V3,
    dir: V3,
    lambda: f32,
    dist_sq: f32,
) -> Option<[Elem; 2]> {
    if !(0.0..=1.0).contains(&lambda) {
        return None;
    }
    let ev = bm.edge(e).v;
    let (e_v, factor) = if lambda < 0.5 {
        (ev[0], lambda)
    } else {
        (ev[1], 1.0 - lambda)
    };
    if v != e_v {
        let dist_sq_vert = factor * factor * len_squared(dir);
        if dist_sq_vert < dist_sq {
            return None;
        }
        let closest = madd(co, dir, lambda);
        if len_squared(sub(bm.vert(v).co, closest)) < dist_sq {
            return Some([edge_elem(bm, e, lambda), Elem::Vert(v)]);
        }
    }
    None
}

/// `bm_edgexedge_isect_impl`.
#[allow(clippy::too_many_arguments)]
fn edgexedge_isect(
    bm: &mut BMesh,
    d: &SplitData,
    e_a: EdgeId,
    e_b: EdgeId,
    co_a: V3,
    dir_a: V3,
    co_b: V3,
    dir_b: V3,
    lambda_a: f32,
    lambda_b: f32,
) -> Option<[Elem; 2]> {
    let (va, vb) = (bm.edge(e_a).v, bm.edge(e_b).v);
    let (e_a_v, fa) = if lambda_a < 0.5 {
        (va[0], lambda_a)
    } else {
        (va[1], 1.0 - lambda_a)
    };
    let (e_b_v, fb) = if lambda_b < 0.5 {
        (vb[0], lambda_b)
    } else {
        (vb[1], 1.0 - lambda_b)
    };
    if e_a_v != e_b_v {
        if !(0.0..=1.0).contains(&lambda_a) || !(0.0..=1.0).contains(&lambda_b) {
            return None;
        }
        let dist_sq_va = fa * fa * len_squared(dir_a);
        let dist_sq_vb = fb * fb * len_squared(dir_b);
        if dist_sq_va < d.dist_sq || dist_sq_vb < d.dist_sq {
            return None;
        }
        let near_a = madd(co_a, dir_a, lambda_a);
        let near_b = madd(co_b, dir_b, lambda_b);
        if len_squared(sub(near_a, near_b)) < d.dist_sq {
            return Some([edge_elem(bm, e_a, lambda_a), edge_elem(bm, e_b, lambda_b)]);
        }
    }
    None
}

/// Which overlap callback a traversal runs.
#[derive(Copy, Clone)]
enum Cb {
    VertVertSelf,
    VertVert,
    EdgeVert,
    EdgeEdgeSelf,
    EdgeEdge,
}

/// The overlap callbacks (`bm_*_isect_cb`): push the pair on the thread's stack and return
/// whether it counts as an interaction (vertex pairs only).
fn overlap(bm: &mut BMesh, d: &mut SplitData, cb: Cb, a: i32, b: i32, stack: usize) -> bool {
    let (a, b) = (a as usize, b as usize);
    match cb {
        Cb::VertVertSelf | Cb::VertVert => {
            if matches!(cb, Cb::VertVertSelf) && a >= b {
                return false;
            }
            d.stacks[stack].push([Elem::Vert(d.verts[a]), Elem::Vert(d.verts[b])]);
            true
        }
        Cb::EdgeVert => {
            let (e, v) = (d.edges[a], d.verts[b]);
            let ev = bm.edge(e).v;
            let co = bm.vert(ev[0]).co;
            let dir = sub(bm.vert(ev[1]).co, co);
            let lambda = ray_point_factor_v3_ex(bm.vert(v).co, co, dir, 0.0, -1.0);
            if let Some(p) = edgexvert_isect(bm, v, e, co, dir, lambda, d.dist_sq) {
                d.stacks[stack].push(p);
            }
            false
        }
        Cb::EdgeEdgeSelf | Cb::EdgeEdge => {
            if matches!(cb, Cb::EdgeEdgeSelf) && a >= b {
                return false;
            }
            let (e_a, e_b) = (d.edges[a], d.edges[b]);
            let (va, vb) = (bm.edge(e_a).v, bm.edge(e_b).v);
            if va[0] == vb[0] || va[0] == vb[1] || va[1] == vb[0] || va[1] == vb[1] {
                return false;
            }
            let co_a = bm.vert(va[0]).co;
            let dir_a = sub(bm.vert(va[1]).co, co_a);
            let co_b = bm.vert(vb[0]).co;
            let dir_b = sub(bm.vert(vb[1]).co, co_b);
            if let Some((la, lb)) = isect_ray_ray_epsilon_v3(co_a, dir_a, co_b, dir_b, d.dist_sq_sq)
                && let Some(p) = edgexedge_isect(bm, d, e_a, e_b, co_a, dir_a, co_b, dir_b, la, lb)
            {
                d.stacks[stack].push(p);
            }
            false
        }
    }
}

/// `bm_elemxelem_bvhtree_overlap`: one overlap query into the given half of the stacks.
fn bvh_overlap(
    bm: &mut BMesh,
    d: &mut SplitData,
    t1: &BvhTree,
    t2: &BvhTree,
    cb: Cb,
    stack_base: usize,
) {
    t1.overlap_ex(t2, 1, true, &mut |a, b, thread| {
        overlap(bm, d, cb, a, b, stack_base + thread)
    });
}

/// `BLI_ghash` with Blender's duplicate-key behaviour: lookups see the latest insert.
#[derive(Default)]
struct GhashMap {
    entries: Vec<(VertId, VertId)>,
    latest: rustc_hash::FxHashMap<VertId, usize>,
}

impl GhashMap {
    fn insert(&mut self, k: VertId, v: VertId) {
        self.latest.insert(k, self.entries.len());
        self.entries.push((k, v));
    }
    fn lookup(&self, k: VertId) -> Option<VertId> {
        self.latest.get(&k).map(|&i| self.entries[i].1)
    }
    fn set(&mut self, k: VertId, v: VertId) {
        if let Some(&i) = self.latest.get(&k) {
            self.entries[i].1 = v;
        }
    }
}

/// `BM_mesh_intersect_edges(bm, hflag, dist, split_faces, r_targetmap)`: returns whether
/// anything was found; `targetmap` receives the vertex pairs to weld (`weld_verts`' map
/// format, insertion order; a later entry for a key replaces an earlier one).
/// `select_only`: the tested elements are the selected ones (`hflag = BM_ELEM_SELECT`;
/// otherwise every visible vertex is tested).
pub fn intersect_edges(
    bm: &mut BMesh,
    select_only: bool,
    dist: f32,
    split_faces: bool,
    targetmap: &mut Vec<(VertId, VertId)>,
) -> bool {
    // `BM_ELEM_SELECT`, or `BM_ELEM_SELECT | BM_ELEM_TAG` (the transform's mirrored vertices
    // tagged by the caller).
    let hflag = if select_only {
        ElemFlag::SELECT
    } else {
        ElemFlag::SELECT | ElemFlag::TAG
    };
    let dist_sq = dist * dist;
    let dist_half = dist / 2.0;
    let mut d = SplitData {
        verts: bm.vert_ids(),
        edges: bm.edge_ids(),
        stacks: Default::default(),
        dist_sq,
        dist_sq_sq: dist_sq * dist_sq,
    };

    // Tag and count the vertices to test.
    let (mut verts_act_len, mut verts_remain_len) = (0, 0);
    for &v in &d.verts {
        let flag = bm.vert(v).flag;
        if flag.intersects(hflag) {
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
            verts_act_len += 1;
        } else {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
            if !flag.contains(ElemFlag::HIDDEN) {
                verts_remain_len += 1;
            }
        }
        bm.index_set(BMElem::Vert(v), -1);
    }

    let mut tree_verts_act =
        (verts_act_len > 0).then(|| BvhTree::new(dist_half, KDOP_TREE_TYPE, KDOP_AXIS_LEN));
    let mut tree_verts_remain =
        (verts_remain_len > 0).then(|| BvhTree::new(dist_half, KDOP_TREE_TYPE, KDOP_AXIS_LEN));
    if tree_verts_act.is_some() || tree_verts_remain.is_some() {
        for (i, &v) in d.verts.iter().enumerate() {
            let flag = bm.vert(v).flag;
            if flag.contains(ElemFlag::TAG) {
                if let Some(t) = &mut tree_verts_act {
                    t.insert(i as i32, &[bm.vert(v).co]);
                }
            } else if !flag.contains(ElemFlag::HIDDEN)
                && let Some(t) = &mut tree_verts_remain
            {
                t.insert(i as i32, &[bm.vert(v).co]);
            }
        }
        if let Some(t) = &mut tree_verts_act {
            t.balance();
            let t = t.clone();
            bvh_overlap(bm, &mut d, &t, &t, Cb::VertVertSelf, 0);
        }
        if let Some(t) = &mut tree_verts_remain {
            t.balance();
        }
        if let (Some(ta), Some(tr)) = (&tree_verts_act, &tree_verts_remain) {
            bvh_overlap(bm, &mut d, tr, ta, Cb::VertVert, 0);
        }
    }
    let vertxvert_pair_len: usize = d.stacks[..KDOP_TREE_TYPE].iter().map(Vec::len).sum();
    let mut pair_len = vertxvert_pair_len;

    // Tag and count the edges.
    let (mut edges_act_len, mut edges_remain_len) = (0, 0);
    for &e in &d.edges {
        let ev = bm.edge(e).v;
        let hidden = bm.edge(e).flag.contains(ElemFlag::HIDDEN);
        if hidden || len_squared(sub(bm.vert(ev[0]).co, bm.vert(ev[1]).co)) < dist_sq {
            // Hidden or shorter than the distance: handled by the vertex overlap.
            bm.index_set(BMElem::Edge(e), 0);
            if split_faces {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
            continue;
        }
        if bm.vert(ev[0]).flag.contains(ElemFlag::TAG)
            || bm.vert(ev[1]).flag.contains(ElemFlag::TAG)
        {
            bm.index_set(BMElem::Edge(e), EDGE_ACT_TO_TEST);
            edges_act_len += 1;
        } else {
            bm.index_set(BMElem::Edge(e), EDGE_REMAIN_TO_TEST);
            edges_remain_len += 1;
            if split_faces {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
        }
    }

    let mut pairs: Vec<[Elem; 2]> = Vec::new();
    let mut tree_edges_act =
        (edges_act_len > 0).then(|| BvhTree::new(dist_half, KDOP_TREE_TYPE, KDOP_AXIS_LEN));
    let mut tree_edges_remain = (edges_remain_len > 0
        && (tree_edges_act.is_some() || tree_verts_act.is_some()))
    .then(|| BvhTree::new(dist_half, KDOP_TREE_TYPE, KDOP_AXIS_LEN));

    if tree_edges_act.is_some() || tree_edges_remain.is_some() {
        for (i, &e) in d.edges.iter().enumerate() {
            let test = bm.index_get(BMElem::Edge(e));
            let ev = bm.edge(e).v;
            let co = [bm.vert(ev[0]).co, bm.vert(ev[1]).co];
            if test == EDGE_ACT_TO_TEST {
                bm.index_set(BMElem::Edge(e), 0);
                if let Some(t) = &mut tree_edges_act {
                    t.insert(i as i32, &co);
                }
            } else if test == EDGE_REMAIN_TO_TEST {
                bm.index_set(BMElem::Edge(e), 0);
                if let Some(t) = &mut tree_edges_remain {
                    t.insert(i as i32, &co);
                }
            }
            // The tag marks the edges already mapped when converting the pairs.
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        if let Some(t) = &mut tree_edges_act {
            t.balance();
        }
        if let Some(t) = &mut tree_edges_remain {
            t.balance();
        }
        let base = KDOP_TREE_TYPE;
        if let Some(tea) = &tree_edges_act {
            bvh_overlap(bm, &mut d, tea, tea, Cb::EdgeEdgeSelf, base);
            if let Some(ter) = &tree_edges_remain {
                bvh_overlap(bm, &mut d, ter, tea, Cb::EdgeEdge, base);
            }
            if let Some(tva) = &tree_verts_act {
                bvh_overlap(bm, &mut d, tea, tva, Cb::EdgeVert, base);
            }
            if let Some(tvr) = &tree_verts_remain {
                bvh_overlap(bm, &mut d, tea, tvr, Cb::EdgeVert, base);
            }
        }
        if let (Some(tva), Some(ter)) = (&tree_verts_act, &tree_edges_remain) {
            bvh_overlap(bm, &mut d, ter, tva, Cb::EdgeVert, base);
        }
        let edgexelem_pair_len: usize = d.stacks[base..].iter().map(Vec::len).sum();
        pair_len += edgexelem_pair_len;

        if edgexelem_pair_len > 0 {
            // The pair array: every stack in order (each in push order).
            for s in &d.stacks {
                pairs.extend_from_slice(s);
            }
            // Map the cuts per edge, edges in order of first appearance.
            let mut e_map: Vec<(EdgeId, Vec<usize>)> = Vec::new();
            let mut e_slot: rustc_hash::FxHashMap<EdgeId, usize> = Default::default();
            let flat_start = 2 * vertxvert_pair_len;
            for i in 0..2 * edgexelem_pair_len {
                let fi = flat_start + i;
                let Elem::Edge(e, _) = pairs[fi / 2][fi % 2] else {
                    continue;
                };
                if !bm.edge(e).flag.contains(ElemFlag::TAG) {
                    bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                    e_slot.insert(e, e_map.len());
                    e_map.push((e, vec![i]));
                } else {
                    let s = e_slot[&e];
                    e_map[s].1.push(i);
                }
            }
            // Split the edges at their cuts, nearest to `v1` first.
            let lambda_of = |pairs: &[[Elem; 2]], i: usize| -> f32 {
                let fi = flat_start + i;
                match pairs[fi / 2][fi % 2] {
                    Elem::Edge(_, l) => l,
                    Elem::Vert(_) => 0.0,
                }
            };
            // Blender's loop (`for (i = 0; i < map_len; e_map_iter = &as_int[i], i += …)`)
            // runs its first body with `e_map_iter` still on the last entry created above, so
            // the last edge is split first, then the others in order.
            if let Some(last) = e_map.pop() {
                e_map.insert(0, last);
            }
            for (e, cuts) in &mut e_map {
                let p = &pairs;
                c_qsort(cuts, &|&a, &b| {
                    if lambda_of(p, a) > lambda_of(p, b) {
                        1
                    } else {
                        -1
                    }
                });
                let mut lambda_prev = 0.0f32;
                for &i in cuts.iter() {
                    let pl = lambda_of(&pairs, i);
                    let lambda = (pl - lambda_prev) / (1.0 - lambda_prev);
                    lambda_prev = pl;
                    if split_faces {
                        // Tagged edges are ignored when splitting faces.
                        bm.edge_mut(*e).flag.remove(ElemFlag::TAG);
                    }
                    let v1 = bm.edge(*e).v[0];
                    let (v_new, _) = bm.edge_split(*e, v1, lambda);
                    let fi = flat_start + i;
                    pairs[fi / 2][fi % 2] = Elem::Vert(v_new);
                }
                // The faces' normals are needed by the face test below.
                for f in bm.edge_faces(*e) {
                    bm.face_normal_update(f);
                }
            }
        }
    }
    drop((tree_verts_act, tree_verts_remain));

    if pair_len == 0 {
        return false;
    }
    if pairs.is_empty() {
        for s in &d.stacks {
            pairs.extend_from_slice(s);
        }
    }

    let mut map = GhashMap::default();
    for p in &pairs {
        map.insert(p[0].vert(), p[1].vert());
    }
    // Every key of a collapsed group points at the group's last vertex.
    for (i, p) in pairs.iter_mut().enumerate() {
        let v_key = p[0].vert();
        let mut v_val = p[1].vert();
        let mut guard = 0usize;
        while let Some(t) = map.lookup(v_val) {
            v_val = t;
            guard += 1;
            if guard > map.entries.len() {
                break;
            }
        }
        if v_val != p[1].vert() {
            map.set(v_key, v_val);
            p[1] = Elem::Vert(v_val);
        }
        if split_faces {
            bm.index_set(BMElem::Vert(v_key), (i * 2) as i32);
            bm.index_set(BMElem::Vert(v_val), (i * 2 + 1) as i32);
        }
    }

    if split_faces {
        split_faces_by_edgenets(bm, &pairs, dist);
    }
    targetmap.extend(map.entries.iter().copied());
    true
}

/// The `split_faces` pass: every untagged edge with an end that moves onto another vertex
/// walks the chain of edges the moved vertices drag and splits the best face it lies on.
fn split_faces_by_edgenets(bm: &mut BMesh, pairs: &[[Elem; 2]], dist: f32) {
    let flat = |i: i32| -> VertId { pairs[(i / 2) as usize][(i % 2) as usize].vert() };
    let partner = |i: i32| if i % 2 != 0 { i - 1 } else { i + 1 };
    let vtag = |bm: &BMesh, v: VertId| bm.vert(v).flag.contains(ElemFlag::TAG);
    let vidx = |bm: &BMesh, v: VertId| bm.index_get(BMElem::Vert(v));
    let mut edgenet: Vec<EdgeId> = Vec::new();
    // `BM_ITER_MESH` while edges are created: the pool is walked live, so an edge created in
    // a later slot is visited too.
    let mut slot = 0u32;
    while (slot as usize) < bm.edges.slots.len() {
        let s = slot;
        slot += 1;
        if !bm.edges.contains(s) {
            continue;
        }
        let e = EdgeId(s);
        if bm.edge(e).flag.contains(ElemFlag::TAG) {
            continue;
        }
        let ev = bm.edge(e).v;
        let (mut va, mut vb) = (ev[0], ev[1]);
        let mut v_cut = vidx(bm, va);
        let mut v_cut_other = vidx(bm, vb);
        if v_cut == -1 && v_cut_other == -1 {
            if !vtag(bm, va) && !vtag(bm, vb) {
                // Out of context.
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
            continue;
        }
        bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        if v_cut == -1 {
            std::mem::swap(&mut va, &mut vb);
            v_cut = v_cut_other;
            v_cut_other = -1;
        }
        let va_dest = flat(partner(v_cut));
        if vert_pair_share_face_check(bm, va, va_dest) {
            continue;
        }
        let mut best_face: Option<FaceId> = None;
        let mut v_other = vb;
        let mut e_net = e;
        edgenet.clear();
        loop {
            let v_other_dest;
            if v_cut_other != -1 {
                v_other_dest = flat(partner(v_cut_other));
                if vert_pair_share_face_check(bm, v_other, v_other_dest) {
                    break;
                }
            } else {
                v_other_dest = v_other;
            }
            if va_dest == v_other_dest {
                // Edge (net) to vertex: no face to split.
                break;
            }
            if edgenet.is_empty() && bm.edge_exists(va_dest, v_other_dest).is_some() {
                // Edge to edge.
                break;
            }
            edgenet.push(e_net);
            best_face = vert_pair_best_face_get(bm, va_dest, v_other_dest, &edgenet, dist);
            if best_face.is_some() {
                if va_dest != va && bm.edge_exists(va_dest, va).is_none() {
                    let e0 = edgenet[0];
                    let vb2 = if edgenet.len() > 1 {
                        bm.edge_other_vert(e0, va)
                    } else {
                        v_other_dest
                    };
                    edgenet[0] = bm.edge_create(va_dest, vb2, Some(e0), CreateFlag::empty());
                }
                let n = edgenet.len();
                if n > 1
                    && v_other_dest != v_other
                    && bm.edge_exists(v_other_dest, v_other).is_none()
                {
                    let el = edgenet[n - 1];
                    let other = bm.edge_other_vert(el, v_other);
                    edgenet[n - 1] =
                        bm.edge_create(v_other_dest, other, Some(el), CreateFlag::empty());
                }
                break;
            }
            // Walk on along the dragged chain.
            let mut e_next = None;
            let mut e_test = e_net;
            loop {
                e_test = bm.disk_edge_next(e_test, v_other);
                if e_test == e_net {
                    break;
                }
                if !bm.edge_is_wire(e_test) {
                    if bm.edge(e_test).flag.contains(ElemFlag::TAG) {
                        continue;
                    }
                    let tv = bm.edge(e_test).v;
                    if !vtag(bm, tv[0]) && !vtag(bm, tv[1]) {
                        continue;
                    }
                    // Avoids an endless loop.
                    bm.edge_mut(e_test).flag.insert(ElemFlag::TAG);
                } else if !bm.edge_is_wire(e_net) {
                    continue;
                }
                e_next = Some(e_test);
                break;
            }
            let Some(en) = e_next else {
                break;
            };
            e_net = en;
            v_other = bm.edge_other_vert(e_net, v_other);
            if v_other == va {
                break;
            }
            v_cut_other = vidx(bm, v_other);
        }
        if let Some(f) = best_face {
            let faces = face_split_edgenet(bm, f, &edgenet);
            for nf in faces {
                bm.face_normal_update(nf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_ray_factors() {
        // x axis and a line through (0.5, -1, 0) along y cross at 0.5 / 1.0.
        let (la, lb) = isect_ray_ray_epsilon_v3(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.5, -1.0, 0.0],
            [0.0, 2.0, 0.0],
            f32::MIN_POSITIVE,
        )
        .unwrap();
        assert!((la - 0.5).abs() < 1e-6 && (lb - 0.5).abs() < 1e-6);
        assert!(
            isect_ray_ray_epsilon_v3(
                [0.0; 3],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 0.0, 0.0],
                1e-12
            )
            .is_none()
        );
    }

    #[test]
    fn point_in_polygon() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert!(isect_point_poly_v2([0.5, 0.5], &sq));
        assert!(!isect_point_poly_v2([1.5, 0.5], &sq));
    }
}
