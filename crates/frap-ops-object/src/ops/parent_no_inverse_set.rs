// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.parent_no_inverse_set(keep_transform=False)`, tier T0
//! (`tests/golden/cases/object/parent/*.py`). Port of `parent_noinv_set_exec`
//! (`editors/object/object_relations.cc`) and `BKE_object_apply_parent_inverse`
//! (`blenkernel/intern/object.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4: the active object becomes the parent of the other selected editable
//! objects with an identity parent inverse and `location = (0, 0, 0)` (the child sits at the
//! parent's origin, keeping rotation and scale); with `keep_transform=True` the parent inverse
//! is the parent's inverse times its orthonormalized matrix (parent scale and shear only) and
//! the channels keep the world matrix. A parent loop reports `Error: Loop in parents`: Blender
//! still parents the other children, frap rolls the operator back (D-44 candidate).

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::would_cycle;
use frap_spec_ops_object::object;
use glam::Mat4;

use crate::support::xform::{self, Worlds};

#[operator(
    poll = poll::ACTIVE_EDITABLE,
    tier = T0,
    notes = [
        "A parent loop (Error: Loop in parents) rolls the whole operator back; Blender still \
         parents the other selected objects."
    ]
)]
fn exec(cx: &mut OpCtx, p: object::ParentNoInverseSet) -> OpResult {
    let keep = p.keep_transform.value;
    let (par, children) = {
        let c = cx.ctx();
        let Some(par) = c.active_object() else {
            return Err(OpError::Runtime("no active object".into()));
        };
        (par, c.selected_editable_objects())
    };
    let worlds = Worlds::of(cx.main());
    let par_world = worlds.get(par);
    let (main, log) = cx.main_mut();
    let mut loop_error = false;
    for ob in children.into_iter().filter(|o| *o != par) {
        if would_cycle(&main.objects, ob, par) {
            loop_error = true;
            continue;
        }
        let Some(o) = main.objects.get_mut(ob, log) else {
            continue;
        };
        o.parent = Some(par);
        o.parent_type = "OBJECT".into();
        if keep {
            xform::apply_parent_inverse(o, &par_world, &worlds.get(ob));
            continue;
        }
        o.matrix_parent_inverse = Mat4::IDENTITY;
        o.location = [0.0; 3];
    }
    log.context_changed = true;
    if loop_error {
        return Err(OpError::Runtime("Loop in parents".into()));
    }
    Ok(OpStatus::Finished)
}
