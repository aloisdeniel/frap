// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.vertices_smooth_laplacian(repeat=1, lambda_factor=1.0, lambda_border=5e-05,
//! use_x=True, use_y=True, use_z=True, preserve_volume=True)`, tier T2 (tol 1e-5,
//! `tests/golden/cases/edit/tools_m8/vertices_smooth_laplacian.py`). Port of
//! `edbm_do_smooth_laplacian_vertex_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with a selected vertex, `repeat` times (0 counts as 1):
//! `bmesh.ops.smooth_laplacian_vert` on the selected visible vertices
//! (`frap_bmesh::ops::smooth_laplacian`). A mesh whose active shape key is locked is skipped
//! with the ERROR `The active shape key of <object> is locked`. Nothing selected anywhere (and
//! nothing locked): WARNING `No selected vertex`; `CANCELLED` when no mesh was smoothed. frap
//! meshes have no X-mirror editing (`use_mirror_x`), so Blender's mirror step never runs.
//! The linear solve is `faer`'s sparse LU instead of Eigen's `SparseLU` (same normal
//! equations in double precision), hence tier T2.

use frap_bmesh::ops::smooth_laplacian::{SmoothLaplacianIn, smooth_laplacian_vert};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::blend_from_shape::active_block_locked;
use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::VerticesSmoothLaplacian) -> OpResult {
    let repeat = if p.repeat.value == 0 {
        1
    } else {
        p.repeat.value
    };
    let (mut tot_selected, mut tot_locked) = (0, 0);
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        if edbm::read(cx, me, edbm::sel_counts).unwrap_or_default().0 == 0 {
            continue;
        }
        if active_block_locked(cx, ob) {
            let name = cx.main().objects.name_of(ob).unwrap_or_default().to_owned();
            cx.report(
                ReportType::Error,
                format!("The active shape key of {name} is locked"),
            );
            tot_locked += 1;
            continue;
        }
        tot_selected += 1;
        for _ in 0..repeat {
            let bm = cx.bmesh_mut(me)?;
            let verts = edbm::selected_verts(bm);
            let r = smooth_laplacian_vert(
                bm,
                &SmoothLaplacianIn {
                    verts,
                    lambda_factor: p.lambda_factor.value,
                    lambda_border: p.lambda_border.value,
                    use_x: p.use_x.value,
                    use_y: p.use_y.value,
                    use_z: p.use_z.value,
                    preserve_volume: p.preserve_volume.value,
                },
            );
            if edbm::finish(cx, r)?.is_none() {
                break;
            }
        }
    }
    if tot_selected == 0 && tot_locked == 0 {
        cx.report(ReportType::Warning, "No selected vertex");
    }
    Ok(if tot_selected > 0 {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
