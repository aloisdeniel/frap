// SPDX-License-Identifier: GPL-3.0-or-later
//! Walkers (`bmesh/intern/bmesh_walkers.cc`, `bmesh_walkers_impl.cc`; ARCHITECTURE §6.2): edge
//! loop, edge ring, face island and connected shell, visiting elements in Blender's order.
//! CONTRACT frozen by M2 W0, implemented by WP 2.1.
//!
//! Ported from Blender 5.2.4. The walker framework is emulated exactly: a state list where
//! depth-first walkers push at the head and breadth-first ones at the tail, `BMW_begin` returns
//! the first `step`, `BMW_step` steps until a step yields an element, and the begin functions
//! of the loop and ring walkers "rewind" to one end before walking forward. The contract
//! wrappers use `BMW_FLAG_TEST_HIDDEN` (as every selection operator does) and no delimiters;
//! the `_ex` variants take Blender's walker flags.

use std::collections::VecDeque;

use bitflags::bitflags;
use rustc_hash::FxHashSet;

use crate::types::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Which elements a walker may cross (`BMW_MASK_NOP` and the delimit flags of
/// `mesh.select_linked`: `SEAM`, `SHARP`, `MATERIAL`, `NORMAL`, `UV`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct WalkDelimit {
    /// Stop at seams.
    pub seam: bool,
    /// Stop at sharp edges.
    pub sharp: bool,
    /// Stop between faces of different materials.
    pub material: bool,
    /// Stop between faces whose normals differ.
    pub normal: bool,
    /// Stop at UV boundaries.
    pub uv: bool,
}

bitflags! {
    /// `BMWDelimitFlag` (edge loop and edge ring walkers).
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct LoopDelimit: u8 {
        /// `BMW_DELIMIT_EDGE_LOOP_INNER_CORNERS` (`INNER_CORNERS`).
        const INNER_CORNERS = 1;
        /// `BMW_DELIMIT_EDGE_LOOP_OUTER_CORNERS` (`OUTER_CORNERS`).
        const OUTER_CORNERS = 2;
        /// `BMW_DELIMIT_EDGE_LOOP_NGONS` (`NGONS` of `delimit_edge_loop`).
        const LOOP_NGONS = 4;
        /// `BMW_DELIMIT_EDGE_RING_NGONS` (`NGONS` of `delimit_edge_ring`).
        const RING_NGONS = 8;
        /// `BMW_DELIMIT_EDGE_MARK_SEAM` (`SEAM`).
        const SEAM = 16;
        /// `BMW_DELIMIT_EDGE_MARK_SHARP` (`SHARP`).
        const SHARP = 32;
        /// `BMW_DELIMIT_FACE_MARK_MATERIAL` (`MATERIAL`).
        const MATERIAL = 64;
    }
}

/// An edge mask of a walker (`mask_edge`: only edges for which it returns true are crossed).
pub type EdgeMask<'a> = &'a dyn Fn(&BMesh, EdgeId) -> bool;
/// A face mask of a walker (`mask_face`).
pub type FaceMask<'a> = &'a dyn Fn(&BMesh, FaceId) -> bool;

/// Walker options: `BMW_FLAG_TEST_HIDDEN`, the delimit flags and an optional edge mask
/// (`mask_edge`: only edges for which it returns true are walked onto).
#[derive(Copy, Clone)]
pub struct WalkOpts<'a> {
    /// `BMW_FLAG_TEST_HIDDEN`.
    pub test_hidden: bool,
    /// Delimit flags.
    pub delimit: LoopDelimit,
    /// `mask_edge`.
    pub mask_edge: Option<EdgeMask<'a>>,
    /// `mask_face`.
    pub mask_face: Option<FaceMask<'a>>,
}

impl Default for WalkOpts<'_> {
    fn default() -> Self {
        WalkOpts {
            test_hidden: true,
            delimit: LoopDelimit::empty(),
            mask_edge: None,
            mask_face: None,
        }
    }
}

impl std::fmt::Debug for WalkOpts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalkOpts")
            .field("test_hidden", &self.test_hidden)
            .field("delimit", &self.delimit)
            .finish_non_exhaustive()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Order {
    Depth,
    Breadth,
}

/// The `BMWalker` state list.
struct States<S> {
    q: VecDeque<S>,
    order: Order,
}

impl<S: Clone> States<S> {
    fn new(order: Order) -> Self {
        States {
            q: VecDeque::new(),
            order,
        }
    }
    fn add(&mut self, s: S) {
        match self.order {
            Order::Depth => self.q.push_front(s),
            Order::Breadth => self.q.push_back(s),
        }
    }
    fn current(&self) -> Option<&S> {
        self.q.front()
    }
    fn remove(&mut self) -> Option<S> {
        self.q.pop_front()
    }
}

impl WalkOpts<'_> {
    fn edge_ok(&self, bm: &BMesh, e: EdgeId) -> bool {
        if self.test_hidden && bm.edge(e).flag.contains(ElemFlag::HIDDEN) {
            return false;
        }
        self.mask_edge.is_none_or(|m| m(bm, e))
    }
    fn face_ok(&self, bm: &BMesh, f: FaceId) -> bool {
        if self.test_hidden && bm.face(f).flag.contains(ElemFlag::HIDDEN) {
            return false;
        }
        self.mask_face.is_none_or(|m| m(bm, f))
    }
}

#[derive(Clone)]
struct LoopState {
    cur: EdgeId,
    lastv: VertId,
    is_boundary: bool,
    is_single: bool,
    f_hub: Option<FaceId>,
}

#[derive(Clone)]
struct RingState {
    l: Option<LoopId>,
    wireedge: Option<EdgeId>,
    no_calc: bool,
}

impl BMesh {
    /// `bm_edge_is_single`: a boundary edge of an n-gon (5+) next to another boundary edge.
    fn edge_is_single(&self, e: EdgeId) -> bool {
        if !self.edge_is_boundary(e) {
            return false;
        }
        let l = self.edge(e).l.expect("boundary edge has a loop");
        let ll = self.lp(l);
        self.face(ll.f).len > 4
            && (self.edge_is_boundary(self.lp(ll.next).e)
                || self.edge_is_boundary(self.lp(ll.prev).e))
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_EdgeLoopWalker_delimit_by_mark).
    fn loop_delimit_by_mark(
        &self,
        o: &WalkOpts<'_>,
        v: VertId,
        e: EdgeId,
        l: LoopId,
        mark: &dyn Fn(&BMesh, EdgeId) -> bool,
    ) -> bool {
        let hidden = |x: EdgeId| o.test_hidden && self.edge(x).flag.contains(ElemFlag::HIDDEN);
        if mark(self, e) {
            let le = self.lp(l).e;
            if !mark(self, le) && !hidden(le) && !self.edge_is_wire(le) {
                return true;
            }
        } else {
            for eo in self.vert_edges(v) {
                if mark(self, eo) && !hidden(eo) && !self.edge_is_wire(eo) {
                    return true;
                }
            }
        }
        false
    }

    fn loop_delimit_mark_check(&self, o: &WalkOpts<'_>, v: VertId, e: EdgeId, l: LoopId) -> bool {
        if o.delimit.contains(LoopDelimit::SEAM)
            && self.loop_delimit_by_mark(o, v, e, l, &|bm, x| {
                bm.edge(x).flag.contains(ElemFlag::SEAM)
            })
        {
            return true;
        }
        if o.delimit.contains(LoopDelimit::SHARP)
            && self.loop_delimit_by_mark(o, v, e, l, &|bm, x| {
                !bm.edge(x).flag.contains(ElemFlag::SMOOTH)
            })
        {
            return true;
        }
        false
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_EdgeLoopWalker_step).
    fn edge_loop_step(
        &self,
        o: &WalkOpts<'_>,
        st: &mut States<LoopState>,
        visit: &mut FxHashSet<EdgeId>,
    ) -> Option<EdgeId> {
        let owalk = st.remove()?;
        let e = owalk.cur;
        let mut l = self.edge(e).l;
        let push = |st: &mut States<LoopState>, cur: EdgeId, lastv: VertId| {
            st.add(LoopState {
                cur,
                lastv,
                is_boundary: owalk.is_boundary,
                is_single: owalk.is_single,
                f_hub: owalk.f_hub,
            });
        };
        if let Some(f_hub) = owalk.f_hub {
            // Interior n-gon edge.
            let v = self.edge_other_vert(e, owalk.lastv);
            if self.vert_edge_count_nonwire(v) == 3
                && let Some(lo) = self.face_other_vert_loop(f_hub, owalk.lastv, v)
                && let Some(nexte) = self.edge_exists(v, self.lp(lo).v)
                && o.edge_ok(self, nexte)
                && !visit.contains(&nexte)
                && !self.edge_is_boundary(nexte)
            {
                push(st, nexte, v);
                visit.insert(nexte);
            }
        } else if l.is_none() {
            // Wire edge: mark all connected wire edges.
            for v in self.edge(e).v {
                for nexte in self.vert_edges(v) {
                    if self.edge(nexte).l.is_none()
                        && o.edge_ok(self, nexte)
                        && !visit.contains(&nexte)
                    {
                        push(st, nexte, v);
                        visit.insert(nexte);
                    }
                }
            }
        } else if !owalk.is_boundary {
            // Normal edge with faces.
            let v = self.edge_other_vert(e, owalk.lastv);
            let vert_edge_tot = self.vert_edge_count_nonwire(v);
            if vert_edge_tot == 4 || vert_edge_tot == 2 {
                let i_opposite = vert_edge_tot / 2;
                let mut i = 0;
                loop {
                    let lo = self.loop_other_edge_loop(l.expect("loop"), v);
                    if self.edge_is_manifold(self.lp(lo).e) {
                        l = Some(self.lp(lo).radial_next);
                    } else {
                        l = None;
                        break;
                    }
                    i += 1;
                    if i == i_opposite {
                        break;
                    }
                }
            } else {
                l = None;
            }
            if let Some(ll) = l
                && self.loop_delimit_mark_check(o, v, e, ll)
            {
                l = None;
            }
            if let Some(ll) = l {
                let le = self.lp(ll).e;
                if Some(ll) != self.edge(e).l && o.edge_ok(self, le) && !visit.contains(&le) {
                    push(st, le, v);
                    visit.insert(le);
                }
            }
        } else {
            // Boundary edge with faces.
            let v = self.edge_other_vert(e, owalk.lastv);
            let vert_edge_tot = self.vert_edge_count_nonwire(v);
            let mut has_corner_delimit = false;
            if o.delimit.contains(LoopDelimit::INNER_CORNERS) && vert_edge_tot > 3 {
                has_corner_delimit = true;
            }
            if o.delimit.contains(LoopDelimit::OUTER_CORNERS)
                && !has_corner_delimit
                && vert_edge_tot == 2
                && !self.edge_is_single(e)
            {
                has_corner_delimit = true;
            }
            if !has_corner_delimit {
                loop {
                    let lo = self.loop_other_edge_loop(l.expect("loop"), v);
                    let le = self.lp(lo).e;
                    if self.edge_is_manifold(le) {
                        l = Some(self.lp(lo).radial_next);
                    } else if self.edge_is_boundary(le) {
                        l = Some(lo);
                        break;
                    } else {
                        l = None;
                        break;
                    }
                }
            }
            if let Some(ll) = l
                && self.loop_delimit_mark_check(o, v, e, ll)
            {
                l = None;
            }
            if let Some(ll) = l
                && o.delimit.contains(LoopDelimit::LOOP_NGONS)
                && owalk.is_single != self.edge_is_single(self.lp(ll).e)
            {
                l = None;
            }
            if let Some(ll) = l {
                let le = self.lp(ll).e;
                if Some(ll) != self.edge(e).l && o.edge_ok(self, le) && !visit.contains(&le) {
                    push(st, le, v);
                    visit.insert(le);
                }
            }
        }
        Some(owalk.cur)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_EdgeLoopWalker_begin, BMW_begin/BMW_step).
    /// `BMW_EDGELOOP` from `e` with explicit walker options.
    pub fn walk_edge_loop_ex(&self, e: EdgeId, o: &WalkOpts<'_>) -> Vec<EdgeId> {
        let mut st = States::new(Order::Depth);
        let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
        let [v1, v2] = self.edge(e).v;
        let is_boundary = self.edge_is_boundary(e);
        let mut f_hub = None;
        if !is_boundary
            && ((self.vert_edge_count_nonwire(v1) == 3 && self.vert_face_count(v1) == 3)
                || (self.vert_edge_count_nonwire(v2) == 3 && self.vert_face_count(v2) == 3))
        {
            let mut f_best: Option<FaceId> = None;
            for f in self.edge_faces(e) {
                if f_best.is_none_or(|b| self.face(b).len < self.face(f).len) {
                    f_best = Some(f);
                }
            }
            f_hub = f_best.filter(|&b| self.face(b).len > 4);
        }
        st.add(LoopState {
            cur: e,
            lastv: v1,
            is_boundary,
            is_single: is_boundary && self.edge_is_single(e),
            f_hub,
        });
        visit.insert(e);
        // Rewind.
        let mut owalk = None;
        while let Some(cur) = st.current() {
            owalk = Some(cur.clone());
            self.walk_step_until(&mut st, |bm, st| bm.edge_loop_step(o, st, &mut visit));
        }
        let mut owalk = owalk.expect("begin pushed a state");
        owalk.lastv = self.edge_other_vert(owalk.cur, owalk.lastv);
        let cur = owalk.cur;
        st.add(owalk);
        visit.clear();
        visit.insert(cur);
        let mut out = Vec::new();
        if st.current().is_some()
            && let Some(x) = self.edge_loop_step(o, &mut st, &mut visit)
        {
            out.push(x);
            while let Some(x) =
                self.walk_step_until(&mut st, |bm, st| bm.edge_loop_step(o, st, &mut visit))
            {
                out.push(x);
            }
        }
        out
    }

    /// `BMW_walk`: steps until a step yields an element or no state is left.
    fn walk_step_until<S, T>(
        &self,
        st: &mut States<S>,
        mut step: impl FnMut(&BMesh, &mut States<S>) -> Option<T>,
    ) -> Option<T>
    where
        S: Clone,
    {
        while st.current().is_some() {
            if let Some(x) = step(self, st) {
                return Some(x);
            }
        }
        None
    }

    /// `BMW_EDGELOOP` from `e` (hidden edges are not walked onto; no delimiters).
    pub fn walk_edge_loop(&self, e: EdgeId) -> Vec<EdgeId> {
        self.walk_edge_loop_ex(e, &WalkOpts::default())
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_EdgeringWalker_delimit_check).
    fn ring_delimit_check(&self, e: EdgeId, d: LoopDelimit) -> bool {
        if !self.edge_is_manifold(e) {
            return false;
        }
        let ef = self.edge(e).flag;
        if d.contains(LoopDelimit::SEAM) && ef.contains(ElemFlag::SEAM) {
            return true;
        }
        if d.contains(LoopDelimit::SHARP) && !ef.contains(ElemFlag::SMOOTH) {
            return true;
        }
        if d.contains(LoopDelimit::MATERIAL) {
            let l = self.edge(e).l.expect("manifold");
            if self.face(self.lp(l).f).mat_nr != self.face(self.lp(self.lp(l).radial_next).f).mat_nr
            {
                return true;
            }
        }
        false
    }

    fn ring_edge_check(&self, o: &WalkOpts<'_>, e: EdgeId) -> bool {
        o.edge_ok(self, e) && (self.edge_is_boundary(e) || self.edge_is_manifold(e))
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_EdgeringWalker_step).
    fn edge_ring_step(
        &self,
        o: &WalkOpts<'_>,
        st: &mut States<RingState>,
        visit: &mut FxHashSet<EdgeId>,
    ) -> Option<EdgeId> {
        let owalk = st.remove()?;
        let Some(ol) = owalk.l else {
            return owalk.wireedge;
        };
        let e = self.lp(ol).e;
        if !self.ring_edge_check(o, e) || owalk.no_calc {
            return Some(e);
        }
        let delimit_ngon = o.delimit.contains(LoopDelimit::RING_NGONS);
        let step_ok;
        let mut l;
        if delimit_ngon {
            l = self.lp(ol).radial_next;
            l = self.lp(self.lp(l).next).next;
            let lf = self.lp(l).f;
            if self.face(lf).len != 4
                || !self.ring_edge_check(o, self.lp(l).e)
                || !o.face_ok(self, lf)
            {
                l = self.lp(self.lp(ol).next).next;
            }
            step_ok = self.face(self.lp(l).f).len == 4
                && self.ring_edge_check(o, self.lp(l).e)
                && !visit.contains(&self.lp(l).e);
        } else {
            l = self.lp(ol).radial_next;
            let len = self.face(self.lp(l).f).len as i64;
            let mut i = len;
            while i > 0 {
                l = self.lp(l).next;
                i -= 2;
            }
            if len <= 0
                || len % 2 != 0
                || !self.ring_edge_check(o, self.lp(l).e)
                || !o.face_ok(self, self.lp(l).f)
            {
                l = ol;
                let mut i = len;
                while i > 0 {
                    l = self.lp(l).next;
                    i -= 2;
                }
            }
            step_ok = self.face(self.lp(l).f).len.is_multiple_of(2)
                && self.ring_edge_check(o, self.lp(l).e)
                && !visit.contains(&self.lp(l).e);
        }
        if step_ok {
            let le = self.lp(l).e;
            st.add(RingState {
                l: Some(l),
                wireedge: None,
                no_calc: self.ring_delimit_check(le, o.delimit),
            });
            visit.insert(le);
        }
        Some(e)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_EdgeringWalker_begin, BMW_begin/BMW_step).
    /// `BMW_EDGERING` from `e` with explicit walker options.
    pub fn walk_edge_ring_ex(&self, e: EdgeId, o: &WalkOpts<'_>) -> Vec<EdgeId> {
        let mut st = States::new(Order::Depth);
        let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
        let el = self.edge(e).l;
        let run = |bm: &BMesh, st: &mut States<RingState>, visit: &mut FxHashSet<EdgeId>| {
            let mut out = Vec::new();
            if st.current().is_some()
                && let Some(x) = bm.edge_ring_step(o, st, visit)
            {
                out.push(x);
                while let Some(x) = bm.walk_step_until(st, |bm, st| bm.edge_ring_step(o, st, visit))
                {
                    out.push(x);
                }
            }
            out
        };
        let Some(el) = el else {
            st.add(RingState {
                l: None,
                wireedge: Some(e),
                no_calc: false,
            });
            return run(self, &mut st, &mut visit);
        };
        if self.ring_delimit_check(e, o.delimit) {
            st.add(RingState {
                l: None,
                wireedge: Some(e),
                no_calc: false,
            });
            return run(self, &mut st, &mut visit);
        }
        st.add(RingState {
            l: Some(el),
            wireedge: None,
            no_calc: false,
        });
        visit.insert(self.lp(el).e);
        // Rewind.
        let mut owalk = None;
        while let Some(cur) = st.current() {
            owalk = Some(cur.clone());
            self.walk_step_until(&mut st, |bm, st| bm.edge_ring_step(o, st, &mut visit));
        }
        let mut lwalk = owalk.expect("begin pushed a state");
        let delimit_ngon = o.delimit.contains(LoopDelimit::RING_NGONS);
        let odd = |bm: &BMesh, l: LoopId| {
            let n = bm.face(bm.lp(l).f).len;
            if delimit_ngon {
                n != 4
            } else {
                !n.is_multiple_of(2)
            }
        };
        let mut ll = lwalk.l.expect("ring state has a loop");
        if odd(self, ll) {
            ll = self.lp(ll).radial_next;
            lwalk.l = Some(ll);
        }
        visit.clear();
        visit.insert(self.lp(ll).e);
        let no_calc = lwalk.no_calc;
        st.add(lwalk);
        if self.lp(ll).radial_next != ll && (no_calc || odd(self, ll)) {
            st.add(RingState {
                l: Some(self.lp(ll).radial_next),
                wireedge: None,
                no_calc: false,
            });
        }
        run(self, &mut st, &mut visit)
    }

    /// `BMW_EDGERING` from `e` (hidden edges are not walked onto; no delimiters).
    pub fn walk_edge_ring(&self, e: EdgeId) -> Vec<EdgeId> {
        self.walk_edge_ring_ex(e, &WalkOpts::default())
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_select.cc
    // (select_linked_delimit_test).
    /// Whether `e` delimits a linked selection (`select_linked_delimit_test`); `UV` compares
    /// the active UV map on both sides.
    pub fn edge_delimits(&self, e: EdgeId, d: WalkDelimit) -> bool {
        let ef = self.edge(e).flag;
        if d.seam && ef.contains(ElemFlag::SEAM) {
            return true;
        }
        if d.sharp && !ef.contains(ElemFlag::SMOOTH) {
            return true;
        }
        if d.normal && !self.edge_is_contiguous(e) {
            return true;
        }
        if d.material
            && let Some(l) = self.edge(e).l
            && self.lp(l).radial_next != l
        {
            let mat = self.face(self.lp(l).f).mat_nr;
            let mut it = self.lp(l).radial_next;
            while it != l {
                if self.face(self.lp(it).f).mat_nr != mat {
                    return true;
                }
                it = self.lp(it).radial_next;
            }
        }
        if d.uv && !self.edge_is_contiguous_uv(e) {
            return true;
        }
        false
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_query.cc
    // (BM_edge_is_contiguous_loop_cd on the active FLOAT2 layer).
    fn edge_is_contiguous_uv(&self, e: EdgeId) -> bool {
        let names = self.ldata.float2_names();
        let active = self
            .ldata
            .uv_active
            .as_ref()
            .filter(|n| names.contains(n))
            .or(names.first());
        let Some(layer) = active.and_then(|n| self.ldata.find(n)) else {
            // No UV layer: every edge is contiguous for this test.
            return true;
        };
        let Some(l) = self.edge(e).l else {
            return true;
        };
        if self.lp(l).radial_next == l {
            return true;
        }
        let uv = |x: LoopId| self.ldata.float2(layer, x.0).unwrap_or([0.0; 2]);
        // `layerEqual_propfloat2`.
        let eq = |a: [f32; 2], b: [f32; 2]| {
            let d = [a[0] - b[0], a[1] - b[1]];
            d[0] * d[0] + d[1] * d[1] < 0.00001
        };
        let base_v1 = l;
        let base_v2 = self.lp(l).next;
        let mut it = self.lp(l).radial_next;
        loop {
            let li = self.lp(it);
            let (i1, i2) = if li.v == self.lp(base_v1).v {
                (it, li.next)
            } else {
                (li.next, it)
            };
            if !eq(uv(base_v1), uv(i1)) || !eq(uv(base_v2), uv(i2)) {
                return false;
            }
            it = li.radial_next;
            if it == l {
                break;
            }
        }
        true
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_IslandWalker_begin/step_ex).
    /// `BMW_ISLAND` (faces connected over non-delimited edges) from `f`, with the selection
    /// operators' `BMW_FLAG_TEST_HIDDEN`.
    pub fn walk_face_island(&self, f: FaceId, delimit: WalkDelimit) -> Vec<FaceId> {
        let any = delimit != WalkDelimit::default();
        let mask = move |bm: &BMesh, e: EdgeId| !any || !bm.edge_delimits(e, delimit);
        let o = WalkOpts {
            mask_edge: Some(&mask),
            ..WalkOpts::default()
        };
        self.walk_face_island_ex(f, &o, false)
    }

    /// `BMW_ISLAND` / `BMW_ISLAND_MANIFOLD` (`only_manifold`) with explicit options.
    pub fn walk_face_island_ex(
        &self,
        f: FaceId,
        o: &WalkOpts<'_>,
        only_manifold: bool,
    ) -> Vec<FaceId> {
        let mut st: States<FaceId> = States::new(Order::Breadth);
        let mut visit: FxHashSet<FaceId> = FxHashSet::default();
        if !o.face_ok(self, f) {
            return Vec::new();
        }
        st.add(f);
        visit.insert(f);
        let mut out = Vec::new();
        while let Some(cur) = st.remove() {
            for l in self.face_loops(cur) {
                let le = self.lp(l).e;
                if !o.edge_ok(self, le) {
                    continue;
                }
                if only_manifold && self.lp(l).radial_next != l {
                    let mut face_count = 1;
                    let mut r = self.lp(l).radial_next;
                    loop {
                        if o.face_ok(self, self.lp(r).f) {
                            face_count += 1;
                            if face_count == 3 {
                                break;
                            }
                        }
                        r = self.lp(r).radial_next;
                        if r == l {
                            break;
                        }
                    }
                    if face_count != 2 {
                        continue;
                    }
                }
                let mut r = self.lp(l).radial_next;
                while r != l {
                    let rf = self.lp(r).f;
                    if o.face_ok(self, rf) && rf != cur && !visit.contains(&rf) {
                        st.add(rf);
                        visit.insert(rf);
                        break;
                    }
                    r = self.lp(r).radial_next;
                }
            }
            out.push(cur);
        }
        out
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc
    // (bmw_VertShellWalker_begin/step).
    /// `BMW_VERT_SHELL` from `v` with explicit options: the edges in walk order.
    pub fn walk_vert_shell_edges(&self, v: VertId, o: &WalkOpts<'_>) -> Vec<EdgeId> {
        let mut st: States<EdgeId> = States::new(Order::Breadth);
        let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
        let mut visit_edge = |st: &mut States<EdgeId>, e: EdgeId| {
            if visit.contains(&e) || !o.edge_ok(self, e) {
                return;
            }
            st.add(e);
            visit.insert(e);
        };
        for e in self.vert_edges(v) {
            visit_edge(&mut st, e);
        }
        let mut out = Vec::new();
        while let Some(e) = st.remove() {
            for x in self.edge(e).v {
                for e2 in self.vert_edges(x) {
                    visit_edge(&mut st, e2);
                }
            }
            out.push(e);
        }
        out
    }

    /// `BMW_VERT_SHELL` (vertices connected by edges) from `v`: `v`, then the vertices of the
    /// walked edges in walk order (hidden edges are not crossed).
    pub fn walk_vert_shell(&self, v: VertId) -> Vec<VertId> {
        let mut seen: FxHashSet<VertId> = FxHashSet::default();
        let mut out = vec![v];
        seen.insert(v);
        for e in self.walk_vert_shell_edges(v, &WalkOpts::default()) {
            for x in self.edge(e).v {
                if seen.insert(x) {
                    out.push(x);
                }
            }
        }
        out
    }
}
