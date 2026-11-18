// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the WP 2.4 selection operators: the multi-object edit loop, read access
//! to an edit BMesh, Blender's `EDBM_*` selection utilities and the walker-driven selection of
//! `walker_select` (`editors/mesh/editmesh_select.cc`, `editmesh_utils.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).

use std::collections::BTreeSet;

use frap_bmesh::walkers::LoopDelimit;
use frap_bmesh::{BMElem, BMesh, ElemFlag, SelectMode};
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};
use smol_str::SmolStr;

/// `BKE_view_layer_array_from_objects_in_edit_mode_unique_data`: every mesh object in Edit
/// Mode, one per mesh, active first.
pub(crate) fn edit_meshes(cx: &OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    helpers::iter_edit_meshes(cx)
}

/// Runs `f` on the edit BMesh of `me` without marking it changed (`None` when the mesh has no
/// edit data).
pub(crate) fn read<R>(cx: &mut OpCtx, me: IdRef<MeshId>, f: impl FnOnce(&BMesh) -> R) -> Option<R> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| f(&em.bm))
}

/// `bm->totvertsel || bm->totedgesel || bm->totfacesel`.
pub(crate) fn any_selected(bm: &BMesh) -> bool {
    let (v, e, f) = bm.selected_counts();
    v + e + f > 0
}

/// Whether the element is hidden.
pub(crate) fn hidden(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    bm.elem_flag(e.into()).contains(ElemFlag::HIDDEN)
}

/// Whether the element is selected.
pub(crate) fn selected(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    bm.elem_flag(e.into()).contains(ElemFlag::SELECT)
}

/// `EDBM_flag_disable_all(em, BM_ELEM_SELECT)`: deselects every element (hidden ones too) and
/// clears the selection history (`BM_mesh_elem_hflag_disable_all` fast path).
pub(crate) fn deselect_all(bm: &mut BMesh) {
    bm.deselect_all();
}

/// `EDBM_selectmode_flush(em)` (`BM_mesh_select_mode_flush_ex` with the edit mesh's mode:
/// flush up, drop unselected history entries).
pub(crate) fn selectmode_flush(bm: &mut BMesh) {
    bm.select_flush_mode();
}

/// `walker_select`: (de)selects every element the walker yields with `BM_elem_select_set`; a
/// deselected element also leaves the selection history. Returns whether anything was walked.
pub(crate) fn walker_select(bm: &mut BMesh, elems: &[BMElem], select: bool) -> bool {
    for &e in elems {
        if !select {
            bm.select_history_remove(e);
        }
        bm.select_set(e, select);
    }
    !elems.is_empty()
}

/// `walker_select_count`: `[unselected, selected]` counts of the yielded elements, both `-1`
/// as soon as both kinds were seen (Blender's early exit).
pub(crate) fn walker_select_count(bm: &BMesh, elems: &[BMElem]) -> [i32; 2] {
    let mut c = [0i32; 2];
    for &e in elems {
        c[usize::from(bm.is_selected(e))] += 1;
        if c[0] != 0 && c[1] != 0 {
            return [-1, -1];
        }
    }
    c
}

/// Which `NGONS` bit an enum-flag property means: `delimit_edge_loop` uses
/// `BMW_DELIMIT_EDGE_LOOP_NGONS`, `delimit_edge_ring` `BMW_DELIMIT_EDGE_RING_NGONS`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ngons {
    /// `delimit_edge_loop` / `delimit_face_loop`.
    Loop,
    /// `delimit_edge_ring`.
    Ring,
}

/// `BMWDelimitFlag` from the identifiers of `rna_enum_mesh_walk_delimit_*_items`.
pub(crate) fn walk_delimit(set: &BTreeSet<SmolStr>, ngons: Ngons) -> LoopDelimit {
    let mut d = LoopDelimit::empty();
    for id in set {
        d |= match id.as_str() {
            "SEAM" => LoopDelimit::SEAM,
            "SHARP" => LoopDelimit::SHARP,
            "MATERIAL" => LoopDelimit::MATERIAL,
            "INNER_CORNERS" => LoopDelimit::INNER_CORNERS,
            "OUTER_CORNERS" => LoopDelimit::OUTER_CORNERS,
            "NGONS" if ngons == Ngons::Loop => LoopDelimit::LOOP_NGONS,
            "NGONS" => LoopDelimit::RING_NGONS,
            _ => LoopDelimit::empty(),
        };
    }
    d
}

/// The element kind of the highest-priority select mode (vertex, else edge, else face), as
/// most operators test `em->selectmode & SCE_SELECT_VERTEX` first.
pub(crate) fn primary_mode(m: SelectMode) -> SelectMode {
    if m.contains(SelectMode::VERT) {
        SelectMode::VERT
    } else if m.contains(SelectMode::EDGE) {
        SelectMode::EDGE
    } else {
        SelectMode::FACE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimit_identifiers_map_to_walker_bits() {
        let set: BTreeSet<SmolStr> = ["NGONS", "OUTER_CORNERS", "SEAM"]
            .into_iter()
            .map(SmolStr::new)
            .collect();
        assert_eq!(
            walk_delimit(&set, Ngons::Loop),
            LoopDelimit::LOOP_NGONS | LoopDelimit::OUTER_CORNERS | LoopDelimit::SEAM
        );
        let ring: BTreeSet<SmolStr> = ["NGONS", "MATERIAL"].into_iter().map(SmolStr::new).collect();
        assert_eq!(
            walk_delimit(&ring, Ngons::Ring),
            LoopDelimit::RING_NGONS | LoopDelimit::MATERIAL
        );
    }

    #[test]
    fn select_count_stops_when_mixed() {
        let mut bm = BMesh::new();
        let a = bm.vert_create([0.0; 3], None, Default::default());
        let b = bm.vert_create([1.0, 0.0, 0.0], None, Default::default());
        let elems = [BMElem::Vert(a), BMElem::Vert(b)];
        assert_eq!(walker_select_count(&bm, &elems), [2, 0]);
        bm.vert_select_set(a, true);
        assert_eq!(walker_select_count(&bm, &elems), [-1, -1]);
        assert_eq!(walker_select_count(&bm, &elems[..1]), [0, 1]);
        assert_eq!(walker_select_count(&bm, &[]), [0, 0]);
    }
}
