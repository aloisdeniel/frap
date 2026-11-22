// SPDX-License-Identifier: GPL-3.0-or-later
//! `similar`: the topology core of `mesh.select_similar_region` (no `bmesh.ops` counterpart):
//! `BM_mesh_region_match`, which finds the face regions with the same topology as a given
//! region, and the selected-face groups it starts from. Frozen by M2 W0 (WP 2.0) as a stub;
//! implemented by WP 8.3.1, which replaced the stub's placeholder slots with these functions
//! (the operator's `type`/`compare`/`threshold` comparisons live in `frap-ops-mesh`).
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_region_match.cc (with
//! `USE_WALKER_REUSE`, `USE_PIVOT_FASTMATCH`, `USE_PIVOT_SEARCH`) and
//! blender/source/blender/bmesh/intern/bmesh_query.cc (`BM_mesh_calc_face_groups` with
//! `hflag_test = BM_ELEM_SELECT`, `htype_step = BM_VERT`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Neither function creates or kills elements (ARCHITECTURE §6.2). Blender keeps its state in
//! `BM_ELEM_TAG` flags and hash maps; the hashes are order independent (XOR sums), so frap's
//! maps give the same matches. Two Blender details are reproduced as they behave in practice:
//! the "fast match" vertex hash adds the neighbours of the first pass at every pass
//! (`bm_vert_fasthash_create` never updates `id_prev`), and a vertex reached by the walk but
//! connected to no vertex of an earlier pass (only possible inside n-gons of five or more
//! sides) gets the UID 0 where Blender reads uninitialised memory (`BLI_ghash_ensure_p`
//! leaves the value unset).

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{BMesh, EdgeId, ElemFlag, FaceId, VertId};

/// `PRIME_VERT_INIT`.
const PRIME_VERT_INIT: u64 = 100_003;

// ---------------------------------------------------------------------------- face groups

/// `BM_mesh_calc_face_groups(bm, …, nullptr, nullptr, nullptr, BM_ELEM_SELECT, BM_VERT)`: the
/// groups of selected faces connected through their edges, each in Blender's stack order.
pub fn selected_face_groups(bm: &BMesh) -> Vec<Vec<FaceId>> {
    let faces = bm.face_ids();
    let mut tag = vec![false; bm.faces.capacity()];
    let mut tot_faces = 0usize;
    for &f in &faces {
        if bm.face(f).flag.contains(ElemFlag::SELECT) {
            tot_faces += 1;
        } else {
            // Never walk over tagged.
            tag[f.slot()] = true;
        }
    }
    let mut groups = Vec::new();
    let mut next = 0usize;
    let mut tot_touch = 0usize;
    while tot_touch != tot_faces {
        let mut stack = Vec::new();
        while next < faces.len() {
            let f = faces[next];
            next += 1;
            if !tag[f.slot()] {
                tag[f.slot()] = true;
                stack.push(f);
                break;
            }
        }
        if stack.is_empty() {
            break;
        }
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            group.push(f);
            tot_touch += 1;
            // `BM_LOOPS_OF_LOOP`: the other loops of each corner's edge.
            for l in bm.face_loops(f) {
                let mut lr = bm.lp(l).radial_next;
                while lr != l {
                    let fo = bm.lp(lr).f;
                    if !tag[fo.slot()] {
                        tag[fo.slot()] = true;
                        stack.push(fo);
                    }
                    lr = bm.lp(lr).radial_next;
                }
            }
        }
        groups.push(group);
    }
    groups
}

// ------------------------------------------------------------------------------- the tags

/// The `BM_ELEM_TAG` flags of the search, per slot.
struct Tags {
    v: Vec<bool>,
    e: Vec<bool>,
    f: Vec<bool>,
}

impl Tags {
    fn new(bm: &BMesh) -> Self {
        Tags {
            v: vec![false; bm.verts.capacity()],
            e: vec![false; bm.edges.capacity()],
            f: vec![false; bm.faces.capacity()],
        }
    }
}

/// `bm_face_array_visit`: tags the faces' edges and vertices (and the faces with
/// `visit_faces`); returns the number of vertices newly tagged.
fn face_array_visit(bm: &BMesh, tags: &mut Tags, faces: &[FaceId], visit_faces: bool) -> usize {
    let mut verts_len = 0;
    for &f in faces {
        for l in bm.face_loops(f) {
            let ll = bm.lp(l);
            if !tags.v[ll.v.slot()] {
                verts_len += 1;
            }
            tags.e[ll.e.slot()] = true;
            tags.v[ll.v.slot()] = true;
        }
        if visit_faces {
            tags.f[f.slot()] = true;
        }
    }
    verts_len
}

// --------------------------------------------------------------------------------- UIDWalk

/// `UIDFaceStepItem`: faces sharing a UID.
#[derive(Debug)]
struct StepItem {
    uid: u64,
    list: Vec<FaceId>,
}

/// `UIDFaceStep`: faces a walk may step onto.
#[derive(Debug, Default)]
struct FaceStep {
    /// `faces` (a prepended linked list: newest first).
    faces: Vec<FaceId>,
    /// `items`, sorted by UID.
    items: Vec<StepItem>,
}

/// `UIDWalk`.
#[derive(Default)]
struct UidWalk {
    /// `faces_step`: step ids, head first.
    faces_step: VecDeque<usize>,
    steps: Vec<FaceStep>,
    verts_uid: FxHashMap<VertId, u64>,
    faces_uid: FxHashMap<FaceId, u64>,
    use_face_isolate: bool,
    pass: u64,
}

impl UidWalk {
    fn new() -> Self {
        UidWalk {
            pass: 1,
            ..UidWalk::default()
        }
    }

    /// `bm_uidwalk_face_test`.
    fn face_test(&self, tags: &Tags, f: FaceId) -> bool {
        !self.use_face_isolate || tags.f[f.slot()]
    }

    /// `bm_uidwalk_calc_vert_uid`.
    fn calc_vert_uid(&self, bm: &BMesh, v: VertId) -> u64 {
        let mut uid = self.pass.wrapping_mul(1031);
        let mut tot = 0u64;
        for e in bm.vert_edges(v) {
            let o = bm.edge_other_vert(e, v);
            if let Some(&u) = self.verts_uid.get(&o) {
                uid ^= u.wrapping_mul(7);
                tot += 1;
            }
        }
        uid ^= tot.wrapping_mul(43);
        let mut tot = 0u64;
        for l in bm.vert_loops(v) {
            if let Some(&u) = self.faces_uid.get(&bm.lp(l).f) {
                uid ^= u.wrapping_mul(13);
                tot += 1;
            }
        }
        uid ^ tot.wrapping_mul(53)
    }

    /// `bm_uidwalk_calc_face_uid`.
    fn calc_face_uid(&self, bm: &BMesh, f: FaceId) -> u64 {
        let mut uid = self
            .pass
            .wrapping_mul(u64::from(bm.face(f).len))
            .wrapping_mul(1013);
        let loops = bm.face_loops(f);
        for &l in &loops {
            if let Some(&u) = self.verts_uid.get(&bm.lp(l).v) {
                uid ^= u.wrapping_mul(11);
            }
        }
        for &l in &loops {
            let mut lr = bm.lp(l).radial_next;
            while lr != l {
                if let Some(&u) = self.faces_uid.get(&bm.lp(lr).f) {
                    uid ^= u.wrapping_mul(17);
                }
                lr = bm.lp(lr).radial_next;
            }
        }
        uid
    }

    /// `bm_uidwalk_rehash`: every UID recomputed from the old ones.
    fn rehash(&mut self, bm: &BMesh) {
        let verts: Vec<(VertId, u64)> = self
            .verts_uid
            .keys()
            .map(|&v| (v, self.calc_vert_uid(bm, v)))
            .collect();
        self.verts_uid.extend(verts);
        let faces: Vec<(FaceId, u64)> = self
            .faces_uid
            .keys()
            .map(|&f| (f, self.calc_face_uid(bm, f)))
            .collect();
        self.faces_uid.extend(faces);
    }

    /// `bm_uidwalk_rehash_facelinks`.
    fn rehash_facelinks(&mut self, bm: &BMesh, faces: &[FaceId]) {
        let uids: Vec<u64> = faces.iter().map(|&f| self.calc_face_uid(bm, f)).collect();
        for (&f, u) in faces.iter().zip(uids) {
            self.faces_uid.insert(f, u);
        }
    }

    /// `bm_vert_is_uid_connect`.
    fn vert_is_uid_connect(&self, bm: &BMesh, v: VertId) -> bool {
        bm.vert_edges(v)
            .into_iter()
            .any(|e| self.verts_uid.contains_key(&bm.edge_other_vert(e, v)))
    }

    /// `bm_uidwalk_pass_add`.
    fn pass_add(&mut self, bm: &BMesh, tags: &Tags, faces_pass: &[FaceId]) {
        self.rehash_facelinks(bm, faces_pass);
        let mut verts_pass: Vec<(VertId, u64)> = Vec::new();
        let mut verts_pass_set: FxHashSet<VertId> = FxHashSet::default();
        let mut faces_step_next: FxHashSet<FaceId> = FxHashSet::default();
        let mut step = FaceStep::default();
        for &f in faces_pass {
            for l in bm.face_loops(f) {
                let v = bm.lp(l).v;
                if !self.verts_uid.contains_key(&v) && verts_pass_set.insert(v) {
                    // Blender leaves the value unset when the vertex is not connected.
                    let uid = if self.vert_is_uid_connect(bm, v) {
                        self.calc_vert_uid(bm, v)
                    } else {
                        0
                    };
                    verts_pass.push((v, uid));
                }
                let mut lr = bm.lp(l).radial_next;
                while lr != l {
                    let fo = bm.lp(lr).f;
                    if !self.faces_uid.contains_key(&fo)
                        && !faces_step_next.contains(&fo)
                        && self.face_test(tags, fo)
                    {
                        faces_step_next.insert(fo);
                        // `BLI_linklist_prepend_pool`.
                        step.faces.insert(0, fo);
                    }
                    lr = bm.lp(lr).radial_next;
                }
            }
        }
        let id = self.steps.len();
        self.steps.push(step);
        self.faces_step.push_front(id);
        self.verts_uid.extend(verts_pass);
        self.rehash_facelinks(bm, faces_pass);
        self.pass += 1;
    }

    /// `bm_uidwalk_init_from_edge`: returns the number of face steps.
    fn init_from_edge(&mut self, bm: &BMesh, tags: &Tags, e: EdgeId) -> usize {
        let mut f_arr: Vec<FaceId> = bm
            .edge_faces(e)
            .into_iter()
            .filter(|&f| self.face_test(tags, f))
            .collect();
        f_arr.sort_by_key(|&f| bm.face(f).len);
        let [v1, v2] = bm.edge(e).v;
        self.verts_uid.insert(v1, PRIME_VERT_INIT);
        self.verts_uid.insert(v2, PRIME_VERT_INIT);
        let mut fstep_num = 0;
        let mut i = 0;
        while i < f_arr.len() {
            let f_len = bm.face(f_arr[i]).len;
            let mut faces_pass = Vec::new();
            while i < f_arr.len() && bm.face(f_arr[i]).len == f_len {
                faces_pass.insert(0, f_arr[i]);
                i += 1;
            }
            self.pass_add(bm, tags, &faces_pass);
            fstep_num += 1;
        }
        fstep_num
    }

    /// `bm_uidwalk_facestep_begin`: sorts the step's remaining faces into UID lists.
    fn facestep_begin(&mut self, bm: &BMesh, id: usize) -> bool {
        let mut ok = false;
        let faces = std::mem::take(&mut self.steps[id].faces);
        let mut kept = Vec::with_capacity(faces.len());
        let mut items: Vec<StepItem> = Vec::new();
        let mut by_uid: FxHashMap<u64, usize> = FxHashMap::default();
        for f in faces {
            if self.faces_uid.contains_key(&f) {
                continue;
            }
            let uid = self.calc_face_uid(bm, f);
            ok = true;
            let k = *by_uid.entry(uid).or_insert_with(|| {
                items.push(StepItem {
                    uid,
                    list: Vec::new(),
                });
                items.len() - 1
            });
            items[k].list.insert(0, f);
            kept.push(f);
        }
        items.sort_by_key(|it| it.uid);
        let step = &mut self.steps[id];
        step.faces = kept;
        step.items = items;
        ok
    }

    /// `bm_uidwalk_facestep_free`.
    fn facestep_free(&mut self, id: usize) {
        self.faces_step.retain(|&x| x != id);
    }
}

/// `bm_mesh_region_match_pair`: the region matching the source region through `e_src` ↔
/// `e_dst`, if any.
fn region_match_pair(
    bm: &BMesh,
    tags: &Tags,
    e_src: EdgeId,
    e_dst: EdgeId,
    faces_src_region_len: usize,
) -> Option<Vec<FaceId>> {
    let mut w_src = UidWalk::new();
    let mut w_dst = UidWalk::new();
    w_src.use_face_isolate = true;
    if w_src.init_from_edge(bm, tags, e_src) != w_dst.init_from_edge(bm, tags, e_dst) {
        return None;
    }
    loop {
        frap_math::budget::check();
        let mut ok = false;
        let src_steps: Vec<usize> = w_src.faces_step.iter().copied().collect();
        let dst_steps: Vec<usize> = w_dst.faces_step.iter().copied().collect();
        for (&s, &d) in src_steps.iter().zip(dst_steps.iter()) {
            if w_src.steps[s].faces.is_empty() {
                w_src.facestep_free(s);
                w_dst.facestep_free(d);
                continue;
            }
            if w_src.facestep_begin(bm, s) && w_dst.facestep_begin(bm, d) {
                let src_items = std::mem::take(&mut w_src.steps[s].items);
                let dst_items = std::mem::take(&mut w_dst.steps[d].items);
                let mut j = 0usize;
                for src_item in &src_items {
                    if j >= dst_items.len() {
                        break;
                    }
                    while j < dst_items.len() && dst_items[j].uid < src_item.uid {
                        j += 1;
                    }
                    let dst_item = dst_items.get(j);
                    match dst_item {
                        Some(di)
                            if di.uid == src_item.uid && src_item.list.len() <= di.list.len() =>
                        {
                            if src_item.list.len() == di.list.len() {
                                w_src.pass_add(bm, tags, &src_item.list);
                                w_dst.pass_add(bm, tags, &di.list);
                                ok = true;
                            }
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                    j += 1;
                }
            }
            // `bm_uidwalk_facestep_end`.
            w_src.steps[s].items.clear();
            w_dst.steps[d].items.clear();
        }
        if !ok {
            return None;
        }
        if w_dst.faces_uid.len() == faces_src_region_len {
            let mut faces: Vec<FaceId> = w_dst.faces_uid.keys().copied().collect();
            faces.sort_unstable();
            return Some(faces);
        }
        w_src.rehash(bm);
        w_dst.rehash(bm);
    }
}

// --------------------------------------------------------------------------- pivot search

/// `bm_edge_is_region_boundary`.
fn edge_is_region_boundary(bm: &BMesh, tags: &Tags, e: EdgeId) -> bool {
    let loops = bm.edge_loops(e);
    if loops.len() > 1 {
        return loops.iter().any(|&l| !tags.f[bm.lp(l).f.slot()]);
    }
    true
}

/// `bm_face_region_pivot_edge_use_best`.
fn pivot_edge_use_best(
    bm: &BMesh,
    gh: &FxHashMap<VertId, i64>,
    e_test: EdgeId,
    best: &mut Option<EdgeId>,
    best_id: &mut [i64; 2],
) {
    let [v1, v2] = bm.edge(e_test).v;
    let mut id = [
        gh.get(&v1).copied().unwrap_or(0),
        gh.get(&v2).copied().unwrap_or(0),
    ];
    if id[0] > id[1] {
        id.swap(0, 1);
    }
    let better = if best_id[0] != id[0] {
        best_id[0] < id[0]
    } else {
        best_id[1] < id[1]
    };
    if best.is_none() || better {
        *best_id = id;
        *best = Some(e_test);
    }
}

/// `bm_face_region_vert_boundary_id`.
fn vert_boundary_id(bm: &BMesh, tags: &Tags, v: VertId) -> i64 {
    let mut tot: i64 = 0;
    let mut id: i64 = 103;
    for l in bm.vert_loops(v) {
        let ll = bm.lp(l);
        let is_boundary_vert = edge_is_region_boundary(bm, tags, ll.e)
            || edge_is_region_boundary(bm, tags, bm.lp(ll.prev).e);
        let len = i64::from(bm.face(ll.f).len);
        id ^= len.wrapping_mul(if is_boundary_vert { 7 } else { 13 });
        tot += 1;
    }
    id ^= tot.wrapping_mul(131);
    if id != 0 { id.wrapping_abs() } else { 1 }
}

/// `bm_face_region_vert_pass_id`.
fn vert_pass_id(bm: &BMesh, tags: &Tags, gh: &FxHashMap<VertId, i64>, v: VertId) -> i64 {
    let mut tot: i64 = 0;
    let mut v_sum_face_len: i64 = 0;
    let mut v_sum_id: i64 = 0;
    let id_min = i64::MIN + 1;
    for e in bm.vert_edges(v) {
        if !tags.e[e.slot()] {
            continue;
        }
        let o = bm.edge_other_vert(e, v);
        if !tags.v[o.slot()] {
            continue;
        }
        let o_id = gh.get(&o).copied().unwrap_or(0);
        if o_id > 0 {
            v_sum_id = v_sum_id.wrapping_add(o_id);
            tot += 1;
            for l in bm.edge_loops(e) {
                let f = bm.lp(l).f;
                if tags.f[f.slot()] {
                    v_sum_face_len += i64::from(bm.face(f).len);
                }
            }
        }
    }
    let mut id = tot.wrapping_mul(23);
    id ^= v_sum_face_len.wrapping_mul(31);
    id ^= v_sum_id;
    id = if id == 0 {
        1
    } else if id < id_min {
        id_min
    } else {
        id
    };
    id.wrapping_abs()
}

/// `bm_face_region_pivot_edge_find`: the innermost edge of the region and its depth (passes
/// from the boundary).
fn pivot_edge_find(
    bm: &BMesh,
    tags: &mut Tags,
    faces_region: &[FaceId],
    verts_region_len: usize,
) -> (Option<EdgeId>, u32) {
    let mut gh: FxHashMap<VertId, i64> = FxHashMap::default();
    // `BLI_LINKSTACK`: pushes and pops at the head (index 0 here is the head).
    let mut prev: VecDeque<VertId> = VecDeque::new();
    let mut next: VecDeque<VertId> = VecDeque::new();
    let mut e_pivot_fallback = None;
    let mut pass: i64 = 0;
    let mut used = 0usize;
    for &f in faces_region {
        for l in bm.face_loops(f) {
            let e = bm.lp(l).e;
            if edge_is_region_boundary(bm, tags, e) {
                for v in bm.edge(e).v {
                    if let std::collections::hash_map::Entry::Vacant(slot) = gh.entry(v) {
                        slot.insert(vert_boundary_id(bm, tags, v));
                        prev.push_front(v);
                        used += 1;
                    }
                }
            } else {
                // Used when there are no interior vertices (depth 0).
                e_pivot_fallback = Some(e);
            }
        }
    }
    while !prev.is_empty() {
        while let Some(v) = prev.pop_front() {
            for e in bm.vert_edges(v) {
                if !tags.e[e.slot()] {
                    continue;
                }
                let o = bm.edge_other_vert(e, v);
                if tags.v[o.slot()] && !gh.contains_key(&o) {
                    // Added as negative, so they are not read this pass.
                    let id = -vert_pass_id(bm, tags, &gh, o);
                    next.push_front(o);
                    used += 1;
                    gh.insert(o, id);
                }
            }
        }
        for v in &next {
            if let Some(id) = gh.get_mut(v) {
                *id = -*id;
            }
        }
        std::mem::swap(&mut prev, &mut next);
        pass += 1;
        if used == verts_region_len {
            break;
        }
    }
    let mut e_pivot = None;
    if prev.len() >= 2 {
        let mut best = None;
        let mut best_id = [0i64; 2];
        for v in &prev {
            tags.v[v.slot()] = false;
        }
        for &v in &prev {
            tags.v[v.slot()] = true;
            for e in bm.vert_edges(v) {
                if tags.e[e.slot()] && !tags.v[bm.edge_other_vert(e, v).slot()] {
                    pivot_edge_use_best(bm, &gh, e, &mut best, &mut best_id);
                }
            }
        }
        e_pivot = best;
    }
    if e_pivot.is_none() && !prev.is_empty() {
        let mut best = None;
        let mut best_id = [0i64; 2];
        pass -= 1;
        for &v in &prev {
            for e in bm.vert_edges(v) {
                if tags.e[e.slot()] && tags.v[bm.edge_other_vert(e, v).slot()] {
                    pivot_edge_use_best(bm, &gh, e, &mut best, &mut best_id);
                }
            }
        }
        e_pivot = best;
    }
    if e_pivot.is_none() {
        e_pivot = e_pivot_fallback;
        pass = 0;
    }
    (e_pivot, pass.max(0) as u32)
}

// ------------------------------------------------------------------------------ fast match

/// `bm_vert_fasthash_single`.
fn vert_fasthash_single(bm: &BMesh, v: VertId) -> u64 {
    let (mut e_num, mut f_num, mut l_num) = (0u64, 0u64, 0u64);
    for e in bm.vert_edges(v) {
        if bm.edge_is_wire(e) {
            continue;
        }
        e_num += 1;
        for l in bm.edge_loops(e) {
            f_num += 1;
            l_num += u64::from(bm.face(bm.lp(l).f).len);
        }
    }
    (e_num.wrapping_mul(7)) ^ (f_num.wrapping_mul(31)).wrapping_mul(l_num.wrapping_mul(61))
}

/// `bm_vert_fasthash_create`: per vertex index. Blender copies `id_prev` and adds the first
/// pass's neighbour values at every pass without updating `id_prev`, so any `depth > 0` gives
/// the one-pass result.
fn vert_fasthash_create(bm: &BMesh, verts: &[VertId], index: &[usize], depth: u32) -> Vec<u64> {
    let id_prev: Vec<u64> = verts.iter().map(|&v| vert_fasthash_single(bm, v)).collect();
    let mut id_curr = id_prev.clone();
    for _ in 0..depth {
        id_curr.copy_from_slice(&id_prev);
        for e in bm.edge_ids() {
            if bm.edge_is_wire(e) {
                continue;
            }
            let [a, b] = bm.edge(e).v;
            let (i1, i2) = (index[a.slot()], index[b.slot()]);
            id_curr[i1] = id_curr[i1].wrapping_add(id_prev[i2]);
            id_curr[i2] = id_curr[i2].wrapping_add(id_prev[i1]);
        }
    }
    id_curr
}

/// `bm_vert_fasthash_edge_order`.
fn fasthash_edge_order(bm: &BMesh, fm: &[u64], index: &[usize], e: EdgeId) -> [u64; 2] {
    let [a, b] = bm.edge(e).v;
    let mut r = [fm[index[a.slot()]], fm[index[b.slot()]]];
    if r[0] > r[1] {
        r.swap(0, 1);
    }
    r
}

// ------------------------------------------------------------------------------- the match

/// `BM_mesh_region_match`: the regions (faces, sorted) with the topology of `faces_region`
/// elsewhere in the mesh, in the order of the edges they were found from.
pub fn region_match(bm: &BMesh, faces_region: &[FaceId]) -> Vec<Vec<FaceId>> {
    let mut tags = Tags::new(bm);
    let verts_region_len = face_array_visit(bm, &mut tags, faces_region, true);
    let (e_src, depth) = pivot_edge_find(bm, &mut tags, faces_region, verts_region_len);
    let Some(e_src) = e_src else {
        return Vec::new();
    };
    let verts = bm.vert_ids();
    let mut index = vec![0usize; bm.verts.capacity()];
    for (i, v) in verts.iter().enumerate() {
        index[v.slot()] = i;
    }
    let fm = (depth > 0).then(|| vert_fasthash_create(bm, &verts, &index, depth));
    let src_fm = fm
        .as_ref()
        .map(|fm| fasthash_edge_order(bm, fm, &index, e_src));
    let mut regions = Vec::new();
    for e_dst in bm.edge_ids() {
        if tags.e[e_dst.slot()] || bm.edge_is_wire(e_dst) {
            continue;
        }
        if let (Some(fm), Some(s)) = (fm.as_ref(), src_fm)
            && fasthash_edge_order(bm, fm, &index, e_dst) != s
        {
            continue;
        }
        if let Some(found) = region_match_pair(bm, &tags, e_src, e_dst, faces_region.len()) {
            face_array_visit(bm, &mut tags, &found, false);
            regions.push(found);
        }
    }
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `n`×1 strip of quads.
    fn strip(n: usize) -> BMesh {
        let mut bm = BMesh::new();
        let mut bottom = Vec::new();
        let mut top = Vec::new();
        for i in 0..=n {
            bottom.push(bm.vert_create([i as f32, 0.0, 0.0], None, Default::default()));
            top.push(bm.vert_create([i as f32, 1.0, 0.0], None, Default::default()));
        }
        for i in 0..n {
            bm.face_create_verts(
                &[bottom[i], bottom[i + 1], top[i + 1], top[i]],
                None,
                Default::default(),
                true,
            )
            .expect("quad");
        }
        bm
    }

    #[test]
    fn groups_follow_selected_edges() {
        let mut bm = strip(4);
        let faces = bm.face_ids();
        for &f in &[faces[0], faces[1], faces[3]] {
            bm.face_mut(f).flag.insert(ElemFlag::SELECT);
        }
        let groups = selected_face_groups(&bm);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1], vec![faces[3]]);
    }

    #[test]
    fn a_quad_pair_on_a_strip_matches_nothing() {
        // Verified on Blender 5.2.4: two quads of a 6×1 grid find no other region (the
        // destination walk is not isolated and outgrows the source region).
        let bm = strip(6);
        let faces = bm.face_ids();
        assert!(region_match(&bm, &faces[0..2]).is_empty());
        assert!(region_match(&BMesh::new(), &[]).is_empty());
    }
}
