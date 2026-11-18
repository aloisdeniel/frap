// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_scanfill` (`blenlib/intern/scanfill.cc`, Blender 5.2.4): sweep-line triangulation of
//! polygons with holes, used by `bmesh.ops.triangle_fill`.
//!
//! Blender's intrusive `ListBase`s become index links in arenas (`verts`, `edges`); every list
//! operation (`BLI_addtail`, `BLI_remlink`, `BLI_insertlinkbefore`, `BLI_movelisttolist`)
//! keeps Blender's order, so the triangles come out in the same order with the same corners.

use crate::ops::contextual_create::bli::{self, V3};

/// `BLI_SCANFILL_CALC_REMOVE_DOUBLES`.
pub const CALC_REMOVE_DOUBLES: u32 = 1 << 1;
/// `BLI_SCANFILL_CALC_HOLES`.
pub const CALC_HOLES: u32 = 1 << 2;
/// `BLI_SCANFILL_CALC_POLYS`.
pub const CALC_POLYS: u32 = 1 << 3;
/// `BLI_SCANFILL_CALC_LOOSE`.
pub const CALC_LOOSE: u32 = 1 << 4;

const SF_EPSILON: f32 = 0.00003;
const SF_EPSILON_SQ: f32 = SF_EPSILON * SF_EPSILON;
const SF_VERT_NEW: u8 = 0;
const SF_VERT_AVAILABLE: u8 = 1;
const SF_VERT_ZERO_LEN: u8 = 2;
const SF_EDGE_NEW: u8 = 0;
const SF_EDGE_INTERNAL: u8 = 2;
const SF_POLY_NEW: u8 = 0;
const SF_POLY_VALID: u8 = 1;
const SF_POLY_UNSET: u16 = u16::MAX;

type Ix = usize;

/// `ScanFillVert`.
#[derive(Clone, Debug)]
pub struct SfVert {
    /// Position.
    pub co: V3,
    /// Projected position.
    pub xy: [f32; 2],
    /// Caller payload (`tmp.p`): an index chosen by the caller.
    pub user: usize,
    /// `tmp.v` (zero-length merges).
    tmp_v: Option<Ix>,
    poly_nr: u16,
    edge_count: u32,
    f: u8,
    prev: Option<Ix>,
    next: Option<Ix>,
}

#[derive(Clone, Debug)]
struct SfEdge {
    v1: Ix,
    v2: Ix,
    poly_nr: u16,
    f: u8,
    prev: Option<Ix>,
    next: Option<Ix>,
}

#[derive(Copy, Clone, Debug, Default)]
struct Lb {
    first: Option<Ix>,
    last: Option<Ix>,
}

trait Linked {
    fn prev(&self) -> Option<Ix>;
    fn next(&self) -> Option<Ix>;
    fn set_prev(&mut self, p: Option<Ix>);
    fn set_next(&mut self, n: Option<Ix>);
}
macro_rules! linked {
    ($t:ty) => {
        impl Linked for $t {
            fn prev(&self) -> Option<Ix> {
                self.prev
            }
            fn next(&self) -> Option<Ix> {
                self.next
            }
            fn set_prev(&mut self, p: Option<Ix>) {
                self.prev = p;
            }
            fn set_next(&mut self, n: Option<Ix>) {
                self.next = n;
            }
        }
    };
}
linked!(SfVert);
linked!(SfEdge);

fn addtail<T: Linked>(a: &mut [T], lb: &mut Lb, i: Ix) {
    a[i].set_next(None);
    a[i].set_prev(lb.last);
    if let Some(l) = lb.last {
        a[l].set_next(Some(i));
    }
    if lb.first.is_none() {
        lb.first = Some(i);
    }
    lb.last = Some(i);
}

fn remlink<T: Linked>(a: &mut [T], lb: &mut Lb, i: Ix) {
    let (p, n) = (a[i].prev(), a[i].next());
    if let Some(n) = n {
        a[n].set_prev(p);
    }
    if let Some(p) = p {
        a[p].set_next(n);
    }
    if lb.last == Some(i) {
        lb.last = p;
    }
    if lb.first == Some(i) {
        lb.first = n;
    }
}

/// `BLI_insertlinkbefore(lb, nextlink, newlink)`.
fn insertbefore<T: Linked>(a: &mut [T], lb: &mut Lb, next: Option<Ix>, new: Ix) {
    let Some(nx) = next else {
        addtail(a, lb, new);
        return;
    };
    if lb.first.is_none() {
        lb.first = Some(new);
        lb.last = Some(new);
        return;
    }
    let p = a[nx].prev();
    a[new].set_next(Some(nx));
    a[new].set_prev(p);
    a[nx].set_prev(Some(new));
    if let Some(p) = p {
        a[p].set_next(Some(new));
    }
    if lb.first == Some(nx) {
        lb.first = Some(new);
    }
}

/// `BLI_movelisttolist(dst, src)`: appends `src` to `dst`, emptying `src`.
fn movelisttolist<T: Linked>(a: &mut [T], dst: &mut Lb, src: &mut Lb) {
    let Some(sf) = src.first else {
        return;
    };
    match dst.last {
        None => {
            dst.first = src.first;
            dst.last = src.last;
        }
        Some(dl) => {
            a[dl].set_next(Some(sf));
            a[sf].set_prev(Some(dl));
            dst.last = src.last;
        }
    }
    *src = Lb::default();
}

fn collect<T: Linked>(a: &[T], lb: &Lb) -> Vec<Ix> {
    let mut out = Vec::new();
    let mut it = lb.first;
    while let Some(i) = it {
        out.push(i);
        it = a[i].next();
    }
    out
}

#[derive(Clone, Debug)]
struct PolyFill {
    edges: u32,
    verts: u32,
    min_xy: [f32; 2],
    max_xy: [f32; 2],
    nr: u16,
    f: u8,
}

#[derive(Clone, Debug)]
struct ScanVert {
    vert: Ix,
    edges: Lb,
}

/// `ScanFillContext`.
#[derive(Clone, Debug, Default)]
pub struct ScanFill {
    /// Every vertex added (arena).
    pub verts: Vec<SfVert>,
    edges: Vec<SfEdge>,
    vbase: Lb,
    ebase: Lb,
    /// `fillfacebase`: triangles of vertex arena indices.
    pub faces: Vec<[Ix; 3]>,
}

fn compare_v2v2(a: [f32; 2], b: [f32; 2], limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit
}

fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit && (a[2] - b[2]).abs() <= limit
}

/// `dist_squared_to_line_v2`.
fn dist_squared_to_line_v2(l1: [f32; 2], l2: [f32; 2], p: [f32; 2]) -> f32 {
    // closest_to_line_v2
    let u = [l2[0] - l1[0], l2[1] - l1[1]];
    let h = [p[0] - l1[0], p[1] - l1[1]];
    let dot = u[0] * u[0] + u[1] * u[1];
    let lambda = if dot != 0.0 { (u[0] * h[0] + u[1] * h[1]) / dot } else { 0.0 };
    let closest = [l1[0] + u[0] * lambda, l1[1] + u[1] * lambda];
    let d = [closest[0] - p[0], closest[1] - p[1]];
    d[0] * d[0] + d[1] * d[1]
}

/// `cos_v2v2v2`.
fn cos_v2v2v2(p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> f32 {
    let n = |v: [f32; 2]| {
        let d = v[0] * v[0] + v[1] * v[1];
        if d > 1.0e-35 {
            let d = d.sqrt();
            [v[0] / d, v[1] / d]
        } else {
            [0.0, 0.0]
        }
    };
    let v1 = n([p2[0] - p1[0], p2[1] - p1[1]]);
    let v2 = n([p2[0] - p3[0], p2[1] - p3[1]]);
    v1[0] * v2[0] + v1[1] * v2[1]
}

/// `testedgeside`: `v3` is right of `v1`–`v2` (false when on an end point).
fn testedgeside(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> bool {
    let inp = (v2[0] - v1[0]) * (v1[1] - v3[1]) + (v1[1] - v2[1]) * (v1[0] - v3[0]);
    if inp < 0.0 {
        return false;
    }
    if inp == 0.0 {
        if v1[0] == v3[0] && v1[1] == v3[1] {
            return false;
        }
        if v2[0] == v3[0] && v2[1] == v3[1] {
            return false;
        }
    }
    true
}

impl ScanFill {
    /// `BLI_scanfill_begin`.
    pub fn new() -> Self {
        ScanFill::default()
    }

    /// `BLI_scanfill_vert_add`.
    pub fn vert_add(&mut self, co: V3, user: usize) -> Ix {
        let i = self.verts.len();
        self.verts.push(SfVert {
            co,
            xy: [0.0; 2],
            user,
            tmp_v: None,
            poly_nr: SF_POLY_UNSET,
            edge_count: 0,
            f: SF_VERT_NEW,
            prev: None,
            next: None,
        });
        addtail(&mut self.verts, &mut self.vbase, i);
        i
    }

    /// `BLI_scanfill_edge_add`.
    pub fn edge_add(&mut self, v1: Ix, v2: Ix) -> Ix {
        let i = self.new_edge(v1, v2);
        addtail(&mut self.edges, &mut self.ebase, i);
        i
    }

    fn new_edge(&mut self, v1: Ix, v2: Ix) -> Ix {
        let i = self.edges.len();
        self.edges.push(SfEdge {
            v1,
            v2,
            poly_nr: SF_POLY_UNSET,
            f: SF_EDGE_NEW,
            prev: None,
            next: None,
        });
        i
    }

    /// The vertices currently in `fillvertbase`, in order.
    pub fn vert_list(&self) -> Vec<Ix> {
        collect(&self.verts, &self.vbase)
    }

    fn xy(&self, v: Ix) -> [f32; 2] {
        self.verts[v].xy
    }

    /// `vergscdata` order: descending y, then ascending x.
    fn cmp_sc(&self, a: Ix, b: Ix) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        let (x1, x2) = (self.xy(a), self.xy(b));
        if x1[1] < x2[1] {
            return Greater;
        }
        if x1[1] > x2[1] {
            return Less;
        }
        if x1[0] > x2[0] {
            return Greater;
        }
        if x1[0] < x2[0] {
            return Less;
        }
        Equal
    }

    /// `boundisect`.
    fn boundisect(pf2: &PolyFill, pf1: &PolyFill) -> bool {
        if pf1.edges == 0 || pf2.edges == 0 {
            return false;
        }
        !(pf2.max_xy[0] < pf1.min_xy[0]
            || pf2.max_xy[1] < pf1.min_xy[1]
            || pf2.min_xy[0] > pf1.max_xy[0]
            || pf2.min_xy[1] > pf1.max_xy[1])
    }

    fn fill_target_map_recursive(pf_list: &[PolyFill], target: usize, test: usize, map: &mut [usize]) {
        for b in (target + 1)..pf_list.len() {
            if map[b] != b {
                continue;
            }
            if Self::boundisect(&pf_list[test], &pf_list[b]) {
                map[b] = target;
                Self::fill_target_map_recursive(pf_list, target, b, map);
            }
        }
    }

    /// `mergepolysSimp`: adds `pf2` to `pf1`.
    fn merge_polys(&mut self, pfl: &mut [PolyFill], i1: usize, i2: usize) {
        let (n1, n2) = (pfl[i1].nr, pfl[i2].nr);
        for v in collect(&self.verts, &self.vbase) {
            if self.verts[v].poly_nr == n2 {
                self.verts[v].poly_nr = n1;
            }
        }
        for e in collect(&self.edges, &self.ebase) {
            if self.edges[e].poly_nr == n2 {
                self.edges[e].poly_nr = n1;
            }
        }
        let p2 = pfl[i2].clone();
        let p1 = &mut pfl[i1];
        p1.verts += p2.verts;
        p1.edges += p2.edges;
        p1.max_xy[0] = p1.max_xy[0].max(p2.max_xy[0]);
        p1.max_xy[1] = p1.max_xy[1].max(p2.max_xy[1]);
        p1.min_xy[0] = p1.min_xy[0].min(p2.min_xy[0]);
        p1.min_xy[1] = p1.min_xy[1].min(p2.min_xy[1]);
        p1.f |= p2.f;
        pfl[i2].verts = 0;
        pfl[i2].edges = 0;
    }

    /// `addedgetoscanvert`: inserts `eed` into `sc`'s edge list, false when an edge to the
    /// same vertex exists.
    fn add_edge_to_scanvert(&mut self, sc: &mut ScanVert, eed: Ix) -> bool {
        if sc.edges.first.is_none() {
            sc.edges = Lb {
                first: Some(eed),
                last: Some(eed),
            };
            self.edges[eed].prev = None;
            self.edges[eed].next = None;
            return true;
        }
        let e = &self.edges[eed];
        let [x, y] = self.xy(e.v1);
        let v2xy = self.xy(e.v2);
        let mut fac1 = v2xy[1] - y;
        if fac1 == 0.0 {
            fac1 = 1.0e10 * (v2xy[0] - x);
        } else {
            fac1 = (x - v2xy[0]) / fac1;
        }
        let eed_v2 = e.v2;
        let mut ed = sc.edges.first;
        while let Some(edi) = ed {
            if self.edges[edi].v2 == eed_v2 {
                return false;
            }
            let ev2 = self.xy(self.edges[edi].v2);
            let mut fac = ev2[1] - y;
            if fac == 0.0 {
                fac = 1.0e10 * (ev2[0] - x);
            } else {
                fac = (x - ev2[0]) / fac;
            }
            if fac > fac1 {
                break;
            }
            ed = self.edges[edi].next;
        }
        insertbefore(&mut self.edges, &mut sc.edges, ed, eed);
        true
    }

    /// `addedgetoscanlist`: `Some(sc index)` when the edge already exists there.
    fn add_edge_to_scanlist(&mut self, scdata: &mut [ScanVert], eed: Ix) -> Option<usize> {
        let (v1, v2) = (self.edges[eed].v1, self.edges[eed].v2);
        let (a, b) = (self.xy(v1), self.xy(v2));
        if a[1] == b[1] {
            if a[0] > b[0] {
                self.edges[eed].v1 = v2;
                self.edges[eed].v2 = v1;
            }
        } else if a[1] < b[1] {
            self.edges[eed].v1 = v2;
            self.edges[eed].v2 = v1;
        }
        let key = self.edges[eed].v1;
        // bsearch
        let mut lo = 0usize;
        let mut hi = scdata.len();
        let mut found = None;
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.cmp_sc(key, scdata[mid].vert) {
                std::cmp::Ordering::Less => hi = mid,
                std::cmp::Ordering::Greater => lo = mid + 1,
                std::cmp::Ordering::Equal => {
                    found = Some(mid);
                    break;
                }
            }
        }
        let si = found?;
        let mut sc = scdata[si].clone();
        let ok = self.add_edge_to_scanvert(&mut sc, eed);
        scdata[si] = sc;
        if !ok {
            return Some(si);
        }
        None
    }

    /// `boundinsideEV`.
    fn bound_inside_ev(&self, eed: Ix, eve: Ix) -> bool {
        let e = &self.edges[eed];
        let (a, b) = (self.xy(e.v1), self.xy(e.v2));
        let p = self.xy(eve);
        let (minx, maxx) = if a[0] < b[0] { (a[0], b[0]) } else { (b[0], a[0]) };
        if p[0] >= minx && p[0] <= maxx {
            let (miny, maxy) = if a[1] < b[1] { (a[1], b[1]) } else { (b[1], a[1]) };
            if p[1] >= miny && p[1] <= maxy {
                return true;
            }
        }
        false
    }

    /// `testvertexnearedge`.
    fn test_vertex_near_edge(&mut self) {
        for eve in collect(&self.verts, &self.vbase) {
            if self.verts[eve].edge_count != 1 {
                continue;
            }
            let mut ed1 = self.ebase.first.expect("edge of a vertex");
            while !(self.edges[ed1].v1 == eve || self.edges[ed1].v2 == eve) {
                ed1 = self.edges[ed1].next.expect("edge of a vertex");
            }
            if self.edges[ed1].v1 == eve {
                self.edges[ed1].v1 = self.edges[ed1].v2;
                self.edges[ed1].v2 = eve;
            }
            let mut it = self.ebase.first;
            while let Some(eed) = it {
                it = self.edges[eed].next;
                let (ev1, ev2) = (self.edges[eed].v1, self.edges[eed].v2);
                if eve != ev1 && eve != ev2 && self.verts[eve].poly_nr == self.edges[eed].poly_nr {
                    if compare_v2v2(self.xy(eve), self.xy(ev1), SF_EPSILON) {
                        self.edges[ed1].v2 = ev1;
                        self.verts[ev1].edge_count += 1;
                        self.verts[eve].edge_count = 0;
                        break;
                    }
                    if compare_v2v2(self.xy(eve), self.xy(ev2), SF_EPSILON) {
                        self.edges[ed1].v2 = ev2;
                        self.verts[ev2].edge_count += 1;
                        self.verts[eve].edge_count = 0;
                        break;
                    }
                    if self.bound_inside_ev(eed, eve) {
                        let dist = dist_squared_to_line_v2(self.xy(ev1), self.xy(ev2), self.xy(eve));
                        if dist < SF_EPSILON_SQ {
                            let ne = self.edge_add(ev1, eve);
                            self.edges[ne].poly_nr = self.edges[eed].poly_nr;
                            self.edges[eed].v1 = eve;
                            self.verts[eve].edge_count = 3;
                            break;
                        }
                    }
                }
            }
        }
    }

    /// `splitlist`.
    fn splitlist(&mut self, tempve: &mut Lb, temped: &mut Lb, nr: u16) {
        movelisttolist(&mut self.verts, tempve, &mut self.vbase);
        movelisttolist(&mut self.edges, temped, &mut self.ebase);
        for v in collect(&self.verts, tempve) {
            if self.verts[v].poly_nr == nr {
                remlink(&mut self.verts, tempve, v);
                addtail(&mut self.verts, &mut self.vbase, v);
            }
        }
        for e in collect(&self.edges, temped) {
            if self.edges[e].poly_nr == nr {
                remlink(&mut self.edges, temped, e);
                addtail(&mut self.edges, &mut self.ebase, e);
            }
        }
    }

    /// `scanfill`: triangulates one poly; returns the triangle count.
    fn scanfill(&mut self, pf: &PolyFill, flag: u32) -> u32 {
        let nr = pf.nr;
        // STEP 0: zero sized edges.
        if flag & CALC_REMOVE_DOUBLES != 0 {
            for e in collect(&self.edges, &self.ebase) {
                let (v1, v2) = (self.edges[e].v1, self.edges[e].v2);
                if self.xy(v1) == self.xy(v2) {
                    let (f1, f2) = (self.verts[v1].f, self.verts[v2].f);
                    if f1 == SF_VERT_ZERO_LEN && f2 != SF_VERT_ZERO_LEN {
                        self.verts[v2].f = SF_VERT_ZERO_LEN;
                        self.verts[v2].tmp_v = self.verts[v1].tmp_v;
                    } else if f2 == SF_VERT_ZERO_LEN && f1 != SF_VERT_ZERO_LEN {
                        self.verts[v1].f = SF_VERT_ZERO_LEN;
                        self.verts[v1].tmp_v = self.verts[v2].tmp_v;
                    } else if f2 == SF_VERT_ZERO_LEN && f1 == SF_VERT_ZERO_LEN {
                        self.verts[v1].tmp_v = self.verts[v2].tmp_v;
                    } else {
                        self.verts[v2].f = SF_VERT_ZERO_LEN;
                        self.verts[v2].tmp_v = Some(v1);
                    }
                }
            }
        }
        // STEP 1: sorted scan vertices.
        let mut scdata: Vec<ScanVert> = Vec::new();
        for v in collect(&self.verts, &self.vbase) {
            if self.verts[v].poly_nr == nr && self.verts[v].f != SF_VERT_ZERO_LEN {
                self.verts[v].f = SF_VERT_NEW;
                scdata.push(ScanVert {
                    vert: v,
                    edges: Lb::default(),
                });
            }
        }
        scdata.sort_by(|a, b| self.cmp_sc(a.vert, b.vert));
        let verts = scdata.len();
        for e in collect(&self.edges, &self.ebase) {
            remlink(&mut self.edges, &mut self.ebase, e);
            if flag & CALC_REMOVE_DOUBLES != 0 {
                for k in 0..2 {
                    let vv = if k == 0 { self.edges[e].v1 } else { self.edges[e].v2 };
                    if self.verts[vv].f == SF_VERT_ZERO_LEN {
                        let start = vv;
                        let mut cur = vv;
                        while self.verts[cur].f == SF_VERT_ZERO_LEN
                            && self.verts[cur].tmp_v != Some(start)
                            && Some(cur) != self.verts[cur].tmp_v
                        {
                            match self.verts[cur].tmp_v {
                                Some(t) => cur = t,
                                None => break,
                            }
                        }
                        if k == 0 {
                            self.edges[e].v1 = cur;
                        } else {
                            self.edges[e].v2 = cur;
                        }
                    }
                }
            }
            if self.edges[e].v1 != self.edges[e].v2 {
                self.add_edge_to_scanlist(&mut scdata, e);
            }
        }
        // STEP 2: fill loop.
        let twoconnected = pf.f == SF_POLY_NEW;
        let mut totface = 0u32;
        let maxface = if flag & CALC_HOLES != 0 {
            2 * verts as u32
        } else {
            (verts as u32).saturating_sub(2)
        };
        let mut a = 0usize;
        while a < verts {
            frap_math::budget::check();
            // Set connect-flags.
            let mut it = scdata[a].edges.first;
            while let Some(ed1) = it {
                it = self.edges[ed1].next;
                let (v1, v2) = (self.edges[ed1].v1, self.edges[ed1].v2);
                if self.verts[v1].edge_count == 1 || self.verts[v2].edge_count == 1 {
                    remlink(&mut self.edges, &mut scdata[a].edges, ed1);
                    addtail(&mut self.edges, &mut self.ebase, ed1);
                    if self.verts[v1].edge_count > 1 {
                        self.verts[v1].edge_count -= 1;
                    }
                    if self.verts[v2].edge_count > 1 {
                        self.verts[v2].edge_count -= 1;
                    }
                } else {
                    self.verts[v2].f = SF_VERT_AVAILABLE;
                }
            }
            let mut abort = false;
            while let Some(ed1) = scdata[a].edges.first {
                if totface >= maxface {
                    abort = true;
                    break;
                }
                let ed2 = self.edges[ed1].next;
                match ed2 {
                    None => {
                        scdata[a].edges = Lb::default();
                        addtail(&mut self.edges, &mut self.ebase, ed1);
                        let (ev1, ev2) = (self.edges[ed1].v1, self.edges[ed1].v2);
                        self.verts[ev2].f = SF_VERT_NEW;
                        self.verts[ev1].edge_count -= 1;
                        self.verts[ev2].edge_count -= 1;
                    }
                    Some(ed2) => {
                        let v1 = self.edges[ed1].v2;
                        let v2 = self.edges[ed1].v1;
                        let v3 = self.edges[ed2].v2;
                        if v1 == v2 || v2 == v3 {
                            break;
                        }
                        let miny = self.xy(v1)[1].min(self.xy(v3)[1]);
                        let mut best_sc: Option<usize> = None;
                        let mut angle_best_cos = -1.0f32;
                        let mut firsttime = false;
                        for (b, sc1) in scdata.iter().enumerate().skip(a + 1) {
                            let sv = sc1.vert;
                            if self.verts[sv].f == SF_VERT_NEW {
                                if self.xy(sv)[1] <= miny {
                                    break;
                                }
                                let p = self.xy(sv);
                                if testedgeside(self.xy(v1), self.xy(v2), p)
                                    && testedgeside(self.xy(v2), self.xy(v3), p)
                                    && testedgeside(self.xy(v3), self.xy(v1), p)
                                {
                                    match best_sc {
                                        None => best_sc = Some(b),
                                        Some(bs) => {
                                            if !firsttime {
                                                angle_best_cos = cos_v2v2v2(
                                                    self.xy(v2),
                                                    self.xy(v1),
                                                    self.xy(scdata[bs].vert),
                                                );
                                                firsttime = true;
                                            }
                                            let c = cos_v2v2v2(self.xy(v2), self.xy(v1), p);
                                            if c > angle_best_cos {
                                                best_sc = Some(b);
                                                angle_best_cos = c;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(bs) = best_sc {
                            let bv = scdata[bs].vert;
                            let ed3 = self.new_edge(v2, bv);
                            insertbefore(&mut self.edges, &mut scdata[a].edges, Some(ed2), ed3);
                            self.verts[bv].f = SF_VERT_AVAILABLE;
                            self.edges[ed3].f = SF_EDGE_INTERNAL;
                            self.verts[v2].edge_count += 1;
                            self.verts[bv].edge_count += 1;
                        } else {
                            self.faces.push([v1, v2, v3]);
                            totface += 1;
                            remlink(&mut self.edges, &mut scdata[a].edges, ed1);
                            addtail(&mut self.edges, &mut self.ebase, ed1);
                            let (e1v1, e1v2) = (self.edges[ed1].v1, self.edges[ed1].v2);
                            self.verts[e1v2].f = SF_VERT_NEW;
                            self.verts[e1v1].edge_count -= 1;
                            self.verts[e1v2].edge_count -= 1;
                            if self.edges[ed2].f == SF_EDGE_NEW && twoconnected {
                                remlink(&mut self.edges, &mut scdata[a].edges, ed2);
                                addtail(&mut self.edges, &mut self.ebase, ed2);
                                let (e2v1, e2v2) = (self.edges[ed2].v1, self.edges[ed2].v2);
                                self.verts[e2v2].f = SF_VERT_NEW;
                                self.verts[e2v1].edge_count -= 1;
                                self.verts[e2v2].edge_count -= 1;
                            }
                            let ed3 = self.new_edge(v1, v3);
                            self.edges[ed3].f = SF_EDGE_INTERNAL;
                            self.verts[v1].edge_count += 1;
                            self.verts[v3].edge_count += 1;
                            if let Some(si) = self.add_edge_to_scanlist(&mut scdata, ed3) {
                                let (n1, n3) = (self.edges[ed3].v1, self.edges[ed3].v2);
                                self.verts[n1].edge_count -= 1;
                                self.verts[n3].edge_count -= 1;
                                let mut it = scdata[si].edges.first;
                                while let Some(e3) = it {
                                    let (x1, x2) = (self.edges[e3].v1, self.edges[e3].v2);
                                    if (x1 == v1 && x2 == v3) || (x1 == v3 && x2 == v1) {
                                        if twoconnected {
                                            remlink(&mut self.edges, &mut scdata[si].edges, e3);
                                            addtail(&mut self.edges, &mut self.ebase, e3);
                                            self.verts[x1].edge_count -= 1;
                                            self.verts[x2].edge_count -= 1;
                                        }
                                        break;
                                    }
                                    it = self.edges[e3].next;
                                }
                            }
                        }
                    }
                }
                // Test for loose edges.
                let mut it = scdata[a].edges.first;
                while let Some(e) = it {
                    it = self.edges[e].next;
                    let (v1, v2) = (self.edges[e].v1, self.edges[e].v2);
                    if self.verts[v1].edge_count < 2 || self.verts[v2].edge_count < 2 {
                        remlink(&mut self.edges, &mut scdata[a].edges, e);
                        addtail(&mut self.edges, &mut self.ebase, e);
                        if self.verts[v1].edge_count > 1 {
                            self.verts[v1].edge_count -= 1;
                        }
                        if self.verts[v2].edge_count > 1 {
                            self.verts[v2].edge_count -= 1;
                        }
                    }
                }
            }
            if abort {
                break;
            }
            a += 1;
        }
        totface
    }

    // Ported from blender/source/blender/blenlib/intern/scanfill.cc (BLI_scanfill_calc_ex).
    /// `BLI_scanfill_calc_ex(flag, nor_proj)`: fills `faces`; returns the triangle count.
    pub fn calc_ex(&mut self, flag: u32, nor_proj: Option<V3>) -> u32 {
        for e in collect(&self.edges, &self.ebase) {
            let (v1, v2) = (self.edges[e].v1, self.edges[e].v2);
            self.verts[v1].f = SF_VERT_AVAILABLE;
            self.verts[v2].f = SF_VERT_AVAILABLE;
        }
        if !collect(&self.verts, &self.vbase)
            .iter()
            .any(|&v| self.verts[v].f == SF_VERT_AVAILABLE)
        {
            return 0;
        }
        let n = match nor_proj {
            Some(n) => n,
            None => {
                let mut n = [0.0f32; 3];
                let list = collect(&self.verts, &self.vbase);
                let mut v_prev = self.verts[*list.last().expect("verts")].co;
                for &v in &list {
                    let co = self.verts[v].co;
                    if !compare_v3v3(v_prev, co, SF_EPSILON) {
                        bli::add_newell_cross_v3_v3v3(&mut n, v_prev, co);
                        v_prev = co;
                    }
                }
                n
            }
        };
        let (n, l) = bli::normalize_v3(n);
        if l == 0.0 {
            return 0;
        }
        let (rx, ry) = frap_geom::polyfill::ortho_basis_v3v3_v3(bli::negate(n));
        let project = |co: V3| [bli::dot(rx, co), bli::dot(ry, co)];
        // STEP 1: count polys.
        let mut poly: u16 = 0;
        if flag & CALC_POLYS != 0 {
            for eve in collect(&self.verts, &self.vbase) {
                self.verts[eve].xy = project(self.verts[eve].co);
                if self.verts[eve].poly_nr == SF_POLY_UNSET {
                    let mut toggle = 0u32;
                    let mut ok = true;
                    self.verts[eve].poly_nr = poly;
                    while ok {
                        ok = false;
                        toggle += 1;
                        let mut list = collect(&self.edges, &self.ebase);
                        if toggle & 1 == 0 {
                            list.reverse();
                        }
                        for eed in list {
                            let (v1, v2) = (self.edges[eed].v1, self.edges[eed].v2);
                            if self.verts[v1].poly_nr == SF_POLY_UNSET && self.verts[v2].poly_nr == poly {
                                self.verts[v1].poly_nr = poly;
                                self.edges[eed].poly_nr = poly;
                                ok = true;
                            } else if self.verts[v2].poly_nr == SF_POLY_UNSET
                                && self.verts[v1].poly_nr == poly
                            {
                                self.verts[v2].poly_nr = poly;
                                self.edges[eed].poly_nr = poly;
                                ok = true;
                            } else if self.edges[eed].poly_nr == SF_POLY_UNSET
                                && self.verts[v1].poly_nr == poly
                                && self.verts[v2].poly_nr == poly
                            {
                                self.edges[eed].poly_nr = poly;
                                ok = true;
                            }
                        }
                    }
                    poly += 1;
                }
            }
        } else {
            poly = 1;
            for eve in collect(&self.verts, &self.vbase) {
                self.verts[eve].xy = project(self.verts[eve].co);
                self.verts[eve].poly_nr = 0;
            }
            for e in collect(&self.edges, &self.ebase) {
                self.edges[e].poly_nr = 0;
            }
        }
        // STEP 2: remove loose edges and strings of edges.
        if flag & CALC_LOOSE != 0 {
            for e in collect(&self.edges, &self.ebase) {
                let (v1, v2) = (self.edges[e].v1, self.edges[e].v2);
                let c1 = self.verts[v1].edge_count;
                self.verts[v1].edge_count += 1;
                if c1 > 250 {
                    return 0;
                }
                let c2 = self.verts[v2].edge_count;
                self.verts[v2].edge_count += 1;
                if c2 > 250 {
                    return 0;
                }
            }
            self.test_vertex_near_edge();
            let mut ok = true;
            let mut toggle = 0u32;
            while ok {
                ok = false;
                toggle += 1;
                let mut it = if toggle & 1 == 1 { self.ebase.first } else { self.ebase.last };
                while let Some(eed) = it {
                    it = if toggle & 1 == 1 {
                        self.edges[eed].next
                    } else {
                        self.edges[eed].prev
                    };
                    let (v1, v2) = (self.edges[eed].v1, self.edges[eed].v2);
                    if self.verts[v1].edge_count == 1 {
                        self.verts[v2].edge_count -= 1;
                        remlink(&mut self.verts, &mut self.vbase, v1);
                        remlink(&mut self.edges, &mut self.ebase, eed);
                        ok = true;
                    } else if self.verts[v2].edge_count == 1 {
                        self.verts[v1].edge_count -= 1;
                        remlink(&mut self.verts, &mut self.vbase, v2);
                        remlink(&mut self.edges, &mut self.ebase, eed);
                        ok = true;
                    }
                }
            }
            if self.ebase.first.is_none() {
                return 0;
            }
        } else {
            for e in collect(&self.edges, &self.ebase) {
                let (v1, v2) = (self.edges[e].v1, self.edges[e].v2);
                self.verts[v1].edge_count += 1;
                self.verts[v2].edge_count += 1;
            }
        }
        // STEP 3: polyfill structs.
        let mut pflist: Vec<PolyFill> = (0..poly)
            .map(|a| PolyFill {
                edges: 0,
                verts: 0,
                min_xy: [1.0e20, 1.0e20],
                max_xy: [-1.0e20, -1.0e20],
                f: SF_POLY_NEW,
                nr: a,
            })
            .collect();
        for e in collect(&self.edges, &self.ebase) {
            pflist[self.edges[e].poly_nr as usize].edges += 1;
        }
        for v in collect(&self.verts, &self.vbase) {
            let sv = &self.verts[v];
            let pf = &mut pflist[sv.poly_nr as usize];
            pf.verts += 1;
            pf.min_xy[0] = if pf.min_xy[0] < sv.xy[0] { pf.min_xy[0] } else { sv.xy[0] };
            pf.min_xy[1] = if pf.min_xy[1] < sv.xy[1] { pf.min_xy[1] } else { sv.xy[1] };
            pf.max_xy[0] = if pf.max_xy[0] > sv.xy[0] { pf.max_xy[0] } else { sv.xy[0] };
            pf.max_xy[1] = if pf.max_xy[1] > sv.xy[1] { pf.max_xy[1] } else { sv.xy[1] };
            if sv.edge_count > 2 {
                pf.f = SF_POLY_VALID;
            }
        }
        // STEP 4: find holes or bounds, join them.
        if flag & CALC_HOLES != 0 && poly > 1 {
            let n = poly as usize;
            let mut map: Vec<usize> = (0..n).collect();
            for a in 0..n {
                if map[a] != a {
                    continue;
                }
                Self::fill_target_map_recursive(&pflist, a, a, &mut map);
            }
            for a in 0..n {
                if map[a] != a {
                    self.merge_polys(&mut pflist, map[a], a);
                }
            }
        }
        // STEP 5: make triangles.
        let mut tempve = self.vbase;
        let mut temped = self.ebase;
        self.vbase = Lb::default();
        self.ebase = Lb::default();
        let mut totfaces = 0;
        for pf in pflist.clone() {
            if pf.edges > 1 {
                self.splitlist(&mut tempve, &mut temped, pf.nr);
                totfaces += self.scanfill(&pf, flag);
            }
        }
        movelisttolist(&mut self.verts, &mut self.vbase, &mut tempve);
        movelisttolist(&mut self.edges, &mut self.ebase, &mut temped);
        totfaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_two_triangles() {
        let mut sf = ScanFill::new();
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
        let v: Vec<usize> = p.iter().enumerate().map(|(i, &c)| sf.vert_add(c, i)).collect();
        for i in 0..4 {
            sf.edge_add(v[i], v[(i + 1) % 4]);
        }
        let n = sf.calc_ex(CALC_HOLES | CALC_POLYS | CALC_LOOSE, Some([0.0, 0.0, 1.0]));
        assert_eq!(n, 2);
    }

    #[test]
    fn square_with_hole() {
        let mut sf = ScanFill::new();
        let outer = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]];
        let inner = [[1.0, 1.0, 0.0], [3.0, 1.0, 0.0], [3.0, 3.0, 0.0], [1.0, 3.0, 0.0]];
        for ring in [outer, inner] {
            let v: Vec<usize> = ring.iter().map(|&c| sf.vert_add(c, 0)).collect();
            for i in 0..4 {
                sf.edge_add(v[i], v[(i + 1) % 4]);
            }
        }
        let n = sf.calc_ex(CALC_HOLES | CALC_POLYS | CALC_LOOSE, Some([0.0, 0.0, 1.0]));
        assert_eq!(n, 8);
    }
}
