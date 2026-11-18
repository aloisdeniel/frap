// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.quads_convert_to_tris(quad_method='BEAUTY', ngon_method='BEAUTY')`: tier T1
//! for `FIXED`/`FIXED_ALTERNATE` and the diagonal methods, T2 for `BEAUTY` (its edge-rotation
//! costs are float comparisons where a last-bit difference can flip a tie;
//! `tests/golden/cases/edit/misc/quads_convert_to_tris_*.py`). Port of
//! `edbm_quads_convert_to_tris_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selected face (multi-object edit):
//! `bmesh.ops.triangulate` of the selected faces (`frap_bmesh::ops::triangulate`: each face's
//! last triangle keeps the face slot, the others are appended), the triangles are selected,
//! triangles duplicating an existing one are killed, and the selection is flushed for the
//! select mode. The default cube becomes 12 triangles, `f0 = [1, 2, 0]` … `f6 = [1, 3, 2]`
//! with BEAUTY ([MESH §4.13]). Operator identifiers map to the bmesh ones: `FIXED_ALTERNATE`
//! → `ALTERNATE`, `SHORTEST_DIAGONAL` → `SHORT_EDGE`, `LONGEST_DIAGONAL` → `LONG_EDGE`,
//! `CLIP` → `EAR_CLIP`. Custom split normals are not carried (deviations §2).

use frap_bmesh::ops::triangulate::{TriangulateIn, triangulate};
use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{
    self,
    quads_convert_to_tris::{NgonMethod, QuadMethod},
};

use super::split::edbm;

/// The `bmesh.ops.triangulate` identifier of an operator `quad_method`.
pub(crate) fn quad_method_id(m: QuadMethod) -> &'static str {
    match m {
        QuadMethod::Beauty => "BEAUTY",
        QuadMethod::Fixed => "FIXED",
        QuadMethod::FixedAlternate => "ALTERNATE",
        QuadMethod::ShortestDiagonal => "SHORT_EDGE",
        QuadMethod::LongestDiagonal => "LONG_EDGE",
    }
}

/// The `bmesh.ops.triangulate` identifier of an operator `ngon_method`.
pub(crate) fn ngon_method_id(m: NgonMethod) -> &'static str {
    match m {
        NgonMethod::Beauty => "BEAUTY",
        NgonMethod::Clip => "EAR_CLIP",
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = ["FIXED, FIXED_ALTERNATE and the diagonal methods are index-exact (T1); BEAUTY is compared canonically (T2)"]
)]
fn exec(cx: &mut OpCtx, p: mesh::QuadsConvertToTris) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (_, _, f) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if f == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::selected_faces(bm);
        let r = triangulate(
            bm,
            &TriangulateIn {
                faces,
                quad_method: quad_method_id(p.quad_method.value),
                ngon_method: ngon_method_id(p.ngon_method.value),
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        // Select the output.
        let elems: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
        edbm::select_elems(bm, &elems);
        // Remove the doubles.
        for &(f, _) in &out.face_map_double {
            if bm.faces.contains(f.0) {
                bm.face_kill(f);
            }
        }
        bm.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}
