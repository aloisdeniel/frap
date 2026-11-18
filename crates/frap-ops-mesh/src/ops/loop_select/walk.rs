// SPDX-License-Identifier: GPL-3.0-or-later
//! Walkers the selection operators need beyond `frap_bmesh::walkers` (edge loop, edge ring):
//! `BMW_FACELOOP`, `BMW_EDGEBOUNDARY`, `BMW_EDGELOOP_NONMANIFOLD`, and the persistent-visit
//! forms of `BMW_VERT_SHELL`, `BMW_LOOP_SHELL_WIRE` and `BMW_ISLAND` that
//! `mesh.select_linked` restarts from many elements with one walker.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc and
//! bmesh_walkers.cc (Blender 5.2.4, GPL-2.0-or-later). The `BMWalker` framework is emulated as
//! in `frap_bmesh::walkers`: a state list where depth-first walkers push at the head and
//! breadth-first ones at the tail; `BMW_begin` runs the begin callback, then one step;
//! `BMW_step` steps until a step yields. Every walker here uses `BMW_FLAG_TEST_HIDDEN`, as the
//! selection operators do.

use std::collections::VecDeque;

use frap_bmesh::walkers::LoopDelimit;
use frap_bmesh::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use rustc_hash::FxHashSet;

fn hidden_e(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e).flag.contains(ElemFlag::HIDDEN)
}
fn hidden_f(bm: &BMesh, f: FaceId) -> bool {
    bm.face(f).flag.contains(ElemFlag::HIDDEN)
}
fn hidden_v(bm: &BMesh, v: VertId) -> bool {
    bm.vert(v).flag.contains(ElemFlag::HIDDEN)
}

/// `BM_loop_is_manifold`.
fn loop_is_manifold(bm: &BMesh, l: LoopId) -> bool {
    let r = bm.lp(l).radial_next;
    l != r && l == bm.lp(r).radial_next
}

// ------------------------------------------------------------------------------ face loop

#[derive(Clone)]
struct FaceLoopState {
    l: LoopId,
    no_calc: bool,
}

struct FaceLoop<'a> {
    bm: &'a BMesh,
    delimit: LoopDelimit,
    st: VecDeque<FaceLoopState>,
    visit: FxHashSet<FaceId>,
    visit_alt: FxHashSet<EdgeId>,
}

impl FaceLoop<'_> {
    /// `bmw_FaceLoopWalker_include_face`.
    fn include_face(&self, l: LoopId) -> bool {
        let f = self.bm.lp(l).f;
        if self.bm.face(f).len != 4 || hidden_f(self.bm, f) {
            return false;
        }
        !(self.visit.contains(&f) && self.visit_alt.contains(&self.bm.lp(l).e))
    }

    /// `bmw_FaceLoopWalker_edge_begins_loop`.
    fn edge_begins_loop(&self, e: EdgeId) -> bool {
        let bm = self.bm;
        if bm.edge_is_wire(e) {
            return false;
        }
        if bm.edge_is_boundary(e) && !self.include_face(bm.edge(e).l.expect("boundary")) {
            return false;
        }
        bm.edge_is_manifold(e)
    }

    /// `bmw_FaceLoopWalker_delimit_check`.
    fn delimit_check(&self, f_a: FaceId, e: EdgeId, f_b: FaceId) -> bool {
        let ef = self.bm.edge(e).flag;
        if self.delimit.contains(LoopDelimit::SEAM) && ef.contains(ElemFlag::SEAM) {
            return true;
        }
        if self.delimit.contains(LoopDelimit::SHARP) && !ef.contains(ElemFlag::SMOOTH) {
            return true;
        }
        self.delimit.contains(LoopDelimit::MATERIAL)
            && self.bm.face(f_a).mat_nr != self.bm.face(f_b).mat_nr
    }

    /// `bmw_FaceLoopWalker_step`.
    fn step(&mut self) -> Option<FaceId> {
        let bm = self.bm;
        let owalk = self.st.pop_front()?;
        let f = bm.lp(owalk.l).f;
        let mut l = bm.lp(owalk.l).radial_next;
        if owalk.no_calc {
            return Some(f);
        }
        if !self.include_face(l) {
            l = bm.lp(bm.lp(owalk.l).next).next;
            if !bm.edge_is_manifold(bm.lp(l).e) {
                l = bm.lp(bm.lp(l).prev).prev;
            }
            l = bm.lp(l).radial_next;
        }
        if self.include_face(l) {
            let lf = bm.lp(l).f;
            let le = bm.lp(l).e;
            let no_calc = bm.face(lf).len != 4 || self.delimit_check(f, le, lf);
            self.st.push_front(FaceLoopState {
                l: if no_calc { owalk.l } else { l },
                no_calc,
            });
            self.visit_alt.insert(le);
            self.visit.insert(lf);
        }
        Some(f)
    }
}

/// `BMW_FACELOOP` from `e` (the faces of the face loop across `e`, in walk order; a face may
/// be yielded twice where a delimiter stops the walk, as in Blender).
pub(crate) fn face_loop(bm: &BMesh, e: EdgeId, delimit: LoopDelimit) -> Vec<FaceId> {
    let mut w = FaceLoop {
        bm,
        delimit,
        st: VecDeque::new(),
        visit: FxHashSet::default(),
        visit_alt: FxHashSet::default(),
    };
    if !w.edge_begins_loop(e) {
        return Vec::new();
    }
    let el = bm.edge(e).l.expect("manifold edge has loops");
    w.st.push_front(FaceLoopState {
        l: el,
        no_calc: false,
    });
    w.visit.insert(bm.lp(el).f);
    // Rewind.
    let mut owalk = None;
    while let Some(cur) = w.st.front() {
        owalk = Some(cur.clone());
        w.step();
    }
    let mut lwalk = owalk.expect("begin pushed a state");
    lwalk.no_calc = false;
    let l = lwalk.l;
    w.st.push_front(lwalk);
    w.visit_alt.clear();
    w.visit_alt.insert(bm.lp(l).e);
    w.visit.clear();
    w.visit.insert(bm.lp(l).f);
    // When starting on a delimiting edge, add both sides so both directions are walked.
    let r = bm.lp(l).radial_next;
    if w.delimit_check(bm.lp(l).f, bm.lp(l).e, bm.lp(r).f) {
        w.st.push_front(FaceLoopState {
            l: r,
            no_calc: false,
        });
        w.visit.insert(bm.lp(r).f);
    }
    let mut out = Vec::new();
    while !w.st.is_empty() {
        if let Some(f) = w.step() {
            out.push(f);
        }
    }
    out
}

// -------------------------------------------------------------------------- edge boundary

/// `BMW_EDGEBOUNDARY` from the boundary edge `e` (depth first; a hidden edge is yielded but
/// not stepped from).
pub(crate) fn edge_boundary(bm: &BMesh, e: EdgeId) -> Vec<EdgeId> {
    let mut st: VecDeque<EdgeId> = VecDeque::new();
    let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
    st.push_front(e);
    visit.insert(e);
    let mut out = Vec::new();
    while let Some(e) = st.pop_front() {
        out.push(e);
        if hidden_e(bm, e) {
            continue;
        }
        for v in bm.edge(e).v {
            for eo in bm.vert_edges(v) {
                if eo != e
                    && bm.edge_is_boundary(eo)
                    && !visit.contains(&eo)
                    && !hidden_e(bm, eo)
                {
                    st.push_front(eo);
                    visit.insert(eo);
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------------- non-manifold edge loop

#[derive(Clone)]
struct NonManifoldState {
    start: EdgeId,
    cur: EdgeId,
    startv: VertId,
    lastv: VertId,
    face_count: usize,
}

/// `bmw_NonManifoldLoop_find_next_around_vertex`.
fn non_manifold_next_around_vertex(
    bm: &BMesh,
    mut l: LoopId,
    v: VertId,
    face_count: usize,
) -> Option<LoopId> {
    loop {
        l = bm.loop_other_edge_loop(l, v);
        if loop_is_manifold(bm, l) {
            l = bm.lp(l).radial_next;
        } else if bm.edge_face_count(bm.lp(l).e) == face_count {
            return Some(l);
        } else {
            return None;
        }
    }
}

/// `BMW_EDGELOOP_NONMANIFOLD` from `e` (an edge with more than two faces): the edges with the
/// same face count continuing the loop on both sides; a junction stops the walk.
pub(crate) fn edge_loop_nonmanifold(bm: &BMesh, e: EdgeId) -> Vec<EdgeId> {
    let mut st: VecDeque<NonManifoldState> = VecDeque::new();
    let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
    let v1 = bm.edge(e).v[0];
    st.push_front(NonManifoldState {
        start: e,
        cur: e,
        startv: v1,
        lastv: v1,
        face_count: bm.edge_face_count(e),
    });
    visit.insert(e);
    let mut out = Vec::new();
    while let Some(owalk) = st.pop_front() {
        let face_count = owalk.face_count;
        let mut l_cur: Option<LoopId> = None;
        let mut v = owalk.lastv;
        for pass in 0..2 {
            let mut e = owalk.cur;
            v = bm.edge_other_vert(e, owalk.lastv);
            if pass == 1 {
                e = owalk.start;
                v = owalk.startv;
            }
            let Some(l_first) = bm.edge(e).l else { break };
            let mut l = l_first;
            loop {
                let next = non_manifold_next_around_vertex(bm, l, v, face_count);
                if let Some(ln) = next
                    && !visit.contains(&bm.lp(ln).e)
                {
                    match l_cur {
                        None => l_cur = Some(ln),
                        Some(c) if bm.lp(c).e != bm.lp(ln).e => {
                            // More than one possible edge: a junction, stop walking.
                            l_cur = None;
                            break;
                        }
                        Some(_) => {}
                    }
                }
                l = bm.lp(l).radial_next;
                if l == l_first {
                    break;
                }
            }
            if l_cur.is_some() {
                break;
            }
        }
        if let Some(lc) = l_cur {
            let ce = bm.lp(lc).e;
            st.push_front(NonManifoldState {
                start: owalk.start,
                cur: ce,
                startv: owalk.startv,
                lastv: v,
                face_count,
            });
            visit.insert(ce);
        }
        out.push(owalk.cur);
    }
    out
}

// ------------------------------------------------------------------------------ vert shell

/// `BMW_VERT_SHELL` kept across several starts (`BMW_ITER` on one walker): yields each
/// visible edge connected to the start once over all starts.
pub(crate) struct VertShell {
    visit: FxHashSet<EdgeId>,
}

impl VertShell {
    pub(crate) fn new() -> Self {
        VertShell {
            visit: FxHashSet::default(),
        }
    }

    fn visit_edge(&mut self, bm: &BMesh, st: &mut VecDeque<EdgeId>, e: EdgeId) {
        if self.visit.contains(&e) || hidden_e(bm, e) {
            return;
        }
        st.push_back(e);
        self.visit.insert(e);
    }

    /// Walks from vertex `v` (begin) or edge `e`.
    pub(crate) fn walk(&mut self, bm: &BMesh, from: Start) -> Vec<EdgeId> {
        let mut st = VecDeque::new();
        match from {
            Start::Vert(v) => {
                for e in bm.vert_edges(v) {
                    self.visit_edge(bm, &mut st, e);
                }
            }
            Start::Edge(e) => self.visit_edge(bm, &mut st, e),
        }
        let mut out = Vec::new();
        while let Some(e) = st.pop_front() {
            for v in bm.edge(e).v {
                for e2 in bm.vert_edges(v) {
                    self.visit_edge(bm, &mut st, e2);
                }
            }
            out.push(e);
        }
        out
    }
}

/// Where a shell walk starts.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Start {
    /// A vertex.
    Vert(VertId),
    /// An edge.
    Edge(EdgeId),
}

// -------------------------------------------------------------------- loop shell and wire

/// What `BMW_LOOP_SHELL_WIRE` yields.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ShellItem {
    /// A face corner.
    Loop(LoopId),
    /// A wire edge.
    Edge(EdgeId),
}

/// `BMW_LOOP_SHELL_WIRE` kept across several starts, with an edge mask (`mask_edge`: the
/// edges the walk may cross; `BMO_ELE_TAG` of `select_linked_delimit_begin`).
pub(crate) struct LoopShellWire<'m> {
    mask_edge: &'m dyn Fn(EdgeId) -> bool,
    visit: FxHashSet<LoopId>,
    visit_alt_e: FxHashSet<EdgeId>,
    visit_alt_v: FxHashSet<VertId>,
    st: VecDeque<ShellItem>,
}

impl<'m> LoopShellWire<'m> {
    pub(crate) fn new(mask_edge: &'m dyn Fn(EdgeId) -> bool) -> Self {
        LoopShellWire {
            mask_edge,
            visit: FxHashSet::default(),
            visit_alt_e: FxHashSet::default(),
            visit_alt_v: FxHashSet::default(),
            st: VecDeque::new(),
        }
    }

    fn edge_ok(&self, bm: &BMesh, e: EdgeId) -> bool {
        !hidden_e(bm, e) && (self.mask_edge)(e)
    }

    /// `bmw_edge_is_wire` with `BMW_FLAG_TEST_HIDDEN`: wire, or every face hidden.
    fn edge_is_wire(bm: &BMesh, e: EdgeId) -> bool {
        bm.edge_is_wire(e) || bm.edge_faces(e).into_iter().all(|f| hidden_f(bm, f))
    }

    /// `bmw_LoopShellWalker_visitLoop`.
    fn visit_loop(&mut self, bm: &BMesh, l: LoopId) {
        if self.visit.contains(&l) || hidden_f(bm, bm.lp(l).f) {
            return;
        }
        self.st.push_back(ShellItem::Loop(l));
        self.visit.insert(l);
    }

    /// `bmw_LoopShellWalker_visitEdgeWire`.
    fn visit_edge_wire(&mut self, bm: &BMesh, e: EdgeId) {
        if self.visit_alt_e.contains(&e) || !self.edge_ok(bm, e) {
            return;
        }
        self.st.push_back(ShellItem::Edge(e));
        self.visit_alt_e.insert(e);
    }

    /// `bmw_LoopShellWireWalker_visitVert`.
    fn visit_vert(&mut self, bm: &BMesh, v: VertId, e_from: Option<EdgeId>) {
        if self.visit_alt_v.contains(&v) || hidden_v(bm, v) {
            return;
        }
        for e in bm.vert_edges(v) {
            if Self::edge_is_wire(bm, e) && Some(e) != e_from {
                self.visit_edge_wire(bm, e);
                // Check if we step onto a non-wire vertex.
                let other = bm.edge_other_vert(e, v);
                for l in bm.vert_loops(other) {
                    self.visit_loop(bm, l);
                }
            }
        }
        self.visit_alt_v.insert(v);
    }

    /// `bmw_LoopShellWalker_step_impl`.
    fn loop_step(&mut self, bm: &BMesh, l: LoopId) {
        let ll = bm.lp(l).clone();
        self.visit_loop(bm, ll.next);
        self.visit_loop(bm, ll.prev);
        for e in [ll.e, bm.lp(ll.prev).e] {
            if !self.edge_ok(bm, e) {
                continue;
            }
            let Some(l_first) = bm.edge(e).l else { continue };
            let mut it = l_first;
            loop {
                let r = if bm.lp(it).v == ll.v {
                    it
                } else {
                    bm.lp(it).next
                };
                if r != l {
                    self.visit_loop(bm, r);
                }
                it = bm.lp(it).radial_next;
                if it == l_first {
                    break;
                }
            }
        }
    }

    /// Begin (`bmw_LoopShellWalker_begin` + `bmw_LoopShellWireWalker_begin`) from a vertex or
    /// an edge, then every step; returns the yielded items in order.
    pub(crate) fn walk(&mut self, bm: &BMesh, from: Start) -> Vec<ShellItem> {
        match from {
            Start::Vert(v) => {
                for l in bm.vert_loops(v) {
                    self.visit_loop(bm, l);
                }
                if bm.vert(v).e.is_some() {
                    self.visit_vert(bm, v, None);
                }
            }
            Start::Edge(e) => {
                for l in bm.edge_loops(e) {
                    self.visit_loop(bm, l);
                }
                if self.edge_ok(bm, e) {
                    let [a, b] = bm.edge(e).v;
                    self.visit_vert(bm, a, None);
                    self.visit_vert(bm, b, None);
                } else if let Some(l_first) = bm.edge(e).l {
                    let mut it = l_first;
                    loop {
                        self.visit_loop(bm, it);
                        self.visit_loop(bm, bm.lp(it).next);
                        it = bm.lp(it).radial_next;
                        if it == l_first {
                            break;
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        while let Some(item) = self.st.pop_front() {
            match item {
                ShellItem::Loop(l) => {
                    self.loop_step(bm, l);
                    let v = bm.lp(l).v;
                    self.visit_vert(bm, v, None);
                }
                ShellItem::Edge(e) => {
                    let [a, b] = bm.edge(e).v;
                    self.visit_vert(bm, a, Some(e));
                    self.visit_vert(bm, b, Some(e));
                }
            }
            out.push(item);
        }
        out
    }
}

// ---------------------------------------------------------------------------------- island

/// `BMW_ISLAND` kept across several starts, with an edge mask.
pub(crate) struct Island<'m> {
    mask_edge: &'m dyn Fn(EdgeId) -> bool,
    visit: FxHashSet<FaceId>,
}

impl<'m> Island<'m> {
    pub(crate) fn new(mask_edge: &'m dyn Fn(EdgeId) -> bool) -> Self {
        Island {
            mask_edge,
            visit: FxHashSet::default(),
        }
    }

    /// Walks from face `f` (`bmw_IslandWalker_begin` does not test the visit set, so a visited
    /// start is yielded again).
    pub(crate) fn walk(&mut self, bm: &BMesh, f: FaceId) -> Vec<FaceId> {
        if hidden_f(bm, f) {
            return Vec::new();
        }
        let mut st: VecDeque<FaceId> = VecDeque::new();
        st.push_back(f);
        self.visit.insert(f);
        let mut out = Vec::new();
        while let Some(cur) = st.pop_front() {
            for l in bm.face_loops(cur) {
                let le = bm.lp(l).e;
                if hidden_e(bm, le) || !(self.mask_edge)(le) {
                    continue;
                }
                let mut r = bm.lp(l).radial_next;
                while r != l {
                    let rf = bm.lp(r).f;
                    r = bm.lp(r).radial_next;
                    if hidden_f(bm, rf) || rf == cur || self.visit.contains(&rf) {
                        continue;
                    }
                    st.push_back(rf);
                    self.visit.insert(rf);
                    break;
                }
            }
            out.push(cur);
        }
        out
    }
}
