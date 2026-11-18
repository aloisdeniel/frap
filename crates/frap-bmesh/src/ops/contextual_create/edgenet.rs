// SPDX-License-Identifier: GPL-3.0-or-later
//! Edge-net fill: `BM_mesh_edgenet` (`tools/bmesh_edgenet.cc`) and the `edgenet_prepare` /
//! `edgenet_fill` operators (`operators/bmo_edgenet.cc`), Blender 5.2.4.
//!
//! **Create order**: `BM_mesh_edgenet` takes the next usable edge (the queue of edges of the
//! last created face first, then mesh order), searches the shortest path closing a face
//! (breadth-first from both ends, Blender's link-list stack order), and creates the face
//! with `BM_face_create` over the path's existing edges (loops in path order). `edgenet_prepare`
//! creates at most two edges (`BM_CREATE_NO_DOUBLE`) joining the ends of one or two open
//! chains.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use super::bli;
use super::{attribute_fill, construct};
use crate::types::CreateFlag;
use crate::{BMesh, EdgeId, ElemFlag, FaceId, VertId};

#[derive(Copy, Clone, Default)]
struct VertNetInfo {
    prev: Option<VertId>,
    pass: i32,
    face: i64,
    flag: u8,
}

const VNINFO_FLAG_IS_MIXFACE: u8 = 1;

/// `bm_edge_step_ok`: tagged, wire or boundary.
fn edge_step_ok(bm: &BMesh, e: EdgeId) -> bool {
    let ed = bm.edge(e);
    ed.flag.contains(ElemFlag::TAG)
        && match ed.l {
            None => true,
            Some(l) => bm.lp(l).radial_next == l,
        }
}

struct Net {
    info: FxHashMap<VertId, VertNetInfo>,
    /// Face "indices" (only uniqueness matters): the face slot.
    face_index: FxHashMap<FaceId, i64>,
}

impl Net {
    fn get(&self, v: VertId) -> VertNetInfo {
        self.info.get(&v).copied().unwrap_or_default()
    }
    fn get_mut(&mut self, v: VertId) -> &mut VertNetInfo {
        self.info.entry(v).or_default()
    }
    fn edge_face(&self, bm: &BMesh, e: EdgeId) -> i64 {
        match bm.edge(e).l {
            Some(l) => {
                let f = bm.lp(l).f;
                self.face_index.get(&f).copied().unwrap_or(f.0 as i64)
            }
            None => -1,
        }
    }
}

/// `bm_edgenet_path_from_pass`: prepends the half path ending at `v` onto `path`.
fn path_from_pass(net: &Net, mut v: VertId, path: &mut VecDeque<VertId>) -> usize {
    let mut vn = net.get(v);
    let pass = vn.pass;
    let mut tot = 0;
    loop {
        path.push_front(v);
        tot += 1;
        v = vn.prev.expect("edgenet path link");
        vn = net.get(v);
        if vn.pass != pass {
            break;
        }
    }
    tot
}

/// `bm_edgenet_path_check_overlap`.
fn path_check_overlap(bm: &BMesh, net: &Net, v1: VertId, v2: VertId) -> bool {
    let mut v_ls: VecDeque<VertId> = VecDeque::new();
    for v0 in [v1, v2] {
        let mut v = v0;
        let mut vn = net.get(v);
        let pass = vn.pass;
        loop {
            v_ls.push_front(v);
            v = vn.prev.expect("edgenet path link");
            vn = net.get(v);
            if vn.pass != pass {
                break;
            }
        }
    }
    if v_ls.is_empty() {
        return false;
    }
    let arr: Vec<VertId> = v_ls.into_iter().collect();
    construct::face_exists_overlap_subset(bm, &arr)
}

/// `bm_edgenet_path_step`: pushes the next vertices onto `v_ls` (a stack: last = head);
/// returns the connecting edge when both search fronts meet.
fn path_step(
    bm: &BMesh,
    net: &mut Net,
    mut v_curr: VertId,
    v_ls: &mut Vec<VertId>,
) -> Option<EdgeId> {
    loop {
        let mut tot = 0;
        let mut v_ls_tot = 0;
        let vn_curr = net.get(v_curr);
        for e in bm.vert_edges(v_curr) {
            let v_next = bm.edge_other_vert(e, v_curr);
            if Some(v_next) == vn_curr.prev {
                continue;
            }
            if edge_step_ok(bm, e) {
                let vn_next = net.get(v_next);
                if vn_curr.pass != vn_next.pass {
                    if vn_curr.pass == -vn_next.pass {
                        if ((vn_curr.flag & VNINFO_FLAG_IS_MIXFACE) != 0
                            || (vn_next.flag & VNINFO_FLAG_IS_MIXFACE) != 0)
                            && !path_check_overlap(bm, net, v_curr, v_next)
                        {
                            return Some(e);
                        }
                    } else {
                        let face = net.edge_face(bm, e);
                        let n = net.get_mut(v_next);
                        n.face = face;
                        n.pass = vn_curr.pass;
                        n.prev = Some(v_curr);
                        n.flag &= !VNINFO_FLAG_IS_MIXFACE;
                        if (vn_curr.flag & VNINFO_FLAG_IS_MIXFACE) != 0
                            || n.face == -1
                            || n.face != vn_curr.face
                        {
                            n.flag |= VNINFO_FLAG_IS_MIXFACE;
                        }
                        v_ls.push(v_next);
                        v_ls_tot += 1;
                    }
                }
            }
            tot += 1;
        }
        // Trick to walk along wire-edge paths.
        if v_ls_tot == 1 && tot == 1 {
            v_curr = v_ls.pop().expect("just pushed");
            continue;
        }
        return None;
    }
}

/// `bm_edgenet_path_calc`: (path, length, cost).
fn path_calc(
    bm: &mut BMesh,
    net: &mut Net,
    e: EdgeId,
    pass_nr: i32,
    path_cost_max: u32,
) -> Option<(VecDeque<VertId>, u32)> {
    let f_index = net.edge_face(bm, e);
    let [v1, v2] = bm.edge(e).v;
    {
        let flag = if f_index == -1 {
            VNINFO_FLAG_IS_MIXFACE
        } else {
            0
        };
        let a = net.get_mut(v1);
        a.pass = pass_nr;
        a.prev = Some(v2);
        a.face = f_index;
        a.flag = flag;
        let b = net.get_mut(v2);
        b.pass = -pass_nr;
        b.prev = Some(v1);
        b.face = f_index;
        b.flag = flag;
    }
    let mut v_ls_prev: Vec<VertId> = vec![v1, v2];
    let mut v_ls_next: Vec<VertId> = Vec::new();
    let mut path_cost_accum: u32 = 0;
    loop {
        frap_math::budget::check();
        let mut found = false;
        if path_cost_accum >= path_cost_max {
            return None;
        }
        while let Some(v) = v_ls_prev.pop() {
            let old_len = v_ls_next.len();
            if let Some(e_found) = path_step(bm, net, v, &mut v_ls_next) {
                let [f1, f2] = bm.edge(e_found).v;
                let mut path: VecDeque<VertId> = VecDeque::new();
                path_from_pass(net, f1, &mut path);
                path.make_contiguous().reverse();
                path_from_pass(net, f2, &mut path);
                return Some((path, path_cost_accum));
            }
            if v_ls_next.len() != old_len {
                found = true;
            }
        }
        path_cost_accum += 1;
        std::mem::swap(&mut v_ls_prev, &mut v_ls_next);
        v_ls_next.clear();
        if !found {
            break;
        }
    }
    // Tag not to search again.
    bm.edge_mut(e).flag.remove(ElemFlag::TAG);
    None
}

/// `bm_edgenet_path_calc_best`.
fn path_calc_best(
    bm: &mut BMesh,
    net: &mut Net,
    e: EdgeId,
    pass_nr: &mut i32,
    path_cost_max: u32,
) -> Option<VecDeque<VertId>> {
    let r = path_calc(bm, net, e, *pass_nr, path_cost_max);
    *pass_nr += 1;
    let (mut path, mut path_cost) = r?;
    if path_cost < 1 {
        return Some(path);
    }
    let vert_arr: Vec<VertId> = path.iter().copied().collect();
    let len = vert_arr.len();
    let mut i_prev = len - 1;
    for i in 0..len {
        let e_other = bm.edge_exists(vert_arr[i], vert_arr[i_prev]);
        if let Some(e_other) = e_other
            && e_other != e
        {
            let r = path_calc(bm, net, e_other, *pass_nr, path_cost);
            *pass_nr += 1;
            if let Some((p, c)) = r {
                path = p;
                path_cost = c;
            }
        }
        i_prev = i;
    }
    Some(path)
}

// Ported from blender/source/blender/bmesh/tools/bmesh_edgenet.cc (BM_mesh_edgenet).
/// `BM_mesh_edgenet(bm, use_edge_tag, use_new_face_tag)`: fills the tagged wire/boundary edge
/// net with faces; returns the new faces in creation order.
pub fn mesh_edgenet(bm: &mut BMesh, use_edge_tag: bool, use_new_face_tag: bool) -> Vec<FaceId> {
    if !use_edge_tag {
        for e in bm.edge_ids() {
            let ok = {
                let ed = bm.edge(e);
                match ed.l {
                    None => true,
                    Some(l) => bm.lp(l).radial_next == l,
                }
            };
            bm.edge_mut(e).flag.set(ElemFlag::TAG, ok);
        }
    }
    let mut net = Net {
        info: FxHashMap::default(),
        face_index: FxHashMap::default(),
    };
    let mut pass_nr = 1;
    let mut edge_queue: Vec<EdgeId> = Vec::new();
    let mut out = Vec::new();
    loop {
        frap_math::budget::check();
        // `bm_edgenet_edge_get_next`.
        let mut e_next = None;
        while let Some(e) = edge_queue.pop() {
            if bm.edges.contains(e.0) && edge_step_ok(bm, e) {
                e_next = Some(e);
                break;
            }
        }
        if e_next.is_none() {
            e_next = bm.edge_ids().into_iter().find(|&e| edge_step_ok(bm, e));
        }
        let Some(e) = e_next else {
            break;
        };
        let Some(path) = path_calc_best(bm, &mut net, e, &mut pass_nr, u32::MAX) else {
            continue;
        };
        let vert_arr: Vec<VertId> = path.into_iter().collect();
        let edges = construct::edges_from_verts(bm, &vert_arr).expect("edgenet path edges");
        let f = bm
            .face_create(&vert_arr, &edges, None, CreateFlag::empty())
            .expect("edgenet face");
        for l in bm.face_loops(f) {
            let le = bm.lp(l).e;
            if edge_step_ok(bm, le) {
                edge_queue.push(le);
            }
        }
        if use_new_face_tag {
            bm.face_mut(f).flag.insert(ElemFlag::TAG);
        }
        net.face_index.insert(f, f.0 as i64);
        out.push(f);
    }
    out
}

/// Output of [`edgenet_fill`].
#[derive(Clone, Debug, Default)]
pub struct EdgenetFillOut {
    /// `faces.out`.
    pub faces: Vec<FaceId>,
}

// Ported from blender/source/blender/bmesh/operators/bmo_edgenet.cc (bmo_edgenet_fill_exec).
/// `bmesh.ops.edgenet_fill(edges, mat_nr, use_smooth, sides)` (`sides` unused, as in Blender).
pub fn edgenet_fill(
    bm: &mut BMesh,
    edges: &[EdgeId],
    mat_nr: i32,
    use_smooth: bool,
) -> EdgenetFillOut {
    if bm.verts_num() == 0 || bm.edges_num() == 0 {
        return EdgenetFillOut::default();
    }
    construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    for &e in edges {
        bm.edge_mut(e).flag.insert(ElemFlag::TAG);
    }
    construct::faces_flag_disable_all(bm, ElemFlag::TAG);
    mesh_edgenet(bm, true, true);
    let faces: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
        .collect();
    for &f in &faces {
        bm.face_mut(f).mat_nr = mat_nr as i16;
        if use_smooth {
            bm.face_mut(f).flag.insert(ElemFlag::SMOOTH);
        }
        bm.face_normal_update(f);
    }
    let fail = attribute_fill::face_attribute_fill(bm, &faces, true, true);
    if !fail.is_empty() {
        attribute_fill::recalc_face_normals(bm, &fail);
    }
    EdgenetFillOut { faces }
}

/// `edge_next` of `bmo_edgenet_prepare_exec`.
fn prepare_edge_next(
    bm: &BMesh,
    mark: &dyn Fn(EdgeId) -> bool,
    vis: &rustc_hash::FxHashSet<EdgeId>,
    e: EdgeId,
) -> Option<EdgeId> {
    for v in bm.edge(e).v {
        for e2 in bm.vert_edges(v) {
            if mark(e2) && !vis.contains(&e2) && e2 != e {
                return Some(e2);
            }
        }
    }
    None
}

// Ported from blender/source/blender/bmesh/operators/bmo_edgenet.cc
// (bmo_edgenet_prepare_exec).
/// `bmesh.ops.edgenet_prepare(edges)`: joins the ends of one or two open chains; returns
/// `edges.out` (the new edges, in mesh order).
pub fn edgenet_prepare(bm: &mut BMesh, edges: &[EdgeId]) -> Vec<EdgeId> {
    let marked: rustc_hash::FxHashSet<EdgeId> = edges.iter().copied().collect();
    let mark = |e: EdgeId| marked.contains(&e);
    let count_marked =
        |bm: &BMesh, v: VertId| bm.vert_edges(v).into_iter().filter(|&e| mark(e)).count();
    for &e in edges {
        for v in bm.edge(e).v {
            if count_marked(bm, v) > 2 {
                return Vec::new();
            }
        }
    }
    let mut vis: rustc_hash::FxHashSet<EdgeId> = rustc_hash::FxHashSet::default();
    let mut edges1: Vec<EdgeId> = Vec::new();
    let mut edges2: Vec<EdgeId> = Vec::new();
    let mut count = 0;
    loop {
        let start = edges.iter().copied().find(|&e| {
            !vis.contains(&e) && {
                let [a, b] = bm.edge(e).v;
                count_marked(bm, a) == 1 || count_marked(bm, b) == 1
            }
        });
        let Some(mut e) = start else {
            break;
        };
        let list = match count {
            0 => &mut edges1,
            1 => &mut edges2,
            _ => break,
        };
        loop {
            vis.insert(e);
            list.push(e);
            match prepare_edge_next(bm, &mark, &vis, e) {
                Some(n) => e = n,
                None => break,
            }
        }
        count += 1;
    }
    if edges1.len() > 2
        && construct::edge_share_vert_check(bm, edges1[0], *edges1.last().expect("len > 2"))
    {
        if edges2.len() > 2
            && construct::edge_share_vert_check(bm, edges2[0], *edges2.last().expect("len > 2"))
        {
            return Vec::new();
        }
        edges1 = std::mem::take(&mut edges2);
    }
    if edges2.len() > 2
        && construct::edge_share_vert_check(bm, edges2[0], *edges2.last().expect("len > 2"))
    {
        edges2.clear();
    }
    let ends = |bm: &BMesh, list: &[EdgeId]| -> (VertId, VertId) {
        if list.len() == 1 {
            let [a, b] = bm.edge(list[0]).v;
            (a, b)
        } else {
            let e0 = bm.edge(list[0]).v;
            let a = if bm.vert_in_edge(list[1], e0[0]) {
                e0[1]
            } else {
                e0[0]
            };
            let i = list.len() - 1;
            let ei = bm.edge(list[i]).v;
            let b = if bm.vert_in_edge(list[i - 1], ei[0]) {
                ei[1]
            } else {
                ei[0]
            };
            (a, b)
        }
    };
    let mut new_edges: Vec<EdgeId> = Vec::new();
    if !edges1.is_empty() && !edges2.is_empty() {
        let (v1, v2) = ends(bm, &edges1);
        let (mut v3, mut v4) = ends(bm, &edges2);
        let co = |v: VertId| bm.vert(v).co;
        let d1 = bli::normal_tri_v3(co(v1), co(v2), co(v4)).0;
        let d2 = bli::normal_tri_v3(co(v1), co(v4), co(v3)).0;
        let dot_24 = bli::dot(d1, d2);
        let d1 = bli::normal_tri_v3(co(v1), co(v2), co(v3)).0;
        let d2 = bli::normal_tri_v3(co(v1), co(v3), co(v4)).0;
        let dot_13 = bli::dot(d1, d2);
        if dot_24 < dot_13 {
            std::mem::swap(&mut v3, &mut v4);
        }
        new_edges.push(bm.edge_create(v1, v3, None, CreateFlag::NO_DOUBLE));
        new_edges.push(bm.edge_create(v2, v4, None, CreateFlag::NO_DOUBLE));
    } else if !edges1.is_empty() && edges1.len() > 1 {
        let (v1, v2) = ends(bm, &edges1);
        new_edges.push(bm.edge_create(v1, v2, None, CreateFlag::NO_DOUBLE));
    }
    // `BMO_slot_buffer_from_enabled_flag(ELE_NEW)`: mesh order.
    let set: rustc_hash::FxHashSet<EdgeId> = new_edges.into_iter().collect();
    bm.edge_ids()
        .into_iter()
        .filter(|e| set.contains(e))
        .collect()
}
