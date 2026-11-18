// SPDX-License-Identifier: GPL-3.0-or-later
//! Selection: flushing, history, mode conversion (`bmesh/intern/bmesh_marking.cc`,
//! `editors/mesh/editmesh_select.cc`, `editmesh_utils.cc`; ARCHITECTURE §6.2, [MESH §2.2,
//! §3.3]). CONTRACT frozen by M2 W0, implemented by WP 2.1.
//!
//! Ported from Blender 5.2.4. Hidden elements are never (de)selected by these functions, as in
//! Blender. The selected counts (`bm->tot*sel`) are computed on demand instead of being kept
//! as counters.

use crate::types::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, LoopId, SelectMode, VertId};

impl BMesh {
    fn sel(&self, e: impl Into<BMElem>) -> bool {
        self.elem_flag(e.into()).contains(ElemFlag::SELECT)
    }

    /// `BM_vert_select_set`.
    pub fn vert_select_set(&mut self, v: VertId, select: bool) {
        let f = &mut self.vert_mut(v).flag;
        if f.contains(ElemFlag::HIDDEN) {
            return;
        }
        f.set(ElemFlag::SELECT, select);
    }
    /// `BM_edge_select_set_noflush`.
    pub fn edge_select_set_noflush(&mut self, e: EdgeId, select: bool) {
        let f = &mut self.edge_mut(e).flag;
        if f.contains(ElemFlag::HIDDEN) {
            return;
        }
        f.set(ElemFlag::SELECT, select);
    }
    /// `BM_face_select_set_noflush`.
    pub fn face_select_set_noflush(&mut self, f: FaceId, select: bool) {
        let fl = &mut self.face_mut(f).flag;
        if fl.contains(ElemFlag::HIDDEN) {
            return;
        }
        fl.set(ElemFlag::SELECT, select);
    }

    /// `bm_vert_is_edge_select_any_other`.
    fn vert_is_edge_select_any_other(&self, v: VertId, e_first: EdgeId) -> bool {
        let mut e = self.disk_edge_next(e_first, v);
        while e != e_first {
            if self.sel(e) {
                return true;
            }
            e = self.disk_edge_next(e, v);
        }
        false
    }
    /// `bm_edge_is_face_select_any_other`.
    fn edge_is_face_select_any_other(&self, l_first: LoopId) -> bool {
        let mut l = self.lp(l_first).radial_next;
        while l != l_first {
            if self.sel(self.lp(l).f) {
                return true;
            }
            l = self.lp(l).radial_next;
        }
        false
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc (BM_edge_select_set).
    /// `BM_edge_select_set`: selecting also selects both vertices; deselecting deselects them
    /// (in vertex mode), or only those without another selected edge.
    pub fn edge_select_set(&mut self, e: EdgeId, select: bool) {
        if self.edge(e).flag.contains(ElemFlag::HIDDEN) {
            return;
        }
        let [v1, v2] = self.edge(e).v;
        if select {
            self.edge_mut(e).flag.insert(ElemFlag::SELECT);
            self.vert_select_set(v1, true);
            self.vert_select_set(v2, true);
        } else {
            self.edge_mut(e).flag.remove(ElemFlag::SELECT);
            if !self.select_mode.contains(SelectMode::VERT) {
                for v in [v1, v2] {
                    if !self.vert_is_edge_select_any_other(v, e) {
                        self.vert_select_set(v, false);
                    }
                }
            } else {
                self.vert_select_set(v1, false);
                self.vert_select_set(v2, false);
            }
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc (BM_face_select_set).
    /// `BM_face_select_set`: selecting also selects the face's vertices and edges; deselecting
    /// flushes down depending on the selection mode.
    pub fn face_select_set(&mut self, f: FaceId, select: bool) {
        if self.face(f).flag.contains(ElemFlag::HIDDEN) {
            return;
        }
        let loops = self.face_loops(f);
        if select {
            self.face_mut(f).flag.insert(ElemFlag::SELECT);
            for &l in &loops {
                let (v, e) = (self.lp(l).v, self.lp(l).e);
                self.vert_select_set(v, true);
                self.edge_select_set(e, true);
            }
            return;
        }
        self.face_mut(f).flag.remove(ElemFlag::SELECT);
        if self.select_mode.contains(SelectMode::VERT) {
            for &l in &loops {
                let (v, e) = (self.lp(l).v, self.lp(l).e);
                self.vert_select_set(v, false);
                self.edge_select_set_noflush(e, false);
            }
        } else {
            if self.select_mode.contains(SelectMode::EDGE) {
                for &l in &loops {
                    let e = self.lp(l).e;
                    self.edge_select_set_noflush(e, false);
                }
            } else {
                for &l in &loops {
                    if !self.edge_is_face_select_any_other(l) {
                        let e = self.lp(l).e;
                        self.edge_select_set_noflush(e, false);
                    }
                }
            }
            for &l in &loops {
                let (v, e) = (self.lp(l).v, self.lp(l).e);
                if !self.vert_is_edge_select_any_other(v, e) {
                    self.vert_select_set(v, false);
                }
            }
        }
    }

    /// `BM_elem_select_set`: dispatches to `BM_vert/edge/face_select_set` (selecting an edge or
    /// face also selects its vertices, and a face its edges; deselecting flushes down by
    /// selection mode, as Blender does).
    pub fn select_set(&mut self, e: BMElem, select: bool) {
        match e {
            BMElem::Vert(v) => self.vert_select_set(v, select),
            BMElem::Edge(x) => self.edge_select_set(x, select),
            BMElem::Face(f) => self.face_select_set(f, select),
        }
    }
    /// Whether `e` is selected.
    pub fn is_selected(&self, e: BMElem) -> bool {
        self.sel(e)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc
    // (BM_mesh_select_mode_flush_ex with the default flags: flush up only).
    /// `BM_mesh_select_mode_flush`: edges follow their vertices (vertex mode), faces follow
    /// their edges (vertex and edge modes); unselected history entries are dropped.
    pub fn select_flush_mode(&mut self) {
        self.select_flush_mode_ex(self.select_mode, false);
    }

    /// `BM_mesh_select_mode_flush_ex(bm, selectmode, flags)`; `flush_down` is
    /// `BMSelectFlushFlag::Down`.
    pub fn select_flush_mode_ex(&mut self, mode: SelectMode, flush_down: bool) {
        if flush_down {
            if mode.contains(SelectMode::VERT) {
                // Pass.
            } else if mode.contains(SelectMode::EDGE) {
                self.flush_edge_to_vert();
            } else if mode.contains(SelectMode::FACE) {
                self.flush_face_to_vert_and_edge();
            }
        }
        if mode.contains(SelectMode::VERT) {
            for e in self.edge_ids() {
                let ed = self.edge(e);
                let ok =
                    !ed.flag.contains(ElemFlag::HIDDEN) && self.sel(ed.v[0]) && self.sel(ed.v[1]);
                self.edge_mut(e).flag.set(ElemFlag::SELECT, ok);
            }
        }
        if mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
            for f in self.face_ids() {
                let ok = !self.face(f).flag.contains(ElemFlag::HIDDEN)
                    && self
                        .face_loops(f)
                        .into_iter()
                        .all(|l| self.sel(self.lp(l).e));
                self.face_mut(f).flag.set(ElemFlag::SELECT, ok);
            }
        }
        self.select_history_validate();
    }

    fn flush_edge_to_vert(&mut self) {
        let mut any_select = false;
        for e in self.edge_ids() {
            if self.edge(e).flag.contains(ElemFlag::HIDDEN) {
                continue;
            }
            if self.sel(e) {
                any_select = true;
            } else {
                let [a, b] = self.edge(e).v;
                self.vert_select_set(a, false);
                self.vert_select_set(b, false);
            }
        }
        if any_select {
            for e in self.edge_ids() {
                if !self.edge(e).flag.contains(ElemFlag::HIDDEN) && self.sel(e) {
                    let [a, b] = self.edge(e).v;
                    self.vert_select_set(a, true);
                    self.vert_select_set(b, true);
                }
            }
        }
    }

    fn flush_face_to_vert_and_edge(&mut self) {
        let mut any_select = false;
        for f in self.face_ids() {
            if self.face(f).flag.contains(ElemFlag::HIDDEN) {
                continue;
            }
            if self.sel(f) {
                any_select = true;
            } else {
                for l in self.face_loops(f) {
                    let (v, e) = (self.lp(l).v, self.lp(l).e);
                    self.vert_select_set(v, false);
                    self.edge_select_set_noflush(e, false);
                }
            }
        }
        if any_select {
            for f in self.face_ids() {
                if !self.face(f).flag.contains(ElemFlag::HIDDEN) && self.sel(f) {
                    for l in self.face_loops(f) {
                        let (v, e) = (self.lp(l).v, self.lp(l).e);
                        self.vert_select_set(v, true);
                        self.edge_select_set_noflush(e, true);
                    }
                }
            }
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc
    // (BM_mesh_select_flush_from_verts, select = true).
    /// `BM_mesh_select_flush_from_verts(bm, true)`: selects edges and faces whose vertices are
    /// all selected.
    pub fn select_flush(&mut self) {
        for e in self.edge_ids() {
            let ed = self.edge(e);
            if self.sel(ed.v[0]) && self.sel(ed.v[1]) && !ed.flag.contains(ElemFlag::HIDDEN) {
                self.edge_mut(e).flag.insert(ElemFlag::SELECT);
            }
        }
        for f in self.face_ids() {
            let ok = !self.face(f).flag.contains(ElemFlag::HIDDEN)
                && self
                    .face_loops(f)
                    .into_iter()
                    .all(|l| self.sel(self.lp(l).v));
            if ok {
                self.face_mut(f).flag.insert(ElemFlag::SELECT);
            }
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc
    // (BM_mesh_select_flush_from_verts, select = false).
    /// `BM_mesh_select_flush_from_verts(bm, false)`: deselects edges with an unselected vertex
    /// and faces with an unselected edge; drops unselected history entries.
    pub fn deselect_flush(&mut self) {
        for e in self.edge_ids() {
            if self.edge(e).flag.contains(ElemFlag::HIDDEN) {
                continue;
            }
            let [a, b] = self.edge(e).v;
            if self.sel(e) && (!self.sel(a) || !self.sel(b)) {
                self.edge_mut(e).flag.remove(ElemFlag::SELECT);
            }
            if self.edge(e).l.is_some() && !self.sel(e) {
                for f in self.edge_faces(e) {
                    self.face_mut(f).flag.remove(ElemFlag::SELECT);
                }
            }
        }
        self.select_history_validate();
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_select.cc
    // (EDBM_selectmode_convert, EDBM_selectmode_set, edbm_strip_selections) and the part of
    // EDBM_selectmode_toggle_multi that calls them.
    /// `EDBM_selectmode_convert` + `EDBM_selectmode_set`: switches `select_mode` with the
    /// conversion table of [MESH §3.3]. `use_expand` (with a single-type `new_mode`) first
    /// converts from the highest old type to the new one (selecting upward, or deselecting
    /// partially covered elements downward).
    pub fn select_mode_convert(&mut self, new_mode: SelectMode, use_expand: bool) {
        let old = self.select_mode;
        if use_expand && new_mode.bits().count_ones() == 1 {
            let old_max = if old.contains(SelectMode::FACE) {
                SelectMode::FACE
            } else if old.contains(SelectMode::EDGE) {
                SelectMode::EDGE
            } else {
                SelectMode::VERT
            };
            self.selectmode_convert(old_max, new_mode);
        }
        self.selectmode_set(new_mode);
    }

    /// `EDBM_selectmode_convert(em, selectmode_old, selectmode_new)`.
    pub fn selectmode_convert(&mut self, old: SelectMode, new: SelectMode) {
        let (tv, te, tf) = self.selected_counts();
        if old == SelectMode::VERT {
            if tv == 0 {
            } else if new == SelectMode::EDGE {
                let tagged: Vec<EdgeId> = self
                    .edge_ids()
                    .into_iter()
                    .filter(|&e| {
                        let [a, b] = self.edge(e).v;
                        self.sel(a) || self.sel(b)
                    })
                    .collect();
                for e in tagged {
                    self.edge_select_set(e, true);
                }
            } else if new == SelectMode::FACE {
                let tagged: Vec<FaceId> = self
                    .face_ids()
                    .into_iter()
                    .filter(|&f| self.face_verts(f).into_iter().any(|v| self.sel(v)))
                    .collect();
                for f in tagged {
                    self.face_select_set(f, true);
                }
            }
        } else if old == SelectMode::EDGE {
            if te == 0 {
            } else if new == SelectMode::FACE {
                let tagged: Vec<FaceId> = self
                    .face_ids()
                    .into_iter()
                    .filter(|&f| self.face_edges(f).into_iter().any(|e| self.sel(e)))
                    .collect();
                for f in tagged {
                    self.face_select_set(f, true);
                }
            } else if new == SelectMode::VERT {
                for v in self.vert_ids() {
                    // BM_vert_is_all_edge_flag_test(v, SELECT, respect_hide = true).
                    let all = self
                        .vert_edges(v)
                        .into_iter()
                        .all(|e| self.edge(e).flag.contains(ElemFlag::HIDDEN) || self.sel(e));
                    if !all {
                        self.vert_select_set(v, false);
                    }
                }
                self.deselect_flush();
            }
        } else if old == SelectMode::FACE {
            if tf == 0 {
            } else if new == SelectMode::EDGE {
                for e in self.edge_ids() {
                    let all = self
                        .edge_faces(e)
                        .into_iter()
                        .all(|f| self.face(f).flag.contains(ElemFlag::HIDDEN) || self.sel(f));
                    if !all {
                        self.edge_select_set(e, false);
                    }
                }
                self.deselect_flush();
            } else if new == SelectMode::VERT {
                for v in self.vert_ids() {
                    let all = self
                        .vert_faces(v)
                        .into_iter()
                        .all(|f| self.face(f).flag.contains(ElemFlag::HIDDEN) || self.sel(f));
                    if !all {
                        self.vert_select_set(v, false);
                    }
                }
                self.deselect_flush();
            }
        }
    }

    /// `EDBM_selectmode_set(em, selectmode)`: strips history entries of disabled types and
    /// re-derives the selection for the new mode.
    pub fn selectmode_set(&mut self, mode: SelectMode) {
        self.select_mode = mode;
        // edbm_strip_selections
        self.select_history.retain(|e| match e {
            BMElem::Vert(_) => mode.contains(SelectMode::VERT),
            BMElem::Edge(_) => mode.contains(SelectMode::EDGE),
            BMElem::Face(_) => mode.contains(SelectMode::FACE),
        });
        let (tv, te, tf) = self.selected_counts();
        if tv == 0 && te == 0 && tf == 0 {
            return;
        }
        if mode.contains(SelectMode::VERT) {
            if tv != 0 {
                self.select_flush();
            }
        } else if mode.contains(SelectMode::EDGE) {
            for v in self.vert_ids() {
                self.vert_select_set(v, false);
            }
            if te != 0 {
                for e in self.edge_ids() {
                    if self.sel(e) {
                        self.edge_select_set(e, true);
                    }
                }
                self.select_flush_mode();
            }
        } else if mode.contains(SelectMode::FACE) {
            for e in self.edge_ids() {
                self.edge_select_set(e, false);
            }
            if tf != 0 {
                for f in self.face_ids() {
                    if self.sel(f) {
                        self.face_select_set(f, true);
                    }
                }
            }
        }
    }

    /// `BM_select_history_check`.
    pub fn select_history_check(&self, e: BMElem) -> bool {
        self.select_history.contains(&e)
    }
    /// `BM_select_history_remove`: drops `e` from the history (returns whether it was there).
    pub fn select_history_remove(&mut self, e: BMElem) -> bool {
        match self.select_history.iter().position(|&x| x == e) {
            Some(i) => {
                self.select_history.remove(i);
                true
            }
            None => false,
        }
    }
    /// `BM_select_history_store`: appends `e` (moving it to the end when already stored, as
    /// the editors do with `remove` + `store`; Blender's raw `BM_select_history_store` leaves
    /// a stored element in place, see [`BMesh::select_history_store_if_absent`]).
    pub fn select_history_store(&mut self, e: BMElem) {
        self.select_history_remove(e);
        self.select_history.push(e);
    }
    /// Blender's `BM_select_history_store`: appends `e` unless it is already stored.
    pub fn select_history_store_if_absent(&mut self, e: BMElem) {
        if !self.select_history_check(e) {
            self.select_history.push(e);
        }
    }
    /// `BM_select_history_store_notest`: appends without checking.
    pub fn select_history_store_notest(&mut self, e: BMElem) {
        self.select_history.push(e);
    }
    /// `BM_select_history_clear`.
    pub fn select_history_clear(&mut self) {
        self.select_history.clear();
    }
    /// `BM_select_history_active`: the last history entry.
    pub fn select_history_active(&self) -> Option<BMElem> {
        self.select_history.last().copied()
    }
    /// `BM_select_history_validate`: drops entries that are dead or unselected.
    pub fn select_history_validate(&mut self) {
        let h = std::mem::take(&mut self.select_history);
        self.select_history = h
            .into_iter()
            .filter(|&e| self.elem_alive(e) && self.sel(e))
            .collect();
    }
    /// Remaps history entries through `map` (`use_select_history` of extrude/duplicate,
    /// `BMO_mesh_selected_remap` without the selection check); entries without a mapping are
    /// dropped. The active face follows the face map when it has an entry.
    pub fn select_history_remap(&mut self, map: &dyn Fn(BMElem) -> Option<BMElem>) {
        let h = std::mem::take(&mut self.select_history);
        self.select_history = h.into_iter().filter_map(map).collect();
        if let Some(f) = self.act_face
            && let Some(BMElem::Face(nf)) = map(BMElem::Face(f))
        {
            self.act_face = Some(nf);
        }
    }
    /// Selected element counts `(verts, edges, faces)` (`bm->totvertsel`, …).
    pub fn selected_counts(&self) -> (usize, usize, usize) {
        let v = self
            .verts
            .iter()
            .filter(|(_, x)| x.flag.contains(ElemFlag::SELECT))
            .count();
        let e = self
            .edges
            .iter()
            .filter(|(_, x)| x.flag.contains(ElemFlag::SELECT))
            .count();
        let f = self
            .faces
            .iter()
            .filter(|(_, x)| x.flag.contains(ElemFlag::SELECT))
            .count();
        (v, e, f)
    }
    /// `BM_mesh_elem_hflag_disable_all(bm, VERT|EDGE|FACE, SELECT)`: clears every selection
    /// flag (hidden elements included) and the history.
    pub fn deselect_all(&mut self) {
        for v in self.verts.slots.iter_mut().flatten() {
            v.flag.remove(ElemFlag::SELECT);
        }
        for e in self.edges.slots.iter_mut().flatten() {
            e.flag.remove(ElemFlag::SELECT);
        }
        for f in self.faces.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::SELECT);
        }
        self.select_history.clear();
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc (BM_vert_hide_set).
    /// `BM_vert_hide_set`: the vertex and its edges and faces (no deselection).
    pub fn vert_hide_set(&mut self, v: VertId, hide: bool) {
        self.vert_mut(v).flag.set(ElemFlag::HIDDEN, hide);
        for e in self.vert_edges(v) {
            self.edge_mut(e).flag.set(ElemFlag::HIDDEN, hide);
            for f in self.edge_faces(e) {
                self.face_mut(f).flag.set(ElemFlag::HIDDEN, hide);
            }
        }
    }

    /// `vert_flush_hide_set`: hidden unless a connected edge is visible.
    fn vert_flush_hide(&mut self, v: VertId) {
        let any_visible = self
            .vert_edges(v)
            .into_iter()
            .any(|e| !self.edge(e).flag.contains(ElemFlag::HIDDEN));
        self.vert_mut(v).flag.set(ElemFlag::HIDDEN, !any_visible);
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc (BM_edge_hide_set).
    /// `BM_edge_hide_set`: the edge and its faces; hiding also hides vertices left without a
    /// visible edge, revealing reveals both vertices.
    pub fn edge_hide_set(&mut self, e: EdgeId, hide: bool) {
        for f in self.edge_faces(e) {
            self.face_mut(f).flag.set(ElemFlag::HIDDEN, hide);
        }
        self.edge_mut(e).flag.set(ElemFlag::HIDDEN, hide);
        let [a, b] = self.edge(e).v;
        if hide {
            self.vert_flush_hide(a);
            self.vert_flush_hide(b);
        } else {
            self.vert_mut(a).flag.remove(ElemFlag::HIDDEN);
            self.vert_mut(b).flag.remove(ElemFlag::HIDDEN);
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc (BM_face_hide_set).
    /// `BM_face_hide_set`: the face; hiding also hides edges without a visible face and
    /// vertices without a visible edge, revealing reveals its edges and vertices.
    pub fn face_hide_set(&mut self, f: FaceId, hide: bool) {
        self.face_mut(f).flag.set(ElemFlag::HIDDEN, hide);
        let loops = self.face_loops(f);
        if hide {
            for &l in &loops {
                let e = self.lp(l).e;
                let any_visible = self
                    .edge_faces(e)
                    .into_iter()
                    .any(|x| !self.face(x).flag.contains(ElemFlag::HIDDEN));
                self.edge_mut(e).flag.set(ElemFlag::HIDDEN, !any_visible);
            }
            for &l in &loops {
                let v = self.lp(l).v;
                self.vert_flush_hide(v);
            }
        } else {
            for &l in &loops {
                let (v, e) = (self.lp(l).v, self.lp(l).e);
                self.edge_mut(e).flag.remove(ElemFlag::HIDDEN);
                self.vert_mut(v).flag.remove(ElemFlag::HIDDEN);
            }
        }
    }

    /// `BM_elem_hide_set`: deselects before hiding (Blender's convention), then hides or
    /// reveals with the flushing of `BM_vert/edge/face_hide_set`.
    pub fn elem_hide_set(&mut self, e: BMElem, hide: bool) {
        if hide {
            self.select_set(e, false);
        }
        match e {
            BMElem::Vert(v) => self.vert_hide_set(v, hide),
            BMElem::Edge(x) => self.edge_hide_set(x, hide),
            BMElem::Face(f) => self.face_hide_set(f, hide),
        }
    }
}
