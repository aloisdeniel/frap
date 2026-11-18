// SPDX-License-Identifier: GPL-3.0-or-later
//! Port of Bullet's `btConvexHullComputer` (Ole Kniemeyer, MAXON Computer GmbH, zlib licence;
//! `extern/bullet2/src/LinearMath/btConvexHullComputer.cpp` as shipped in Blender 5.2.4, built
//! with `BT_USE_DOUBLE_PRECISION`) and of the `plConvexHull*` accessors Blender reads it with
//! (`intern/rigidbody/rb_convex_hull_api.cpp`).
//!
//! This is an altered version of the original software (rewritten in Rust; the pooled pointer
//! graph became index arenas; the unused `shrink` pass is left out). Original notice:
//!
//! > Copyright (c) 2011 Ole Kniemeyer, MAXON, www.maxon.net
//! >
//! > This software is provided 'as-is', without any express or implied warranty. In no event
//! > will the authors be held liable for any damages arising from the use of this software.
//! > Permission is granted to anyone to use this software for any purpose, including commercial
//! > applications, and to alter it and redistribute it freely, subject to the following
//! > restrictions:
//! > 1. The origin of this software must not be misrepresented; you must not claim that you
//! >    wrote the original software. If you use this software in a product, an acknowledgment
//! >    in the product documentation would be appreciated but is not required.
//! > 2. Altered source versions must be plainly marked as such, and must not be misrepresented
//! >    as being the original software.
//! > 3. This notice may not be removed or altered from any source distribution.
//!
//! The algorithm (Preparata–Hong divide and conquer on integer coordinates, O(n log n)) works on
//! the input quantised to a ±5108 integer grid, so every predicate is exact. Coplanar facets
//! come out as one polygon and a planar input as a two-sided polygon, exactly as Blender sees
//! them.

const NIL: usize = usize::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct P32 {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct P64 {
    x: i64,
    y: i64,
    z: i64,
}

impl P64 {
    fn dot(self, b: P64) -> i64 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }
}

impl P32 {
    fn new(x: i32, y: i32, z: i32) -> Self {
        P32 { x, y, z }
    }
    fn sub(self, b: P32) -> P32 {
        P32::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
    fn wide(self) -> P64 {
        P64 {
            x: self.x.into(),
            y: self.y.into(),
            z: self.z.into(),
        }
    }
    fn cross(self, b: P32) -> P64 {
        self.cross64(b.wide())
    }
    fn cross64(self, b: P64) -> P64 {
        let a = self.wide();
        P64 {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }
    fn dot(self, b: P32) -> i64 {
        self.wide().dot(b.wide())
    }
    fn dot64(self, b: P64) -> i64 {
        self.wide().dot(b)
    }
}

/// `btConvexHullInternal::Rational64`.
#[derive(Clone, Copy, Debug)]
struct Rational64 {
    num: u64,
    den: u64,
    sign: i32,
}

impl Rational64 {
    fn new(numerator: i64, denominator: i64) -> Self {
        let (mut sign, num) = match numerator.cmp(&0) {
            std::cmp::Ordering::Greater => (1, numerator as u64),
            std::cmp::Ordering::Less => (-1, numerator.wrapping_neg() as u64),
            std::cmp::Ordering::Equal => (0, 0),
        };
        let den = match denominator.cmp(&0) {
            std::cmp::Ordering::Greater => denominator as u64,
            std::cmp::Ordering::Less => {
                sign = -sign;
                denominator.wrapping_neg() as u64
            }
            std::cmp::Ordering::Equal => 0,
        };
        Rational64 { num, den, sign }
    }
    fn is_negative_infinity(&self) -> bool {
        self.sign < 0 && self.den == 0
    }
    fn is_nan(&self) -> bool {
        self.sign == 0 && self.den == 0
    }
    fn compare(&self, b: &Rational64) -> i32 {
        if self.sign != b.sign {
            return self.sign - b.sign;
        }
        if self.sign == 0 {
            return 0;
        }
        let l = u128::from(self.num) * u128::from(b.den);
        let r = u128::from(self.den) * u128::from(b.num);
        self.sign
            * match l.cmp(&r) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
    }
}

#[derive(Clone, Copy, Debug)]
struct Vertex {
    next: usize,
    prev: usize,
    edges: usize,
    point: P32,
    index: i32,
    copy: i32,
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    next: usize,
    prev: usize,
    reverse: usize,
    target: usize,
    copy: i32,
}

#[derive(Clone, Copy, Debug)]
struct IntermediateHull {
    min_xy: usize,
    max_xy: usize,
    min_yx: usize,
    max_yx: usize,
}

impl Default for IntermediateHull {
    fn default() -> Self {
        IntermediateHull {
            min_xy: NIL,
            max_xy: NIL,
            min_yx: NIL,
            max_yx: NIL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Orientation {
    None,
    Clockwise,
    CounterClockwise,
}

struct Internal {
    v: Vec<Vertex>,
    e: Vec<Edge>,
    merge_stamp: i32,
}

impl Internal {
    fn pt(&self, v: usize) -> P32 {
        self.v[v].point
    }

    /// `Edge::link`.
    fn link(&mut self, e: usize, n: usize) {
        self.e[e].next = n;
        self.e[n].prev = e;
    }

    /// `newEdgePair`.
    fn new_edge_pair(&mut self, from: usize, to: usize) -> usize {
        let e = self.e.len();
        let r = e + 1;
        let stamp = self.merge_stamp;
        self.e.push(Edge {
            next: NIL,
            prev: NIL,
            reverse: r,
            target: to,
            copy: stamp,
        });
        self.e.push(Edge {
            next: NIL,
            prev: NIL,
            reverse: e,
            target: from,
            copy: stamp,
        });
        e
    }

    /// `removeEdgePair` (the pair stays in the arena, unreachable).
    fn remove_edge_pair(&mut self, edge: usize) {
        let n = self.e[edge].next;
        let r = self.e[edge].reverse;
        let r_target = self.e[r].target;
        if n != edge {
            let p = self.e[edge].prev;
            self.e[n].prev = p;
            self.e[p].next = n;
            self.v[r_target].edges = n;
        } else {
            self.v[r_target].edges = NIL;
        }
        let n = self.e[r].next;
        let e_target = self.e[edge].target;
        if n != r {
            let p = self.e[r].prev;
            self.e[n].prev = p;
            self.e[p].next = n;
            self.v[e_target].edges = n;
        } else {
            self.v[e_target].edges = NIL;
        }
    }

    /// `mergeProjection`.
    fn merge_projection(
        &mut self,
        h0: &mut IntermediateHull,
        h1: &mut IntermediateHull,
    ) -> (bool, usize, usize) {
        let mut v0 = h0.max_yx;
        let mut v1 = h1.min_yx;
        if self.pt(v0).x == self.pt(v1).x && self.pt(v0).y == self.pt(v1).y {
            let v1p = self.v[v1].prev;
            if v1p == v1 {
                let c0 = v0;
                if self.v[v1].edges != NIL {
                    v1 = self.e[self.v[v1].edges].target;
                }
                return (false, c0, v1);
            }
            let v1n = self.v[v1].next;
            self.v[v1p].next = v1n;
            self.v[v1n].prev = v1p;
            let (pn, pp) = (self.pt(v1n), self.pt(v1p));
            if v1 == h1.min_xy {
                h1.min_xy = if pn.x < pp.x || (pn.x == pp.x && pn.y < pp.y) {
                    v1n
                } else {
                    v1p
                };
            }
            if v1 == h1.max_xy {
                h1.max_xy = if pn.x > pp.x || (pn.x == pp.x && pn.y > pp.y) {
                    v1n
                } else {
                    v1p
                };
            }
        }

        v0 = h0.max_xy;
        v1 = h1.max_xy;
        let mut v00 = NIL;
        let mut v10 = NIL;
        let mut sign: i32 = 1;

        for side in 0..=1 {
            let mut dx = (self.pt(v1).x - self.pt(v0).x) * sign;
            if dx > 0 {
                loop {
                    let dy = self.pt(v1).y - self.pt(v0).y;
                    let w0 = if side == 1 { self.v[v0].next } else { self.v[v0].prev };
                    if w0 != v0 {
                        let dx0 = (self.pt(w0).x - self.pt(v0).x) * sign;
                        let dy0 = self.pt(w0).y - self.pt(v0).y;
                        if dy0 <= 0 && (dx0 == 0 || (dx0 < 0 && dy0 * dx <= dy * dx0)) {
                            v0 = w0;
                            dx = (self.pt(v1).x - self.pt(v0).x) * sign;
                            continue;
                        }
                    }
                    let w1 = if side == 1 { self.v[v1].next } else { self.v[v1].prev };
                    if w1 != v1 {
                        let dx1 = (self.pt(w1).x - self.pt(v1).x) * sign;
                        let dy1 = self.pt(w1).y - self.pt(v1).y;
                        let dxn = (self.pt(w1).x - self.pt(v0).x) * sign;
                        if dxn > 0 && dy1 < 0 && (dx1 == 0 || (dx1 < 0 && dy1 * dx < dy * dx1)) {
                            v1 = w1;
                            dx = dxn;
                            continue;
                        }
                    }
                    break;
                }
            } else if dx < 0 {
                loop {
                    let dy = self.pt(v1).y - self.pt(v0).y;
                    let w1 = if side == 1 { self.v[v1].prev } else { self.v[v1].next };
                    if w1 != v1 {
                        let dx1 = (self.pt(w1).x - self.pt(v1).x) * sign;
                        let dy1 = self.pt(w1).y - self.pt(v1).y;
                        if dy1 >= 0 && (dx1 == 0 || (dx1 < 0 && dy1 * dx <= dy * dx1)) {
                            v1 = w1;
                            dx = (self.pt(v1).x - self.pt(v0).x) * sign;
                            continue;
                        }
                    }
                    let w0 = if side == 1 { self.v[v0].prev } else { self.v[v0].next };
                    if w0 != v0 {
                        let dx0 = (self.pt(w0).x - self.pt(v0).x) * sign;
                        let dy0 = self.pt(w0).y - self.pt(v0).y;
                        let dxn = (self.pt(v1).x - self.pt(w0).x) * sign;
                        if dxn < 0 && dy0 > 0 && (dx0 == 0 || (dx0 < 0 && dy0 * dx < dy * dx0)) {
                            v0 = w0;
                            dx = dxn;
                            continue;
                        }
                    }
                    break;
                }
            } else {
                let x = self.pt(v0).x;
                let mut y0 = self.pt(v0).y;
                let mut w0 = v0;
                loop {
                    let t = if side == 1 { self.v[w0].next } else { self.v[w0].prev };
                    if t != v0 && self.pt(t).x == x && self.pt(t).y <= y0 {
                        w0 = t;
                        y0 = self.pt(t).y;
                    } else {
                        break;
                    }
                }
                v0 = w0;
                let mut y1 = self.pt(v1).y;
                let mut w1 = v1;
                loop {
                    let t = if side == 1 { self.v[w1].prev } else { self.v[w1].next };
                    if t != v1 && self.pt(t).x == x && self.pt(t).y >= y1 {
                        w1 = t;
                        y1 = self.pt(t).y;
                    } else {
                        break;
                    }
                }
                v1 = w1;
            }

            if side == 0 {
                v00 = v0;
                v10 = v1;
                v0 = h0.min_xy;
                v1 = h1.min_xy;
                sign = -1;
            }
        }

        self.v[v0].prev = v1;
        self.v[v1].next = v0;
        self.v[v00].next = v10;
        self.v[v10].prev = v00;

        if self.pt(h1.min_xy).x < self.pt(h0.min_xy).x {
            h0.min_xy = h1.min_xy;
        }
        if self.pt(h1.max_xy).x >= self.pt(h0.max_xy).x {
            h0.max_xy = h1.max_xy;
        }
        h0.max_yx = h1.max_yx;
        (true, v00, v10)
    }

    /// `computeInternal` (vertex `i` of the arena is `originalVertices[i]`).
    fn compute_internal(&mut self, start: usize, end: usize, result: &mut IntermediateHull) {
        let n = end - start;
        match n {
            0 => {
                *result = IntermediateHull::default();
                return;
            }
            1 | 2 => {
                if n == 2 {
                    let mut v = start;
                    let mut w = start + 1;
                    if self.pt(v) != self.pt(w) {
                        let dx = self.pt(v).x - self.pt(w).x;
                        let dy = self.pt(v).y - self.pt(w).y;
                        if dx == 0 && dy == 0 {
                            if self.pt(v).z > self.pt(w).z {
                                std::mem::swap(&mut v, &mut w);
                            }
                            self.v[v].next = v;
                            self.v[v].prev = v;
                            *result = IntermediateHull {
                                min_xy: v,
                                max_xy: v,
                                min_yx: v,
                                max_yx: v,
                            };
                        } else {
                            self.v[v].next = w;
                            self.v[v].prev = w;
                            self.v[w].next = v;
                            self.v[w].prev = v;
                            if dx < 0 || (dx == 0 && dy < 0) {
                                result.min_xy = v;
                                result.max_xy = w;
                            } else {
                                result.min_xy = w;
                                result.max_xy = v;
                            }
                            if dy < 0 || (dy == 0 && dx < 0) {
                                result.min_yx = v;
                                result.max_yx = w;
                            } else {
                                result.min_yx = w;
                                result.max_yx = v;
                            }
                        }
                        let e = self.new_edge_pair(v, w);
                        self.link(e, e);
                        self.v[v].edges = e;
                        let r = self.e[e].reverse;
                        self.link(r, r);
                        self.v[w].edges = r;
                        return;
                    }
                }
                let v = start;
                self.v[v].edges = NIL;
                self.v[v].next = v;
                self.v[v].prev = v;
                *result = IntermediateHull {
                    min_xy: v,
                    max_xy: v,
                    min_yx: v,
                    max_yx: v,
                };
                return;
            }
            _ => {}
        }
        let split0 = start + n / 2;
        let p = self.pt(split0 - 1);
        let mut split1 = split0;
        while split1 < end && self.pt(split1) == p {
            split1 += 1;
        }
        self.compute_internal(start, split0, result);
        let mut hull1 = IntermediateHull::default();
        self.compute_internal(split1, end, &mut hull1);
        self.merge(result, &mut hull1);
    }

    /// `getOrientation`.
    fn get_orientation(&self, prev: usize, next: usize, s: P32, t: P32) -> Orientation {
        if self.e[prev].next == next {
            if self.e[prev].prev == next {
                let n = t.cross(s);
                let nrt = self.e[self.e[next].reverse].target;
                let m = self
                    .pt(self.e[prev].target)
                    .sub(self.pt(nrt))
                    .cross(self.pt(self.e[next].target).sub(self.pt(nrt)));
                let dot = n.dot(m);
                return if dot > 0 {
                    Orientation::CounterClockwise
                } else {
                    Orientation::Clockwise
                };
            }
            Orientation::CounterClockwise
        } else if self.e[prev].prev == next {
            Orientation::Clockwise
        } else {
            Orientation::None
        }
    }

    /// `findMaxAngle`.
    fn find_max_angle(
        &self,
        ccw: bool,
        start: usize,
        s: P32,
        rxs: P64,
        sxrxs: P64,
        min_cot: &mut Rational64,
    ) -> usize {
        let mut min_edge = NIL;
        let first = self.v[start].edges;
        if first != NIL {
            let mut e = first;
            loop {
                if self.e[e].copy > self.merge_stamp {
                    let t = self.pt(self.e[e].target).sub(self.pt(start));
                    let cot = Rational64::new(t.dot64(sxrxs), t.dot64(rxs));
                    if !cot.is_nan() {
                        if min_edge == NIL {
                            *min_cot = cot;
                            min_edge = e;
                        } else {
                            let cmp = cot.compare(min_cot);
                            if cmp < 0 {
                                *min_cot = cot;
                                min_edge = e;
                            } else if cmp == 0
                                && ccw
                                    == (self.get_orientation(min_edge, e, s, t)
                                        == Orientation::CounterClockwise)
                            {
                                min_edge = e;
                            }
                        }
                    }
                }
                e = self.e[e].next;
                if e == first {
                    break;
                }
            }
        }
        min_edge
    }

    /// `findEdgeForCoplanarFaces`.
    #[allow(clippy::too_many_arguments)]
    fn find_edge_for_coplanar_faces(
        &self,
        c0: usize,
        c1: usize,
        e0: &mut usize,
        e1: &mut usize,
        stop0: usize,
        stop1: usize,
    ) {
        let start0 = *e0;
        let start1 = *e1;
        let mut et0 = if start0 != NIL {
            self.pt(self.e[start0].target)
        } else {
            self.pt(c0)
        };
        let mut et1 = if start1 != NIL {
            self.pt(self.e[start1].target)
        } else {
            self.pt(c1)
        };
        let s = self.pt(c1).sub(self.pt(c0));
        let anchor = if start0 != NIL { start0 } else { start1 };
        let normal = self.pt(self.e[anchor].target).sub(self.pt(c0)).cross(s);
        let dist = self.pt(c0).dot64(normal);
        let perp = s.cross64(normal);

        let mut max_dot0 = et0.dot64(perp);
        if *e0 != NIL {
            while self.e[*e0].target != stop0 {
                let e = self.e[self.e[*e0].reverse].prev;
                let tp = self.pt(self.e[e].target);
                if tp.dot64(normal) < dist {
                    break;
                }
                if self.e[e].copy == self.merge_stamp {
                    break;
                }
                let dot = tp.dot64(perp);
                if dot <= max_dot0 {
                    break;
                }
                max_dot0 = dot;
                *e0 = e;
                et0 = tp;
            }
        }

        let mut max_dot1 = et1.dot64(perp);
        if *e1 != NIL {
            while self.e[*e1].target != stop1 {
                let e = self.e[self.e[*e1].reverse].next;
                let tp = self.pt(self.e[e].target);
                if tp.dot64(normal) < dist {
                    break;
                }
                if self.e[e].copy == self.merge_stamp {
                    break;
                }
                let dot = tp.dot64(perp);
                if dot <= max_dot1 {
                    break;
                }
                max_dot1 = dot;
                *e1 = e;
                et1 = tp;
            }
        }

        let mut dx = max_dot1 - max_dot0;
        if dx > 0 {
            loop {
                let dy = et1.sub(et0).dot(s);
                if *e0 != NIL && self.e[*e0].target != stop0 {
                    let f0 = self.e[self.e[*e0].next].reverse;
                    if self.e[f0].copy > self.merge_stamp {
                        let d0 = self.pt(self.e[f0].target).sub(et0);
                        let dx0 = d0.dot64(perp);
                        let dy0 = d0.dot(s);
                        let ok = if dx0 == 0 {
                            dy0 < 0
                        } else {
                            dx0 < 0
                                && Rational64::new(dy0, dx0).compare(&Rational64::new(dy, dx)) >= 0
                        };
                        if ok {
                            et0 = self.pt(self.e[f0].target);
                            dx = et1.sub(et0).dot64(perp);
                            *e0 = if *e0 == start0 { NIL } else { f0 };
                            continue;
                        }
                    }
                }
                if *e1 != NIL && self.e[*e1].target != stop1 {
                    let f1 = self.e[self.e[*e1].reverse].next;
                    if self.e[f1].copy > self.merge_stamp {
                        let d1 = self.pt(self.e[f1].target).sub(et1);
                        if d1.dot64(normal) == 0 {
                            let dx1 = d1.dot64(perp);
                            let dy1 = d1.dot(s);
                            let dxn = self.pt(self.e[f1].target).sub(et0).dot64(perp);
                            let ok = dxn > 0
                                && if dx1 == 0 {
                                    dy1 < 0
                                } else {
                                    dx1 < 0
                                        && Rational64::new(dy1, dx1)
                                            .compare(&Rational64::new(dy, dx))
                                            > 0
                                };
                            if ok {
                                *e1 = f1;
                                et1 = self.pt(self.e[f1].target);
                                dx = dxn;
                                continue;
                            }
                        }
                    }
                }
                break;
            }
        } else if dx < 0 {
            loop {
                let dy = et1.sub(et0).dot(s);
                if *e1 != NIL && self.e[*e1].target != stop1 {
                    let f1 = self.e[self.e[*e1].prev].reverse;
                    if self.e[f1].copy > self.merge_stamp {
                        let d1 = self.pt(self.e[f1].target).sub(et1);
                        let dx1 = d1.dot64(perp);
                        let dy1 = d1.dot(s);
                        let ok = if dx1 == 0 {
                            dy1 > 0
                        } else {
                            dx1 < 0
                                && Rational64::new(dy1, dx1).compare(&Rational64::new(dy, dx)) <= 0
                        };
                        if ok {
                            et1 = self.pt(self.e[f1].target);
                            dx = et1.sub(et0).dot64(perp);
                            *e1 = if *e1 == start1 { NIL } else { f1 };
                            continue;
                        }
                    }
                }
                if *e0 != NIL && self.e[*e0].target != stop0 {
                    let f0 = self.e[self.e[*e0].reverse].prev;
                    if self.e[f0].copy > self.merge_stamp {
                        let d0 = self.pt(self.e[f0].target).sub(et0);
                        if d0.dot64(normal) == 0 {
                            let dx0 = d0.dot64(perp);
                            let dy0 = d0.dot(s);
                            let dxn = et1.sub(self.pt(self.e[f0].target)).dot64(perp);
                            let ok = dxn < 0
                                && if dx0 == 0 {
                                    dy0 > 0
                                } else {
                                    dx0 < 0
                                        && Rational64::new(dy0, dx0)
                                            .compare(&Rational64::new(dy, dx))
                                            < 0
                                };
                            if ok {
                                *e0 = f0;
                                et0 = self.pt(self.e[f0].target);
                                dx = dxn;
                                continue;
                            }
                        }
                    }
                }
                break;
            }
        }
    }

    /// `merge`.
    fn merge(&mut self, h0: &mut IntermediateHull, h1: &mut IntermediateHull) {
        if h1.max_xy == NIL {
            return;
        }
        if h0.max_xy == NIL {
            *h0 = *h1;
            return;
        }
        self.merge_stamp -= 1;

        let mut to_prev0 = NIL;
        let mut first_new0 = NIL;
        let mut pending_head0 = NIL;
        let mut pending_tail0 = NIL;
        let mut to_prev1 = NIL;
        let mut first_new1 = NIL;
        let mut pending_head1 = NIL;
        let mut pending_tail1 = NIL;
        let mut prev_point;

        let (projected, mut c0, mut c1) = self.merge_projection(h0, h1);
        if projected {
            let s = self.pt(c1).sub(self.pt(c0));
            let down = P32::new(0, 0, -1);
            let normal = down.cross(s);
            let t = s.cross64(normal);

            let mut start0 = NIL;
            let first = self.v[c0].edges;
            if first != NIL {
                let mut e = first;
                loop {
                    let d = self.pt(self.e[e].target).sub(self.pt(c0));
                    if d.dot64(normal) == 0
                        && d.dot64(t) > 0
                        && (start0 == NIL
                            || self.get_orientation(start0, e, s, down) == Orientation::Clockwise)
                    {
                        start0 = e;
                    }
                    e = self.e[e].next;
                    if e == first {
                        break;
                    }
                }
            }
            let mut start1 = NIL;
            let first = self.v[c1].edges;
            if first != NIL {
                let mut e = first;
                loop {
                    let d = self.pt(self.e[e].target).sub(self.pt(c1));
                    if d.dot64(normal) == 0
                        && d.dot64(t) > 0
                        && (start1 == NIL
                            || self.get_orientation(start1, e, s, down)
                                == Orientation::CounterClockwise)
                    {
                        start1 = e;
                    }
                    e = self.e[e].next;
                    if e == first {
                        break;
                    }
                }
            }
            if start0 != NIL || start1 != NIL {
                self.find_edge_for_coplanar_faces(c0, c1, &mut start0, &mut start1, NIL, NIL);
                if start0 != NIL {
                    c0 = self.e[start0].target;
                }
                if start1 != NIL {
                    c1 = self.e[start1].target;
                }
            }
            prev_point = self.pt(c1);
            prev_point.z += 1;
        } else {
            prev_point = self.pt(c1);
            prev_point.x += 1;
        }

        let first0 = c0;
        let first1 = c1;
        let mut first_run = true;

        loop {
            let s = self.pt(c1).sub(self.pt(c0));
            let r = prev_point.sub(self.pt(c0));
            let rxs = r.cross(s);
            let sxrxs = s.cross64(rxs);

            let mut min_cot0 = Rational64::new(0, 0);
            let min0 = self.find_max_angle(false, c0, s, rxs, sxrxs, &mut min_cot0);
            let mut min_cot1 = Rational64::new(0, 0);
            let min1 = self.find_max_angle(true, c1, s, rxs, sxrxs, &mut min_cot1);
            if min0 == NIL && min1 == NIL {
                let e = self.new_edge_pair(c0, c1);
                self.link(e, e);
                self.v[c0].edges = e;
                let r = self.e[e].reverse;
                self.link(r, r);
                self.v[c1].edges = r;
                return;
            }
            let cmp = if min0 == NIL {
                1
            } else if min1 == NIL {
                -1
            } else {
                min_cot0.compare(&min_cot1)
            };
            if first_run
                || if cmp >= 0 {
                    !min_cot1.is_negative_infinity()
                } else {
                    !min_cot0.is_negative_infinity()
                }
            {
                let e = self.new_edge_pair(c0, c1);
                if pending_tail0 != NIL {
                    self.e[pending_tail0].prev = e;
                } else {
                    pending_head0 = e;
                }
                self.e[e].next = pending_tail0;
                pending_tail0 = e;

                let e = self.e[e].reverse;
                if pending_tail1 != NIL {
                    self.e[pending_tail1].next = e;
                } else {
                    pending_head1 = e;
                }
                self.e[e].prev = pending_tail1;
                pending_tail1 = e;
            }

            let mut e0 = min0;
            let mut e1 = min1;
            if cmp == 0 {
                self.find_edge_for_coplanar_faces(c0, c1, &mut e0, &mut e1, NIL, NIL);
            }

            if cmp >= 0 && e1 != NIL {
                if to_prev1 != NIL {
                    let mut e = self.e[to_prev1].next;
                    while e != min1 {
                        let n = self.e[e].next;
                        self.remove_edge_pair(e);
                        e = n;
                    }
                }
                if pending_tail1 != NIL {
                    if to_prev1 != NIL {
                        self.link(to_prev1, pending_head1);
                    } else {
                        let p = self.e[min1].prev;
                        self.link(p, pending_head1);
                        first_new1 = pending_head1;
                    }
                    self.link(pending_tail1, min1);
                    pending_head1 = NIL;
                    pending_tail1 = NIL;
                } else if to_prev1 == NIL {
                    first_new1 = min1;
                }
                prev_point = self.pt(c1);
                c1 = self.e[e1].target;
                to_prev1 = self.e[e1].reverse;
            }

            if cmp <= 0 && e0 != NIL {
                if to_prev0 != NIL {
                    let mut e = self.e[to_prev0].prev;
                    while e != min0 {
                        let n = self.e[e].prev;
                        self.remove_edge_pair(e);
                        e = n;
                    }
                }
                if pending_tail0 != NIL {
                    if to_prev0 != NIL {
                        self.link(pending_head0, to_prev0);
                    } else {
                        let nx = self.e[min0].next;
                        self.link(pending_head0, nx);
                        first_new0 = pending_head0;
                    }
                    self.link(min0, pending_tail0);
                    pending_head0 = NIL;
                    pending_tail0 = NIL;
                } else if to_prev0 == NIL {
                    first_new0 = min0;
                }
                prev_point = self.pt(c0);
                c0 = self.e[e0].target;
                to_prev0 = self.e[e0].reverse;
            }

            if c0 == first0 && c1 == first1 {
                if to_prev0 == NIL {
                    self.link(pending_head0, pending_tail0);
                    self.v[c0].edges = pending_tail0;
                } else {
                    let mut e = self.e[to_prev0].prev;
                    while e != first_new0 {
                        let n = self.e[e].prev;
                        self.remove_edge_pair(e);
                        e = n;
                    }
                    if pending_tail0 != NIL {
                        self.link(pending_head0, to_prev0);
                        self.link(first_new0, pending_tail0);
                    }
                }
                if to_prev1 == NIL {
                    self.link(pending_tail1, pending_head1);
                    self.v[c1].edges = pending_tail1;
                } else {
                    let mut e = self.e[to_prev1].next;
                    while e != first_new1 {
                        let n = self.e[e].next;
                        self.remove_edge_pair(e);
                        e = n;
                    }
                    if pending_tail1 != NIL {
                        self.link(to_prev1, pending_head1);
                        self.link(pending_tail1, first_new1);
                    }
                }
                return;
            }
            first_run = false;
        }
    }
}

/// `pointCmp`.
fn point_lt(p: &P32, q: &P32) -> bool {
    p.y < q.y || (p.y == q.y && (p.x < q.x || (p.x == q.x && p.z < q.z)))
}

/// `btAlignedObjectArray::quickSortInternal` (its exact partition order, which decides the
/// order of duplicate points).
fn quick_sort(a: &mut [(P32, i32)], lo: usize, hi: usize) {
    let mut i = lo as isize;
    let mut j = hi as isize;
    let x = a[(lo + hi) / 2].0;
    loop {
        while point_lt(&a[i as usize].0, &x) {
            i += 1;
        }
        while point_lt(&x, &a[j as usize].0) {
            j -= 1;
        }
        if i <= j {
            a.swap(i as usize, j as usize);
            i += 1;
            j -= 1;
        }
        if i > j {
            break;
        }
    }
    if (lo as isize) < j {
        quick_sort(a, lo, j as usize);
    }
    if i < hi as isize {
        quick_sort(a, i as usize, hi);
    }
}

/// `btVector3::maxAxis`.
fn max_axis(v: [f64; 3]) -> usize {
    if v[0] < v[1] {
        if v[1] < v[2] { 2 } else { 1 }
    } else if v[0] < v[2] {
        2
    } else {
        0
    }
}

/// `btVector3::minAxis`.
fn min_axis(v: [f64; 3]) -> usize {
    if v[0] < v[1] {
        if v[0] < v[2] { 0 } else { 2 }
    } else if v[1] < v[2] {
        1
    } else {
        2
    }
}

/// The convex hull as Blender reads it through `plConvexHull*`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConvexHull {
    /// `plConvexHullGetVertex(.., original_index)`: input index of each hull vertex.
    pub original_index: Vec<usize>,
    /// `plConvexHullGetFaceVertices`: each face's hull-vertex indices, counter-clockwise seen
    /// from outside (a planar input gives both sides; a collinear one a degenerate loop).
    pub faces: Vec<Vec<usize>>,
}

/// `plConvexHullCompute(coords, count)` = `btConvexHullComputer::compute(coords, stride,
/// count, 0, 0)` (float input, double arithmetic).
pub fn compute(coords: &[[f32; 3]]) -> ConvexHull {
    let count = coords.len();
    if count == 0 {
        return ConvexHull::default();
    }
    // `btConvexHullInternal::compute`.
    let mut min = [1e30f64; 3];
    let mut max = [-1e30f64; 3];
    for c in coords {
        for k in 0..3 {
            let p = f64::from(c[k]);
            if p < min[k] {
                min[k] = p;
            }
            if max[k] < p {
                max[k] = p;
            }
        }
    }
    let mut s = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let max_ax = max_axis(s);
    let mut min_ax = min_axis(s);
    if min_ax == max_ax {
        min_ax = (max_ax + 1) % 3;
    }
    let med_ax = 3 - max_ax - min_ax;
    // `s /= btScalar(10216)` is `s *= 1 / 10216`.
    let inv = 1.0f64 / 10216.0;
    for c in &mut s {
        *c *= inv;
    }
    if (med_ax + 1) % 3 != max_ax {
        for c in &mut s {
            *c *= -1.0;
        }
    }
    for c in &mut s {
        if *c != 0.0 {
            *c = 1.0 / *c;
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let mut points: Vec<(P32, i32)> = coords
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let p: [f64; 3] = std::array::from_fn(|k| (f64::from(c[k]) - center[k]) * s[k]);
            (
                P32::new(p[med_ax] as i32, p[max_ax] as i32, p[min_ax] as i32),
                i as i32,
            )
        })
        .collect();
    if points.len() > 1 {
        let hi = points.len() - 1;
        quick_sort(&mut points, 0, hi);
    }
    let mut hull = Internal {
        v: points
            .iter()
            .map(|&(point, index)| Vertex {
                next: NIL,
                prev: NIL,
                edges: NIL,
                point,
                index,
                copy: -1,
            })
            .collect(),
        e: Vec::with_capacity(6 * count),
        merge_stamp: -3,
    };
    let mut ih = IntermediateHull::default();
    hull.compute_internal(0, count, &mut ih);
    let vertex_list = ih.min_xy;

    // `btConvexHullComputer::compute`: copy out vertices, edges and faces.
    struct OutEdge {
        next: usize,
        reverse: usize,
        target: usize,
    }
    let mut out_edges: Vec<OutEdge> = Vec::new();
    let mut old_vertices: Vec<usize> = Vec::new();
    let mut original_index: Vec<usize> = Vec::new();
    let get_vertex_copy = |hull: &mut Internal, old: &mut Vec<usize>, v: usize| -> usize {
        let index = hull.v[v].copy;
        if index < 0 {
            let index = old.len();
            hull.v[v].copy = index as i32;
            old.push(v);
            index
        } else {
            index as usize
        }
    };
    get_vertex_copy(&mut hull, &mut old_vertices, vertex_list);
    let mut copied = 0usize;
    while copied < old_vertices.len() {
        let v = old_vertices[copied];
        original_index.push(hull.v[v].index as usize);
        let first_edge = hull.v[v].edges;
        if first_edge != NIL {
            let mut first_copy = NIL;
            let mut prev_copy = NIL;
            let mut e = first_edge;
            loop {
                if hull.e[e].copy < 0 {
                    let s = out_edges.len();
                    let et = hull.e[e].target;
                    let target = get_vertex_copy(&mut hull, &mut old_vertices, et);
                    out_edges.push(OutEdge {
                        next: NIL,
                        reverse: s + 1,
                        target,
                    });
                    out_edges.push(OutEdge {
                        next: NIL,
                        reverse: s,
                        target: copied,
                    });
                    hull.e[e].copy = s as i32;
                    let r = hull.e[e].reverse;
                    hull.e[r].copy = (s + 1) as i32;
                }
                let ec = hull.e[e].copy as usize;
                if prev_copy != NIL {
                    out_edges[ec].next = prev_copy;
                } else {
                    first_copy = ec;
                }
                prev_copy = ec;
                e = hull.e[e].next;
                if e == first_edge {
                    break;
                }
            }
            out_edges[first_copy].next = prev_copy;
        }
        copied += 1;
    }
    let mut face_starts: Vec<usize> = Vec::new();
    for &v in old_vertices.iter().take(copied) {
        let first_edge = hull.v[v].edges;
        if first_edge == NIL {
            continue;
        }
        let mut e = first_edge;
        loop {
            if hull.e[e].copy >= 0 {
                face_starts.push(hull.e[e].copy as usize);
                let mut f = e;
                loop {
                    hull.e[f].copy = -1;
                    f = hull.e[hull.e[f].reverse].prev;
                    if f == e {
                        break;
                    }
                }
            }
            e = hull.e[e].next;
            if e == first_edge {
                break;
            }
        }
    }
    // `plConvexHullGetFaceVertices`: targets along `getNextEdgeOfFace`.
    let faces = face_starts
        .into_iter()
        .map(|start| {
            let mut out = Vec::new();
            let mut e = start;
            loop {
                out.push(out_edges[e].target);
                e = out_edges[out_edges[e].reverse].next;
                if e == start {
                    break;
                }
            }
            out
        })
        .collect();
    ConvexHull {
        original_index,
        faces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn faces_in_input(h: &ConvexHull) -> Vec<Vec<usize>> {
        h.faces
            .iter()
            .map(|f| f.iter().map(|&k| h.original_index[k]).collect())
            .collect()
    }

    #[test]
    fn cube_gives_six_quads() {
        let mut pts = Vec::new();
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    pts.push([x, y, z]);
                }
            }
        }
        pts.push([0.0, 0.0, 0.0]);
        let h = compute(&pts);
        assert_eq!(h.original_index.len(), 8);
        assert_eq!(h.faces.len(), 6);
        assert!(h.faces.iter().all(|f| f.len() == 4));
        assert!(faces_in_input(&h).iter().all(|f| !f.contains(&8)));
    }

    #[test]
    fn planar_square_gives_two_sided_polygon() {
        let mut pts = Vec::new();
        for y in [-1.0, 0.0, 1.0] {
            for x in [-1.0, 0.0, 1.0] {
                pts.push([x, y, 0.0]);
            }
        }
        let h = compute(&pts);
        assert_eq!(h.faces.len(), 2);
        assert!(h.faces.iter().all(|f| f.len() == 4));
    }

    #[test]
    fn three_points_give_two_triangles() {
        let h = compute(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        assert_eq!(h.faces.len(), 2);
        assert!(h.faces.iter().all(|f| f.len() == 3));
    }

    #[test]
    fn large_cloud_is_fast_and_closed() {
        // 20k points on a sphere (n log n: well under a second even in debug).
        let n = 20_000;
        let pts: Vec<[f32; 3]> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                let z = 1.0 - 2.0 * t;
                let r = (1.0 - z * z).max(0.0).sqrt();
                let a = i as f64 * 2.399_963_229_728_653;
                [(r * libm::cos(a)) as f32, (r * libm::sin(a)) as f32, z as f32]
            })
            .collect();
        let h = compute(&pts);
        // Euler: V - E + F = 2 over polygon faces.
        let e: usize = h.faces.iter().map(Vec::len).sum::<usize>() / 2;
        assert_eq!(h.original_index.len() + h.faces.len(), e + 2);
    }
}
