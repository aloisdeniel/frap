// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.dissolve_edges(use_verts=True, angle_threshold=π, use_face_split=False,
//! use_preserve_quads=True)`, tier T1 (`tests/golden/cases/edit/delete/dissolve_edges*.py`).
//! Port of `edbm_dissolve_edges_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode with selected edges, `bmesh.ops.dissolve_edges` runs on the
//! selected (visible) edges: the two faces of each merge, and with `use_verts` the vertices
//! left between two edges (within `angle_threshold`) dissolve. When no mesh changed, the
//! WARNING `No edges dissolved` is reported. Always `FINISHED`.

use frap_bmesh::ops::dissolve_edges::{DissolveEdgesIn, dissolve_edges};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_edges, totals};

/// The arguments `edbm_dissolve_edges_exec` reads.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Args {
    pub use_verts: bool,
    pub use_face_split: bool,
    pub angle_threshold: f32,
    pub use_preserve_quads: bool,
}

/// The body of `edbm_dissolve_edges_exec` (shared with `mesh.dissolve_mode`).
pub(crate) fn run(cx: &mut OpCtx, a: Args) -> OpResult {
    let mut removed = Removed::default();
    let mut changed_multi = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().1 == 0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let i = DissolveEdgesIn {
            edges: sel_edges(bm),
            use_verts: a.use_verts,
            use_face_split: a.use_face_split,
            angle_threshold: a.angle_threshold,
            use_preserve_quads: a.use_preserve_quads,
        };
        dissolve_edges(bm, &i).map_err(bm_err)?;
        let new = totals(bm);
        removed.add(old, new);
        if old != new {
            changed_multi = true;
        }
    }
    if !changed_multi {
        cx.report(ReportType::Warning, "No edges dissolved");
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "angle_threshold is in radians (default pi: every remaining two-edge vertex dissolves)",
        "WARNING 'No edges dissolved' when nothing changed",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::DissolveEdges) -> OpResult {
    run(
        cx,
        Args {
            use_verts: p.use_verts.value,
            use_face_split: p.use_face_split.value,
            angle_threshold: p.angle_threshold.value,
            use_preserve_quads: p.use_preserve_quads.value,
        },
    )
}
