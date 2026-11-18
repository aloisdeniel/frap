// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.wireframe(use_boundary=True, use_even_offset=True, use_relative_offset=False,
//! use_replace=True, thickness=0.01, offset=0.01, use_crease=False, crease_weight=0.01)`,
//! tier T1 (`tests/golden/cases/edit/misc/wireframe_*.py`). Port of `edbm_wireframe_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selected face: `bmesh.ops.wireframe` of the selected
//! faces (`frap_bmesh::ops::wireframe`), then every element is deselected (history cleared)
//! and the wire faces are selected. Default cube: V40 E96 F48 ([MESH §4.12]).

use frap_bmesh::ops::wireframe::{WireframeIn, wireframe};
use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Wireframe) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (_, _, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::selected_faces(bm);
        let r = wireframe(
            bm,
            &WireframeIn {
                faces,
                thickness: p.thickness.value,
                offset: p.offset.value,
                use_replace: p.use_replace.value,
                use_boundary: p.use_boundary.value,
                use_even_offset: p.use_even_offset.value,
                use_crease: p.use_crease.value,
                crease_weight: p.crease_weight.value,
                use_relative_offset: p.use_relative_offset.value,
                material_offset: 0,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        edbm::deselect_all(bm);
        let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
        edbm::select_elems(bm, &faces);
    }
    Ok(OpStatus::Finished)
}
