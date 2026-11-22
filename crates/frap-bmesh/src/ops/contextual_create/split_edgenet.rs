// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_face_split_edgenet` (`bmesh/intern/bmesh_polygon_edgenet.cc`, Blender 5.2.4): splits a
//! face by a net of wire edges lying on it (used by `mesh.face_split_by_edges`).
//!
//! **Create/kill order**: starting from the face's first vertex, closed loops are walked over
//! boundary and net edges (narrowest turn first), each becoming a new face
//! (`BM_face_create` with the original face as example); the original face then takes over
//! the first new face's loops (`bmesh_face_swap_data`) and that face is killed, so the
//! original face id survives as the first piece.
//!
//! Nets not touching the face boundary are first linked to it by
//! `BM_face_split_edgenet_connect_islands` (`connect_islands.rs`), as Blender's callers do.

use rustc_hash::FxHashSet;

use super::bli::{self, V3};
use crate::types::CreateFlag;
use crate::{BMesh, EdgeId, FaceId, VertId};

struct Net {
    face_net: FxHashSet<FaceId>,
    edge_net: FxHashSet<EdgeId>,
    vert_visit: FxHashSet<VertId>,
    f_orig: FaceId,
}

impl Net {
    fn radial_count(&self, bm: &BMesh, e: EdgeId) -> u32 {
        bm.edge_loops(e)
            .into_iter()
            .filter(|&l| self.face_net.contains(&bm.lp(l).f))
            .count() as u32
    }
    fn radial_first(&self, bm: &BMesh, e: EdgeId) -> Option<crate::LoopId> {
        bm.edge_loops(e)
            .into_iter()
            .find(|&l| self.face_net.contains(&bm.lp(l).f))
    }
}

fn normalize_v2(v: [f32; 2]) -> [f32; 2] {
    let d = v[0] * v[0] + v[1] * v[1];
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = 1.0 / d;
        [v[0] * f, v[1] * f]
    } else {
        [0.0, 0.0]
    }
}

fn dir2(m: &[V3; 3], a: V3, b: V3) -> [f32; 2] {
    normalize_v2(bli::mul_v2_m3v3(m, bli::sub(a, b)))
}

/// `bm_face_split_edgenet_find_loop_pair`.
fn find_loop_pair(bm: &BMesh, net: &Net, v_init: VertId, m: &[V3; 3]) -> Option<[EdgeId; 2]> {
    let mut boundary: Vec<EdgeId> = Vec::new();
    let mut wire: Vec<EdgeId> = Vec::new();
    for e in bm.vert_edges(v_init) {
        if net.edge_net.contains(&e) {
            match net.radial_count(bm, e) {
                1 => boundary.push(e),
                0 => wire.push(e),
                _ => {}
            }
        }
    }
    let e0 = boundary.pop()?;
    let mut search: Vec<EdgeId> = Vec::new();
    let e1;
    if wire.is_empty() {
        e1 = boundary.pop()?;
        if !boundary.is_empty() {
            search = std::mem::take(&mut boundary);
        }
    } else {
        e1 = wire.pop().expect("non-empty");
        if !wire.is_empty() {
            search = std::mem::take(&mut wire);
        }
    }
    let mut pair = [e0, e1];
    if !search.is_empty() {
        let co = |v: VertId| bm.vert(v).co;
        let vi = co(v_init);
        let dir_prev = dir2(m, co(bm.edge_other_vert(pair[0], v_init)), vi);
        let dir_next = dir2(m, co(bm.edge_other_vert(pair[1], v_init)), vi);
        let mut best = dir_next[0] * dir_prev[0] + dir_next[1] * dir_prev[1];
        while let Some(e) = search.pop() {
            let dt = dir2(m, co(bm.edge_other_vert(e, v_init)), vi);
            let c = dir_prev[0] * dt[0] + dir_prev[1] * dt[1];
            if c > best {
                best = c;
                pair[1] = e;
            }
        }
    }
    let l_walk = net.radial_first(bm, pair[0])?;
    let mut swap = bm.lp(l_walk).f == net.f_orig;
    if bm.lp(l_walk).v != v_init {
        swap = !swap;
    }
    if swap {
        pair.swap(0, 1);
    }
    Some(pair)
}

/// `bm_face_split_edgenet_find_loop_pair_exists`.
fn find_loop_pair_exists(bm: &BMesh, net: &Net, v_init: VertId) -> bool {
    let (mut nb, mut nw) = (0, 0);
    for e in bm.vert_edges(v_init) {
        if net.edge_net.contains(&e) {
            match net.radial_count(bm, e) {
                1 => nb += 1,
                0 => nw += 1,
                _ => {}
            }
        }
    }
    nb != 0 && (nw != 0 || nb >= 2)
}

/// `bm_face_split_edgenet_find_loop_walk`.
fn find_loop_walk(
    bm: &mut BMesh,
    net: &mut Net,
    v_init: VertId,
    face_normal: V3,
    pair: [EdgeId; 2],
) -> bool {
    let mut visit: Vec<VertId> = Vec::new();
    let mut stack: Vec<VertId> = Vec::new();
    let mut stack_next: Vec<VertId> = Vec::new();
    let v0 = bm.edge_other_vert(pair[0], v_init);
    bm.vert_mut(v0).e = Some(pair[0]);
    stack.push(v0);
    let v_dst = bm.edge_other_vert(pair[1], v_init);
    let mut found = false;
    // `BLI_SMALLSTACK_POP_EX(vert_stack, vert_stack_next)` pops from `vert_stack` only (the
    // second stack only receives the freed node); the two stacks are swapped whenever new
    // vertices were pushed.
    'outer: while let Some(mut v) = stack.pop() {
        loop {
            frap_math::budget::check();
            if v == v_dst {
                found = true;
                break 'outer;
            }
            let e_first = bm.vert(v).e.expect("walk vertex edge");
            let mut order: Vec<(f32, VertId)> = Vec::new();
            let mut e_next = bm.disk_edge_next(e_first, v);
            if e_next != e_first {
                loop {
                    if net.edge_net.contains(&e_next) && net.radial_count(bm, e_next) < 2 {
                        let v_next = bm.edge_other_vert(e_next, v);
                        if !net.vert_visit.contains(&v_next) {
                            order.push((0.0, v_next));
                            bm.vert_mut(v_next).e = Some(e_next);
                        }
                    }
                    e_next = bm.disk_edge_next(e_next, v);
                    if e_next == e_first {
                        break;
                    }
                }
            }
            if order.len() == 1 {
                v = order[0].1;
                continue;
            }
            if order.len() > 1 {
                let ve = bm.vert(v).e.expect("walk vertex edge");
                let v_prev = bm.edge_other_vert(ve, v);
                for o in &mut order {
                    o.0 = bli::angle_signed_on_axis_v3v3v3_v3(
                        bm.vert(v_prev).co,
                        bm.vert(v).co,
                        bm.vert(o.1).co,
                        face_normal,
                    );
                }
                bli::c_qsort(&mut order, &bli::cmp_float_reverse);
                visit.push(v);
                net.vert_visit.insert(v);
            }
            while let Some((_, vv)) = order.pop() {
                stack_next.push(vv);
            }
            if !stack_next.is_empty() {
                std::mem::swap(&mut stack, &mut stack_next);
            }
            break;
        }
    }
    for v in visit {
        net.vert_visit.remove(&v);
    }
    found
}

/// `bm_face_split_edgenet_find_loop`: (verts, edges, check_face_exists).
fn find_loop(
    bm: &mut BMesh,
    net: &mut Net,
    v_init: VertId,
    face_normal: V3,
    m: &[V3; 3],
) -> Option<(Vec<VertId>, Vec<EdgeId>, bool)> {
    let pair = find_loop_pair(bm, net, v_init, m)?;
    if !find_loop_walk(bm, net, v_init, face_normal, pair) {
        return None;
    }
    let mut check = true;
    let mut verts = vec![v_init];
    let mut edges = vec![pair[1]];
    if bm.edge_is_wire(pair[1]) {
        check = false;
    }
    let mut v = bm.edge_other_vert(pair[1], v_init);
    loop {
        frap_math::budget::check();
        let e = bm.vert(v).e.expect("walk edge");
        verts.push(v);
        edges.push(e);
        if check && bm.edge_is_wire(e) {
            check = false;
        }
        v = bm.edge_other_vert(e, v);
        if v == v_init {
            break;
        }
    }
    if verts.len() < 3 {
        return None;
    }
    Some((verts, edges, check))
}

/// `bmesh_face_swap_data`: `f_a` and `f_b` exchange loops and header data (custom data stays).
fn face_swap_data(bm: &mut BMesh, f_a: FaceId, f_b: FaceId) {
    for l in bm.face_loops(f_a) {
        bm.lp_mut(l).f = f_b;
    }
    let lb = {
        let first = bm.face(f_b).l_first;
        let mut out = Vec::new();
        let mut l = first;
        loop {
            out.push(l);
            l = bm.lp(l).next;
            if l == first {
                break;
            }
        }
        out
    };
    for l in lb {
        bm.lp_mut(l).f = f_a;
    }
    let a = bm.face(f_a).clone();
    let b = bm.face(f_b).clone();
    *bm.face_mut(f_a) = b;
    *bm.face_mut(f_b) = a;
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon_edgenet.cc
// (BM_face_split_edgenet).
/// `BM_face_split_edgenet(f, edge_net)`: the faces `f` was split into (`f` first), empty when
/// nothing was split.
pub fn face_split_edgenet(bm: &mut BMesh, f: FaceId, edge_net: &[EdgeId]) -> Vec<FaceId> {
    if edge_net.is_empty() {
        return Vec::new();
    }
    let mut net = Net {
        face_net: FxHashSet::default(),
        edge_net: FxHashSet::default(),
        vert_visit: FxHashSet::default(),
        f_orig: f,
    };
    net.face_net.insert(f);
    let mut in_queue: FxHashSet<VertId> = FxHashSet::default();
    for &e in edge_net {
        net.edge_net.insert(e);
    }
    for e in bm.face_edges(f) {
        net.edge_net.insert(e);
    }
    let fno = bm.face(f).no;
    let m = bli::axis_dominant_v3_to_m3(fno);
    let l_first = bm.face(f).l_first;
    let mut queue: Vec<VertId> = vec![bm.lp(l_first).v];
    in_queue.insert(bm.lp(l_first).v);
    let mut face_arr: Vec<FaceId> = Vec::new();
    while let Some(v) = queue.pop() {
        frap_math::budget::check();
        in_queue.remove(&v);
        let Some((verts, edges, check)) = find_loop(bm, &mut net, v, fno, &m) else {
            continue;
        };
        if check && bm.face_exists(&verts).is_some() {
            continue;
        }
        let Some(f_new) = bm.face_create(&verts, &edges, Some(f), CreateFlag::empty()) else {
            continue;
        };
        face_arr.push(f_new);
        let (no, flag) = (bm.face(f).no, bm.face(f).flag);
        bm.face_mut(f_new).no = no;
        bm.face_mut(f_new).flag = flag;
        net.face_net.insert(f_new);
        for l in bm.face_loops(f_new) {
            let lv = bm.lp(l).v;
            if !in_queue.contains(&lv) && find_loop_pair_exists(bm, &net, lv) {
                queue.push(lv);
                in_queue.insert(lv);
            }
        }
    }
    if !bm.ldata.layers.is_empty() {
        let f_loops = bm.face_loops(f);
        let mut visited: FxHashSet<VertId> = FxHashSet::default();
        let mut cos_2d: Vec<[f32; 2]> = Vec::new();
        let mut blocks: Vec<u32> = Vec::new();
        for &l in &f_loops {
            let lv = bm.lp(l).v;
            for lo in bm.vert_loops(lv) {
                let lof = bm.lp(lo).f;
                if lof != f && net.face_net.contains(&lof) {
                    bm.ldata.copy_elem(l.0, lo.0);
                }
            }
            visited.insert(lv);
            cos_2d.push(bli::mul_v2_m3v3(&m, bm.vert(lv).co));
            blocks.push(l.0);
        }
        for &e in edge_net {
            for v in bm.edge(e).v {
                if visited.insert(v) {
                    let mut l_src: Option<crate::LoopId> = None;
                    for li in bm.vert_loops(v) {
                        if net.face_net.contains(&bm.lp(li).f) {
                            match l_src {
                                None => {
                                    let co = bli::mul_v2_m3v3(&m, bm.vert(v).co);
                                    let w = crate::query::geom::interp_weights_poly_v2(&cos_2d, co);
                                    bm.ldata.interp(&blocks, &w, li.0);
                                    l_src = Some(li);
                                }
                                Some(src) => bm.ldata.copy_elem(src.0, li.0),
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(&first) = face_arr.first() {
        face_swap_data(bm, f, first);
        bm.face_kill(first);
        face_arr[0] = f;
    }
    face_arr
}
