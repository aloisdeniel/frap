// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.poke(offset=0, use_relative_offset=False, center_mode='MEDIAN_WEIGHTED')`,
//! tier T1 (`tests/golden/cases/edit/misc/poke_*.py`). Port of `edbm_poke_face_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selected face (multi-object edit): `bmesh.ops.poke` of
//! the selected faces (`frap_bmesh::ops::poke`: a centre vertex and a triangle fan per face),
//! then every element is deselected (history cleared) and the centre vertices and the fan
//! triangles are selected. Default cube top: centre vertex 8, triangles `[7,3,8] [3,1,8]
//! [1,5,8] [5,7,8]`, the first one taking face index 5 ([MESH §4.13]). The operator's
//! `MEDIAN_WEIGHTED`/`MEDIAN`/`BOUNDS` are the bmesh op's `MEAN_WEIGHTED`/`MEAN`/`BOUNDS`.

use frap_bmesh::BMElem;
use frap_bmesh::ops::poke::{PokeIn, poke};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, poke::CenterMode};

use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Poke) -> OpResult {
    let center_mode = match p.center_mode.value {
        CenterMode::MedianWeighted => "MEAN_WEIGHTED",
        CenterMode::Median => "MEAN",
        CenterMode::Bounds => "BOUNDS",
    };
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (_, _, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::selected_faces(bm);
        let r = poke(
            bm,
            &PokeIn {
                faces,
                offset: p.offset.value,
                center_mode,
                use_relative_offset: p.use_relative_offset.value,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        edbm::deselect_all(bm);
        let verts: Vec<BMElem> = out.verts.iter().map(|&v| BMElem::Vert(v)).collect();
        edbm::select_elems(bm, &verts);
        let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
        edbm::select_elems(bm, &faces);
    }
    Ok(OpStatus::Finished)
}
