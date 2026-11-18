// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.spin(steps=12, dupli=False, angle=90°, use_auto_merge=True,
//! use_normal_flip=False, center=(0, 0, 0), axis=(0, 0, 0))`, tier T2
//! (`tests/golden/cases/edit/misc/spin*.py`). Port of `edbm_spin_exec`
//! (`editors/mesh/editmesh_extrude_spin.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! **`axis` is required**: Blender's invoke fills it from the view, `exec` does not, so a zero
//! axis is `RuntimeError: Error: Invalid/unset axis` (the operator is cancelled, nothing
//! changes), as in `blender -b` ([MESH §0.8]). `center` and `axis` are world-space values:
//! for every mesh in Edit Mode, `bmesh.ops.spin` runs on the selection with the object's
//! world matrix as `space`, the angle negated (Blender's convention), and `use_merge` when
//! `use_auto_merge` is on, `dupli` is off, `steps >= 3` and the angle is a full turn
//! (`|abs(angle) - 2π| <= 1e-6`). Without merging, the selection becomes the last step's
//! geometry (history cleared); a merged spin keeps the selection. Plane spun 4 × 90°:
//! V20 F18 ([MESH §4.12]). The extrusion is `extrude_face_region` (WP 2.5).

use frap_bmesh::ops::spin::{SpinIn, spin};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

#[operator(
    poll = poll::EDITMESH,
    tier = T2,
    notes = [
        "axis is required (no view to take it from), as in blender -b",
        "the side faces of each step are created in iteration order, one of the orders Blender produces (its order changes from run to run)"
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Spin) -> OpResult {
    let cent = p.center.value;
    let axis = p.axis.value;
    let steps = p.steps.value;
    let angle = p.angle.value;
    let use_normal_flip = p.use_normal_flip.value;
    let dupli = p.dupli.value;
    let use_auto_merge = p.use_auto_merge.value
        && !dupli
        && steps >= 3
        && (angle.abs() - (std::f64::consts::PI * 2.0) as f32).abs() <= 1e-6;
    if axis == [0.0; 3] {
        return Err(OpError::Runtime("Invalid/unset axis".into()));
    }
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let space = world_matrix(&cx.main().objects, ob).to_cols_array_2d();
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_geom(bm);
        let r = spin(
            bm,
            &SpinIn {
                geom,
                cent,
                axis,
                dvec: [0.0; 3],
                steps,
                angle: -angle,
                space,
                use_normal_flip,
                use_duplicate: dupli,
                use_merge: use_auto_merge,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        if !use_auto_merge {
            let bm = cx.bmesh_mut(me)?;
            edbm::deselect_all(bm);
            edbm::select_elems(bm, &out.geom_last);
        }
    }
    Ok(OpStatus::Finished)
}
