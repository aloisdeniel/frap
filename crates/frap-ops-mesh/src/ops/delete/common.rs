// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers the WP 2.7 edit-mode operators share (delete, dissolve, merge): the `%hv`/`%he`/
//! `%hf` slot buffers of `EDBM_op_callf` (selected, visible elements in pool order), mesh
//! statistics for the "Removed …" reports and `result.removed*`, and the
//! `EDBM_op_call_and_selectf` selection of an output slot. Ports of `edbm_report_delete_info`
//! (`editors/mesh/editmesh_tools.cc`) and `bmo_slot_buffer_from_hflag` /
//! `BMO_slot_buffer_hflag_enable` (`bmesh/intern/bmesh_operators.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.

use frap_bmesh::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};

/// Selected and visible (`BMO_FLAG_RESPECT_HIDE`) vertices, in pool order (`%hv`).
pub(crate) fn sel_verts(bm: &BMesh) -> Vec<VertId> {
    bm.verts
        .iter()
        .filter(|(_, v)| v.flag.contains(ElemFlag::SELECT) && !v.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| VertId(i))
        .collect()
}

/// Selected and visible edges, in pool order (`%he`).
pub(crate) fn sel_edges(bm: &BMesh) -> Vec<EdgeId> {
    bm.edges
        .iter()
        .filter(|(_, e)| e.flag.contains(ElemFlag::SELECT) && !e.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| EdgeId(i))
        .collect()
}

/// Selected and visible faces, in pool order (`%hf`).
pub(crate) fn sel_faces(bm: &BMesh) -> Vec<FaceId> {
    bm.faces
        .iter()
        .filter(|(_, f)| f.flag.contains(ElemFlag::SELECT) && !f.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| FaceId(i))
        .collect()
}

/// `(totvert, totedge, totface)`.
pub(crate) fn totals(bm: &BMesh) -> [i64; 3] {
    [
        bm.verts_num() as i64,
        bm.edges_num() as i64,
        bm.faces_num() as i64,
    ]
}

/// The edit BMesh of `me`, read-only (no generation bump: the mesh is not marked modified).
pub(crate) fn edit_bm<'a>(cx: &'a mut OpCtx, me: IdRef<MeshId>) -> Option<&'a BMesh> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| &*em.bm)
}

/// A bmesh operator failure (`BMO_error_raise` with `BMO_ERROR_FATAL`), as Blender's
/// `EDBM_op_finish` reports it.
pub(crate) fn bm_err(e: BmError) -> OpError {
    OpError::Runtime(e.to_string())
}

/// `BMO_slot_buffer_hflag_enable(…, BM_ELEM_SELECT, do_flush = true)` for an output slot.
pub(crate) fn select_elems(bm: &mut BMesh, elems: &[BMElem]) {
    for &e in elems {
        if bm.elem_alive(e) {
            bm.select_set(e, true);
        }
    }
}

/// The report of `mesh.remove_doubles` (`"Removed %d vertex"` / `"Removed %d vertices"`).
pub(crate) fn removed_vertices_report(n: i64) -> String {
    if n == 1 {
        format!("Removed {n} vertex")
    } else {
        format!("Removed {n} vertices")
    }
}

/// Element counts before and after an operator, summed over the edit meshes it ran on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Removed {
    /// Counts before.
    pub old: [i64; 3],
    /// Counts after.
    pub new: [i64; 3],
}

impl Removed {
    /// Adds one mesh's counts.
    pub(crate) fn add(&mut self, old: [i64; 3], new: [i64; 3]) {
        for k in 0..3 {
            self.old[k] += old[k];
            self.new[k] += new[k];
        }
    }
    /// Removed vertices, edges and faces (negative when the operator added some).
    pub(crate) fn delta(&self) -> [i64; 3] {
        [
            self.old[0] - self.new[0],
            self.old[1] - self.new[1],
            self.old[2] - self.new[2],
        ]
    }
    /// `edbm_report_delete_info`: `"Removed: %d vertices, %d edges, %d faces"`.
    pub(crate) fn delete_info(&self) -> String {
        let [v, e, f] = self.delta();
        format!("Removed: {v} vertices, {e} edges, {f} faces")
    }
    /// `result.removed` (vertices), `result.removed_edges`, `result.removed_faces`.
    pub(crate) fn write_result(&self, cx: &mut OpCtx) {
        let [v, e, f] = self.delta();
        cx.result("removed", Value::Int(v));
        cx.result("removed_edges", Value::Int(e));
        cx.result("removed_faces", Value::Int(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_report_wording() {
        assert_eq!(removed_vertices_report(0), "Removed 0 vertices");
        assert_eq!(removed_vertices_report(1), "Removed 1 vertex");
        assert_eq!(removed_vertices_report(6), "Removed 6 vertices");
    }

    #[test]
    fn delete_info_wording() {
        let mut r = Removed::default();
        r.add([8, 12, 6], [4, 4, 1]);
        r.add([3, 3, 1], [3, 3, 1]);
        assert_eq!(r.delete_info(), "Removed: 4 vertices, 8 edges, 5 faces");
        assert_eq!(r.delta(), [4, 8, 5]);
    }
}
