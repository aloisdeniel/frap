// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.parent_clear(type='CLEAR')`, tier T0
//! (`tests/golden/cases/object/parent/*.py`). Port of `parent_clear_exec` and
//! `ED_object_parent_clear` (`editors/object/object_relations.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4, on every selected editable object that has a parent:
//! - `CLEAR`: no parent, channels kept (the object jumps to them), parent inverse reset;
//! - `CLEAR_KEEP_TRANSFORM`: no parent, the channels take the world matrix (as evaluated
//!   before the operator, compatible Euler), parent inverse reset;
//! - `CLEAR_INVERSE`: the parent stays, its inverse is reset (the object jumps);
//! - nothing selected or no parent: `FINISHED`, nothing changes. Poll
//!   `ED_operator_scene_editable` (passes without an active object, as dumped).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object::{self, parent_clear::Type};
use glam::Mat4;

use crate::support::xform::{self, Worlds};

#[operator(poll = poll::SCENE_EDITABLE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::ParentClear) -> OpResult {
    let ty = p.r#type.value;
    let objects = cx.ctx().selected_editable_objects();
    let worlds = Worlds::of(cx.main());
    let (main, log) = cx.main_mut();
    for ob in objects {
        if main.objects.get(ob).is_none_or(|o| o.parent.is_none()) {
            continue;
        }
        let Some(o) = main.objects.get_mut(ob, log) else {
            continue;
        };
        match ty {
            Type::Clear => {
                o.parent = None;
                o.parent_type = "OBJECT".into();
                o.parent_bone = Default::default();
            }
            Type::ClearKeepTransform => {
                o.parent = None;
                xform::apply_mat4(o, &worlds.get(ob), true, None);
            }
            Type::ClearInverse => {}
        }
        o.matrix_parent_inverse = Mat4::IDENTITY;
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
