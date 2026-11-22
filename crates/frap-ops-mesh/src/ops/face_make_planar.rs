// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.face_make_planar(factor=1.0, repeat=1)`, tier T2 (tol 1e-5,
//! `tests/golden/cases/edit/tools_m8/face_make_planar.py`). Port of
//! `edbm_face_make_planar_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with a selected face: `bmesh.ops.planar_faces` on the
//! selected visible faces with `iterations=repeat` (`frap_bmesh::ops::planar_faces`), then
//! the normals are updated. A mesh whose active shape key is locked is skipped with the ERROR
//! `The active shape key of <object> is locked` (a `RuntimeError` when no other mesh was
//! processed). `CANCELLED` when no mesh had a selected face. Iterative (each pass averages per-vertex projections), hence tier T2.

use frap_bmesh::ops::planar_faces::{PlanarFacesIn, planar_faces};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::blend_from_shape::active_block_locked;
use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::FaceMakePlanar) -> OpResult {
    let mut totobjects = 0;
    let mut locked_msg = String::new();
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        if edbm::read(cx, me, edbm::sel_counts).unwrap_or_default().2 == 0 {
            continue;
        }
        if active_block_locked(cx, ob) {
            let name = cx.main().objects.name_of(ob).unwrap_or_default().to_owned();
            locked_msg = format!("The active shape key of {name} is locked");
            cx.report(ReportType::Error, locked_msg.clone());
            continue;
        }
        totobjects += 1;
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::selected_faces(bm);
        let r = planar_faces(
            bm,
            &PlanarFacesIn {
                faces,
                iterations: p.repeat.value,
                factor: p.factor.value,
            },
        );
        if edbm::finish(cx, r)?.is_none() {
            continue;
        }
        cx.bmesh_mut(me)?.normals_update();
    }
    if totobjects == 0 && !locked_msg.is_empty() {
        // Blender's error report makes the call raise.
        return Err(OpError::Runtime(locked_msg));
    }
    Ok(if totobjects > 0 {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
