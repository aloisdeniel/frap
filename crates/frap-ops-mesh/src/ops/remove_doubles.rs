// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.remove_doubles(threshold=1e-4, use_centroid=True, use_unselected=False,
//! use_sharp_edge_from_normals=False)` ("Merge by Distance"), tier T1
//! (`tests/golden/cases/edit/delete/remove_doubles*.py`). Port of `edbm_remove_doubles_exec`
//! (`editors/mesh/editmesh_tools.cc`) and `EDBM_automerge` (`editors/mesh/
//! editmesh_automerge.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode with selected vertices: the selection of the highest-priority
//! select mode is stored in tags; `bmesh.ops.find_doubles` clusters the selected vertices
//! within `threshold` (with `use_unselected`: every vertex, merging only into unselected
//! ones) and `bmesh.ops.weld_verts` merges each cluster (into its centroid with
//! `use_centroid`); the selection is restored from the tags and flushed. Reports `Removed N
//! vertex` / `Removed N vertices` (INFO, all meshes together) and returns the count as
//! `result.removed`. Always `FINISHED`.
//!
//! `use_sharp_edge_from_normals` only affects custom normals, which frap's edit meshes do not
//! carry yet; it is accepted and has no effect.

use frap_bmesh::ops::find_doubles::{FindDoublesIn, find_doubles};
use frap_bmesh::ops::weld_verts::{WeldVertsIn, weld_verts};
use frap_bmesh::{BMElem, BMesh, ElemFlag, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, removed_vertices_report, sel_verts, totals};

/// The elements of one type, in pool order.
fn elems_of(bm: &BMesh, mode: SelectMode) -> Vec<BMElem> {
    if mode == SelectMode::VERT {
        bm.vert_ids().into_iter().map(BMElem::Vert).collect()
    } else if mode == SelectMode::EDGE {
        bm.edge_ids().into_iter().map(BMElem::Edge).collect()
    } else {
        bm.face_ids().into_iter().map(BMElem::Face).collect()
    }
}

/// `BM_mesh_elem_hflag_enable_test(bm, htype, hflag, respecthide = true, overwrite = true,
/// hflag_test)` for `hflag` `TAG` (from `SELECT`) or `SELECT` (from `TAG`).
fn hflag_enable_test(bm: &mut BMesh, htype: SelectMode, to_select: bool) {
    let (set, test) = if to_select {
        (ElemFlag::SELECT, ElemFlag::TAG)
    } else {
        (ElemFlag::TAG, ElemFlag::SELECT)
    };
    for e in elems_of(bm, htype) {
        let f = bm.elem_flag(e);
        if f.contains(ElemFlag::HIDDEN) {
            continue;
        }
        let matched = f.contains(test);
        if set == ElemFlag::SELECT {
            bm.select_set(e, matched);
        } else {
            bm.elem_flag_mut(e).set(set, matched);
        }
    }
}

/// `edbm_automerge_impl(…, BM_ELEM_SELECT, dist, use_connected = false, use_centroid)`:
/// selected vertices merge into unselected ones.
fn automerge(bm: &mut BMesh, dist: f32, use_centroid: bool) -> Result<(), OpError> {
    let verts: Vec<VertId> = bm.vert_ids();
    let keep_verts: Vec<VertId> = bm
        .verts
        .iter()
        .filter(|(_, v)| !v.flag.contains(ElemFlag::SELECT) && !v.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| VertId(i))
        .collect();
    let found = find_doubles(
        bm,
        &FindDoublesIn {
            verts,
            keep_verts,
            use_connected: false,
            dist,
        },
    )
    .map_err(bm_err)?;
    weld_verts(
        bm,
        &WeldVertsIn {
            targetmap: found.targetmap,
            use_centroid,
            average_vert_data: false,
        },
    )
    .map_err(bm_err)?;
    Ok(())
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "INFO report 'Removed N vertices' ('Removed 1 vertex') and result.removed = N, summed over the edit meshes",
        "clusters are not chained: a vertex merges into the cluster target found first (kd-tree order), as in Blender",
        "use_sharp_edge_from_normals has no effect (no custom normals in Edit Mode yet)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::RemoveDoubles) -> OpResult {
    let threshold = p.threshold.value;
    let use_unselected = p.use_unselected.value;
    let use_centroid = p.use_centroid.value;
    let mut count_multi: i64 = 0;
    let mut removed = Removed::default();
    for (_, me) in helpers::iter_edit_meshes(cx) {
        // Selection used as target with `use_unselected`.
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().0 == 0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        // Avoid losing selection state (select -> tags).
        let htype_select = if bm.select_mode.contains(SelectMode::VERT) {
            SelectMode::VERT
        } else if bm.select_mode.contains(SelectMode::EDGE) {
            SelectMode::EDGE
        } else {
            SelectMode::FACE
        };
        hflag_enable_test(bm, htype_select, false);
        if use_unselected {
            automerge(bm, threshold, use_centroid)?;
        } else {
            let found = find_doubles(
                bm,
                &FindDoublesIn {
                    verts: sel_verts(bm),
                    keep_verts: Vec::new(),
                    use_connected: false,
                    dist: threshold,
                },
            )
            .map_err(bm_err)?;
            weld_verts(
                bm,
                &WeldVertsIn {
                    targetmap: found.targetmap,
                    use_centroid,
                    average_vert_data: false,
                },
            )
            .map_err(bm_err)?;
        }
        let new = totals(bm);
        count_multi += old[0] - new[0];
        // Restore selection from tags.
        hflag_enable_test(bm, htype_select, true);
        bm.select_flush_mode();
        removed.add(old, new);
    }
    cx.report(ReportType::Info, removed_vertices_report(count_multi));
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
