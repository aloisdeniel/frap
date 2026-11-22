// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.flatten(factor=1.0, method='BEST_FIT', lock=(False, False, False))` (new in
//! 5.x), tier T1 (`tests/golden/cases/edit/tools_m8/flatten.py`). Port of `edbm_flatten_exec`
//! (`editors/mesh/editmesh_flatten.cc`) and `bmo_flatten_exec` ([`bmo`]), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode: the selected faces (grouped by shared edges) and the selected
//! edges outside them (grouped by shared vertices) are each flattened onto one plane (see
//! [`bmo`]). `VIEW` uses the virtual 3D View's viewing axis (`viewinv[2]`, D-10) in the
//! object's space. The normals are updated. Always `FINISHED` with an edit mesh (Blender's
//! operator cancels only when the bmesh operator fails).

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh::{self, flatten::Method};

use super::split::edbm;

pub(crate) mod bmo;

use bmo::{FlattenIn, FlattenMethod};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Flatten) -> OpResult {
    let method = match p.method.value {
        Method::BestFit => FlattenMethod::BestFit,
        Method::Normal => FlattenMethod::Normal,
        Method::View => FlattenMethod::View,
    };
    let viewinv = cx.ctx().win.view3d.view_matrix.inverse();
    let mut changed = false;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let mut view_normal = [0.0f32, 0.0, 1.0];
        if method == FlattenMethod::View {
            // `normalize(transform_direction(world_to_object, viewinv[2]))`.
            let imat = world_matrix_in(cx.main(), ob).inverse();
            let d = imat.transform_vector3(viewinv.z_axis.truncate());
            view_normal = super::rip_edge::math::normalize3(d.to_array());
        }
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_geom(bm);
        bmo::flatten(
            bm,
            &FlattenIn {
                geom,
                factor: p.factor.value,
                method,
                view_normal,
                lock: p.lock.value,
            },
        );
        bm.normals_update();
        changed = true;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
