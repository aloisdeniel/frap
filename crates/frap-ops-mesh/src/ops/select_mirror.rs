// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_mirror(axis={'X'}, extend=False)`, tier T1 (`tests/golden/cases/edit/
//! select_m8/mirror*.py`). Port of `edbm_select_mirror_exec`, `EDBM_select_mirrored`,
//! `EDBM_verts_mirror_cache_begin_ex` (spatial table), `EDBM_verts_mirror_get(_edge/_face)`
//! (`editors/mesh/editmesh_select.cc`, `editmesh_utils.cc`) and `ED_mesh_report_mirror_ex`
//! (`mesh_data.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with selected vertices and every axis of `axis` (X, Y, Z in
//! that order): the selection of the select mode's element kind is remembered, the mesh is
//! deselected unless `extend`, and each remembered visible element whose mirror (each vertex
//! mirrored across the axis, matched within 0.00002 by Blender's kd-tree of all vertices; an
//! edge or face whose vertices all have mirrors and exist mirrored) exists and is visible gets
//! selected. A later axis starts from the selection the previous one left. The report is
//! Blender's: `<n> vertices/edges/faces mirrored` (INFO), `…, <m> failed` (WARNING), with the
//! counts of the last axis of each mesh summed over the meshes and the element name of the
//! active edit mesh's select mode. Topology mirror (`Mesh.use_mirror_topology`) is not bound;
//! the spatial table is always used.

use frap_bmesh::{BMElem, BMesh, ElemFlag, SelectMode, VertId};
use frap_geom::kdtree::KdTree;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::loop_select::edit_select_mode;
use super::select_more::common;

/// `BM_SEARCH_MAXDIST_MIRR`.
const MAXDIST: f32 = 0.00002;

/// `EDBM_verts_mirror_cache_begin_ex(em, axis, use_self = true, use_select = true,
/// respecthide = false, use_topology = false)`: the mirror index of every vertex slot (the
/// values of unselected vertices are only meaningful when a selected vertex wrote them).
fn mirror_cache(bm: &BMesh, verts: &[VertId], axis: usize) -> Vec<i32> {
    let mut tree = KdTree::new(verts.len());
    for (i, &v) in verts.iter().enumerate() {
        tree.insert(i as i32, bm.vert(v).co);
    }
    tree.balance();
    let mut index = vec![-1i32; verts.len()];
    for (i, &v) in verts.iter().enumerate() {
        if !common::selected(bm, v) {
            continue;
        }
        let mut co = bm.vert(v).co;
        co[axis] *= -1.0;
        let v_mirr = tree.find_nearest(co).and_then(|n| {
            let t = bm.vert(verts[n.index as usize]).co;
            let d = [co[0] - t[0], co[1] - t[1], co[2] - t[2]];
            let len_sq = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            (len_sq < MAXDIST * MAXDIST).then_some(n.index as usize)
        });
        match v_mirr {
            Some(i_mirr) => {
                index[i] = i_mirr as i32;
                index[i_mirr] = i as i32;
            }
            None => index[i] = -1,
        }
    }
    index
}

/// `EDBM_select_mirrored`: `(totmirr, totfail)`.
fn select_mirrored(bm: &mut BMesh, axis: usize, extend: bool) -> (usize, usize) {
    let verts = bm.vert_ids();
    let mut slot_index = vec![usize::MAX; bm.verts.capacity()];
    for (i, v) in verts.iter().enumerate() {
        slot_index[v.slot()] = i;
    }
    let mode = bm.select_mode;
    let primary = common::primary_mode(mode);
    // Flush (select -> tag).
    let tagged: Vec<BMElem> = match primary {
        m if m == SelectMode::VERT => verts
            .iter()
            .filter(|&&v| common::selected(bm, v))
            .map(|&v| BMElem::Vert(v))
            .collect(),
        m if m == SelectMode::EDGE => bm
            .edge_ids()
            .into_iter()
            .filter(|&e| common::selected(bm, e))
            .map(BMElem::Edge)
            .collect(),
        _ => bm
            .face_ids()
            .into_iter()
            .filter(|&f| common::selected(bm, f))
            .map(BMElem::Face)
            .collect(),
    };
    let index = mirror_cache(bm, &verts, axis);
    let mirror_of = |v: VertId| -> Option<VertId> {
        let i = index[slot_index[v.slot()]];
        (i >= 0 && (i as usize) < verts.len()).then(|| verts[i as usize])
    };
    if !extend {
        common::deselect_all(bm);
    }
    let (mut totmirr, mut totfail) = (0, 0);
    for ele in tagged {
        if common::hidden(bm, ele) {
            continue;
        }
        let mirr: Option<BMElem> = match ele {
            BMElem::Vert(v) => mirror_of(v).map(BMElem::Vert),
            BMElem::Edge(e) => {
                let [a, b] = bm.edge(e).v;
                match (mirror_of(a), mirror_of(b)) {
                    (Some(ma), Some(mb)) if ma != mb => bm.edge_exists(ma, mb).map(BMElem::Edge),
                    _ => None,
                }
            }
            BMElem::Face(f) => {
                let mut mv = Vec::new();
                let mut ok = true;
                for l in bm.face_loops(f) {
                    match mirror_of(bm.lp(l).v) {
                        Some(m) => mv.push(m),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    bm.face_exists(&mv).map(BMElem::Face)
                } else {
                    None
                }
            }
        };
        match mirr {
            Some(m) if !bm.elem_flag(m).contains(ElemFlag::HIDDEN) => {
                bm.select_set(m, true);
                totmirr += 1;
            }
            _ => totfail += 1,
        }
    }
    (totmirr, totfail)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "always the spatial mirror table (Mesh.use_mirror_topology is not bound)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectMirror) -> OpResult {
    let axes: Vec<usize> = ["X", "Y", "Z"]
        .iter()
        .enumerate()
        .filter(|(_, a)| p.axis.value.iter().any(|x| x == **a))
        .map(|(i, _)| i)
        .collect();
    let select_mode = edit_select_mode(cx);
    let (mut tot_mirr, mut tot_fail) = (0usize, 0usize);
    for (_, me) in common::edit_meshes(cx) {
        let any = common::read(cx, me, |bm| bm.selected_counts().0 > 0).unwrap_or(false);
        if !any {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let (mut mirr_iter, mut fail_iter) = (0, 0);
        for &axis in &axes {
            (mirr_iter, fail_iter) = select_mirrored(bm, axis, p.extend.value);
        }
        if mirr_iter > 0 {
            common::selectmode_flush(bm);
        }
        tot_mirr += mirr_iter;
        tot_fail += fail_iter;
    }
    if tot_mirr > 0 || tot_fail > 0 {
        let elem = if select_mode.contains(SelectMode::VERT) {
            "vertices"
        } else if select_mode.contains(SelectMode::EDGE) {
            "edges"
        } else {
            "faces"
        };
        if tot_fail > 0 {
            cx.report(
                ReportType::Warning,
                format!("{tot_mirr} {elem} mirrored, {tot_fail} failed"),
            );
        } else {
            cx.report(ReportType::Info, format!("{tot_mirr} {elem} mirrored"));
        }
    }
    Ok(OpStatus::Finished)
}
