// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.decimate(ratio=1.0, use_vertex_group=False, vertex_group_factor=1.0,
//! invert_vertex_group=False, use_symmetry=False, symmetry_axis='Y')`, tier T2
//! (`tests/golden/cases/modifiers/bevel_decimate/op_*.py`). Port of `edbm_decimate_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `frap_bmesh::ops::decimate_collapse` (`BM_mesh_decimate_collapse`, the DECIMATE modifier's
//! COLLAPSE mode).
//!
//! The selected vertices weigh 1 (with `use_vertex_group`, their weight in the active vertex
//! group, `invert_vertex_group` inverting it), every other vertex 0, so only edges between
//! selected vertices collapse. With a partial selection `ratio` is rescaled to the faces that
//! touch the selection (n-gons counted as their triangles). Every mesh in Edit Mode with a
//! selected edge takes part (unique data). `ratio=1` (the default) does nothing. Afterwards the
//! selection is flushed (through edges when neither vertex nor edge select mode is on) and the
//! normals are recalculated.
//!
//! Without vertex weights in the mesh, `use_vertex_group` reports Blender's WARNING `No active
//! vertex group` and falls back to the selection.

use frap_bmesh::ops::decimate_collapse::{DecimateCollapseIn, decimate_collapse};
use frap_bmesh::{ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, decimate::SymmetryAxis};

/// `symmetry_eps` of the operator.
const SYMMETRY_EPS: f32 = 0.00002;

#[operator(
    poll = poll::EDITMESH,
    tier = T2,
    notes = [
        "ratio defaults to 1, which does nothing",
        "only edges between selected vertices collapse; a partial selection rescales ratio to the faces touching it",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Decimate) -> OpResult {
    let ratio = p.ratio.value;
    let vertex_group_factor = p.vertex_group_factor.value;
    let invert_vertex_group = p.invert_vertex_group.value;
    let symmetry_axis = if p.use_symmetry.value {
        match p.symmetry_axis.value {
            SymmetryAxis::X => 0,
            SymmetryAxis::Y => 1,
            SymmetryAxis::Z => 2,
        }
    } else {
        -1
    };

    // Nop.
    if ratio == 1.0 {
        return Ok(OpStatus::Finished);
    }

    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let defbase_act = cx
            .main()
            .objects
            .get(ob)
            .map_or(-1, |o| o.vertex_groups_active_index);
        let has_edge_sel = {
            let (_, edit) = cx.split_edit();
            edit.meshes
                .get(&me.key())
                .is_some_and(|em| em.bm.selected_counts().1 > 0)
        };
        if !has_edge_sel {
            continue;
        }
        let mut use_vertex_group = p.use_vertex_group.value;
        let has_dvert = cx.bmesh_mut(me)?.vdata.deform.is_some();
        if use_vertex_group && !has_dvert {
            cx.report(ReportType::Warning, "No active vertex group");
            use_vertex_group = false;
        }
        let bm = cx.bmesh_mut(me)?;

        // The weights by vertex index (iteration order).
        let verts = bm.vert_ids();
        let vweights: Vec<f32> = verts
            .iter()
            .map(|&v| {
                if !bm.vert(v).flag.contains(ElemFlag::SELECT) {
                    return 0.0;
                }
                if !use_vertex_group {
                    return 1.0;
                }
                // `BKE_defvert_find_weight(dv, defbase_act)`.
                let w = bm
                    .vdata
                    .deform
                    .as_ref()
                    .and_then(|d| d.get(v.slot()))
                    .and_then(|dv| {
                        dv.weights
                            .iter()
                            .find(|w| i64::from(w.0) == i64::from(defbase_act))
                    })
                    .map_or(0.0, |w| w.1);
                if invert_vertex_group { 1.0 - w } else { w }
            })
            .collect();

        let (_, _, totfacesel) = bm.selected_counts();
        let ratio_adjust = if bm.faces_num() == totfacesel || ratio == 0.0 {
            ratio
        } else {
            // A new ratio over the faces that could be removed, so that 0..1 has a meaningful
            // range when operating on the selection; it doesn't have to be totally accurate,
            // but needs to be greater than the number of selected faces.
            let mut vix = vec![usize::MAX; bm.verts.capacity()];
            for (k, &v) in verts.iter().enumerate() {
                vix[v.slot()] = k;
            }
            let mut totface_basis = 0usize;
            let mut totface_adjacent = 0usize;
            for f in bm.face_ids() {
                // Count the faces during decimation: n-gons are triangulated.
                let len = bm.face(f).len as usize;
                let f_len = if len > 4 { len - 2 } else { 1 };
                totface_basis += f_len;
                if bm
                    .face_verts(f)
                    .iter()
                    .any(|v| vweights[vix[v.slot()]] != 0.0)
                {
                    totface_adjacent += f_len;
                }
            }
            let mut r = 1.0 - ratio;
            r *= totface_adjacent as f32 / totface_basis as f32;
            1.0 - r
        };

        let mut i = DecimateCollapseIn::with_factor(ratio_adjust);
        i.vweights = Some(vweights);
        i.vweight_factor = vertex_group_factor;
        i.do_triangulate = false;
        i.symmetry_axis = symmetry_axis;
        i.symmetry_eps = SYMMETRY_EPS;
        decimate_collapse(bm, &i);

        let mut selectmode = bm.select_mode;
        if !selectmode.intersects(SelectMode::VERT | SelectMode::EDGE) {
            // Ensure we flush edges -> faces.
            selectmode |= SelectMode::EDGE;
        }
        bm.select_flush_mode_ex(selectmode, false);
        // `EDBM_uvselect_clear` only invalidates Blender's UV-selection sync, which frap does
        // not keep. `EDBM_update(calc_normals)`:
        bm.normals_update();
    }
    Ok(OpStatus::Finished)
}
