// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.space_edge_loops_evenly(factor=1.0, interpolation='CUBIC',
//! lock=(False, False, False))` (new in 5.x), tier T1
//! (`tests/golden/cases/edit/tools_m8/space_edge_loops_evenly.py`). Port of
//! `edbm_space_edge_loops_evenly_exec` (`editors/mesh/editmesh_space_edge_loops_evenly.cc`)
//! and `bmo_space_edge_loops_evenly_exec` ([`bmo`]), Blender 5.2.4, GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with at least two selected edges of which two share a vertex,
//! the selected visible edges are walked into chains whose vertices are spaced evenly (see
//! [`bmo`]); the normals are updated. Nothing changed: WARNING `No edges selected` (no
//! selected edge anywhere) or `No edge loops found containing 2 or more edges`, and
//! `CANCELLED`.

use frap_bmesh::{BMesh, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, space_edge_loops_evenly::Interpolation as Interp};

use super::split::edbm;

pub(crate) mod bmo;

use bmo::{Interpolation, SpaceIn};

/// `has_connected_selected_edges`: a selected edge with a vertex that has two or more selected
/// edges.
fn has_connected_selected_edges(bm: &BMesh) -> bool {
    let sel_edges_at = |v| {
        bm.vert_edges(v)
            .into_iter()
            .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT))
            .count()
    };
    bm.edges.iter().any(|(_, e)| {
        e.flag.contains(ElemFlag::SELECT)
            && (sel_edges_at(e.v[0]) >= 2 || sel_edges_at(e.v[1]) >= 2)
    })
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SpaceEdgeLoopsEvenly) -> OpResult {
    let interpolation = match p.interpolation.value {
        Interp::Cubic => Interpolation::Cubic,
        Interp::Linear => Interpolation::Linear,
    };
    let mut has_edges_selected = false;
    let mut changed_multi = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let Some((totedgesel, connected)) = edbm::read(cx, me, |bm| {
            (bm.selected_counts().1, has_connected_selected_edges(bm))
        }) else {
            continue;
        };
        if totedgesel > 0 {
            has_edges_selected = true;
        }
        // At least 2 connected edges are needed to form a chain.
        if totedgesel < 2 || !connected {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_edges(bm);
        bmo::space_edge_loops_evenly(
            bm,
            &SpaceIn {
                geom,
                interpolation,
                factor: p.factor.value,
                lock: p.lock.value,
            },
        );
        bm.normals_update();
        changed_multi = true;
    }
    if !changed_multi {
        cx.report(
            ReportType::Warning,
            if has_edges_selected {
                "No edge loops found containing 2 or more edges"
            } else {
                "No edges selected"
            },
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
