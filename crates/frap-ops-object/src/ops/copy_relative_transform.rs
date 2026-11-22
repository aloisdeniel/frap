// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.copy_relative_transform()`, tier T0
//! (`tests/golden/cases/object/transform_m8/copy_global_transform.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/copy_global_transform.py
//! (`OBJECT_OT_copy_relative_transform`, `get_relative_ob`), Blender 5.2.4, GPL-2.0-or-later.
//! Poll: a relative object (`tool_settings.anim_relative_object`, which frap does not bind, so
//! the scene camera) and an active pose bone or object.
//!
//! Copies `relative.matrix_world⁻¹ @ <world matrix of the active pose bone or object>` to the
//! clipboard, which `blender -b` does not have (see `object.copy_global_transform`): the
//! operator finishes without a visible change; frap gives the text as `result.clipboard`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_object::object;

use super::copy_global_transform::{
    context_incorrect, copied, get_matrix, has_active, make_active_hint,
};

/// `get_relative_ob`: `tool_settings.anim_relative_object` (unbound in frap) or the scene
/// camera.
fn relative_ob(c: &Context) -> Option<frap_rna::IdRef<frap_rna::ObjectId>> {
    c.scene().camera.filter(|r| c.main.objects.contains(*r))
}

fn poll_relative(c: &Context) -> Result<(), PollFail> {
    if relative_ob(c).is_none() {
        let mut f = context_incorrect(
            "set the scene camera: bpy.context.scene.camera = bpy.data.objects['<camera>']"
                .into(),
        );
        f.unmet = smallvec::smallvec![PollReq::SceneHasCamera];
        return Err(f);
    }
    if has_active(c) {
        Ok(())
    } else {
        Err(context_incorrect(make_active_hint(c)))
    }
}

/// `OBJECT_OT_copy_relative_transform.poll`.
const RELATIVE_AND_ACTIVE: Poll = Poll {
    requires: &[PollReq::SceneHasCamera, PollReq::ActiveObject],
    check: poll_relative,
};

#[operator(
    poll = RELATIVE_AND_ACTIVE,
    tier = T0,
    support = Partial(
        "headless: nothing reaches a clipboard (as in blender -b); the copied text is in \
         result.clipboard; tool_settings.anim_relative_object is not bound (the scene camera \
         is the relative object)"
    )
)]
fn exec(cx: &mut OpCtx, _p: object::CopyRelativeTransform) -> OpResult {
    let Some(rel) = relative_ob(&cx.ctx()) else {
        return Err(OpError::Runtime(
            "No 'Relative To' object found, set one explicitly or make sure there is an \
             active object"
                .into(),
        ));
    };
    let Some(m) = get_matrix(cx)? else {
        return Ok(OpStatus::Cancelled);
    };
    let rel_world = world_matrix_in(cx.main(), rel);
    copied(cx, &(rel_world.inverse() * m), "relative")
}
