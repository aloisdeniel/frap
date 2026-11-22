// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh selection functions the UV selection operators forward to when the UV selection
//! follows the mesh selection (sync selection without a synced UV selection, D-111).
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_utils.cc (`EDBM_flag_enable_all`,
//! `EDBM_flag_disable_all`, `EDBM_select_more`, `EDBM_select_less`, `EDBM_mesh_hide`,
//! `EDBM_mesh_reveal`), blender/source/blender/editors/mesh/editmesh_select.cc
//! (`EDBM_select_swap`) and blender/source/blender/bmesh/intern/bmesh_marking.cc
//! (`BM_mesh_select_mode_clean_ex`), Blender 5.2.4, GPL-2.0-or-later. `frap-ops-mesh` has
//! the same ports for the `mesh.*` operators; this crate cannot depend on it (same layer).

use frap_bmesh::ops::region_extend::{RegionExtendIn, region_extend};
use frap_bmesh::{BMElem, BMesh, ElemFlag, SelectMode};

fn flag(bm: &BMesh, e: BMElem) -> ElemFlag {
    bm.elem_flag(e)
}

/// Every element: vertices, then edges, then faces.
fn all_elems(bm: &BMesh) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = bm.vert_ids().into_iter().map(BMElem::Vert).collect();
    out.extend(bm.edge_ids().into_iter().map(BMElem::Edge));
    out.extend(bm.face_ids().into_iter().map(BMElem::Face));
    out
}

/// `EDBM_flag_enable_all(em, BM_ELEM_SELECT)`: hidden elements are skipped.
pub(crate) fn flag_enable_all_select(bm: &mut BMesh) {
    for e in all_elems(bm) {
        if !flag(bm, e).contains(ElemFlag::HIDDEN) {
            bm.select_set(e, true);
        }
    }
}

/// `EDBM_flag_disable_all(em, BM_ELEM_SELECT)`: every element (hidden ones too), the
/// selection history cleared.
pub(crate) fn flag_disable_all_select(bm: &mut BMesh) {
    bm.deselect_all();
}

/// `EDBM_select_swap` (then the caller's `EDBM_selectmode_flush`).
pub(crate) fn select_swap(bm: &mut BMesh) {
    let elems: Vec<BMElem> = if bm.select_mode.contains(SelectMode::VERT) {
        bm.vert_ids().into_iter().map(BMElem::Vert).collect()
    } else if bm.select_mode.contains(SelectMode::EDGE) {
        bm.edge_ids().into_iter().map(BMElem::Edge).collect()
    } else {
        bm.face_ids().into_iter().map(BMElem::Face).collect()
    };
    for e in elems {
        let f = flag(bm, e);
        if f.contains(ElemFlag::HIDDEN) {
            continue;
        }
        bm.select_set(e, !f.contains(ElemFlag::SELECT));
    }
}

/// The selected elements, vertices then edges then faces
/// (`BMO_slot_buffer_from_enabled_hflag(…, BM_ELEM_SELECT)`).
fn selected_geom(bm: &BMesh) -> Vec<BMElem> {
    all_elems(bm)
        .into_iter()
        .filter(|&e| bm.is_selected(e))
        .collect()
}

/// `EDBM_select_more(em, use_face_step)` (`contract = false`) / `EDBM_select_less`
/// (`contract = true`, with its final `BM_mesh_select_mode_clean`).
pub(crate) fn select_more_less(bm: &mut BMesh, contract: bool, use_face_step: bool) {
    let use_faces = bm.select_mode == SelectMode::FACE;
    let geom = selected_geom(bm);
    let Ok(out) = region_extend(
        bm,
        &RegionExtendIn {
            geom,
            use_contract: contract,
            use_faces,
            use_face_step,
        },
    ) else {
        return;
    };
    for e in out.geom {
        if use_faces {
            bm.select_set(e, !contract);
        }
        let f = bm.elem_flag_mut(e);
        if contract {
            f.remove(ElemFlag::SELECT);
        } else if !f.contains(ElemFlag::HIDDEN) {
            f.insert(ElemFlag::SELECT);
        }
    }
    bm.select_flush_mode();
    if contract {
        select_mode_clean(bm);
    }
}

/// `BM_mesh_select_mode_clean`.
fn select_mode_clean(bm: &mut BMesh) {
    let mode = bm.select_mode;
    if mode.contains(SelectMode::VERT) {
        return;
    }
    let (tv, te, tf) = bm.selected_counts();
    if mode.contains(SelectMode::EDGE) {
        if tv != 0 {
            for v in bm.vert_ids() {
                bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
            }
        }
        if te != 0 {
            for e in bm.edge_ids() {
                if bm.is_selected(e.into()) {
                    let [a, b] = bm.edge(e).v;
                    bm.vert_select_set(a, true);
                    bm.vert_select_set(b, true);
                }
            }
        }
    } else if mode.contains(SelectMode::FACE) {
        if tv != 0 {
            for v in bm.vert_ids() {
                bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
            }
        }
        if te != 0 {
            for e in bm.edge_ids() {
                bm.edge_mut(e).flag.remove(ElemFlag::SELECT);
            }
        }
        if tf != 0 {
            for f in bm.face_ids() {
                if bm.is_selected(f.into()) {
                    for e in bm.face_edges(f) {
                        bm.edge_select_set(e, true);
                    }
                }
            }
        }
    }
}

/// The element kind of the highest-priority select mode.
fn primary_mode(m: SelectMode) -> SelectMode {
    if m.contains(SelectMode::VERT) {
        SelectMode::VERT
    } else if m.contains(SelectMode::EDGE) {
        SelectMode::EDGE
    } else {
        SelectMode::FACE
    }
}

/// `EDBM_mesh_hide(em, swap)`: returns whether anything was hidden.
pub(crate) fn mesh_hide(bm: &mut BMesh, swap: bool) -> bool {
    let mode = primary_mode(bm.select_mode);
    let elems: Vec<BMElem> = if mode == SelectMode::VERT {
        bm.vert_ids().into_iter().map(BMElem::Vert).collect()
    } else if mode == SelectMode::EDGE {
        bm.edge_ids().into_iter().map(BMElem::Edge).collect()
    } else {
        bm.face_ids().into_iter().map(BMElem::Face).collect()
    };
    let mut changed = false;
    for e in elems {
        let f = flag(bm, e);
        if !f.contains(ElemFlag::HIDDEN) && (f.contains(ElemFlag::SELECT) != swap) {
            bm.elem_hide_set(e, true);
            changed = true;
        }
    }
    if swap {
        if mode == SelectMode::FACE {
            for e in bm.edge_ids() {
                if !bm.edge_is_wire(e) {
                    continue;
                }
                let f = bm.edge(e).flag;
                if !f.contains(ElemFlag::HIDDEN) && !f.contains(ElemFlag::SELECT) {
                    bm.elem_hide_set(BMElem::Edge(e), true);
                    changed = true;
                }
            }
        }
        if mode != SelectMode::VERT {
            for v in bm.vert_ids() {
                if bm.vert(v).e.is_some() {
                    continue;
                }
                let f = bm.vert(v).flag;
                if !f.contains(ElemFlag::HIDDEN) && !f.contains(ElemFlag::SELECT) {
                    bm.elem_hide_set(BMElem::Vert(v), true);
                    changed = true;
                }
            }
        }
    }
    if changed {
        bm.select_flush_mode();
    }
    changed
}

/// `EDBM_mesh_reveal(em, select)`: returns whether anything was hidden.
pub(crate) fn mesh_reveal(bm: &mut BMesh, select: bool) -> bool {
    let hidden = |bm: &BMesh, all: Vec<BMElem>| -> Vec<BMElem> {
        all.into_iter()
            .filter(|&e| flag(bm, e).contains(ElemFlag::HIDDEN))
            .collect()
    };
    let verts = hidden(bm, bm.vert_ids().into_iter().map(BMElem::Vert).collect());
    let edges = hidden(bm, bm.edge_ids().into_iter().map(BMElem::Edge).collect());
    let faces = hidden(bm, bm.face_ids().into_iter().map(BMElem::Face).collect());
    if verts.is_empty() && edges.is_empty() && faces.is_empty() {
        return false;
    }
    for e in all_elems(bm) {
        bm.elem_flag_mut(e).remove(ElemFlag::HIDDEN);
    }
    let mode = bm.select_mode;
    for (bit, elems) in [
        (SelectMode::VERT, verts),
        (SelectMode::EDGE, edges),
        (SelectMode::FACE, faces),
    ] {
        if !mode.contains(bit) {
            continue;
        }
        for e in elems {
            bm.select_set(e, select);
        }
    }
    bm.select_flush_mode();
    bm.normals_update();
    true
}
