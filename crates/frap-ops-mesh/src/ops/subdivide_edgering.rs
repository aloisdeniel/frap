// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.subdivide_edgering(number_cuts=10, interpolation='PATH', smoothness=1,
//! profile_shape_factor=0, profile_shape='SMOOTH')`, tier T1 (`tests/golden/cases/edit/
//! subdivide/subdivide_edgering_*.py`). Port of `edbm_subdivide_edge_ring_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `frap_bmesh::ops::subdivide_edgering`.
//!
//! The selected edges are a ring between two edge loops (or several rings between several
//! loops): each ring edge is cut `number_cuts` times and the faces between the loops are
//! sliced, the new vertices placed along a straight line (`LINEAR`), a blended path between
//! the loop centres (`PATH`) or per-edge surface curves (`SURFACE`), shaped by the profile.
//! Every edit mesh with a selected edge takes part. A selection that is not a ring between
//! loops reports `No edge rings found` (or `Edge-ring pair isn't connected`, `Edge-rings are
//! not connected`) as INFO and leaves that mesh unchanged; the operator still finishes.

use frap_bmesh::ops::subdivide_edgering::{SubdivideEdgeringIn, subdivide_edgering};
use frap_bmesh::{BmError, EdgeId, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = ["a selection that is not an edge ring between two loops reports 'No edge rings found' as INFO and changes nothing"]
)]
fn exec(cx: &mut OpCtx, p: mesh::SubdivideEdgering) -> OpResult {
    // `--max-elements`: `cuts + 1` faces per face between the loops (ARCHITECTURE §12).
    let cuts = u64::try_from(p.number_cuts.value.max(0)).unwrap_or(0);
    super::_growth::reserve(cx, |_, bm| super::_growth::ring_cut_corners(bm, cuts))?;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let any = {
            let (_, edit) = cx.split_edit();
            edit.meshes
                .get(&me.key())
                .is_some_and(|em| em.bm.selected_counts().1 > 0)
        };
        if !any {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let edges: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| {
                let f = bm.edge(e).flag;
                f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN)
            })
            .collect();
        let r = subdivide_edgering(
            bm,
            &SubdivideEdgeringIn {
                edges,
                interp_mode: p.interpolation.value.identifier(),
                smooth: p.smoothness.value,
                cuts: p.number_cuts.value,
                profile_shape: p.profile_shape.value.identifier(),
                profile_shape_factor: p.profile_shape_factor.value,
            },
        );
        match r {
            Ok(_) => {}
            // `EDBM_op_finish`: a cancelled bmesh op is an INFO report.
            Err(BmError::Cancel(msg)) => cx.report(ReportType::Info, msg),
            Err(e) => return Err(OpError::Runtime(e.to_string())),
        }
    }
    Ok(OpStatus::Finished)
}
