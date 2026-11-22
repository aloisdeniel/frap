// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.rotate_normal(value=…, orient_axis=…, orient_type=…, constraint_axis=…)`,
//! tier T1 (`tests/golden/cases/edit/transform_m8/rotate_normal_*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/transform/transform_mode_edge_rotate_normal.cc
//! (`initNormalRotation`, `applyNormalRotation`) and the `TRANSFORM_OT_rotate_normal` exec
//! path of `transform_ops.cc`, Blender 5.2.4, GPL-2.0-or-later. Poll `ED_operator_editmesh`.
//!
//! Rotates the custom split normals of the selection (`custom_normal`, created when missing,
//! keeping the current shading): the loops of the selected elements (by select mode, and the
//! corners of selected faces picked through the selection history in mixed select modes) and
//! the other corners of their smooth fans. The axis is the orientation's `orient_axis`
//! (**not** negated, unlike `transform.rotate`), or the constrained axis with `constraint_axis`.
//! As in Blender, the rotation is applied to the object-space normals with the world-space
//! axis, and nothing happens without a selected vertex (`CANCELLED`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;

use crate::edit::{Mode, Request, matrix, set};

fn axis(a: transform::rotate_normal::OrientAxis) -> usize {
    match a {
        transform::rotate_normal::OrientAxis::X => 0,
        transform::rotate_normal::OrientAxis::Y => 1,
        transform::rotate_normal::OrientAxis::Z => 2,
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = Partial("mirror (mesh symmetry) changes nothing"),
    dyn_enums = [
        ("orient_type", crate::core::orient::orient_type_items),
        ("orient_matrix_type", crate::core::orient::orient_type_items),
    ],
    notes = [
        "Rotates about +orient_axis (transform.rotate turns about the negated axis); the \
         custom_normal attribute is added when the mesh has none."
    ]
)]
fn exec(cx: &mut OpCtx, p: transform::RotateNormal) -> OpResult {
    let mut r = Request::new(Mode::NormalRotation);
    if let Some(v) = set(&p.value) {
        r.values[0] = v;
    }
    r.orient_axis = axis(p.orient_axis.value);
    r.orient_type = set(&p.orient_type).map(|s| s.to_string());
    r.orient_matrix = matrix(&p.orient_matrix);
    r.orient_matrix_type = set(&p.orient_matrix_type).map(|s| s.to_string());
    r.constraint_axis = set(&p.constraint_axis);
    r.has_constraint_prop = true;
    crate::edit::trans::run(cx, r)
}

#[cfg(test)]
mod tests {
    use frap_rna::OpProps;
    use frap_spec_ops_transform::transform;

    #[test]
    fn registered_with_the_generated_spec() {
        let d = frap_op::OPERATORS
            .iter()
            .find(|d| d.spec.idname == "transform.rotate_normal")
            .expect("registered");
        assert!(std::ptr::eq(d.spec, transform::RotateNormal::spec()));
    }
}
