// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.symmetrize(direction='NEGATIVE_X', threshold=1e-4)`, tier T2
//! (`tests/golden/cases/edit/misc/symmetrize.py`). Port of `mesh_symmetrize_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selected vertex: `bmesh.ops.symmetrize` of the
//! selection (`frap_bmesh::ops::symmetrize`: bisect at the axis plane through the origin,
//! clear the side being replaced, mirror a copy of the rest, weld the copies of the cut
//! vertices; `bisect_plane` of WP 2.8, `reverse_faces` of WP 2.9, `weld_verts` of WP 2.7),
//! then every element is deselected (history cleared), the result is selected and the
//! selection is flushed for the select mode. `NEGATIVE_X` copies −X onto +X
//! (`-X` of the bmesh op), `POSITIVE_X` +X onto −X (`X`), likewise for Y and Z; `threshold`
//! snaps vertices that close to the plane onto it. Monkey `+X` → V507 F500 ([MESH §4.14]).
//! With `tool_settings.use_mesh_automerge` Blender also welds the selection by distance
//! afterwards; frap does not (deviations §2).

use frap_bmesh::ops::symmetrize::{SymmetrizeIn, symmetrize};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, symmetrize::Direction};

use super::split::edbm;

#[operator(
    poll = poll::EDITMESH,
    tier = T2,
    support = Partial("tool_settings.use_mesh_automerge is not applied afterwards")
)]
fn exec(cx: &mut OpCtx, p: mesh::Symmetrize) -> OpResult {
    let direction = match p.direction.value {
        Direction::NegativeX => "-X",
        Direction::NegativeY => "-Y",
        Direction::NegativeZ => "-Z",
        Direction::PositiveX => "X",
        Direction::PositiveY => "Y",
        Direction::PositiveZ => "Z",
    };
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (v, _, _) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if v == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let input = edbm::selected_geom(bm);
        let r = symmetrize(
            bm,
            &SymmetrizeIn {
                input,
                direction,
                dist: p.threshold.value,
                use_shapekey: false,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        edbm::deselect_all(bm);
        edbm::select_elems(bm, &out.geom);
        bm.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}
