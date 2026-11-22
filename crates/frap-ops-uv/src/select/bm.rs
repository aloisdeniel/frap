// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BM_*uvselect*` API: UV selection flags, connectivity checks, flushing between UV
//! vertices, edges and faces, and synchronising with the mesh selection.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_uvselect.cc (Blender 5.2.4,
//! GPL-2.0-or-later), the parts the UV selection operators reach (no picking or validation
//! functions). `cd_loop_uv_offset` is the [`UvSel`]'s UV column.

use frap_bmesh::{BMElem, ElemFlag, FaceId, LoopId, SelectMode};

use super::UvSel;

impl UvSel<'_> {
    /// `bm_mesh_uvselect_disable_all`.
    fn bm_disable_all(&mut self) {
        for f in self.faces() {
            for l in self.face_loops(f) {
                self.set_lflag(l, ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE, false);
            }
            self.set_fflag(f, ElemFlag::SELECT_UV, false);
        }
    }

    fn hidden(&self, f: FaceId) -> bool {
        self.fflag(f, ElemFlag::HIDDEN)
    }

    // ------------------------------------------------------------------ low level

    /// `BM_loop_vert_uvselect_test`.
    pub(crate) fn bm_loop_vert_test(&self, l: LoopId) -> bool {
        !self.hidden(self.lf(l)) && self.lflag(l, ElemFlag::SELECT_UV)
    }
    /// `BM_loop_edge_uvselect_test`.
    pub(crate) fn bm_loop_edge_test(&self, l: LoopId) -> bool {
        !self.hidden(self.lf(l)) && self.lflag(l, ElemFlag::SELECT_UV_EDGE)
    }
    /// `BM_face_uvselect_test`.
    pub(crate) fn bm_face_test(&self, f: FaceId) -> bool {
        !self.hidden(f) && self.fflag(f, ElemFlag::SELECT_UV)
    }

    // ------------------------------------------------------------ connectivity checks

    /// The loops at `l`'s vertex other than `l` on visible faces, in disk-radial order.
    fn other_visible_loops_at_vert(&self, l: LoopId) -> Vec<LoopId> {
        self.disk_radial_loops(self.lv(l))
            .into_iter()
            .filter(|&li| li != l && !self.hidden(self.lf(li)))
            .collect()
    }

    /// `BM_loop_vert_uvselect_check_other_loop_vert`.
    pub(crate) fn bm_loop_vert_check_other_loop_vert(&self, l: LoopId, hflag: ElemFlag) -> bool {
        self.other_visible_loops_at_vert(l)
            .into_iter()
            .any(|li| self.lflag(li, hflag) && self.share_vert(l, li))
    }

    /// `BM_loop_vert_uvselect_check_other_loop_edge`.
    pub(crate) fn bm_loop_vert_check_other_loop_edge(&self, l: LoopId, hflag: ElemFlag) -> bool {
        self.other_visible_loops_at_vert(l).into_iter().any(|li| {
            (self.lflag(li, hflag) || self.lflag(self.prev(li), hflag)) && self.share_vert(l, li)
        })
    }

    /// `BM_loop_vert_uvselect_check_other_edge`.
    pub(crate) fn bm_loop_vert_check_other_edge(&self, l: LoopId, hflag: ElemFlag) -> bool {
        self.other_visible_loops_at_vert(l).into_iter().any(|li| {
            let e = self.le(li);
            let ep = self.le(self.prev(li));
            ((!self.eflag(e, ElemFlag::HIDDEN) && self.eflag(e, hflag))
                || (!self.eflag(ep, ElemFlag::HIDDEN) && self.eflag(ep, hflag)))
                && self.share_vert(l, li)
        })
    }

    /// `BM_loop_vert_uvselect_check_other_face`.
    pub(crate) fn bm_loop_vert_check_other_face(&self, l: LoopId, hflag: ElemFlag) -> bool {
        self.other_visible_loops_at_vert(l)
            .into_iter()
            .any(|li| self.fflag(self.lf(li), hflag) && self.share_vert(l, li))
    }

    /// `BM_loop_edge_uvselect_check_other_loop_edge`.
    pub(crate) fn bm_loop_edge_check_other_loop_edge(&self, l: LoopId, hflag: ElemFlag) -> bool {
        self.radial_from(l).into_iter().any(|li| {
            !self.hidden(self.lf(li)) && li != l && self.lflag(li, hflag) && self.share_edge(l, li)
        })
    }

    /// `BM_loop_edge_uvselect_check_other_face`.
    pub(crate) fn bm_loop_edge_check_other_face(&self, l: LoopId, hflag: ElemFlag) -> bool {
        self.radial_from(l).into_iter().any(|li| {
            !self.hidden(self.lf(li))
                && li != l
                && self.fflag(self.lf(li), hflag)
                && self.share_edge(l, li)
        })
    }

    /// `BM_face_uvselect_check_edges_all`.
    pub(crate) fn bm_face_check_edges_all(&self, f: FaceId) -> bool {
        !self.hidden(f)
            && self
                .face_loops(f)
                .into_iter()
                .all(|l| self.lflag(l, ElemFlag::SELECT_UV_EDGE))
    }

    // ------------------------------------------------------------------ set functions

    /// `BM_loop_vert_uvselect_set_noflush`.
    pub(crate) fn bm_loop_vert_set_noflush(&mut self, l: LoopId, select: bool) {
        self.set_lflag(l, ElemFlag::SELECT_UV, select);
    }
    /// `BM_loop_edge_uvselect_set_noflush`.
    pub(crate) fn bm_loop_edge_set_noflush(&mut self, l: LoopId, select: bool) {
        self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, select);
    }
    /// `BM_loop_edge_uvselect_set`.
    pub(crate) fn bm_loop_edge_set(&mut self, l: LoopId, select: bool) {
        self.bm_loop_edge_set_noflush(l, select);
        self.bm_loop_vert_set_noflush(l, select);
        let n = self.next(l);
        self.bm_loop_vert_set_noflush(n, select);
    }
    /// `BM_face_uvselect_set_noflush`.
    pub(crate) fn bm_face_set_noflush(&mut self, f: FaceId, select: bool) {
        self.set_fflag(f, ElemFlag::SELECT_UV, select);
    }
    /// `BM_face_uvselect_set`.
    pub(crate) fn bm_face_set(&mut self, f: FaceId, select: bool) {
        self.bm_face_set_noflush(f, select);
        for l in self.face_loops(f) {
            self.bm_loop_vert_set_noflush(l, select);
            self.bm_loop_edge_set_noflush(l, select);
        }
    }

    /// `BM_loop_vert_uvselect_set_shared`.
    pub(crate) fn bm_loop_vert_set_shared(&mut self, l: LoopId, select: bool) {
        self.bm_loop_vert_set_noflush(l, select);
        for li in self.other_visible_loops_at_vert(l) {
            if self.lflag(li, ElemFlag::SELECT_UV) != select && self.share_vert(l, li) {
                self.bm_loop_vert_set_noflush(li, select);
            }
        }
    }

    /// `BM_loop_edge_uvselect_set_shared`.
    pub(crate) fn bm_loop_edge_set_shared(&mut self, l: LoopId, select: bool) {
        self.bm_loop_edge_set_noflush(l, select);
        let mut li = self.radial_next(l);
        while li != l {
            if !self.hidden(self.lf(li))
                && self.lflag(li, ElemFlag::SELECT_UV_EDGE) != select
                && self.share_edge(l, li)
            {
                self.bm_loop_edge_set_noflush(li, select);
            }
            li = self.radial_next(li);
        }
    }

    /// `BM_face_uvselect_set_shared`.
    pub(crate) fn bm_face_set_shared(&mut self, f: FaceId, select: bool) {
        self.bm_face_set_noflush(f, select);
        for l in self.face_loops(f) {
            self.bm_loop_vert_set_shared(l, select);
            self.bm_loop_edge_set_shared(l, select);
        }
    }

    // --------------------------------------------------- flushing (only select/deselect)

    /// `BM_mesh_uvselect_flush_from_loop_verts_only_select`.
    pub(crate) fn bm_flush_from_loop_verts_only_select(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let mut all = true;
            for l in self.face_loops(f) {
                let n = self.next(l);
                if self.lflag(l, ElemFlag::SELECT_UV) && self.lflag(n, ElemFlag::SELECT_UV) {
                    self.bm_loop_edge_set_noflush(l, true);
                } else {
                    all = false;
                }
            }
            if all {
                self.bm_face_set_noflush(f, true);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_from_loop_verts_only_deselect`.
    pub(crate) fn bm_flush_from_loop_verts_only_deselect(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let mut all = true;
            for l in self.face_loops(f) {
                let n = self.next(l);
                if !(self.lflag(l, ElemFlag::SELECT_UV) && self.lflag(n, ElemFlag::SELECT_UV)) {
                    self.bm_loop_edge_set_noflush(l, false);
                    all = false;
                }
            }
            if !all {
                self.bm_face_set_noflush(f, false);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_from_loop_edges_only_select`.
    pub(crate) fn bm_flush_from_loop_edges_only_select(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let mut all = true;
            for l in self.face_loops(f) {
                if self.lflag(l, ElemFlag::SELECT_UV_EDGE) {
                    self.bm_loop_edge_set(l, true);
                } else {
                    all = false;
                }
            }
            if all {
                self.bm_face_set_noflush(f, true);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_from_loop_edges_only_deselect`.
    pub(crate) fn bm_flush_from_loop_edges_only_deselect(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let mut all = true;
            for l in self.face_loops(f) {
                let p = self.prev(l);
                let s = self.lflag(p, ElemFlag::SELECT_UV_EDGE)
                    || self.lflag(l, ElemFlag::SELECT_UV_EDGE);
                self.bm_loop_vert_set_noflush(l, s);
                if !self.lflag(l, ElemFlag::SELECT_UV_EDGE) {
                    self.bm_loop_edge_set_noflush(l, false);
                    all = false;
                }
            }
            if !all {
                self.bm_face_set_noflush(f, false);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_from_faces_only_select`.
    pub(crate) fn bm_flush_from_faces_only_select(&mut self) {
        for f in self.faces() {
            if self.hidden(f) || !self.fflag(f, ElemFlag::SELECT_UV) {
                continue;
            }
            for l in self.face_loops(f) {
                self.bm_loop_vert_set_noflush(l, true);
                self.bm_loop_edge_set_noflush(l, true);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_from_faces_only_deselect`.
    pub(crate) fn bm_flush_from_faces_only_deselect(&mut self) {
        for f in self.faces() {
            if self.hidden(f) || self.fflag(f, ElemFlag::SELECT_UV) {
                continue;
            }
            for l in self.face_loops(f) {
                self.bm_loop_vert_set_noflush(l, false);
                self.bm_loop_edge_set_noflush(l, false);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_shared_only_select`.
    pub(crate) fn bm_flush_shared_only_select(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            for l in self.face_loops(f) {
                if !self.lflag(l, ElemFlag::SELECT_UV)
                    && self.bm_loop_vert_check_other_loop_vert(l, ElemFlag::SELECT_UV)
                {
                    self.bm_loop_vert_set_noflush(l, true);
                }
                if !self.lflag(l, ElemFlag::SELECT_UV_EDGE)
                    && self.bm_loop_edge_check_other_loop_edge(l, ElemFlag::SELECT_UV_EDGE)
                {
                    self.bm_loop_edge_set_noflush(l, true);
                }
            }
        }
    }

    /// `BM_mesh_uvselect_flush_shared_only_deselect`.
    pub(crate) fn bm_flush_shared_only_deselect(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            for l in self.face_loops(f) {
                if self.lflag(l, ElemFlag::SELECT_UV)
                    && !self.bm_loop_vert_check_other_loop_vert(l, ElemFlag::SELECT_UV)
                {
                    self.bm_loop_vert_set_noflush(l, false);
                }
                if self.lflag(l, ElemFlag::SELECT_UV_EDGE)
                    && !self.bm_loop_edge_check_other_loop_edge(l, ElemFlag::SELECT_UV_EDGE)
                {
                    self.bm_loop_edge_set_noflush(l, false);
                }
            }
        }
    }

    // ----------------------------------------------------- flushing (between elements)

    /// `BM_mesh_uvselect_flush_from_loop_verts`.
    pub(crate) fn bm_flush_from_loop_verts(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let mut all = true;
            for l in self.face_loops(f) {
                let n = self.next(l);
                let s = self.lflag(l, ElemFlag::SELECT_UV) && self.lflag(n, ElemFlag::SELECT_UV);
                self.bm_loop_edge_set_noflush(l, s);
                if !s {
                    all = false;
                }
            }
            self.bm_face_set_noflush(f, all);
        }
    }

    /// `BM_mesh_uvselect_flush_from_loop_edges`.
    pub(crate) fn bm_flush_from_loop_edges(&mut self, flush_down: bool) {
        let faces = self.faces();
        for &f in &faces {
            if self.hidden(f) {
                continue;
            }
            if flush_down {
                for l in self.face_loops(f) {
                    self.bm_loop_vert_set_noflush(l, false);
                }
            }
            self.bm_face_set_noflush(f, false);
        }
        for &f in &faces {
            if self.hidden(f) {
                continue;
            }
            let mut all = true;
            for l in self.face_loops(f) {
                if self.lflag(l, ElemFlag::SELECT_UV_EDGE) {
                    if flush_down {
                        self.bm_loop_vert_set_noflush(l, true);
                        let n = self.next(l);
                        self.bm_loop_vert_set_noflush(n, true);
                    }
                } else {
                    all = false;
                }
            }
            if all {
                self.bm_face_set_noflush(f, true);
            }
        }
    }

    /// `BM_mesh_uvselect_flush_from_faces`.
    pub(crate) fn bm_flush_from_faces(&mut self, flush_down: bool) {
        if !flush_down {
            return;
        }
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let s = self.fflag(f, ElemFlag::SELECT_UV);
            for l in self.face_loops(f) {
                self.bm_loop_vert_set_noflush(l, s);
                self.bm_loop_edge_set_noflush(l, s);
            }
        }
    }

    /// `BM_mesh_uvselect_mode_flush_ex`.
    pub(crate) fn bm_mode_flush_ex(&mut self, selectmode: SelectMode, flush_down: bool) {
        if selectmode.contains(SelectMode::VERT) {
            self.bm_flush_from_loop_verts();
        } else if selectmode.contains(SelectMode::EDGE) {
            self.bm_flush_from_loop_edges(flush_down);
        } else {
            self.bm_flush_from_faces(flush_down);
        }
    }

    /// `BM_mesh_uvselect_mode_flush` (`bm->selectmode`).
    pub(crate) fn bm_mode_flush(&mut self) {
        let m = self.bm.select_mode;
        self.bm_mode_flush_ex(m, false);
    }

    // --------------------------------------------------------- from/to mesh selection

    /// `bm_mesh_uvselect_flush_from_mesh_sticky_vert_for_vert_mode`.
    fn bm_from_mesh_sticky_vert_vert_mode(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            for l in self.face_loops(f) {
                let vs = self.vflag(self.lv(l), ElemFlag::SELECT);
                let es = self.eflag(self.le(l), ElemFlag::SELECT);
                self.set_lflag(l, ElemFlag::SELECT_UV, vs);
                self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, es);
            }
            let fs = self.fflag(f, ElemFlag::SELECT);
            self.set_fflag(f, ElemFlag::SELECT_UV, fs);
        }
        self.sync_valid = true;
    }

    /// `bm_mesh_uvselect_flush_from_mesh_sticky_vert_for_edge_mode`.
    fn bm_from_mesh_sticky_vert_edge_mode(&mut self) {
        self.bm_disable_all();
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            for l in self.face_loops(f) {
                if self.eflag(self.le(l), ElemFlag::SELECT) {
                    self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, true);
                    let n = self.next(l);
                    self.set_lflag(l, ElemFlag::SELECT_UV, true);
                    self.set_lflag(n, ElemFlag::SELECT_UV, true);
                }
            }
            if self.fflag(f, ElemFlag::SELECT) {
                self.set_fflag(f, ElemFlag::SELECT_UV, true);
            }
        }
        self.sync_valid = true;
    }

    /// `bm_mesh_uvselect_flush_from_mesh_sticky_vert_for_face_mode`.
    fn bm_from_mesh_sticky_vert_face_mode(&mut self) {
        self.bm_disable_all();
        for f in self.faces() {
            if self.hidden(f) || !self.fflag(f, ElemFlag::SELECT) {
                continue;
            }
            for l in self.face_loops(f) {
                self.set_lflag(l, ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE, true);
            }
            self.set_fflag(f, ElemFlag::SELECT_UV, true);
        }
        self.sync_valid = true;
    }

    /// `bm_mesh_uvselect_flush_from_mesh_sticky_location_for_edge_mode`.
    fn bm_from_mesh_sticky_location_edge_mode(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            let first = self.first_loop(f);
            let mut e_prev = self.eflag(self.le(self.prev(first)), ElemFlag::SELECT);
            for l in self.face_loops(f) {
                let e_iter = self.eflag(self.le(l), ElemFlag::SELECT);
                let v_iter = self.vflag(self.lv(l), ElemFlag::SELECT)
                    && (e_prev
                        || e_iter
                        || self.bm_loop_vert_check_other_edge(l, ElemFlag::SELECT));
                self.set_lflag(l, ElemFlag::SELECT_UV, v_iter);
                self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, e_iter);
                e_prev = e_iter;
            }
            let fs = self.fflag(f, ElemFlag::SELECT);
            self.set_fflag(f, ElemFlag::SELECT_UV, fs);
        }
        self.sync_valid = true;
    }

    /// `bm_mesh_uvselect_flush_from_mesh_sticky_location_for_face_mode`.
    fn bm_from_mesh_sticky_location_face_mode(&mut self) {
        for f in self.faces() {
            if self.hidden(f) {
                continue;
            }
            if self.fflag(f, ElemFlag::SELECT) {
                for l in self.face_loops(f) {
                    self.set_lflag(l, ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE, true);
                }
                self.set_fflag(f, ElemFlag::SELECT_UV, true);
            } else {
                for l in self.face_loops(f) {
                    let vs = self.vflag(self.lv(l), ElemFlag::SELECT)
                        && self.bm_loop_vert_check_other_face(l, ElemFlag::SELECT);
                    let es = self.eflag(self.le(l), ElemFlag::SELECT)
                        && self.bm_loop_edge_check_other_face(l, ElemFlag::SELECT);
                    self.set_lflag(l, ElemFlag::SELECT_UV, vs);
                    self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, es);
                }
                self.set_fflag(f, ElemFlag::SELECT_UV, false);
            }
        }
        self.sync_valid = true;
    }

    /// `BM_mesh_uvselect_sync_from_mesh_sticky_location`.
    pub(crate) fn bm_sync_from_mesh_sticky_location(&mut self) {
        let m = self.bm.select_mode;
        if m.contains(SelectMode::VERT) {
            self.bm_from_mesh_sticky_vert_vert_mode();
        } else if m.contains(SelectMode::EDGE) {
            self.bm_from_mesh_sticky_location_edge_mode();
        } else {
            self.bm_from_mesh_sticky_location_face_mode();
        }
    }

    /// `BM_mesh_uvselect_sync_from_mesh_sticky_disabled`.
    pub(crate) fn bm_sync_from_mesh_sticky_disabled(&mut self) {
        self.bm_from_mesh_sticky_vert_vert_mode();
    }

    /// `BM_mesh_uvselect_sync_from_mesh_sticky_vert`.
    pub(crate) fn bm_sync_from_mesh_sticky_vert(&mut self) {
        let m = self.bm.select_mode;
        if m.contains(SelectMode::VERT) {
            self.bm_from_mesh_sticky_vert_vert_mode();
        } else if m.contains(SelectMode::EDGE) {
            self.bm_from_mesh_sticky_vert_edge_mode();
        } else {
            self.bm_from_mesh_sticky_vert_face_mode();
        }
    }

    /// `BM_mesh_uvselect_sync_to_mesh`.
    pub(crate) fn bm_sync_to_mesh(&mut self) {
        // `BM_SELECT_HISTORY_BACKUP` around `BM_mesh_elem_hflag_disable_all(…, SELECT)`.
        let history: Vec<BMElem> = std::mem::take(&mut self.bm.select_history);
        super::mesh::flag_disable_all_select(self.bm);
        let faces = self.faces();
        let m = self.bm.select_mode;
        if m.contains(SelectMode::VERT) {
            for &f in &faces {
                if self.hidden(f) {
                    continue;
                }
                for l in self.face_loops(f) {
                    if self.lflag(l, ElemFlag::SELECT_UV) {
                        let v = self.lv(l);
                        self.bm.vert_select_set(v, true);
                    }
                }
            }
            self.bm.select_flush();
        } else if m.contains(SelectMode::EDGE) {
            for &f in &faces {
                if self.hidden(f) {
                    continue;
                }
                let mut face_select = true;
                for l in self.face_loops(f) {
                    if self.lflag(l, ElemFlag::SELECT_UV) {
                        let v = self.lv(l);
                        self.bm.vert_select_set(v, true);
                    } else {
                        face_select = false;
                    }
                    if self.lflag(l, ElemFlag::SELECT_UV_EDGE) {
                        let e = self.le(l);
                        self.bm.edge_select_set(e, true);
                    } else {
                        face_select = false;
                    }
                }
                if face_select {
                    self.bm.face_select_set_noflush(f, true);
                }
            }
            for &f in &faces {
                if self.fflag(f, ElemFlag::HIDDEN) || self.fflag(f, ElemFlag::SELECT) {
                    continue;
                }
                let all = self
                    .face_loops(f)
                    .into_iter()
                    .all(|l| self.eflag(self.le(l), ElemFlag::SELECT));
                if all {
                    self.bm.face_select_set_noflush(f, true);
                }
            }
        } else {
            for &f in &faces {
                if self.hidden(f) {
                    continue;
                }
                if self.fflag(f, ElemFlag::SELECT_UV) {
                    self.bm.face_select_set(f, true);
                }
            }
        }
        self.bm.select_history = history;
        self.bm.select_history_validate();
    }
}
