// SPDX-License-Identifier: GPL-3.0-or-later
//! Structural validation (`BM_mesh_validate`; ARCHITECTURE §6.2): disk and radial cycle
//! consistency, face loop cycles, pool bookkeeping. Used by the proptests of every op (random
//! kernel and op sequences keep `validate` green). CONTRACT frozen by M2 W0, implemented by
//! WP 2.1.
//!
//! [`BMesh::validate`] checks what the kernel guarantees (the `BM_CHECK_ELEMENT` /
//! `bmesh_disk_validate` / `bmesh_radial_validate` / `bmesh_loop_validate` invariants plus
//! pool and custom-data bookkeeping); [`BMesh::validate_strict`] adds the topology checks of
//! Blender's `BM_mesh_is_valid` (duplicate edges and faces, repeated vertices or edges in a
//! face, hidden and selected elements), which a valid editing session can violate
//! temporarily.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

impl BMesh {
    /// Every inconsistency found, as messages (empty when valid).
    pub fn validate(&self) -> Vec<String> {
        let mut err = Vec::new();
        self.validate_pools(&mut err);
        if !err.is_empty() {
            return err;
        }
        self.validate_verts(&mut err);
        self.validate_edges(&mut err);
        self.validate_faces(&mut err);
        self.validate_loops_owned(&mut err);
        for &h in &self.select_history {
            if !self.elem_alive(h) {
                err.push(format!("select history: dead element {h:?}"));
            }
        }
        if let Some(f) = self.act_face
            && !self.faces.contains(f.0)
        {
            err.push(format!("act_face: dead face {}", f.0));
        }
        err
    }

    /// [`BMesh::validate`] plus `BM_mesh_is_valid`'s duplicate and flag checks.
    pub fn validate_strict(&self) -> Vec<String> {
        let mut err = self.validate();
        if !err.is_empty() {
            return err;
        }
        let mut edge_hash: FxHashMap<(u32, u32), EdgeId> = FxHashMap::default();
        for (i, e) in self.edges.iter() {
            let k = (e.v[0].0.min(e.v[1].0), e.v[0].0.max(e.v[1].0));
            if let Some(o) = edge_hash.insert(k, EdgeId(i)) {
                err.push(format!("edge {i}, {}: are duplicates", o.0));
            }
        }
        let check_hs = |what: &str, i: u32, f: ElemFlag, err: &mut Vec<String>| {
            if f.contains(ElemFlag::SELECT | ElemFlag::HIDDEN) {
                err.push(format!("{what} {i}: is hidden and selected"));
            }
        };
        for (i, v) in self.verts.iter() {
            check_hs("vert", i, v.flag, &mut err);
        }
        for (i, e) in self.edges.iter() {
            check_hs("edge", i, e.flag, &mut err);
        }
        let mut face_map: FxHashMap<Vec<u32>, u32> = FxHashMap::default();
        for (i, f) in self.faces.iter() {
            check_hs("face", i, f.flag, &mut err);
            let loops = self.face_loops(FaceId(i));
            let mut vs = FxHashSet::default();
            let mut es = FxHashSet::default();
            for (j, &l) in loops.iter().enumerate() {
                if !vs.insert(self.lp(l).v) {
                    err.push(format!("face {i}: has duplicate vert at corner: {j}"));
                }
                if !es.insert(self.lp(l).e) {
                    err.push(format!("face {i}: has duplicate edge at corner: {j}"));
                }
            }
            // Ordered vertex list starting at the lowest vertex, winding-independent.
            let verts: Vec<u32> = loops.iter().map(|&l| self.lp(l).v.0).collect();
            let n = verts.len();
            let (imin, _) = verts
                .iter()
                .enumerate()
                .min_by_key(|&(_, v)| *v)
                .expect("faces have loops");
            let fwd = verts[(imin + 1) % n] < verts[(imin + n - 1) % n];
            let key: Vec<u32> = (0..n)
                .map(|k| {
                    if fwd {
                        verts[(imin + k) % n]
                    } else {
                        verts[(imin + n - k) % n]
                    }
                })
                .collect();
            if let Some(o) = face_map.insert(key, i) {
                err.push(format!("face {i}: duplicate of {o}"));
            }
        }
        err
    }

    fn validate_pools(&self, err: &mut Vec<String>) {
        fn pool<T>(name: &str, p: &crate::pool::Pool<T>, err: &mut Vec<String>) {
            let live = p.slots.iter().filter(|s| s.is_some()).count();
            if live != p.live as usize {
                err.push(format!("{name} pool: live count {} != {live}", p.live));
            }
            let mut seen = FxHashSet::default();
            for &f in &p.free {
                if !seen.insert(f) {
                    err.push(format!("{name} pool: slot {f} freed twice"));
                }
                match p.slots.get(f as usize) {
                    Some(None) => {}
                    _ => err.push(format!(
                        "{name} pool: free slot {f} is live or out of range"
                    )),
                }
            }
            let free_slots = p.slots.iter().filter(|s| s.is_none()).count();
            if free_slots != p.free.len() {
                err.push(format!(
                    "{name} pool: {free_slots} empty slots but {} on the free list",
                    p.free.len()
                ));
            }
        }
        pool("vert", &self.verts, err);
        pool("edge", &self.edges, err);
        pool("loop", &self.loops, err);
        pool("face", &self.faces, err);
        let cd =
            |name: &str, d: &crate::customdata::CustomData, cap: usize, err: &mut Vec<String>| {
                for l in &d.layers {
                    if l.data.len() < cap {
                        err.push(format!(
                            "{name} custom data '{}': {} values for {cap} slots",
                            l.name,
                            l.data.len()
                        ));
                    }
                }
            };
        cd("vert", &self.vdata, self.verts.capacity(), err);
        cd("edge", &self.edata, self.edges.capacity(), err);
        cd("loop", &self.ldata, self.loops.capacity(), err);
        cd("face", &self.pdata, self.faces.capacity(), err);
    }

    fn validate_verts(&self, err: &mut Vec<String>) {
        for (i, v) in self.verts.iter() {
            let vid = VertId(i);
            let Some(e) = v.e else {
                continue;
            };
            if !self.edges.contains(e.0) {
                err.push(format!("vert {i}: references dead edge {}", e.0));
                continue;
            }
            if !self.vert_in_edge(e, vid) {
                err.push(format!("vert {i}: is not in its referenced edge: {}", e.0));
                continue;
            }
            // Disk cycle: every step stays on edges using `v`, `prev` mirrors `next`, and the
            // cycle closes without repeating.
            let mut seen = FxHashSet::default();
            let mut it = e;
            loop {
                if !seen.insert(it) {
                    err.push(format!(
                        "vert {i}: disk cycle does not close at edge {}",
                        it.0
                    ));
                    break;
                }
                let next = self.disk_edge_next(it, vid);
                if !self.edges.contains(next.0) || !self.vert_in_edge(next, vid) {
                    err.push(format!(
                        "vert {i}: disk link of edge {} leaves the vertex",
                        it.0
                    ));
                    break;
                }
                if self.disk_edge_prev(next, vid) != it {
                    err.push(format!(
                        "vert {i}: disk prev/next mismatch at edge {}",
                        next.0
                    ));
                    break;
                }
                it = next;
                if it == e {
                    break;
                }
            }
        }
    }

    fn validate_edges(&self, err: &mut Vec<String>) {
        for (i, e) in self.edges.iter() {
            let eid = EdgeId(i);
            if e.v[0] == e.v[1] {
                err.push(format!("edge {i}: duplicate index: {}", e.v[0].0));
                continue;
            }
            let mut ok = true;
            for v in e.v {
                if !self.verts.contains(v.0) {
                    err.push(format!("edge {i}: references dead vert {}", v.0));
                    ok = false;
                }
            }
            if !ok {
                continue;
            }
            // The edge must be reachable from both vertices' disk cycles.
            for v in e.v {
                let Some(first) = self.vert(v).e else {
                    err.push(format!("edge {i}: vert {} has no disk cycle", v.0));
                    continue;
                };
                let mut it = first;
                let mut found = false;
                let mut n = 0usize;
                loop {
                    if it == eid {
                        found = true;
                        break;
                    }
                    let next = self.disk_edge_next(it, v);
                    if !self.edges.contains(next.0) || !self.vert_in_edge(next, v) {
                        break;
                    }
                    it = next;
                    n += 1;
                    if it == first || n > self.edges.capacity() {
                        break;
                    }
                }
                if !found {
                    err.push(format!(
                        "edge {i}: missing from the disk cycle of vert {}",
                        v.0
                    ));
                }
            }
            let Some(l0) = e.l else {
                continue;
            };
            if !self.loops.contains(l0.0) {
                err.push(format!("edge {i}: references dead loop {}", l0.0));
                continue;
            }
            let mut seen = FxHashSet::default();
            let mut it = l0;
            loop {
                if !seen.insert(it) {
                    err.push(format!("edge {i}: radial cycle does not close"));
                    break;
                }
                let ll = self.lp(it);
                if ll.e != eid {
                    err.push(format!("edge {i}: has invalid loop {}", it.0));
                    break;
                }
                if !self.vert_in_edge(eid, ll.v) {
                    err.push(format!("edge {i}: has invalid loop with vert not in edge"));
                }
                if self.loops.contains(ll.next.0) && !self.vert_in_edge(eid, self.lp(ll.next).v) {
                    err.push(format!(
                        "edge {i}: has invalid loop with next vert not in edge"
                    ));
                }
                if !self.loops.contains(ll.radial_next.0)
                    || self.lp(ll.radial_next).radial_prev != it
                {
                    err.push(format!(
                        "edge {i}: radial prev/next mismatch at loop {}",
                        it.0
                    ));
                    break;
                }
                it = ll.radial_next;
                if it == l0 {
                    break;
                }
            }
        }
    }

    fn validate_faces(&self, err: &mut Vec<String>) {
        for (i, f) in self.faces.iter() {
            let fid = FaceId(i);
            if !self.loops.contains(f.l_first.0) {
                err.push(format!("face {i}: references dead loop {}", f.l_first.0));
                continue;
            }
            let mut n = 0u32;
            let mut it = f.l_first;
            let mut seen = FxHashSet::default();
            loop {
                if !seen.insert(it) {
                    err.push(format!("face {i}: loop cycle does not close"));
                    break;
                }
                let ll = self.lp(it);
                if ll.f != fid {
                    err.push(format!(
                        "face {i}: has loop that points to face: {}",
                        ll.f.0
                    ));
                }
                if !self.loops.contains(ll.next.0) || !self.loops.contains(ll.prev.0) {
                    err.push(format!("face {i}: dead next/prev at loop {}", it.0));
                    break;
                }
                if self.lp(ll.next).prev != it {
                    err.push(format!(
                        "face {i}: has invalid 'next/prev' at loop {}",
                        it.0
                    ));
                }
                if self.lp(ll.prev).next != it {
                    err.push(format!(
                        "face {i}: has invalid 'prev/next' at loop {}",
                        it.0
                    ));
                }
                if !self.edges.contains(ll.e.0) || !self.verts.contains(ll.v.0) {
                    err.push(format!("face {i}: loop {} references dead elements", it.0));
                    break;
                }
                if !self.verts_in_edge(ll.v, self.lp(ll.next).v, ll.e) {
                    err.push(format!(
                        "face {i}: loop {} edge does not join its vert and the next",
                        it.0
                    ));
                }
                // The loop must be in its edge's radial cycle.
                let found = self.edge_loops(ll.e).contains(&it);
                if !found {
                    err.push(format!(
                        "face {i}: loop {} missing from its edge's radial cycle",
                        it.0
                    ));
                }
                n += 1;
                it = ll.next;
                if it == f.l_first {
                    break;
                }
            }
            if n != f.len {
                err.push(format!(
                    "face {i}: has length of {} but should be {n}",
                    f.len
                ));
            }
            if f.len < 1 {
                err.push(format!("face {i}: is empty"));
            }
        }
    }

    /// Every live loop belongs to a live face whose cycle contains it.
    fn validate_loops_owned(&self, err: &mut Vec<String>) {
        let mut owned: FxHashSet<LoopId> = FxHashSet::default();
        for (i, _) in self.faces.iter() {
            owned.extend(self.face_loops(FaceId(i)));
        }
        for (i, _) in self.loops.iter() {
            if !owned.contains(&LoopId(i)) {
                err.push(format!("loop {i}: not in any face cycle"));
            }
        }
    }
}
