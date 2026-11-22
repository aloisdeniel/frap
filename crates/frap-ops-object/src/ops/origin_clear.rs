// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.origin_clear()`, tier T1
//! (`tests/golden/cases/object/transform_m8/origin_clear*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/object/object_transform.cc
//! (`object_origin_clear_exec`), Blender 5.2.4, GPL-2.0-or-later. Poll
//! `ED_operator_scene_editable` (every mode).
//!
//! "Clear Origin" moves each selected editable **parented** object onto its parent's origin
//! without touching its channels: the translation of `matrix_parent_inverse` becomes
//! `-R · location` with `R` its 3×3 part. Unparented objects are left alone (verified on
//! 5.2.4); always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;
use glam::{Mat3, Vec3};

#[operator(poll = poll::SCENE_EDITABLE, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::OriginClear) -> OpResult {
    let objects = cx.ctx().selected_editable_objects();
    for ob in objects {
        let o = cx.object_mut(ob)?;
        if o.parent.is_none() {
            continue;
        }
        let mat = Mat3::from_mat4(o.matrix_parent_inverse);
        let v = mat * -Vec3::from_array(o.location);
        o.matrix_parent_inverse.w_axis = v.extend(o.matrix_parent_inverse.w_axis.w);
    }
    Ok(OpStatus::Finished)
}
