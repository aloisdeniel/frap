// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.split()`, tier T1 (`tests/golden/cases/edit/misc/split_*.py`). Port of
//! `edbm_split_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selection (multi-object edit): `bmesh.ops.split` on the
//! selected vertices, edges and faces (`frap_bmesh::ops::split`: duplicate, then delete the
//! input with the `FACES` context), then every element is deselected (the selection history
//! is cleared) and the split-off copy is selected. The default cube's top face keeps index 5
//! with new vertices 8–11 ([MESH §4.11]). Custom split normals are not carried (frap's edit
//! BMesh has no custom normal spaces, deviations §2).

use frap_bmesh::ops::split::{SplitIn, split};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

pub(crate) mod edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::Split) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (v, e, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if v == 0 && e == 0 && f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_geom(bm);
        let r = split(
            bm,
            &SplitIn {
                geom,
                ..SplitIn::default()
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        edbm::deselect_all(bm);
        edbm::select_elems(bm, &out.geom);
    }
    Ok(OpStatus::Finished)
}
