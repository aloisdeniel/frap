// SPDX-License-Identifier: GPL-3.0-or-later
//! Kernel Euler operators with Blender's names, so ports are 1:1 and creation order matches
//! (`bmesh/intern/bmesh_core.cc`, `bmesh_structure.cc`, `bmesh_construct.cc`, `bmesh_mods.cc`,
//! `bmesh_delete.cc`, `bmesh_operators.cc`; ARCHITECTURE §6.2). CONTRACT frozen by M2 W0,
//! implemented by WP 2.1. Every function keeps the disk and radial cycles valid (`validate.rs`)
//! and allocates and frees in Blender's order.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_core.cc (Blender 5.2.4) unless noted.
//! Pointer links become pool slots; Blender's "null" disk/radial links of a detached element
//! are represented by self-links (they are always overwritten before being read).

use smallvec::SmallVec;

use crate::types::{
    BMEdge, BMElem, BMFace, BMLoop, BMVert, BMesh, CreateFlag, DiskLink, EdgeId, ElemFlag,
    ElemTypes, FaceId, LoopId, OpTypeFlag, VertId,
};

/// `DEL_*` contexts of `bmesh.ops.delete` / `BMO_mesh_delete_oflag_context`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DeleteContext {
    /// `VERTS` (`DEL_VERTS`).
    Verts,
    /// `EDGES` (`DEL_EDGES`).
    Edges,
    /// `FACES_ONLY` (`DEL_ONLYFACES`).
    FacesOnly,
    /// `EDGES_FACES` (`DEL_EDGESFACES`).
    EdgesFaces,
    /// `FACES` (`DEL_FACES`).
    Faces,
    /// `FACES_KEEP_BOUNDARY` (`DEL_FACES_KEEP_BOUNDARY`).
    FacesKeepBoundary,
    /// `TAGGED_ONLY` (`DEL_ONLYTAGGED`).
    TaggedOnly,
}

impl DeleteContext {
    /// Parses Blender's enum identifier.
    pub fn from_identifier(s: &str) -> Option<DeleteContext> {
        Some(match s {
            "VERTS" => DeleteContext::Verts,
            "EDGES" => DeleteContext::Edges,
            "FACES_ONLY" => DeleteContext::FacesOnly,
            "EDGES_FACES" => DeleteContext::EdgesFaces,
            "FACES" => DeleteContext::Faces,
            "FACES_KEEP_BOUNDARY" => DeleteContext::FacesKeepBoundary,
            "TAGGED_ONLY" => DeleteContext::TaggedOnly,
            _ => return None,
        })
    }
}

/// Tool-flag bit used by [`BMesh::delete_context`] (`DEL_INPUT`).
const DEL_INPUT: u16 = 1;

impl BMesh {
    // ================================================================ low-level allocation

    fn alloc_vert(&mut self, co: [f32; 3]) -> VertId {
        let s = self.verts.alloc(BMVert {
            co,
            no: [0.0; 3],
            e: None,
            flag: ElemFlag::empty(),
        });
        let i = s as usize;
        self.vdata.ensure_slots(i + 1);
        set_index(&mut self.index.vert, i, -1);
        self.oflags.clear_slot(0, i);
        for sl in &mut self.shape_layers {
            if sl.co.len() <= i {
                sl.co.resize(i + 1, [0.0; 3]);
            }
        }
        self.index.dirty |= ElemTypes::VERT;
        self.lnor_spacearr.dirty_all = true;
        VertId(s)
    }
    fn alloc_edge(&mut self, v1: VertId, v2: VertId) -> EdgeId {
        let next = EdgeId(self.edges.next_slot());
        let link = DiskLink { next, prev: next };
        let s = self.edges.alloc(BMEdge {
            v: [v1, v2],
            l: None,
            disk: [link, link],
            flag: ElemFlag::SMOOTH,
        });
        let i = s as usize;
        self.edata.ensure_slots(i + 1);
        set_index(&mut self.index.edge, i, -1);
        self.oflags.clear_slot(1, i);
        self.index.dirty |= ElemTypes::EDGE;
        self.lnor_spacearr.dirty_all = true;
        EdgeId(s)
    }
    fn alloc_loop(&mut self, v: VertId, e: EdgeId, f: FaceId) -> LoopId {
        // `--max-elements` backstop (ARCHITECTURE §12): growth no operator pre-check estimated
        // stops at the limit (rolled back, `LIMIT_EXCEEDED`) instead of exhausting memory.
        frap_math::budget::check_corners(self.loops.live() as u64 + 1);
        let me = LoopId(self.loops.next_slot());
        let s = self.loops.alloc(BMLoop {
            v,
            e,
            f,
            next: me,
            prev: me,
            radial_next: me,
            radial_prev: me,
            flag: ElemFlag::empty(),
        });
        let i = s as usize;
        self.ldata.ensure_slots(i + 1);
        set_index(&mut self.index.loops, i, -1);
        self.index.dirty |= ElemTypes::LOOP;
        self.lnor_spacearr.dirty_all = true;
        LoopId(s)
    }
    /// `bm_face_create__internal`.
    fn alloc_face(&mut self) -> FaceId {
        let s = self.faces.alloc(BMFace {
            l_first: LoopId(u32::MAX),
            len: 0,
            no: [0.0; 3],
            mat_nr: 0,
            flag: ElemFlag::empty(),
        });
        let i = s as usize;
        self.pdata.ensure_slots(i + 1);
        set_index(&mut self.index.face, i, -1);
        self.oflags.clear_slot(2, i);
        self.index.dirty |= ElemTypes::FACE;
        self.lnor_spacearr.dirty_all = true;
        FaceId(s)
    }

    /// `bm_kill_only_vert`.
    fn kill_only_vert(&mut self, v: VertId) {
        self.select_history_remove(BMElem::Vert(v));
        self.verts.free(v.0);
        self.index.dirty |= ElemTypes::VERT;
        self.lnor_spacearr.dirty_all = true;
    }
    /// `bm_kill_only_edge`.
    fn kill_only_edge(&mut self, e: EdgeId) {
        self.select_history_remove(BMElem::Edge(e));
        self.edges.free(e.0);
        self.index.dirty |= ElemTypes::EDGE;
        self.lnor_spacearr.dirty_all = true;
    }
    /// `bm_kill_only_face`.
    fn kill_only_face(&mut self, f: FaceId) {
        if self.act_face == Some(f) {
            self.act_face = None;
        }
        self.select_history_remove(BMElem::Face(f));
        self.faces.free(f.0);
        self.index.dirty |= ElemTypes::FACE;
        self.lnor_spacearr.dirty_all = true;
    }
    /// `bm_kill_only_loop`.
    fn kill_only_loop(&mut self, l: LoopId) {
        self.loops.free(l.0);
        self.index.dirty |= ElemTypes::LOOP;
        self.lnor_spacearr.dirty_all = true;
    }

    // ================================================================ disk / radial cycles
    // Ported from blender/source/blender/bmesh/intern/bmesh_structure.cc.

    fn disk_link_mut(&mut self, e: EdgeId, v: VertId) -> &mut DiskLink {
        let ed = self.edge_mut(e);
        let i = (ed.v[1] == v) as usize;
        &mut ed.disk[i]
    }
    fn disk_link(&self, e: EdgeId, v: VertId) -> DiskLink {
        let ed = self.edge(e);
        ed.disk[(ed.v[1] == v) as usize]
    }

    /// `bmesh_disk_edge_append`: inserts `e` before `v->e` in the disk cycle of `v`.
    pub(crate) fn disk_edge_append(&mut self, e: EdgeId, v: VertId) {
        match self.vert(v).e {
            None => {
                *self.disk_link_mut(e, v) = DiskLink { next: e, prev: e };
                self.vert_mut(v).e = Some(e);
            }
            Some(ve) => {
                let dl2 = self.disk_link(ve, v);
                *self.disk_link_mut(e, v) = DiskLink {
                    next: ve,
                    prev: dl2.prev,
                };
                self.disk_link_mut(ve, v).prev = e;
                // `dl3`: the old previous edge (when `ve` was alone this is `ve` itself).
                self.disk_link_mut(dl2.prev, v).next = e;
            }
        }
    }
    /// `bmesh_disk_edge_remove`.
    pub(crate) fn disk_edge_remove(&mut self, e: EdgeId, v: VertId) {
        let dl1 = self.disk_link(e, v);
        if dl1.prev != e {
            self.disk_link_mut(dl1.prev, v).next = dl1.next;
        }
        if dl1.next != e {
            self.disk_link_mut(dl1.next, v).prev = dl1.prev;
        }
        if self.vert(v).e == Some(e) {
            self.vert_mut(v).e = if e != dl1.next { Some(dl1.next) } else { None };
        }
        *self.disk_link_mut(e, v) = DiskLink { next: e, prev: e };
    }
    /// `bmesh_disk_vert_swap`.
    fn disk_vert_swap(&mut self, e: EdgeId, v_dst: VertId, v_src: VertId) {
        let ed = self.edge_mut(e);
        let i = if ed.v[0] == v_src {
            0
        } else if ed.v[1] == v_src {
            1
        } else {
            debug_assert!(false, "disk_vert_swap: vertex not in edge");
            return;
        };
        ed.v[i] = v_dst;
        ed.disk[i] = DiskLink { next: e, prev: e };
    }
    /// `bmesh_disk_vert_replace`.
    pub(crate) fn disk_vert_replace(&mut self, e: EdgeId, v_dst: VertId, v_src: VertId) {
        self.disk_edge_remove(e, v_src);
        self.disk_vert_swap(e, v_dst, v_src);
        self.disk_edge_append(e, v_dst);
    }
    /// `bmesh_edge_vert_swap`: moves `e` (and its loops) from `v_src` to `v_dst`.
    pub(crate) fn edge_vert_swap(&mut self, e: EdgeId, v_dst: VertId, v_src: VertId) {
        if let Some(first) = self.edge(e).l {
            let mut l = first;
            loop {
                let (lv, next) = (self.lp(l).v, self.lp(l).next);
                if lv == v_src {
                    self.lp_mut(l).v = v_dst;
                } else if self.lp(next).v == v_src {
                    self.lp_mut(next).v = v_dst;
                }
                l = self.lp(l).radial_next;
                if l == first {
                    break;
                }
            }
        }
        self.disk_vert_replace(e, v_dst, v_src);
    }
    /// `bmesh_radial_loop_append`: `l` becomes `e->l`, inserted after the old `e->l`.
    pub(crate) fn radial_loop_append(&mut self, e: EdgeId, l: LoopId) {
        match self.edge(e).l {
            None => {
                let ll = self.lp_mut(l);
                ll.radial_next = l;
                ll.radial_prev = l;
            }
            Some(el) => {
                let el_next = self.lp(el).radial_next;
                {
                    let ll = self.lp_mut(l);
                    ll.radial_prev = el;
                    ll.radial_next = el_next;
                }
                self.lp_mut(el_next).radial_prev = l;
                self.lp_mut(el).radial_next = l;
            }
        }
        self.edge_mut(e).l = Some(l);
        self.lp_mut(l).e = e;
    }
    /// `bmesh_radial_loop_remove`.
    pub(crate) fn radial_loop_remove(&mut self, e: EdgeId, l: LoopId) {
        let (rn, rp) = (self.lp(l).radial_next, self.lp(l).radial_prev);
        if rn != l {
            if self.edge(e).l == Some(l) {
                self.edge_mut(e).l = Some(rn);
            }
            self.lp_mut(rn).radial_prev = rp;
            self.lp_mut(rp).radial_next = rn;
        } else if self.edge(e).l == Some(l) {
            self.edge_mut(e).l = None;
        }
        let ll = self.lp_mut(l);
        ll.radial_next = l;
        ll.radial_prev = l;
    }
    /// `bmesh_radial_loop_unlink`: detaches `l` from its radial cycle without touching `e->l`.
    fn radial_loop_unlink(&mut self, l: LoopId) {
        let (rn, rp) = (self.lp(l).radial_next, self.lp(l).radial_prev);
        if rn != l {
            self.lp_mut(rn).radial_prev = rp;
            self.lp_mut(rp).radial_next = rn;
        }
        let ll = self.lp_mut(l);
        ll.radial_next = l;
        ll.radial_prev = l;
    }

    // ================================================================ creation

    /// `BM_vert_create`: a vertex at `co`, custom data copied from `example` (else defaults).
    pub fn vert_create(
        &mut self,
        co: [f32; 3],
        example: Option<VertId>,
        flag: CreateFlag,
    ) -> VertId {
        debug_assert!(!flag.contains(CreateFlag::NO_DOUBLE));
        let v = self.alloc_vert(co);
        if !flag.contains(CreateFlag::SKIP_CD) {
            if let Some(ex) = example {
                // Handles `v->no` too.
                self.vert_attrs_copy(ex, v);
                self.shape_copy(Some(ex), v);
            } else {
                self.vdata.set_default(v.0);
                self.shape_copy(None, v);
                self.vert_mut(v).no = [0.0; 3];
            }
        } else {
            let no = example.map(|ex| self.vert(ex).no).unwrap_or([0.0; 3]);
            self.vert_mut(v).no = no;
        }
        v
    }

    /// `BM_edge_create`: an edge `v1`–`v2` (the existing one with `NO_DOUBLE`).
    pub fn edge_create(
        &mut self,
        v1: VertId,
        v2: VertId,
        example: Option<EdgeId>,
        flag: CreateFlag,
    ) -> EdgeId {
        debug_assert_ne!(v1, v2);
        if flag.contains(CreateFlag::NO_DOUBLE)
            && let Some(e) = self.edge_exists(v1, v2)
        {
            return e;
        }
        let e = self.alloc_edge(v1, v2);
        self.disk_edge_append(e, v1);
        self.disk_edge_append(e, v2);
        if !flag.contains(CreateFlag::SKIP_CD) {
            if let Some(ex) = example {
                self.edge_attrs_copy(ex, e);
            } else {
                self.edata.set_default(e.0);
            }
        }
        e
    }

    /// `bm_loop_create`: custom data from `example` (no flags), else defaults.
    fn loop_create(
        &mut self,
        v: VertId,
        e: EdgeId,
        f: FaceId,
        example: Option<LoopId>,
        flag: CreateFlag,
    ) -> LoopId {
        let l = self.alloc_loop(v, e, f);
        if !flag.contains(CreateFlag::SKIP_CD) {
            match example {
                Some(ex) => self.ldata.copy_elem(ex.0, l.0),
                None => self.ldata.set_default(l.0),
            }
        }
        l
    }

    /// `BM_face_create`: a face over `verts` with the given `edges` (`edges[i]` joins
    /// `verts[i]` and `verts[i + 1]`); `None` when `verts` is empty.
    pub fn face_create(
        &mut self,
        verts: &[VertId],
        edges: &[EdgeId],
        example: Option<FaceId>,
        flag: CreateFlag,
    ) -> Option<FaceId> {
        let len = verts.len();
        if len == 0 || edges.len() < len {
            return None;
        }
        if flag.contains(CreateFlag::NO_DOUBLE)
            && let Some(f) = self.face_exists(verts)
        {
            return Some(f);
        }
        let f = self.alloc_face();
        // `bm_face_boundary_add`.
        let startl = self.loop_create(verts[0], edges[0], f, None, flag);
        self.radial_loop_append(edges[0], startl);
        self.face_mut(f).l_first = startl;
        let mut lastl = startl;
        for i in 1..len {
            let l = self.loop_create(verts[i], edges[i], f, None, flag);
            self.radial_loop_append(edges[i], l);
            self.lp_mut(l).prev = lastl;
            self.lp_mut(lastl).next = l;
            lastl = l;
        }
        self.lp_mut(startl).prev = lastl;
        self.lp_mut(lastl).next = startl;
        self.face_mut(f).len = len as u32;
        if !flag.contains(CreateFlag::SKIP_CD) {
            if let Some(ex) = example {
                self.face_attrs_copy(ex, f);
            } else {
                self.pdata.set_default(f.0);
                self.face_mut(f).no = [0.0; 3];
            }
        } else {
            let no = example.map(|ex| self.face(ex).no).unwrap_or([0.0; 3]);
            self.face_mut(f).no = no;
        }
        Some(f)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc
    // (BM_edges_from_verts_ensure / BM_edges_from_verts).
    /// `BM_face_create_verts`: a face over `verts`, creating missing edges when
    /// `create_edges` (in `(v[n-1], v[0]), (v[0], v[1]), …` order).
    pub fn face_create_verts(
        &mut self,
        verts: &[VertId],
        example: Option<FaceId>,
        flag: CreateFlag,
        create_edges: bool,
    ) -> Option<FaceId> {
        let len = verts.len();
        if len == 0 {
            return None;
        }
        let mut edges = vec![EdgeId(0); len];
        let mut i_prev = len - 1;
        for i in 0..len {
            edges[i_prev] = if create_edges {
                self.edge_create(verts[i_prev], verts[i], None, CreateFlag::NO_DOUBLE)
            } else {
                self.edge_exists(verts[i_prev], verts[i])?
            };
            i_prev = i;
        }
        self.face_create(verts, &edges, example, flag)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc
    // (bm_edges_sort_winding, BM_face_create_ngon).
    /// `BM_face_create_ngon`: a face over an unordered edge loop, wound so that it starts with
    /// `v1` → `v2`; `None` when the edges do not form one simple loop.
    pub fn face_create_ngon(
        &mut self,
        v1: VertId,
        v2: VertId,
        edges: &[EdgeId],
        example: Option<FaceId>,
        flag: CreateFlag,
    ) -> Option<FaceId> {
        let len = edges.len();
        let mut mf: rustc_hash::FxHashSet<EdgeId> = edges.iter().copied().collect();
        let mut mv: rustc_hash::FxHashSet<VertId> =
            edges.iter().flat_map(|&e| self.edge(e).v).collect();
        let mut v_iter = v1;
        let mut e_first = self.vert(v1).e?;
        let mut e_iter = e_first;
        let mut found = false;
        loop {
            if mf.contains(&e_iter) && self.edge_other_vert(e_iter, v_iter) == v2 {
                found = true;
                break;
            }
            e_iter = self.disk_edge_next(e_iter, v_iter);
            if e_iter == e_first {
                break;
            }
        }
        if !found {
            return None;
        }
        let mut edges_sort = Vec::with_capacity(len);
        let mut verts_sort = Vec::with_capacity(len);
        loop {
            if mf.contains(&e_iter) {
                if !mv.contains(&v_iter) {
                    return None;
                }
                mf.remove(&e_iter);
                edges_sort.push(e_iter);
                mv.remove(&v_iter);
                verts_sort.push(v_iter);
                v_iter = self.edge_other_vert(e_iter, v_iter);
                if edges_sort.len() == len {
                    if v_iter != verts_sort[0] {
                        return None;
                    }
                    break;
                }
                e_first = e_iter;
            }
            e_iter = self.disk_edge_next(e_iter, v_iter);
            if e_iter == e_first {
                break;
            }
        }
        if edges_sort.len() != len {
            return None;
        }
        self.face_create(&verts_sort, &edges_sort, example, flag)
    }

    // ================================================================ killing

    /// `BM_vert_kill`: the vertex and every edge and face using it.
    pub fn vert_kill(&mut self, v: VertId) {
        while let Some(e) = self.vert(v).e {
            self.edge_kill(e);
        }
        self.kill_only_vert(v);
    }
    /// `BM_edge_kill`: the edge and every face using it.
    pub fn edge_kill(&mut self, e: EdgeId) {
        while let Some(l) = self.edge(e).l {
            let f = self.lp(l).f;
            self.face_kill(f);
        }
        let [v1, v2] = self.edge(e).v;
        self.disk_edge_remove(e, v1);
        self.disk_edge_remove(e, v2);
        self.kill_only_edge(e);
    }
    /// `BM_face_kill`: the face and its loops (edges and vertices stay).
    pub fn face_kill(&mut self, f: FaceId) {
        let first = self.face(f).l_first;
        if self.loops.contains(first.0) {
            let mut l = first;
            loop {
                let next = self.lp(l).next;
                let e = self.lp(l).e;
                self.radial_loop_remove(e, l);
                self.kill_only_loop(l);
                l = next;
                if l == first {
                    break;
                }
            }
        }
        self.kill_only_face(f);
    }
    /// `BM_face_kill_loose`: the face, and the edges and vertices only it used.
    pub fn face_kill_loose(&mut self, f: FaceId) {
        let first = self.face(f).l_first;
        let mut l = first;
        loop {
            let next = self.lp(l).next;
            let e = self.lp(l).e;
            self.radial_loop_remove(e, l);
            self.kill_only_loop(l);
            if self.edge(e).l.is_none() {
                let [v1, v2] = self.edge(e).v;
                self.disk_edge_remove(e, v1);
                self.disk_edge_remove(e, v2);
                self.kill_only_edge(e);
                if self.vert(v1).e.is_none() {
                    self.kill_only_vert(v1);
                }
                if self.vert(v2).e.is_none() {
                    self.kill_only_vert(v2);
                }
            }
            l = next;
            if l == first {
                break;
            }
        }
        self.kill_only_face(f);
    }
    /// `BM_face_edges_kill`: kills every edge of `f` (and so `f` and its neighbours).
    pub fn face_edges_kill(&mut self, f: FaceId) {
        for e in self.face_edges(f) {
            if self.edges.contains(e.0) {
                self.edge_kill(e);
            }
        }
    }
    /// `BM_face_verts_kill`: kills every vertex of `f`.
    pub fn face_verts_kill(&mut self, f: FaceId) {
        for v in self.face_verts(f) {
            if self.verts.contains(v.0) {
                self.vert_kill(v);
            }
        }
    }

    // ================================================================ Euler operators

    /// `bmesh_kernel_split_edge_make_vert` (SEMV): splits `e` next to `tv`; returns the new
    /// vertex and the new edge (`tv`–new vertex; `e` now joins the new vertex and the other
    /// end).
    pub fn split_edge_make_vert(&mut self, tv: VertId, e: EdgeId) -> (VertId, EdgeId) {
        debug_assert!(self.vert_in_edge(e, tv));
        let tv_co = self.vert(tv).co;
        // Order of `e_new` verts matches `e` (so extruded faces don't flip).
        let v_new = self.vert_create(tv_co, Some(tv), CreateFlag::empty());
        let e_new = self.edge_create(tv, v_new, Some(e), CreateFlag::empty());

        self.disk_edge_remove(e_new, tv);
        self.disk_edge_remove(e_new, v_new);
        self.disk_vert_replace(e, v_new, tv);
        self.disk_edge_append(e_new, v_new);
        self.disk_edge_append(e_new, tv);

        // Split the radial cycle if present.
        let mut l_next = self.edge(e).l;
        self.edge_mut(e).l = None;
        while let Some(l) = l_next {
            let f = self.lp(l).f;
            self.face_mut(f).len += 1;
            let rn = self.lp(l).radial_next;
            l_next = if rn != l { Some(rn) } else { None };
            self.radial_loop_unlink(l);

            let l_new = self.alloc_loop(v_new, e, f);
            self.ldata.copy_elem(l.0, l_new.0);
            let l_nx = self.lp(l).next;
            {
                let ln = self.lp_mut(l_new);
                ln.prev = l;
                ln.next = l_nx;
            }
            self.lp_mut(l).next = l_new;
            self.lp_mut(l_nx).prev = l_new;

            let nv = self.lp(l_nx).v;
            if self.verts_in_edge(v_new, nv, e) {
                self.lp_mut(l_new).e = e;
                self.lp_mut(l).e = e_new;
                self.radial_loop_append(e, l_new);
                self.radial_loop_append(e_new, l);
            } else if self.verts_in_edge(v_new, nv, e_new) {
                self.lp_mut(l_new).e = e_new;
                self.lp_mut(l).e = e;
                self.radial_loop_append(e_new, l_new);
                self.radial_loop_append(e, l);
            }
        }
        (v_new, e_new)
    }

    /// `bmesh_kernel_join_edge_kill_vert` (JEKV): removes `v_kill` (valence 2) and `e_kill`,
    /// joining the other edge over them; returns the surviving edge.
    pub fn join_edge_kill_vert(
        &mut self,
        e_kill: EdgeId,
        v_kill: VertId,
        do_del: bool,
        check_edge_exists: bool,
        kill_degenerate_faces: bool,
        kill_duplicate_faces: bool,
    ) -> Option<EdgeId> {
        if !self.vert_in_edge(e_kill, v_kill) {
            return None;
        }
        if self.vert_edge_count_at_most(v_kill, 3) != 2 {
            return None;
        }
        let e_old = self.disk_edge_next(e_kill, v_kill);
        let v_target = self.edge_other_vert(e_kill, v_kill);
        let v_old = self.edge_other_vert(e_old, v_kill);
        // Check for double edges.
        if self.verts_in_edge(v_kill, v_target, e_old) {
            return None;
        }
        let mut faces_degenerate: SmallVec<[FaceId; 8]> = SmallVec::new();
        let mut faces_duplicate: SmallVec<[FaceId; 8]> = SmallVec::new();
        let e_splice = if check_edge_exists {
            self.edge_exists(v_target, v_old)
        } else {
            None
        };
        self.disk_vert_replace(e_old, v_target, v_kill);
        self.disk_edge_remove(e_kill, v_target);
        if let Some(first) = self.edge(e_kill).l {
            let mut l_kill = first;
            loop {
                let lk = self.lp(l_kill).clone();
                if self.lp(lk.next).v == v_kill {
                    self.lp_mut(lk.next).v = v_target;
                }
                self.lp_mut(lk.next).prev = lk.prev;
                self.lp_mut(lk.prev).next = lk.next;
                if self.face(lk.f).l_first == l_kill {
                    self.face_mut(lk.f).l_first = lk.next;
                }
                self.face_mut(lk.f).len -= 1;
                if kill_degenerate_faces && self.face(lk.f).len < 3 {
                    faces_degenerate.push(lk.f);
                } else if kill_duplicate_faces {
                    faces_duplicate.push(lk.f);
                }
                let l_kill_next = lk.radial_next;
                self.kill_only_loop(l_kill);
                l_kill = l_kill_next;
                if l_kill == first {
                    break;
                }
            }
        }
        self.kill_only_edge(e_kill);
        if do_del {
            self.kill_only_vert(v_kill);
        } else {
            self.vert_mut(v_kill).e = None;
        }
        if let Some(es) = e_splice {
            self.edge_splice(e_old, es);
        }
        // BLI_SMALLSTACK_POP: last pushed first.
        while let Some(f) = faces_degenerate.pop() {
            if self.faces.contains(f.0) {
                self.face_kill(f);
            }
        }
        while let Some(f) = faces_duplicate.pop() {
            if self.faces.contains(f.0) && self.face_find_double(f).is_some() {
                self.face_kill(f);
            }
        }
        Some(e_old)
    }

    /// `bmesh_kernel_join_vert_kill_edge` (JVKE, `BM_edge_collapse`): collapses `e_kill`,
    /// merging `v_kill` into the other vertex; returns the surviving vertex.
    pub fn join_vert_kill_edge(
        &mut self,
        e_kill: EdgeId,
        v_kill: VertId,
        do_del: bool,
        check_edge_exists: bool,
        kill_degenerate_faces: bool,
    ) -> VertId {
        let mut faces_degenerate: SmallVec<[FaceId; 8]> = SmallVec::new();
        let v_target = self.edge_other_vert(e_kill, v_kill);
        if let Some(first) = self.edge(e_kill).l {
            let mut l_kill = first;
            loop {
                let lk = self.lp(l_kill).clone();
                if self.lp(lk.next).v == v_kill {
                    self.lp_mut(lk.next).v = v_target;
                }
                self.lp_mut(lk.next).prev = lk.prev;
                self.lp_mut(lk.prev).next = lk.next;
                if self.face(lk.f).l_first == l_kill {
                    self.face_mut(lk.f).l_first = lk.next;
                }
                self.face_mut(lk.f).len -= 1;
                if kill_degenerate_faces && self.face(lk.f).len < 3 {
                    faces_degenerate.push(lk.f);
                }
                let l_kill_next = lk.radial_next;
                self.kill_only_loop(l_kill);
                l_kill = l_kill_next;
                if l_kill == first {
                    break;
                }
            }
            self.edge_mut(e_kill).l = None;
        }
        self.edge_kill(e_kill);
        // Blender only moves the edges when `v_target` still has some (`v_target->e &&
        // v_kill->e`) and asserts `v_kill->e == nullptr` before freeing it; when `e_kill` was
        // `v_target`'s only edge that would free a vertex still in use, so frap moves the
        // edges whenever `v_kill` has any (identical result in every case Blender supports).
        if self.vert(v_kill).e.is_some() {
            while let Some(e) = self.vert(v_kill).e {
                let e_target = if check_edge_exists {
                    let o = self.edge_other_vert(e, v_kill);
                    self.edge_exists(v_target, o)
                } else {
                    None
                };
                self.edge_vert_swap(e, v_target, v_kill);
                if let Some(et) = e_target {
                    self.edge_splice(et, e);
                }
            }
        }
        while let Some(f) = faces_degenerate.pop() {
            if self.faces.contains(f.0) {
                self.face_kill(f);
            }
        }
        if do_del {
            self.kill_only_vert(v_kill);
        }
        v_target
    }

    /// `bmesh_kernel_split_face_make_edge` (SFME): splits `f` between the corners `l_v1` and
    /// `l_v2`; returns the new face and its loop on the new edge.
    pub fn split_face_make_edge(
        &mut self,
        f: FaceId,
        l_v1: LoopId,
        l_v2: LoopId,
        example: Option<EdgeId>,
        no_double: bool,
    ) -> Option<(FaceId, LoopId)> {
        if self.lp(l_v1).f != f || self.lp(l_v2).f != f {
            return None;
        }
        let (v1, v2) = (self.lp(l_v1).v, self.lp(l_v2).v);
        let e = self.edge_create(
            v1,
            v2,
            example,
            if no_double {
                CreateFlag::NO_DOUBLE
            } else {
                CreateFlag::empty()
            },
        );
        // `bm_face_create__sfme`.
        let f2 = self.alloc_face();
        self.face_attrs_copy(f, f2);
        let l_f1 = self.loop_create(v2, e, f, Some(l_v2), CreateFlag::empty());
        let l_f2 = self.loop_create(v1, e, f2, Some(l_v1), CreateFlag::empty());

        let (v2_prev, v1_prev) = (self.lp(l_v2).prev, self.lp(l_v1).prev);
        self.lp_mut(l_f1).prev = v2_prev;
        self.lp_mut(l_f2).prev = v1_prev;
        self.lp_mut(v2_prev).next = l_f1;
        self.lp_mut(v1_prev).next = l_f2;
        self.lp_mut(l_f1).next = l_v1;
        self.lp_mut(l_f2).next = l_v2;
        self.lp_mut(l_v1).prev = l_f1;
        self.lp_mut(l_v2).prev = l_f2;

        // Find which of the faces the original first loop is in.
        let f_first = self.face(f).l_first;
        let mut first_loop_f1 = false;
        // The walks below are linear in the face size: `--stmt-timeout` cancellation points
        // (cutting an n-gon n times is quadratic, e.g. `mesh.loopcut(number_cuts=10000)`).
        let mut it = l_f1;
        loop {
            frap_math::budget::check();
            if it == f_first {
                first_loop_f1 = true;
            }
            it = self.lp(it).next;
            if it == l_f1 {
                break;
            }
        }
        if first_loop_f1 {
            let ff = self.lp(f_first).clone();
            self.face_mut(f2).l_first = if ff.prev == l_f1 {
                self.lp(l_f2).prev
            } else if ff.next == l_f1 {
                self.lp(l_f2).next
            } else {
                l_f2
            };
        } else {
            self.face_mut(f2).l_first = f_first;
            let ff = self.lp(f_first).clone();
            self.face_mut(f).l_first = if ff.prev == l_f2 {
                self.lp(l_f1).prev
            } else if ff.next == l_f2 {
                self.lp(l_f1).next
            } else {
                l_f1
            };
        }
        // Make f2's loops point to it.
        let first2 = self.face(f2).l_first;
        let mut f2len = 0;
        let mut it = first2;
        loop {
            frap_math::budget::check();
            self.lp_mut(it).f = f2;
            f2len += 1;
            it = self.lp(it).next;
            if it == first2 {
                break;
            }
        }
        self.radial_loop_append(e, l_f1);
        self.radial_loop_append(e, l_f2);
        self.face_mut(f2).len = f2len;
        let first1 = self.face(f).l_first;
        let mut f1len = 0;
        let mut it = first1;
        loop {
            frap_math::budget::check();
            f1len += 1;
            it = self.lp(it).next;
            if it == first1 {
                break;
            }
        }
        self.face_mut(f).len = f1len;
        Some((f2, l_f2))
    }

    /// `bmesh_kernel_join_face_kill_edge` (JFKE): joins `f1` and `f2` over `e`; returns the
    /// surviving face (`f1`).
    pub fn join_face_kill_edge(&mut self, f1: FaceId, f2: FaceId, e: EdgeId) -> Option<FaceId> {
        if f1 == f2 || !self.edge_is_manifold(e) {
            return None;
        }
        let l_f1 = self.face_edge_share_loop(f1, e)?;
        let l_f2 = self.face_edge_share_loop(f2, e)?;
        if self.lp(l_f1).v == self.lp(l_f2).v {
            return None;
        }
        let a = self.lp(l_f1).clone();
        let b = self.lp(l_f2).clone();
        if self.edge_in_face(self.lp(a.next).e, f2)
            || self.edge_in_face(self.lp(a.prev).e, f2)
            || self.edge_in_face(self.lp(b.next).e, f1)
            || self.edge_in_face(self.lp(b.prev).e, f1)
        {
            return None;
        }
        if self.face_share_edge_count(f1, f2) > 1 {
            return None;
        }
        // Validate no internal join.
        let f1_verts: rustc_hash::FxHashSet<VertId> = self
            .face_loops(f1)
            .into_iter()
            .filter(|&l| l != l_f1)
            .map(|l| self.lp(l).v)
            .collect();
        if self
            .face_loops(f2)
            .into_iter()
            .any(|l| l != l_f2 && f1_verts.contains(&self.lp(l).v))
        {
            return None;
        }
        // Join the two loops.
        self.lp_mut(a.prev).next = b.next;
        self.lp_mut(b.next).prev = a.prev;
        self.lp_mut(a.next).prev = b.prev;
        self.lp_mut(b.prev).next = a.next;
        if self.face(f1).l_first == l_f1 {
            self.face_mut(f1).l_first = a.next;
        }
        let f2len = self.face(f2).len;
        self.face_mut(f1).len += f2len - 2;
        let first = self.face(f1).l_first;
        let mut it = first;
        loop {
            self.lp_mut(it).f = f1;
            it = self.lp(it).next;
            if it == first {
                break;
            }
        }
        let [ev1, ev2] = self.edge(e).v;
        self.disk_edge_remove(e, ev1);
        self.disk_edge_remove(e, ev2);
        // Blender frees these directly (`BLI_mempool_free`, no history/active clean-up); frap
        // also drops them from the selection history so no stale id survives.
        self.kill_only_edge(e);
        self.kill_only_loop(l_f1);
        self.kill_only_loop(l_f2);
        self.kill_only_face(f2);
        Some(f1)
    }

    /// `BM_vert_splice`: merges `v_src` into `v_dst` (no shared face allowed).
    pub fn vert_splice(&mut self, v_dst: VertId, v_src: VertId) -> bool {
        if v_src == v_dst {
            return false;
        }
        while let Some(e) = self.vert(v_src).e {
            self.edge_vert_swap(e, v_dst, v_src);
        }
        self.vert_kill(v_src);
        true
    }

    /// `BM_edge_splice`: merges `e_src` into `e_dst` (same vertices).
    pub fn edge_splice(&mut self, e_dst: EdgeId, e_src: EdgeId) -> bool {
        let [a, b] = self.edge(e_dst).v;
        if !self.vert_in_edge(e_src, a) || !self.vert_in_edge(e_src, b) {
            return false;
        }
        while let Some(l) = self.edge(e_src).l {
            self.radial_loop_remove(e_src, l);
            self.radial_loop_append(e_dst, l);
        }
        self.edge_kill(e_src);
        true
    }

    /// `bmesh_kernel_loop_reverse`: reverses the winding of `f`, keeping `l_first`.
    pub fn loop_reverse(&mut self, f: FaceId) {
        let l_first = self.face(f).l_first;
        let lp = self.lp(self.lp(l_first).prev).clone();
        let mut e_prev = lp.e;
        let mut l_prev_radial_next = lp.radial_next;
        let mut l_prev_radial_prev = lp.radial_prev;
        let mut is_prev_boundary = l_prev_radial_next == self.lp(l_prev_radial_next).radial_next;
        let mut l_iter = l_first;
        loop {
            let li = self.lp(l_iter).clone();
            let e_iter = li.e;
            let l_iter_radial_next = li.radial_next;
            let l_iter_radial_prev = li.radial_prev;
            let is_iter_boundary = l_iter_radial_next == self.lp(l_iter_radial_next).radial_next;
            if is_prev_boundary {
                let ll = self.lp_mut(l_iter);
                ll.radial_next = l_iter;
                ll.radial_prev = l_iter;
            } else {
                {
                    let ll = self.lp_mut(l_iter);
                    ll.radial_next = l_prev_radial_next;
                    ll.radial_prev = l_prev_radial_prev;
                }
                self.lp_mut(l_prev_radial_next).radial_prev = l_iter;
                self.lp_mut(l_prev_radial_prev).radial_next = l_iter;
            }
            if self.edge(e_iter).l == Some(l_iter) {
                self.edge_mut(e_iter).l = Some(li.next);
            }
            {
                let ll = self.lp_mut(l_iter);
                ll.e = e_prev;
                std::mem::swap(&mut ll.next, &mut ll.prev);
            }
            e_prev = e_iter;
            l_prev_radial_next = l_iter_radial_next;
            l_prev_radial_prev = l_iter_radial_prev;
            is_prev_boundary = is_iter_boundary;
            // Step to next (now swapped).
            l_iter = self.lp(l_iter).prev;
            if l_iter == l_first {
                break;
            }
        }
        self.index.dirty |= ElemTypes::LOOP;
    }

    /// `BM_face_normal_flip`: reverses the winding, keeping `l_first` ([MESH §4.9]), and
    /// negates the normal.
    pub fn face_normal_flip(&mut self, f: FaceId) {
        self.loop_reverse(f);
        let n = self.face(f).no;
        self.face_mut(f).no = [-n[0], -n[1], -n[2]];
    }

    /// `bmesh_kernel_edge_separate`: moves `l_sep` off `e` onto a new copy of `e`.
    pub fn edge_separate(&mut self, e: EdgeId, l_sep: LoopId, copy_select: bool) {
        if self.edge_is_boundary(e) {
            return;
        }
        if self.edge(e).l == Some(l_sep) {
            self.edge_mut(e).l = Some(self.lp(l_sep).radial_next);
        }
        let [v1, v2] = self.edge(e).v;
        let e_new = self.edge_create(v1, v2, Some(e), CreateFlag::empty());
        self.radial_loop_remove(e, l_sep);
        self.radial_loop_append(e_new, l_sep);
        self.lp_mut(l_sep).e = e_new;
        if copy_select {
            self.elem_select_copy(BMElem::Edge(e_new), BMElem::Edge(e));
        }
    }

    /// `BM_elem_select_copy`: gives `dst` the selection state of `src`.
    pub fn elem_select_copy(&mut self, dst: BMElem, src: BMElem) {
        let s = self.elem_flag(src).contains(ElemFlag::SELECT);
        if self.elem_flag(dst).contains(ElemFlag::SELECT) != s {
            self.select_set(dst, s);
        }
    }

    /// `bmesh_kernel_vert_separate`: one vertex per edge fan of `v`; returns `v` then the new
    /// vertices, most recent first (`BLI_SMALLSTACK_AS_TABLE`).
    fn kernel_vert_separate(&mut self, v: VertId, copy_select: bool) -> Vec<VertId> {
        let mut verts_new: Vec<VertId> = Vec::new();
        let edges_all = self.vert_edges(v);
        let v_edges_num = edges_all.len();
        if v_edges_num > 0 {
            let mut visit: rustc_hash::FxHashSet<EdgeId> = edges_all.iter().copied().collect();
            let mut edges_found = 0;
            loop {
                let mut edges: Vec<EdgeId> = Vec::new();
                let mut edges_search: Vec<EdgeId> = Vec::new();
                let Some(mut e) = self.vert(v).e else {
                    break;
                };
                visit.remove(&e);
                loop {
                    edges.push(e);
                    edges_found += 1;
                    for l in self.edge_loops(e) {
                        let ll = self.lp(l);
                        let l_adj = if ll.v == v { ll.prev } else { ll.next };
                        let ae = self.lp(l_adj).e;
                        if visit.remove(&ae) {
                            edges_search.push(ae);
                        }
                    }
                    match edges_search.pop() {
                        Some(n) => e = n,
                        None => break,
                    }
                }
                if edges_found == v_edges_num {
                    break;
                }
                let co = self.vert(v).co;
                let v_new = self.vert_create(co, Some(v), CreateFlag::empty());
                if copy_select {
                    self.elem_select_copy(BMElem::Vert(v_new), BMElem::Vert(v));
                }
                while let Some(e) = edges.pop() {
                    self.edge_vert_swap(e, v_new, v);
                }
                verts_new.push(v_new);
            }
        }
        let mut out = vec![v];
        out.extend(verts_new.into_iter().rev());
        out
    }

    /// `BM_vert_separate`: splits `v` into one vertex per edge fan, after separating the
    /// edges of `e_in` that have several faces (they are spliced back where possible
    /// afterwards); returns the vertices (the original first, then the new ones, most recent
    /// first).
    pub fn vert_separate(&mut self, v: VertId, e_in: &[EdgeId], copy_select: bool) -> Vec<VertId> {
        let mut edges_separate: Vec<Vec<EdgeId>> = Vec::new();
        for &e in e_in {
            if self.edge_supports_separate(e) {
                let mut edges_orig: Vec<EdgeId> = Vec::new();
                loop {
                    let l_sep = self.edge(e).l.expect("separable edge has faces");
                    self.edge_separate(e, l_sep, copy_select);
                    edges_orig.insert(0, self.lp(l_sep).e);
                    if !self.edge_supports_separate(e) {
                        break;
                    }
                }
                edges_orig.insert(0, e);
                edges_separate.insert(0, edges_orig);
            }
        }
        let out = self.kernel_vert_separate(v, copy_select);
        // `bmesh_kernel_vert_separate__cleanup`.
        for mut group in edges_separate {
            let mut i_orig = 0;
            while i_orig + 1 < group.len() {
                let e_orig = group[i_orig];
                let mut j = i_orig + 1;
                while j < group.len() {
                    let e = group[j];
                    let same = self.edges.contains(e.0)
                        && self.edges.contains(e_orig.0)
                        && self.edge(e).v == self.edge(e_orig).v;
                    if same && self.edge_splice(e_orig, e) {
                        group.remove(j);
                    } else {
                        j += 1;
                    }
                }
                i_orig += 1;
            }
        }
        out
    }

    fn edge_supports_separate(&self, e: EdgeId) -> bool {
        match self.edge(e).l {
            Some(l) => self.lp(l).radial_next != l,
            None => false,
        }
    }

    // ================================================================ mid-level (bmesh_mods)

    // Ported from blender/source/blender/bmesh/intern/bmesh_mods.cc (BM_edge_split).
    /// `BM_edge_split`: splits `e` at `fac` from `v` towards the other end; returns the new
    /// vertex and the new edge (`v`–new vertex). Custom data is interpolated.
    pub fn edge_split(&mut self, e: EdgeId, v: VertId, fac: f32) -> (VertId, EdgeId) {
        let v_other = self.edge_other_vert(e, v);
        let (v_new, e_new) = self.split_edge_make_vert(v, e);
        let (co_o, co_v) = (self.vert(v_other).co, self.vert(v).co);
        let d = [co_o[0] - co_v[0], co_o[1] - co_v[1], co_o[2] - co_v[2]];
        self.vert_mut(v_new).co = [
            co_v[0] + d[0] * fac,
            co_v[1] + d[1] * fac,
            co_v[2] + d[2] * fac,
        ];
        let ef = self.edge(e).flag;
        self.edge_mut(e_new).flag = ef;
        self.edge_attrs_copy(e, e_new);
        self.data_interp_face_vert_edge(v_other, v_new, e, fac);
        self.data_interp_from_verts(v, v_other, v_new, fac);
        (v_new, e_new)
    }

    /// `BM_edge_split_n`: `numcuts` evenly spaced cuts; returns the new vertices from `v1` to
    /// `v2`.
    pub fn edge_split_n(&mut self, e: EdgeId, numcuts: usize) -> Vec<VertId> {
        let mut out = vec![VertId(0); numcuts];
        for i in 0..numcuts {
            let percent = 1.0 / (numcuts + 1 - i) as f32;
            let v2 = self.edge(e).v[1];
            let (v_new, _) = self.edge_split(e, v2, percent);
            out[numcuts - i - 1] = v_new;
        }
        out
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_mods.cc (BM_face_split, no
    // multires).
    /// `BM_face_split`: splits `f` between the non-adjacent corners `l_a` and `l_b`; returns
    /// the new face and its loop on the new edge.
    pub fn face_split(
        &mut self,
        f: FaceId,
        l_a: LoopId,
        l_b: LoopId,
        example: Option<EdgeId>,
        no_double: bool,
    ) -> Option<(FaceId, LoopId)> {
        if l_a == l_b
            || self.lp(l_a).next == l_b
            || self.lp(l_a).prev == l_b
            || self.lp(l_a).f != f
            || self.lp(l_b).f != f
        {
            return None;
        }
        self.split_face_make_edge(f, l_a, l_b, example, no_double)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_core.cc (BM_faces_join, no
    // multires).
    /// `BM_faces_join`: joins a contiguous manifold region of faces into one; with `do_del`
    /// the interior edges and vertices are removed. Returns the new face (or the existing
    /// double it found) and the double when `want_double`.
    pub fn faces_join(
        &mut self,
        faces: &[FaceId],
        do_del: bool,
        want_double: bool,
    ) -> (Option<FaceId>, Option<FaceId>) {
        if faces.is_empty() {
            return (None, None);
        }
        if faces.len() == 1 {
            return (Some(faces[0]), None);
        }
        let had_active_face = self.act_face.is_some();
        let jf: rustc_hash::FxHashSet<FaceId> = faces.iter().copied().collect();
        let mut jf_edges: rustc_hash::FxHashSet<EdgeId> = Default::default();
        let mut jf_verts: rustc_hash::FxHashSet<VertId> = Default::default();
        let mut edges: Vec<EdgeId> = Vec::new();
        let mut deledges: Vec<EdgeId> = Vec::new();
        let mut delverts: Vec<VertId> = Vec::new();
        let (mut v1, mut v2) = (None, None);
        let manifold_flagged = |bm: &BMesh, v: VertId| -> bool {
            let Some(e0) = bm.vert(v).e else {
                return false;
            };
            let mut e = e0;
            loop {
                let Some(l0) = bm.edge(e).l else {
                    return false;
                };
                if bm.edge_is_boundary(e) {
                    return false;
                }
                let mut l = l0;
                loop {
                    if !jf.contains(&bm.lp(l).f) {
                        return false;
                    }
                    l = bm.lp(l).radial_next;
                    if l == l0 {
                        break;
                    }
                }
                e = bm.disk_edge_next(e, v);
                if e == e0 {
                    return true;
                }
            }
        };
        for &f in faces {
            for l in self.face_loops(f) {
                let ll = self.lp(l).clone();
                let rlen = self
                    .edge_loops(ll.e)
                    .into_iter()
                    .filter(|&r| jf.contains(&self.lp(r).f))
                    .count();
                if rlen > 2 {
                    return (None, None);
                }
                if rlen == 1 {
                    edges.push(ll.e);
                    if v1.is_none() {
                        v1 = Some(ll.v);
                        v2 = Some(self.edge_other_vert(ll.e, ll.v));
                    }
                } else if rlen == 2 {
                    let [ev1, ev2] = self.edge(ll.e).v;
                    let d1 = manifold_flagged(self, ev1);
                    let d2 = manifold_flagged(self, ev2);
                    if !d1 && !d2 && !jf_edges.contains(&ll.e) {
                        if self.edge_face_count_at_most(ll.e, 3) != 3 {
                            if do_del {
                                deledges.push(ll.e);
                            }
                            jf_edges.insert(ll.e);
                        }
                    } else {
                        if d1 && jf_verts.insert(ev1) && do_del {
                            delverts.push(ev1);
                        }
                        if d2 && jf_verts.insert(ev2) && do_del {
                            delverts.push(ev2);
                        }
                    }
                }
            }
        }
        let f_new = match (v1, v2) {
            (Some(a), Some(b)) if !edges.is_empty() => {
                self.face_create_ngon(a, b, &edges, Some(faces[0]), CreateFlag::empty())
            }
            _ => None,
        };
        let Some(mut f_new) = f_new else {
            return (None, None);
        };
        let f_existing = self.face_find_double(f_new);
        let mut r_double = None;
        if let Some(fe) = f_existing {
            if want_double {
                r_double = Some(fe);
            } else {
                self.face_kill(f_new);
                f_new = fe;
            }
        }
        let reusing_face = f_existing.is_some() && !want_double;
        if !reusing_face {
            for l in self.face_loops(f_new) {
                let mut l2 = self.lp(l).radial_next;
                while l2 != l {
                    if jf.contains(&self.lp(l2).f) {
                        break;
                    }
                    l2 = self.lp(l2).radial_next;
                }
                if l2 != l {
                    if self.lp(l2).v != self.lp(l).v {
                        l2 = self.lp(l2).next;
                    }
                    self.loop_attrs_copy(l2, l);
                }
            }
        }
        if do_del {
            for e in deledges {
                if self.edges.contains(e.0) {
                    self.edge_kill(e);
                }
            }
            for v in delverts {
                if self.verts.contains(v.0) {
                    self.vert_kill(v);
                }
            }
        } else {
            for &f in faces {
                if self.faces.contains(f.0) {
                    self.face_kill(f);
                }
            }
        }
        if had_active_face && self.act_face.is_none() {
            self.act_face = Some(f_new);
        }
        (Some(f_new), r_double)
    }

    /// `BM_edge_face_count_at_most`.
    pub fn edge_face_count_at_most(&self, e: EdgeId, n: usize) -> usize {
        self.edge_loops(e).len().min(n)
    }
    /// `bmesh_disk_count_at_most`.
    pub fn vert_edge_count_at_most(&self, v: VertId, n: usize) -> usize {
        self.vert_edges(v).len().min(n)
    }

    // ================================================================ indices

    /// `BM_mesh_elem_index_ensure`: dense indices (iteration order) of the given element
    /// types, read with [`BMesh::index_of`]; only stale types are recomputed.
    pub fn elem_index_ensure(&mut self, which: ElemTypes) {
        let needed = self.index.dirty & which;
        if needed.contains(ElemTypes::VERT) {
            let ids = self.vert_ids();
            self.index.vert.resize(self.verts.capacity(), -1);
            for (i, v) in ids.into_iter().enumerate() {
                self.index.vert[v.slot()] = i as i32;
            }
        }
        if needed.contains(ElemTypes::EDGE) {
            let ids = self.edge_ids();
            self.index.edge.resize(self.edges.capacity(), -1);
            for (i, e) in ids.into_iter().enumerate() {
                self.index.edge[e.slot()] = i as i32;
            }
        }
        if needed.intersects(ElemTypes::FACE | ElemTypes::LOOP) {
            let ids = self.face_ids();
            self.index.face.resize(self.faces.capacity(), -1);
            self.index.loops.resize(self.loops.capacity(), -1);
            let mut li = 0i32;
            for (i, f) in ids.into_iter().enumerate() {
                if needed.contains(ElemTypes::FACE) {
                    self.index.face[f.slot()] = i as i32;
                }
                if needed.contains(ElemTypes::LOOP) {
                    for l in self.face_loops(f) {
                        self.index.loops[l.slot()] = li;
                        li += 1;
                    }
                }
            }
        }
        self.index.dirty &= !which;
    }
    /// The dense index (`BM_elem_index_get`) of an element after `elem_index_ensure`
    /// (`usize::MAX` for an unset index).
    pub fn index_of(&self, e: BMElem) -> usize {
        let (col, s) = match e {
            BMElem::Vert(v) => (&self.index.vert, v.slot()),
            BMElem::Edge(x) => (&self.index.edge, x.slot()),
            BMElem::Face(f) => (&self.index.face, f.slot()),
        };
        col.get(s)
            .and_then(|&i| usize::try_from(i).ok())
            .unwrap_or(usize::MAX)
    }
    /// The index of a loop after `elem_index_ensure(LOOP)`.
    pub fn loop_index_of(&self, l: LoopId) -> usize {
        self.index
            .loops
            .get(l.slot())
            .and_then(|&i| usize::try_from(i).ok())
            .unwrap_or(usize::MAX)
    }
    /// `BM_elem_index_set` (scratch use by ops; does not change the dirty flags).
    pub fn index_set(&mut self, e: BMElem, i: i32) {
        let (col, s) = match e {
            BMElem::Vert(v) => (&mut self.index.vert, v.slot()),
            BMElem::Edge(x) => (&mut self.index.edge, x.slot()),
            BMElem::Face(f) => (&mut self.index.face, f.slot()),
        };
        set_index(col, s, i);
    }
    /// The raw `BM_elem_index_get` value (may be stale or -1).
    pub fn index_get(&self, e: BMElem) -> i32 {
        let (col, s) = match e {
            BMElem::Vert(v) => (&self.index.vert, v.slot()),
            BMElem::Edge(x) => (&self.index.edge, x.slot()),
            BMElem::Face(f) => (&self.index.face, f.slot()),
        };
        col.get(s).copied().unwrap_or(-1)
    }

    // ================================================================ delete

    // Ported from blender/source/blender/bmesh/intern/bmesh_delete.cc
    // (BMO_mesh_delete_oflag_context).
    /// `BM_mesh_delete_hflag_context` core (`bmesh_delete.cc`): deletes `geom` with a
    /// `bmesh.ops.delete` context (`VERTS`, `EDGES`, `FACES_ONLY`, `EDGES_FACES`, `FACES`,
    /// `FACES_KEEP_BOUNDARY`, `TAGGED_ONLY`) in Blender's kill order. Unknown contexts do
    /// nothing.
    pub fn delete_context(&mut self, geom: &[BMElem], context: &str) {
        let Some(ctx) = DeleteContext::from_identifier(context) else {
            return;
        };
        let caps = [
            self.verts.capacity(),
            self.edges.capacity(),
            self.faces.capacity(),
        ];
        self.oflags.push(caps);
        for &g in geom {
            if self.elem_alive(g) {
                self.oflags.enable(g, DEL_INPUT);
            }
        }
        self.delete_oflag_context(DEL_INPUT, ctx);
        self.oflags.pop();
    }

    /// `BMO_mesh_delete_oflag_context` on the running operator's flag layer.
    pub fn delete_oflag_context(&mut self, oflag: u16, ctx: DeleteContext) {
        match ctx {
            DeleteContext::Verts => self.remove_tagged_verts(oflag, false),
            DeleteContext::Edges => {
                for e in self.edge_ids() {
                    if self.oflag_test(e, oflag) {
                        let [a, b] = self.edge(e).v;
                        self.oflag_enable(a, oflag);
                        self.oflag_enable(b, oflag);
                    }
                }
                self.remove_tagged_edges(oflag);
                self.remove_tagged_verts(oflag, true);
            }
            DeleteContext::EdgesFaces => self.remove_tagged_edges(oflag),
            DeleteContext::FacesOnly => self.remove_tagged_faces(oflag),
            DeleteContext::TaggedOnly => {
                self.remove_tagged_faces(oflag);
                self.remove_tagged_edges(oflag);
                self.remove_tagged_verts(oflag, false);
            }
            DeleteContext::Faces | DeleteContext::FacesKeepBoundary => {
                for f in self.face_ids() {
                    if self.oflag_test(f, oflag) {
                        for l in self.face_loops(f) {
                            let (v, e) = (self.lp(l).v, self.lp(l).e);
                            self.oflag_enable(v, oflag);
                            self.oflag_enable(e, oflag);
                        }
                    }
                }
                for f in self.face_ids() {
                    if !self.oflag_test(f, oflag) {
                        for l in self.face_loops(f) {
                            let (v, e) = (self.lp(l).v, self.lp(l).e);
                            self.oflag_disable(v, oflag);
                            self.oflag_disable(e, oflag);
                        }
                    }
                }
                for e in self.edge_ids() {
                    if ctx == DeleteContext::FacesKeepBoundary && self.edge_is_boundary(e) {
                        self.oflag_disable(e, oflag);
                    }
                    if !self.oflag_test(e, oflag) {
                        let [a, b] = self.edge(e).v;
                        self.oflag_disable(a, oflag);
                        self.oflag_disable(b, oflag);
                    }
                }
                self.remove_tagged_faces(oflag);
                self.remove_tagged_edges(oflag);
                self.remove_tagged_verts(oflag, false);
            }
        }
    }

    fn remove_tagged_faces(&mut self, oflag: u16) {
        for f in self.face_ids() {
            if self.faces.contains(f.0) && self.oflag_test(f, oflag) {
                frap_math::budget::check();
                self.face_kill(f);
            }
        }
    }
    fn remove_tagged_edges(&mut self, oflag: u16) {
        for e in self.edge_ids() {
            if self.edges.contains(e.0) && self.oflag_test(e, oflag) {
                frap_math::budget::check();
                self.edge_kill(e);
            }
        }
    }
    fn remove_tagged_verts(&mut self, oflag: u16, loose_only: bool) {
        for v in self.vert_ids() {
            if self.verts.contains(v.0)
                && self.oflag_test(v, oflag)
                && (!loose_only || self.vert(v).e.is_none())
            {
                frap_math::budget::check();
                self.vert_kill(v);
            }
        }
    }

    // ================================================================ operator execution

    // Ported from blender/source/blender/bmesh/intern/bmesh_operators.cc (BMO_op_exec) and
    // bmesh_mesh.cc (bmesh_edit_end).
    /// `BMO_op_exec`: runs `exec` with a fresh tool-flag layer; for a top-level operator
    /// (no operator running) it then performs `bmesh_edit_end` with `type_flag`: normals
    /// update, selection flush (keeping the history unless `SELECT_VALIDATE`).
    pub fn bmo_exec<R>(&mut self, type_flag: OpTypeFlag, exec: impl FnOnce(&mut BMesh) -> R) -> R {
        let caps = [
            self.verts.capacity(),
            self.edges.capacity(),
            self.faces.capacity(),
        ];
        self.oflags.push(caps);
        let top = self.oflags.depth() == 1;
        let r = exec(self);
        if top {
            self.bmesh_edit_end(type_flag);
        }
        self.oflags.pop();
        r
    }

    /// `bmesh_edit_end`.
    pub fn bmesh_edit_end(&mut self, type_flag: OpTypeFlag) {
        if type_flag.contains(OpTypeFlag::NORMALS_CALC) {
            self.lnor_spacearr.dirty_all = true;
            self.normals_update();
        }
        let keep = !type_flag.contains(OpTypeFlag::SELECT_VALIDATE);
        let saved = if keep {
            Some(std::mem::take(&mut self.select_history))
        } else {
            None
        };
        if type_flag.contains(OpTypeFlag::SELECT_FLUSH) {
            self.select_flush_mode();
        }
        if let Some(h) = saved {
            self.select_history = h;
        }
        if type_flag.contains(OpTypeFlag::INVALIDATE_CLNOR_ALL) {
            self.lnor_spacearr.dirty_all = true;
        }
    }
}

fn set_index(col: &mut Vec<i32>, i: usize, value: i32) {
    if col.len() <= i {
        col.resize(i + 1, -1);
    }
    col[i] = value;
}
