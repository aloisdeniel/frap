// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.subdivide(number_cuts=1, smoothness=0, ngon=True, quadcorner='STRAIGHT_CUT',
//! fractal=0, fractal_along_normal=0, seed=0)`, tier T1 (`tests/golden/cases/edit/subdivide/
//! subdivide_*.py`). Port of `edbm_subdivide_exec` (`editors/mesh/editmesh_tools.cc`, Blender
//! 5.2.4, GPL-2.0-or-later) over `frap_bmesh::ops::subdivide_edges::mesh_esubdivide`
//! (`BM_mesh_esubdivide`).
//!
//! Every mesh in Edit Mode with a selected edge or face is subdivided (multi-object edit):
//! the selected edges are cut `number_cuts` times, fully cut quads and triangles get an inner
//! grid, `ngon=False` fills partially cut quads with triangles and quads (`quadcorner`
//! `STRAIGHT_CUT` then acts as `INNERVERT`), and the new inner elements join the selection.
//! `fractal` is divided by 2.5 before it reaches the bmesh op, as in Blender. The smoothing
//! falloff is always `LINEAR`.

use frap_bmesh::ops::subdivide_edges::{ESubdivide, SubdivSelect, mesh_esubdivide};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, subdivide::Quadcorner};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "new vertices: edge cuts in edge order (from each edge's first vertex), then grid centres in reversed face order",
        "ngon=False with quadcorner='STRAIGHT_CUT' uses INNERVERT, as Blender does",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Subdivide) -> OpResult {
    let cuts = p.number_cuts.value;
    let smooth = p.smoothness.value;
    let fractal = p.fractal.value / 2.5;
    let along_normal = p.fractal_along_normal.value;
    let use_quad_tri = !p.ngon.value;
    let mut quadcorner = p.quadcorner.value;
    if use_quad_tri && quadcorner == Quadcorner::StraightCut {
        quadcorner = Quadcorner::Innervert;
    }
    let args = ESubdivide {
        smooth,
        smooth_falloff: "LINEAR",
        use_smooth_even: false,
        fractal,
        along_normal,
        numcuts: cuts,
        seltype: SubdivSelect::Orig,
        cornertype: quadcorner.identifier(),
        use_single_edge: use_quad_tri,
        use_grid_fill: true,
        use_only_quads: false,
        seed: p.seed.value,
    };
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let bm = cx.bmesh_mut(me)?;
        let (_, te, tf) = bm.selected_counts();
        if te == 0 && tf == 0 {
            continue;
        }
        mesh_esubdivide(bm, &args).map_err(|e| OpError::Runtime(e.to_string()))?;
    }
    Ok(OpStatus::Finished)
}
