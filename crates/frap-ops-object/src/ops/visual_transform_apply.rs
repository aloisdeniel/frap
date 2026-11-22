// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.visual_transform_apply()`, tier T1
//! (`tests/golden/cases/object/transform_m8/visual_transform_apply*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/object/object_transform.cc
//! (`visual_transform_apply_exec`) and blenkernel/intern/object.cc (`BKE_object_where_is_calc`,
//! `BKE_object_apply_mat4`, `BKE_object_transform_copy`), Blender 5.2.4, GPL-2.0-or-later.
//! Poll `ED_operator_scene_editable` (every mode).
//!
//! Each selected editable object's evaluated world matrix is decomposed back into its
//! channels through its parent (compatible Euler, delta channels taken out). frap has no
//! object constraints, so the visual transform is the channels' own: the call normalizes them
//! as Blender's decomposition does (a scaled quaternion becomes unit, an Euler angle beyond π
//! stays compatible, a negative scale moves into the rotation the way `mat4_to_loc_rot_size`
//! splits it). Nothing selected → `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use crate::support::xform::{Worlds, apply_mat4, where_is};

#[operator(
    poll = poll::SCENE_EDITABLE,
    tier = T1,
    support = Partial("object constraints are not implemented in frap (OUT), so the visual transform is the channels' own")
)]
fn exec(cx: &mut OpCtx, _p: object::VisualTransformApply) -> OpResult {
    let (objects, worlds) = {
        let c = cx.ctx();
        (c.selected_editable_objects(), Worlds::of(c.main))
    };
    if objects.is_empty() {
        return Ok(OpStatus::Cancelled);
    }
    for ob in objects {
        let (world, parent) = {
            let main = cx.main();
            let Some(o) = main.objects.get(ob) else {
                continue;
            };
            // `BKE_object_where_is_calc` on the evaluated copy: the evaluated parent's matrix
            // (from before the operator) and the object's channels.
            let parent = worlds.parent_matrix(main, o);
            (where_is(o, parent.as_ref()), parent)
        };
        apply_mat4(cx.object_mut(ob)?, &world, true, parent.as_ref());
    }
    Ok(OpStatus::Finished)
}
