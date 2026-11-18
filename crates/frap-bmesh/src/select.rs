// SPDX-License-Identifier: GPL-3.0-or-later
//! Selection: flushing, history, mode conversion (`bmesh/intern/bmesh_marking.cc`,
//! `editors/mesh/editmesh_select.cc`; ARCHITECTURE §6.2, [MESH §2.2, §3.3]). CONTRACT frozen by
//! M2 W0, implemented by WP 2.1.

use crate::types::{BMElem, BMesh, SelectMode};

impl BMesh {
    /// `BM_elem_select_set`: selecting an edge or face also selects its vertices (and a face
    /// its edges); deselecting does not flush down.
    pub fn select_set(&mut self, e: BMElem, select: bool) {
        let _ = (e, select);
        todo!("WP 2.1: BM_elem_select_set")
    }
    /// Whether `e` is selected.
    pub fn is_selected(&self, e: BMElem) -> bool {
        let _ = e;
        todo!("WP 2.1: BM_elem_flag_test(e, BM_ELEM_SELECT)")
    }
    /// `BM_mesh_select_mode_flush`: edges and faces follow the vertices (VERT), faces follow
    /// the edges (EDGE), edges and vertices follow the faces (FACE).
    pub fn select_flush_mode(&mut self) {
        todo!("WP 2.1: BM_mesh_select_mode_flush")
    }
    /// `BM_mesh_select_flush`: selects edges/faces whose vertices are all selected.
    pub fn select_flush(&mut self) {
        todo!("WP 2.1: BM_mesh_select_flush")
    }
    /// `BM_mesh_deselect_flush`.
    pub fn deselect_flush(&mut self) {
        todo!("WP 2.1: BM_mesh_deselect_flush")
    }
    /// `EDBM_selectmode_convert` + `EDBM_selectmode_set`: switches `select_mode` with the
    /// conversion table of [MESH §3.3] (`use_expand` selects upward on expand).
    pub fn select_mode_convert(&mut self, new_mode: SelectMode, use_expand: bool) {
        let _ = (new_mode, use_expand);
        todo!("WP 2.1: EDBM_selectmode_convert")
    }
    /// `BM_select_history_store`: appends `e` (moving it to the end when already stored).
    pub fn select_history_store(&mut self, e: BMElem) {
        let _ = e;
        todo!("WP 2.1: BM_select_history_store")
    }
    /// `BM_select_history_active`: the last history entry.
    pub fn select_history_active(&self) -> Option<BMElem> {
        self.select_history.last().copied()
    }
    /// `BM_select_history_validate`: drops entries that are dead or unselected.
    pub fn select_history_validate(&mut self) {
        todo!("WP 2.1: BM_select_history_validate")
    }
    /// Remaps history entries through `map` (`use_select_history` of extrude/duplicate);
    /// entries without a mapping are dropped.
    pub fn select_history_remap(&mut self, map: &dyn Fn(BMElem) -> Option<BMElem>) {
        let _ = map;
        todo!("WP 2.1: BMO select-history remap")
    }
    /// Selected element counts `(verts, edges, faces)` (`bm->totvertsel`, …).
    pub fn selected_counts(&self) -> (usize, usize, usize) {
        todo!("WP 2.1: bm->tot*sel")
    }
}
