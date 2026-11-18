// SPDX-License-Identifier: GPL-3.0-or-later
//! Queries (`bmesh/intern/bmesh_query.cc`, `bmesh_iterators.cc`, `bmesh_structure.cc`,
//! `bmesh_polygon.cc`, `bmesh_mesh_normals.cc`; ARCHITECTURE §6.2). CONTRACT frozen by M2 W0,
//! implemented by WP 2.1. Iterators follow Blender's cycle order (disk cycle from `v->e`,
//! radial cycle from `e->l`, face loops from `l_first`), so ports keep Blender's visiting order.

pub mod geom;

use geom::V3;

use crate::types::{BMesh, EdgeId, FaceId, LoopId, VertId};

impl BMesh {
    // ---------------------------------------------------------------- disk and radial cycles

    /// `bmesh_disk_edge_next(e, v)` / `BM_DISK_EDGE_NEXT`.
    #[inline]
    pub fn disk_edge_next(&self, e: EdgeId, v: VertId) -> EdgeId {
        let ed = self.edge(e);
        ed.disk[(ed.v[1] == v) as usize].next
    }
    /// `bmesh_disk_edge_prev(e, v)`.
    #[inline]
    pub fn disk_edge_prev(&self, e: EdgeId, v: VertId) -> EdgeId {
        let ed = self.edge(e);
        ed.disk[(ed.v[1] == v) as usize].prev
    }
    /// `BM_vert_in_edge`.
    #[inline]
    pub fn vert_in_edge(&self, e: EdgeId, v: VertId) -> bool {
        let ed = self.edge(e);
        ed.v[0] == v || ed.v[1] == v
    }
    /// `BM_verts_in_edge`.
    #[inline]
    pub fn verts_in_edge(&self, a: VertId, b: VertId, e: EdgeId) -> bool {
        let ed = self.edge(e);
        (ed.v[0] == a && ed.v[1] == b) || (ed.v[0] == b && ed.v[1] == a)
    }

    /// `BM_ITER_ELEM(e, v, BM_EDGES_OF_VERT)`: the disk cycle of `v`.
    pub fn vert_edges(&self, v: VertId) -> Vec<EdgeId> {
        let mut out = Vec::new();
        let Some(first) = self.vert(v).e else {
            return out;
        };
        let mut e = first;
        loop {
            out.push(e);
            e = self.disk_edge_next(e, v);
            if e == first {
                break;
            }
            if out.len() > self.edges.capacity() {
                panic!("BMesh: disk cycle of vertex {} does not close", v.0);
            }
        }
        out
    }

    /// `bmesh_radial_facevert_count`.
    fn radial_facevert_count(&self, l: LoopId, v: VertId) -> usize {
        let mut count = 0;
        let mut it = l;
        loop {
            if self.lp(it).v == v {
                count += 1;
            }
            it = self.lp(it).radial_next;
            if it == l {
                break;
            }
        }
        count
    }
    /// `bmesh_radial_facevert_check`.
    fn radial_facevert_check(&self, l: LoopId, v: VertId) -> bool {
        let mut it = l;
        loop {
            if self.lp(it).v == v {
                return true;
            }
            it = self.lp(it).radial_next;
            if it == l {
                return false;
            }
        }
    }
    /// `bmesh_disk_facevert_count` (`BM_vert_face_count`): corners using `v`.
    pub fn vert_face_count(&self, v: VertId) -> usize {
        let mut count = 0;
        for e in self.vert_edges(v) {
            if let Some(l) = self.edge(e).l {
                count += self.radial_facevert_count(l, v);
            }
        }
        count
    }
    /// `bmesh_disk_faceloop_find_first`.
    fn disk_faceloop_find_first(&self, e: EdgeId, v: VertId) -> Option<LoopId> {
        let mut it = e;
        loop {
            if let Some(l) = self.edge(it).l {
                let ll = self.lp(l);
                return Some(if ll.v == v { l } else { ll.next });
            }
            it = self.disk_edge_next(it, v);
            if it == e {
                return None;
            }
        }
    }
    /// `bmesh_disk_faceedge_find_next`.
    fn disk_faceedge_find_next(&self, e: EdgeId, v: VertId) -> EdgeId {
        let mut f = self.disk_edge_next(e, v);
        loop {
            if let Some(l) = self.edge(f).l
                && self.radial_facevert_check(l, v)
            {
                return f;
            }
            f = self.disk_edge_next(f, v);
            if f == e {
                return e;
            }
        }
    }
    /// `bmesh_radial_faceloop_find_first`.
    fn radial_faceloop_find_first(&self, l: LoopId, v: VertId) -> Option<LoopId> {
        let mut it = l;
        loop {
            if self.lp(it).v == v {
                return Some(it);
            }
            it = self.lp(it).radial_next;
            if it == l {
                return None;
            }
        }
    }
    /// `bmesh_radial_faceloop_find_next`.
    fn radial_faceloop_find_next(&self, l: LoopId, v: VertId) -> LoopId {
        let mut it = self.lp(l).radial_next;
        loop {
            if self.lp(it).v == v {
                return it;
            }
            it = self.lp(it).radial_next;
            if it == l {
                return l;
            }
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_iterators.cc
    // (bmiter__loop_of_vert_begin/step).
    /// `BM_LOOPS_OF_VERT`.
    pub fn vert_loops(&self, v: VertId) -> Vec<LoopId> {
        let mut out = Vec::new();
        let mut count = self.vert_face_count(v);
        if count == 0 {
            return out;
        }
        let Some(e0) = self.vert(v).e else {
            return out;
        };
        let Some(mut l_first) = self.disk_faceloop_find_first(e0, v) else {
            return out;
        };
        let mut e_next = self.lp(l_first).e;
        let mut l_next = l_first;
        while count > 0 {
            out.push(l_next);
            count -= 1;
            l_next = self.radial_faceloop_find_next(l_next, v);
            if l_next == l_first {
                e_next = self.disk_faceedge_find_next(e_next, v);
                let Some(l) = self
                    .edge(e_next)
                    .l
                    .and_then(|l| self.radial_faceloop_find_first(l, v))
                else {
                    break;
                };
                l_first = l;
                l_next = l_first;
            }
        }
        out
    }
    /// `BM_FACES_OF_VERT` (one entry per corner using `v`).
    pub fn vert_faces(&self, v: VertId) -> Vec<FaceId> {
        self.vert_loops(v)
            .into_iter()
            .map(|l| self.lp(l).f)
            .collect()
    }
    /// `BM_LOOPS_OF_EDGE`: the radial cycle of `e`.
    pub fn edge_loops(&self, e: EdgeId) -> Vec<LoopId> {
        let mut out = Vec::new();
        let Some(first) = self.edge(e).l else {
            return out;
        };
        let mut l = first;
        loop {
            out.push(l);
            l = self.lp(l).radial_next;
            if l == first {
                break;
            }
            if out.len() > self.loops.capacity() {
                panic!("BMesh: radial cycle of edge {} does not close", e.0);
            }
        }
        out
    }
    /// `BM_FACES_OF_EDGE`.
    pub fn edge_faces(&self, e: EdgeId) -> Vec<FaceId> {
        self.edge_loops(e)
            .into_iter()
            .map(|l| self.lp(l).f)
            .collect()
    }
    /// `BM_LOOPS_OF_FACE`, from `l_first`.
    pub fn face_loops(&self, f: FaceId) -> Vec<LoopId> {
        let fa = self.face(f);
        let mut out = Vec::with_capacity(fa.len as usize);
        let first = fa.l_first;
        let mut l = first;
        loop {
            out.push(l);
            l = self.lp(l).next;
            if l == first || out.len() > self.loops.capacity() {
                break;
            }
        }
        out
    }
    /// `BM_VERTS_OF_FACE`, from `l_first`.
    pub fn face_verts(&self, f: FaceId) -> Vec<VertId> {
        self.face_loops(f)
            .into_iter()
            .map(|l| self.lp(l).v)
            .collect()
    }
    /// `BM_EDGES_OF_FACE`, from `l_first`.
    pub fn face_edges(&self, f: FaceId) -> Vec<EdgeId> {
        self.face_loops(f)
            .into_iter()
            .map(|l| self.lp(l).e)
            .collect()
    }

    // ---------------------------------------------------------------- existence

    /// `BM_edge_exists`.
    pub fn edge_exists(&self, a: VertId, b: VertId) -> Option<EdgeId> {
        let first = self.vert(a).e?;
        let mut e = first;
        loop {
            if self.verts_in_edge(a, b, e) {
                return Some(e);
            }
            e = self.disk_edge_next(e, a);
            if e == first {
                return None;
            }
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_face_exists).
    /// `BM_face_exists`: a face using exactly `verts` in this cyclic order, in either winding.
    pub fn face_exists(&self, verts: &[VertId]) -> Option<FaceId> {
        let len = verts.len();
        if len < 2 {
            return None;
        }
        let first = self.vert(verts[0]).e?;
        let mut e = first;
        loop {
            if let Some(lf) = self.edge(e).l {
                let mut lr = lf;
                loop {
                    let l = self.lp(lr);
                    if l.v == verts[0] && self.face(l.f).len as usize == len {
                        let mut i_walk = 2;
                        if self.lp(l.next).v == verts[1] {
                            let mut w = self.lp(l.next).next;
                            while i_walk != len {
                                if self.lp(w).v != verts[i_walk] {
                                    break;
                                }
                                w = self.lp(w).next;
                                i_walk += 1;
                            }
                        } else if self.lp(l.prev).v == verts[1] {
                            let mut w = self.lp(l.prev).prev;
                            while i_walk != len {
                                if self.lp(w).v != verts[i_walk] {
                                    break;
                                }
                                w = self.lp(w).prev;
                                i_walk += 1;
                            }
                        }
                        if i_walk == len {
                            return Some(l.f);
                        }
                    }
                    lr = self.lp(lr).radial_next;
                    if lr == lf {
                        break;
                    }
                }
            }
            e = self.disk_edge_next(e, verts[0]);
            if e == first {
                return None;
            }
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_face_find_double).
    /// `BM_face_find_double`: another face over exactly the same edges.
    pub fn face_find_double(&self, f: FaceId) -> Option<FaceId> {
        let l_first = self.face(f).l_first;
        let flen = self.face(f).len;
        let mut l_iter = self.lp(l_first).radial_next;
        while l_iter != l_first {
            let li = self.lp(l_iter);
            if self.face(li.f).len == flen {
                let same_dir = li.v == self.lp(l_first).v;
                let (mut a, mut b) = (l_first, l_iter);
                loop {
                    if self.lp(a).e != self.lp(b).e {
                        break;
                    }
                    a = if same_dir {
                        self.lp(a).next
                    } else {
                        self.lp(a).prev
                    };
                    b = self.lp(b).next;
                    if b == l_iter {
                        break;
                    }
                }
                if b == l_iter {
                    return Some(li.f);
                }
            }
            l_iter = li.radial_next;
        }
        None
    }

    // ---------------------------------------------------------------- element relations

    /// `BM_edge_other_vert`.
    pub fn edge_other_vert(&self, e: EdgeId, v: VertId) -> VertId {
        let ed = self.edge(e);
        if ed.v[0] == v { ed.v[1] } else { ed.v[0] }
    }
    /// `BM_loop_other_edge_loop`: the other loop of `l`'s face at vertex `v` of `l->e`.
    pub fn loop_other_edge_loop(&self, l: LoopId, v: VertId) -> LoopId {
        let ll = self.lp(l);
        if ll.v == v { ll.prev } else { ll.next }
    }
    /// `BM_face_vert_share_loop`.
    pub fn face_vert_share_loop(&self, f: FaceId, v: VertId) -> Option<LoopId> {
        self.face_loops(f).into_iter().find(|&l| self.lp(l).v == v)
    }
    /// `BM_face_edge_share_loop`.
    pub fn face_edge_share_loop(&self, f: FaceId, e: EdgeId) -> Option<LoopId> {
        self.edge_loops(e).into_iter().find(|&l| self.lp(l).f == f)
    }
    /// `BM_face_other_vert_loop`: the loop after `v` going away from `v_prev`.
    pub fn face_other_vert_loop(&self, f: FaceId, v_prev: VertId, v: VertId) -> Option<LoopId> {
        let l = self.face_vert_share_loop(f, v)?;
        let ll = self.lp(l);
        if self.lp(ll.prev).v == v_prev {
            Some(ll.next)
        } else if self.lp(ll.next).v == v_prev {
            Some(ll.prev)
        } else {
            None
        }
    }
    /// `BM_edge_other_loop`: the loop of the other face at the same vertex as `l`.
    pub fn edge_other_loop(&self, e: EdgeId, l: LoopId) -> LoopId {
        let ll = self.lp(l);
        let mut lo = if ll.e == e { l } else { ll.prev };
        lo = self.lp(lo).radial_next;
        if self.lp(lo).v != ll.v && self.lp(self.lp(lo).next).v == ll.v {
            lo = self.lp(lo).next;
        }
        lo
    }
    /// `BM_vert_step_fan_loop`: steps around `l->v` over manifold edges.
    pub fn vert_step_fan_loop(&self, l: LoopId, e_step: &mut EdgeId) -> Option<LoopId> {
        let ll = self.lp(l);
        let e_prev = *e_step;
        let e_next = if ll.e == e_prev {
            self.lp(ll.prev).e
        } else if self.lp(ll.prev).e == e_prev {
            ll.e
        } else {
            return None;
        };
        if self.edge_is_manifold(e_next) {
            *e_step = e_next;
            Some(self.edge_other_loop(e_next, l))
        } else {
            None
        }
    }
    /// `BM_edge_in_face`.
    pub fn edge_in_face(&self, e: EdgeId, f: FaceId) -> bool {
        self.edge_loops(e).into_iter().any(|l| self.lp(l).f == f)
    }
    /// `BM_face_share_edge_count`.
    pub fn face_share_edge_count(&self, a: FaceId, b: FaceId) -> usize {
        self.face_loops(a)
            .into_iter()
            .filter(|&l| self.edge_in_face(self.lp(l).e, b))
            .count()
    }

    // ---------------------------------------------------------------- topology tests

    /// `BM_edge_is_manifold`: exactly two faces.
    pub fn edge_is_manifold(&self, e: EdgeId) -> bool {
        match self.edge(e).l {
            Some(l) => {
                let r = self.lp(l).radial_next;
                r != l && self.lp(r).radial_next == l
            }
            None => false,
        }
    }
    /// `BM_edge_is_boundary`: exactly one face.
    pub fn edge_is_boundary(&self, e: EdgeId) -> bool {
        match self.edge(e).l {
            Some(l) => self.lp(l).radial_next == l,
            None => false,
        }
    }
    /// `BM_edge_is_wire`: no face.
    pub fn edge_is_wire(&self, e: EdgeId) -> bool {
        self.edge(e).l.is_none()
    }
    /// `BM_edge_is_contiguous`: manifold with consistent winding.
    pub fn edge_is_contiguous(&self, e: EdgeId) -> bool {
        let Some(l) = self.edge(e).l else {
            return false;
        };
        let o = self.lp(l).radial_next;
        o != l && self.lp(o).radial_next == l && self.lp(o).v != self.lp(l).v
    }
    /// `BM_edge_face_count`.
    pub fn edge_face_count(&self, e: EdgeId) -> usize {
        self.edge_loops(e).len()
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_vert_is_manifold).
    /// `BM_vert_is_manifold`.
    pub fn vert_is_manifold(&self, v: VertId) -> bool {
        let Some(e_first) = self.vert(v).e else {
            return false;
        };
        let mut loop_num = 0;
        let mut boundary_num = 0;
        let mut l_first = self.edge(e_first).l;
        let mut e = e_first;
        loop {
            let Some(l) = self.edge(e).l else {
                return false;
            };
            if l != self.lp(self.lp(l).radial_next).radial_next {
                return false;
            }
            if self.lp(l).v == v {
                loop_num += 1;
            }
            if !self.edge_is_boundary(e) {
                if self.lp(self.lp(l).radial_next).v == v {
                    loop_num += 1;
                }
            } else {
                l_first = Some(l);
                boundary_num += 1;
                if boundary_num == 3 {
                    return false;
                }
            }
            e = self.disk_edge_next(e, v);
            if e == e_first {
                break;
            }
        }
        let Some(lf) = l_first else {
            return false;
        };
        let mut e_prev = self.lp(lf).e;
        let lf = if self.lp(lf).v == v {
            lf
        } else {
            self.lp(lf).next
        };
        let mut l_iter = lf;
        let mut loop_num_region = 0;
        loop {
            loop_num_region += 1;
            match self.vert_step_fan_loop(l_iter, &mut e_prev) {
                Some(n) if n != lf => l_iter = n,
                _ => break,
            }
        }
        loop_num == loop_num_region
    }
    /// `BM_vert_edge_count`.
    pub fn vert_edge_count(&self, v: VertId) -> usize {
        self.vert_edges(v).len()
    }
    /// `BM_vert_edge_count_nonwire`.
    pub fn vert_edge_count_nonwire(&self, v: VertId) -> usize {
        self.vert_edges(v)
            .into_iter()
            .filter(|&e| self.edge(e).l.is_some())
            .count()
    }

    // ---------------------------------------------------------------- geometry

    fn loop_cos(&self, f: FaceId) -> Vec<V3> {
        self.face_loops(f)
            .into_iter()
            .map(|l| self.vert(self.lp(l).v).co)
            .collect()
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc (BM_face_calc_normal).
    /// `BM_face_calc_normal` (quad and triangle special cases, Newell otherwise); a zero vector
    /// for degenerate faces.
    pub fn face_calc_normal(&self, f: FaceId) -> [f32; 3] {
        let c = self.loop_cos(f);
        match c.len() {
            4 => geom::normal_quad_v3(c[0], c[1], c[2], c[3]).0,
            3 => geom::normal_tri_v3(c[0], c[1], c[2]).0,
            _ => geom::normal_poly_newell(&c).0,
        }
    }
    /// `BM_face_normal_update`.
    pub fn face_normal_update(&mut self, f: FaceId) {
        let n = self.face_calc_normal(f);
        self.face_mut(f).no = n;
    }
    /// `BM_face_calc_center_median`.
    pub fn face_calc_center_median(&self, f: FaceId) -> [f32; 3] {
        let c = self.loop_cos(f);
        let mut r = [0.0f32; 3];
        for p in &c {
            r = geom::add(r, *p);
        }
        geom::scale(r, 1.0 / c.len() as f32)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc
    // (bm_vert_calc_normals_impl).
    /// `BM_vert_normal_update` with the angle weighting of `BM_mesh_normals_update` (face
    /// normals must be current).
    pub fn vert_normal_update(&mut self, v: VertId) {
        let mut v_no = [0.0f32; 3];
        let vco = self.vert(v).co;
        if let Some(e_first) = self.vert(v).e {
            let mut e = e_first;
            loop {
                if let Some(l_first) = self.edge(e).l {
                    let ed = self.edge(e);
                    let (e2diff, _) =
                        geom::normalize_v3(geom::sub(self.vert(ed.v[0]).co, self.vert(ed.v[1]).co));
                    let mut l = l_first;
                    loop {
                        let ll = self.lp(l);
                        if ll.v == v {
                            let lp = self.lp(ll.prev);
                            let ep = self.edge(lp.e);
                            let (e1diff, _) = geom::normalize_v3(geom::sub(
                                self.vert(ep.v[0]).co,
                                self.vert(ep.v[1]).co,
                            ));
                            let mut dotprod = geom::dot(e1diff, e2diff);
                            if (ep.v[0] == lp.v) ^ (self.edge(ll.e).v[0] == ll.v) {
                                dotprod = -dotprod;
                            }
                            let fac = geom::safe_acos_approx(-dotprod);
                            geom::madd_v3_v3fl(&mut v_no, self.face(ll.f).no, fac);
                        }
                        l = ll.radial_next;
                        if l == l_first {
                            break;
                        }
                    }
                }
                e = self.disk_edge_next(e, v);
                if e == e_first {
                    break;
                }
            }
            let (n, len) = geom::normalize_v3(v_no);
            if len != 0.0 {
                self.vert_mut(v).no = n;
                return;
            }
        }
        self.vert_mut(v).no = geom::normalize_v3(vco).0;
    }

    /// `BM_mesh_normals_update`: face normals, then angle-weighted vertex normals.
    pub fn normals_update(&mut self) {
        for f in self.face_ids() {
            frap_math::budget::check();
            self.face_normal_update(f);
        }
        for v in self.vert_ids() {
            frap_math::budget::check();
            self.vert_normal_update(v);
        }
    }
}
