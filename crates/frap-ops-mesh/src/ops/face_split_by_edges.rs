// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.face_split_by_edges()` (Weld Edges into Faces), tier T1
//! (`tests/golden/cases/edit/create/face_split_by_edges.py`). Port of
//! `edbm_face_split_by_edges_exec` and `bm_face_split_by_edges`
//! (`editors/mesh/editmesh_intersect.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `BM_face_split_edgenet`.
//!
//! Selected wire edges connected to the boundary of a selected face (directly or through
//! other selected wire edges) split that face; each wire edge goes to the face whose corner
//! it leaves into. The pieces are selected.
//!
//! `USE_NET_ISLAND_CONNECT`: selected wire edges still loose after that pass go to the
//! closest selected face (by their midpoint); a free end of such an edge (or a vertex shared
//! with another face's edges) is spliced into the closest edge of that face, then
//! `BM_face_split_edgenet_connect_islands` links the remaining islands to the boundary before
//! the face is split, and redundant connecting edges are dissolved again. The closest face is
//! found through Blender's BVH (`kdopbvh`), so equally close faces resolve the same way.
//! Blender walks the faces of that pass in hash-map order (pointer hashes); frap walks them in
//! mesh order, which only matters when loose edges of one face also touch another face's
//! edges.

use frap_bmesh::ops::contextual_create::kdopbvh::BvhTree;
use frap_bmesh::ops::contextual_create::{bli, connect_islands, split_edgenet};
use frap_bmesh::ops::rotate_edges::edge_face_pair;
use frap_bmesh::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use rustc_hash::FxHashMap;

use super::edge_face_add::edbm;

type V3 = [f32; 3];

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (closest_on_tri_to_point_v3).
/// `closest_on_tri_to_point_v3`.
fn closest_on_tri_to_point_v3(p: V3, v1: V3, v2: V3, v3: V3) -> V3 {
    let ab = bli::sub(v2, v1);
    let ac = bli::sub(v3, v1);
    let ap = bli::sub(p, v1);
    let d1 = bli::dot(ab, ap);
    let d2 = bli::dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return v1;
    }
    let bp = bli::sub(p, v2);
    let d3 = bli::dot(ab, bp);
    let d4 = bli::dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return v2;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let ab_squared = d1 - d3;
        if ab_squared == 0.0 {
            return v1;
        }
        return bli::madd_v3_v3v3fl(v1, ab, d1 / ab_squared);
    }
    let cp = bli::sub(p, v3);
    let d5 = bli::dot(ab, cp);
    let d6 = bli::dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return v3;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let ac_squared = d2 - d6;
        if ac_squared == 0.0 {
            return v1;
        }
        return bli::madd_v3_v3v3fl(v1, ac, d2 / ac_squared);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc_squared = (d4 - d3) + (d5 - d6);
        if bc_squared == 0.0 {
            return v2;
        }
        let r = bli::scale(bli::sub(v3, v2), (d4 - d3) / bc_squared);
        return bli::add(r, v2);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let ac_w = bli::scale(ac, w);
    let r = bli::madd_v3_v3v3fl(v1, ab, v);
    bli::add(r, ac_w)
}

/// The loop triangles of `f` (`BM_face_calc_tessellation` / `mesh_calc_tessellation`:
/// quads split on the 0–2 diagonal unless degenerate, n-gons by polyfill).
fn face_tris(bm: &BMesh, f: FaceId) -> Vec<[V3; 3]> {
    let co: Vec<V3> = bm.face_verts(f).into_iter().map(|v| bm.vert(v).co).collect();
    match co.len() {
        0..=2 => Vec::new(),
        3 => vec![[co[0], co[1], co[2]]],
        4 => {
            if frap_geom::polyfill::is_quad_flip_v3_first_third_fast(co[0], co[1], co[2], co[3]) {
                vec![[co[0], co[1], co[3]], [co[1], co[2], co[3]]]
            } else {
                vec![[co[0], co[1], co[2]], [co[0], co[2], co[3]]]
            }
        }
        n => {
            let idx: Vec<i32> = (0..n as i32).collect();
            frap_geom::polyfill::polyfill_mesh_face(&co, &idx)
                .into_iter()
                .map(|t| [co[t[0] as usize], co[t[1] as usize], co[t[2] as usize]])
                .collect()
        }
    }
}

/// `BKE_bmbvh_new(bm, looptris, BMBVH_RESPECT_SELECT)`: a BVH over the loop triangles of the
/// selected faces (indexed by loop triangle).
fn bmbvh_new(tris: &[(FaceId, [V3; 3])], selected: &dyn Fn(FaceId) -> bool) -> BvhTree {
    let mut t = BvhTree::new(f32::EPSILON * 2.0, 8, 8);
    for (i, (f, tri)) in tris.iter().enumerate() {
        if selected(*f) {
            t.insert(i as i32, tri);
        }
    }
    t.balance();
    t
}

/// `BKE_bmbvh_find_face_closest(bmbvh, co, FLT_MAX)`.
fn find_face_closest(bvh: &BvhTree, tris: &[(FaceId, [V3; 3])], co: V3) -> Option<FaceId> {
    let dist_max_sq = f32::MAX * f32::MAX;
    let mut cb = |index: i32, dist_sq_best: &mut f32| -> bool {
        let t = &tris[index as usize].1;
        let c = closest_on_tri_to_point_v3(co, t[0], t[1], t[2]);
        let dist_sq = bli::len_squared_v3v3(co, c);
        if dist_sq < *dist_sq_best && dist_sq < dist_max_sq {
            *dist_sq_best = dist_sq;
            true
        } else {
            false
        }
    };
    bvh.find_nearest(co, dist_max_sq, &mut cb)
        .map(|i| tris[i as usize].0)
}

/// `bm_vert_in_faces_radial`.
fn vert_in_faces_radial(bm: &BMesh, v: VertId, e_radial: EdgeId, f_ignore: FaceId) -> bool {
    bm.edge_loops(e_radial).into_iter().any(|l| {
        let f = bm.lp(l).f;
        f != f_ignore && bm.face_verts(f).contains(&v)
    })
}

// Ported from blender/source/blender/editors/mesh/editmesh_intersect.cc
// (bm_face_split_edge_find).
/// `bm_face_split_edge_find`: the edge of `f_a` to splice `v_pivot` into, with the factor
/// along it.
fn split_edge_find(
    bm: &mut BMesh,
    e_a: EdgeId,
    f_a: FaceId,
    v_pivot: VertId,
    ftable: &[FaceId],
    index: &FxHashMap<EdgeId, i64>,
) -> Option<(EdgeId, f32)> {
    let idx = |e: EdgeId| index.get(&e).copied().unwrap_or(-1);
    let f_a_index = idx(e_a);
    let mut found_other_self = false;
    let mut found_other_face = 0;
    let mut face_stack: Vec<FaceId> = Vec::new();
    for e_b in bm.vert_edges(v_pivot) {
        if e_b != e_a {
            let f_b_index = idx(e_b);
            if f_b_index == f_a_index {
                found_other_self = true;
            } else if f_b_index != -1 {
                let f_b = ftable[f_b_index as usize];
                if !bm.face(f_b).flag.contains(ElemFlag::INTERNAL_TAG) {
                    bm.face_mut(f_b).flag.insert(ElemFlag::INTERNAL_TAG);
                    face_stack.push(f_b);
                    found_other_face += 1;
                }
            }
        }
    }
    let mut e_split: Option<(EdgeId, f32)> = None;
    if !found_other_self || found_other_face != 0 {
        let mut dist_best_sq = f32::MAX;
        let p = bm.vert(v_pivot).co;
        for l_iter in bm.face_loops(f_a) {
            let e = bm.lp(l_iter).e;
            let [ev1, ev2] = bm.edge(e).v;
            let (c1, c2) = (bm.vert(ev1).co, bm.vert(ev2).co);
            let u = bli::sub(c2, c1);
            let h = bli::sub(p, c1);
            let dot = bli::len_squared_v3(u);
            let fac = if dot > 0.0 { bli::dot(u, h) / dot } else { 0.0 };
            let fac = fac.clamp(0.0, 1.0);
            let co_test = bli::interp_v3_v3v3(c1, c2, fac);
            let dist_test_sq = bli::len_squared_v3v3(co_test, p);
            if dist_test_sq < dist_best_sq || e_split.is_none() {
                let mut ok = true;
                if bm.edge_exists(v_pivot, ev1).is_some() || bm.edge_exists(v_pivot, ev2).is_some() {
                    ok = false;
                } else if found_other_face != 0 {
                    let mut other_face_shared = 0;
                    let mut l_radial: LoopId = bm.lp(l_iter).radial_next;
                    while l_radial != l_iter {
                        if bm.face(bm.lp(l_radial).f).flag.contains(ElemFlag::INTERNAL_TAG) {
                            other_face_shared += 1;
                        }
                        l_radial = bm.lp(l_radial).radial_next;
                    }
                    if other_face_shared != found_other_face {
                        ok = false;
                    }
                }
                if ok {
                    e_split = Some((e, fac));
                    dist_best_sq = dist_test_sq;
                }
            }
        }
    }
    while let Some(f) = face_stack.pop() {
        bm.face_mut(f).flag.remove(ElemFlag::INTERNAL_TAG);
    }
    e_split
}

// Ported from blender/source/blender/editors/mesh/editmesh_intersect.cc
// (bm_face_split_by_edges_island_connect).
fn island_connect(bm: &mut BMesh, f: FaceId, e_link: &[EdgeId]) {
    let e_link_len = e_link.len();
    let mut edge_arr: Vec<EdgeId> = e_link.to_vec();
    if let Some(new) = connect_islands::face_split_edgenet_connect_islands(bm, f, &edge_arr, true) {
        edge_arr = new;
    }
    split_edgenet::face_split_edgenet(bm, f, &edge_arr);
    for &e in &edge_arr[e_link_len.min(edge_arr.len())..] {
        bm.edge_select_set(e, true);
    }
    if e_link_len != edge_arr.len() {
        // Blender sorts the first `edge_arr_len - e_link_len` entries (longest first), then
        // walks the entries after `e_link_len`.
        let n = edge_arr.len() - e_link_len;
        let len_sq = |bm: &BMesh, e: EdgeId| {
            let [a, b] = bm.edge(e).v;
            bli::len_squared_v3v3(bm.vert(a).co, bm.vert(b).co)
        };
        {
            let head = &mut edge_arr[..n];
            let bm_ref: &BMesh = bm;
            bli::c_qsort(head, &|a: &EdgeId, b: &EdgeId| {
                let va = -len_sq(bm_ref, *a);
                let vb = -len_sq(bm_ref, *b);
                if va > vb {
                    1
                } else if va < vb {
                    -1
                } else {
                    0
                }
            });
        }
        for &e in &edge_arr[e_link_len..] {
            if !bm.edges.contains(e.0) {
                continue;
            }
            if let Some((fa, fb)) = edge_face_pair(bm, e) {
                let share = bm
                    .face_verts(fa)
                    .into_iter()
                    .filter(|v| bm.face_verts(fb).contains(v))
                    .count();
                if share == 2
                    && let (Some(f_new), _) = bm.faces_join(&[fa, fb], true, true)
                {
                    bm.face_select_set(f_new, true);
                }
            }
        }
    }
}

// Ported from blender/source/blender/editors/mesh/editmesh_intersect.cc
// (edbm_face_split_by_edges_exec, USE_NET_ISLAND_CONNECT part).
fn net_island_connect(bm: &mut BMesh, edges_loose: Vec<EdgeId>) {
    let ftable: Vec<FaceId> = bm.face_ids();
    let fidx: FxHashMap<FaceId, i64> = ftable.iter().enumerate().map(|(i, &f)| (f, i as i64)).collect();
    let tris: Vec<(FaceId, [V3; 3])> = ftable
        .iter()
        .flat_map(|&f| face_tris(bm, f).into_iter().map(move |t| (f, t)))
        .collect();
    let bvh = bmbvh_new(&tris, &|f| bm.face(f).flag.contains(ElemFlag::SELECT));
    let mut index: FxHashMap<EdgeId, i64> = FxHashMap::default();
    let mut face_edges: FxHashMap<FaceId, Vec<EdgeId>> = FxHashMap::default();
    // `BLI_stack_pop`: last pushed first; each edge is prepended to its face's list.
    for &e in edges_loose.iter().rev() {
        let [a, b] = bm.edge(e).v;
        let center = bli::mid_v3_v3v3(bm.vert(a).co, bm.vert(b).co);
        if let Some(f) = find_face_closest(&bvh, &tris, center) {
            face_edges.entry(f).or_default().insert(0, e);
            index.insert(e, fidx[&f]);
        }
    }
    let faces: Vec<FaceId> = ftable.iter().copied().filter(|f| face_edges.contains_key(f)).collect();
    for &f in &faces {
        for &e in &face_edges[&f] {
            for j in 0..2 {
                let v_pivot = bm.edge(e).v[j];
                if bm.face_verts(f).contains(&v_pivot) {
                    continue;
                }
                let Some((e_split, fac)) = split_edge_find(bm, e, f, v_pivot, &ftable, &index) else {
                    continue;
                };
                if !vert_in_faces_radial(bm, v_pivot, e_split, f) {
                    let v1 = bm.edge(e_split).v[0];
                    let (v_new, e_new) = bm.edge_split(e_split, v1, fac);
                    bm.vert_splice(v_pivot, v_new);
                    let ix = index.get(&e_split).copied().unwrap_or(-1);
                    index.insert(e_new, ix);
                }
            }
        }
    }
    for &f in &faces {
        island_connect(bm, f, &face_edges[&f]);
    }
}

// Ported from blender/source/blender/editors/mesh/editmesh_intersect.cc
// (bm_face_split_by_edges).
fn split_face(bm: &mut BMesh, f: FaceId, f_index: i64, index: &mut FxHashMap<EdgeId, i64>) {
    let tagged = |bm: &BMesh, e: EdgeId| bm.edge(e).flag.contains(ElemFlag::TAG);
    let mut net: Vec<EdgeId> = Vec::new();
    let mut stack: Vec<VertId> = Vec::new();
    let mut stack_next: Vec<VertId> = Vec::new();
    for l in bm.face_loops(f) {
        let lv = bm.lp(l).v;
        for e in bm.vert_edges(lv) {
            if tagged(bm, e) && index.get(&e).copied().unwrap_or(-1) == f_index {
                let v = bm.edge_other_vert(e, lv);
                bm.vert_mut(v).e = Some(e);
                stack.push(v);
                net.push(e);
            }
        }
    }
    while let Some(v) = stack.pop() {
        for e_next in bm.vert_edges(v) {
            if tagged(bm, e_next) && index.get(&e_next).copied().unwrap_or(-1) == -1 {
                let v_next = bm.edge_other_vert(e_next, v);
                index.insert(e_next, f_index);
                stack_next.push(v_next);
                net.push(e_next);
            }
        }
        if stack.is_empty() {
            std::mem::swap(&mut stack, &mut stack_next);
        }
    }
    let faces = split_edgenet::face_split_edgenet(bm, f, &net);
    for nf in faces {
        bm.face_select_set(nf, true);
        bm.face_mut(nf).flag.remove(ElemFlag::TAG);
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::FaceSplitByEdges) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let Some((_, te, tf)) = edbm::with_bm(cx, me, |bm| bm.selected_counts()) else {
            continue;
        };
        if te == 0 || tf == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
        let mut index: FxHashMap<EdgeId, i64> = FxHashMap::default();
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::SELECT) && bm.edge_is_wire(e) {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                let [a, b] = bm.edge(e).v;
                bm.vert_mut(a).flag.insert(ElemFlag::TAG);
                bm.vert_mut(b).flag.insert(ElemFlag::TAG);
            } else {
                bm.edge_mut(e).flag.remove(ElemFlag::TAG);
            }
            index.insert(e, -1);
        }
        let mut face_index: FxHashMap<FaceId, i64> = FxHashMap::default();
        for (i, f) in bm.face_ids().into_iter().enumerate() {
            let sel = bm.face(f).flag.contains(ElemFlag::SELECT);
            bm.face_mut(f).flag.set(ElemFlag::TAG, sel);
            bm.face_mut(f).flag.remove(ElemFlag::INTERNAL_TAG);
            face_index.insert(f, i as i64);
        }
        for e in bm.edge_ids() {
            if !bm.edge(e).flag.contains(ElemFlag::TAG) {
                continue;
            }
            for v in bm.edge(e).v {
                let loops: Vec<_> = bm
                    .vert_loops(v)
                    .into_iter()
                    .filter(|&l| bm.face(bm.lp(l).f).flag.contains(ElemFlag::TAG))
                    .collect();
                let l_best = match loops.len() {
                    0 => None,
                    1 => Some(loops[0]),
                    _ => {
                        let v_other = bm.edge_other_vert(e, v);
                        let e_dir = bli::normalize_v3(bli::sub(bm.vert(v_other).co, bm.vert(v).co)).0;
                        let mut dot_best = f32::MAX;
                        let mut best = None;
                        // `BLI_SMALLSTACK_POP`: the last pushed loop first.
                        for &l in loops.iter().rev() {
                            let ll = bm.lp(l);
                            let fno = bm.face(ll.f).no;
                            let dot_test = bli::dot(e_dir, fno).abs();
                            if dot_test < dot_best {
                                let co = |x: VertId| bm.vert(x).co;
                                let pv = bm.lp(ll.prev).v;
                                let nv = bm.lp(ll.next).v;
                                if bli::angle_signed_on_axis_v3v3v3_v3(co(pv), co(ll.v), co(v_other), fno)
                                    < bli::angle_signed_on_axis_v3v3v3_v3(co(pv), co(ll.v), co(nv), fno)
                                {
                                    dot_best = dot_test;
                                    best = Some(l);
                                }
                            }
                        }
                        best
                    }
                };
                if let Some(lb) = l_best {
                    index.insert(e, face_index[&bm.lp(lb).f]);
                }
            }
        }
        for f in bm.face_ids() {
            if bm.faces.contains(f.0) && bm.face(f).flag.contains(ElemFlag::TAG) {
                let fi = face_index[&f];
                split_face(bm, f, fi, &mut index);
            }
        }
        let edges_loose: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT) && bm.edge_is_wire(e))
            .collect();
        bm.normals_update();
        if !edges_loose.is_empty() {
            net_island_connect(bm, edges_loose);
            bm.normals_update();
        }
        bm.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}
