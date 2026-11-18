// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.solidify(thickness=0.01)`, tier T2 (`tests/golden/cases/edit/misc/
//! solidify.py`). Port of `edbm_solidify_exec` (`editors/mesh/editmesh_tools.cc`, Blender
//! 5.2.4, GPL-2.0-or-later). Edit-mode solidify, not the SOLIDIFY modifier.
//!
//! For every mesh in Edit Mode with a selected face: the selected faces are deselected
//! (wire edges and loose vertices stay selected), `bmesh.ops.solidify` extrudes them into an
//! inner shell `thickness` away (`frap_bmesh::ops::solidify`, using `extrude_face_region` of
//! WP 2.5 and `reverse_faces` of WP 2.9), the new shell's faces are selected and the
//! selection history is cleared. Default cube, all faces: V16 F12 ([MESH §4.12]).

use frap_bmesh::ops::solidify::{SolidifyIn, solidify};
use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::Solidify) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (_, _, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let geom: Vec<BMElem> = edbm::selected_faces(bm)
            .into_iter()
            .map(BMElem::Face)
            .collect();
        // Deselect only the faces of the region (wire edges and loose vertices stay
        // selected: there is no corresponding new geometry to select).
        edbm::deselect_elems(bm, &geom);
        let r = solidify(
            bm,
            &SolidifyIn {
                geom,
                thickness: p.thickness.value,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        let faces: Vec<BMElem> = out
            .geom
            .iter()
            .copied()
            .filter(|g| matches!(g, BMElem::Face(_)))
            .collect();
        edbm::select_elems(bm, &faces);
        // No need to flush the selection: any selection history is no longer valid.
        bm.select_history.clear();
    }
    Ok(OpStatus::Finished)
}
