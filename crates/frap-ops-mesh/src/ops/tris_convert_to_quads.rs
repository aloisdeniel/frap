// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.tris_convert_to_quads(face_threshold=40°, shape_threshold=40°,
//! topology_influence=0, uvs=False, vcols=False, seam=False, sharp=False, materials=False,
//! deselect_joined=False)`, tier T2 (`tests/golden/cases/edit/misc/tris_convert_to_quads_*.py`).
//! Port of `edbm_tris_convert_to_quads_exec` (`editors/mesh/editmesh_tools.cc`, Blender
//! 5.2.4, GPL-2.0-or-later).
//!
//! When exactly two faces are selected over all meshes in Edit Mode, an unset
//! `face_threshold` / `shape_threshold` is 180° (no limit: a one-off join). For every mesh
//! with a selected face: `bmesh.ops.join_triangles` of the selected faces
//! (`frap_bmesh::ops::join_triangles`), then the joined quads are added to the selection (or,
//! with `deselect_joined`, the selection is replaced by the triangles left alone and flushed
//! for vertex/edge modes). A re-joined triangulated cube gives 6 quads with another face
//! order and loop start than the original ([MESH §4.13]), so goldens compare at T2.

use frap_bmesh::ops::join_triangles::{JoinTrianglesIn, join_triangles};
use frap_bmesh::{BMElem, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

/// `DEG2RADF(180.0f)`.
const DEG180: f32 = 180.0 * (std::f32::consts::PI / 180.0);

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::TrisConvertToQuads) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let mut faces_sel = 0usize;
    for &(_, me) in &meshes {
        faces_sel += edbm::read(cx, me, edbm::sel_counts).unwrap_or_default().2;
    }
    let is_face_pair = faces_sel == 2;
    // When joining exactly 2 faces, no limit: useful for one-off joins while editing.
    let angle_face_threshold = if is_face_pair && !p.face_threshold.is_set {
        DEG180
    } else {
        p.face_threshold.value
    };
    let angle_shape_threshold = if is_face_pair && !p.shape_threshold.is_set {
        DEG180
    } else {
        p.shape_threshold.value
    };
    let deselect_joined = p.deselect_joined.value;
    for (_, me) in meshes {
        let (_, _, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::selected_faces(bm);
        let r = join_triangles(
            bm,
            &JoinTrianglesIn {
                faces,
                cmp_seam: p.seam.value,
                cmp_sharp: p.sharp.value,
                cmp_uvs: p.uvs.value,
                cmp_vcols: p.vcols.value,
                cmp_materials: p.materials.value,
                angle_face_threshold,
                angle_shape_threshold,
                topology_influence: p.topology_influence.value,
                deselect_joined,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        // `EDBM_op_call_and_selectf(…, "faces.out", extend = !deselect_joined, …)`.
        if deselect_joined {
            edbm::deselect_all(bm);
        }
        let elems: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
        edbm::select_elems(bm, &elems);
        if deselect_joined && bm.select_mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
            // Failing to flush would leave an invalid selection.
            let mode = bm.select_mode;
            bm.select_flush_mode_ex(mode, false);
        }
    }
    Ok(OpStatus::Finished)
}
