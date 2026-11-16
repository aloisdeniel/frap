// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared declarative poll library with Blender's `ED_operator_*` names (ARCHITECTURE §8).
//! M1 W0 declares every named entry with its `requires`; WP 1.12 implements the checks with
//! `unmet`/`hint` (the `todo!` bodies below), as Blender's `ED_operator_*` in `screen_ops.cc`.

use frap_scene::{Context, ContextMode};

use crate::def::{Poll, PollFail, PollReq};

fn ok(_: &Context) -> Result<(), PollFail> {
    Ok(())
}
/// Always passes (primitive add operators: poll matrix all Y).
pub const ALWAYS: Poll = Poll {
    requires: &[],
    check: ok,
};

fn objectmode(c: &Context) -> Result<(), PollFail> {
    if c.mode() == ContextMode::Object {
        Ok(())
    } else {
        Err(PollFail {
            message: None,
            unmet: smallvec::smallvec![PollReq::ModeObject],
            hint: Some("bpy.ops.object.mode_set(mode='OBJECT')".into()),
        })
    }
}
/// `ED_operator_objectmode`
pub const OBJECTMODE: Poll = Poll {
    requires: &[PollReq::ModeObject],
    check: objectmode,
};

fn todo_poll(_: &Context) -> Result<(), PollFail> {
    todo!("WP 1.12: declarative poll check")
}

/// `ED_operator_editmesh`
pub const EDITMESH: Poll = Poll {
    requires: &[PollReq::ModeEditMesh],
    check: todo_poll,
};
/// `ED_operator_object_active`
pub const ACTIVE_OBJECT: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: todo_poll,
};
/// `ED_operator_object_active_editable_mesh`
pub const ACTIVE_EDITABLE_MESH: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: todo_poll,
};
/// `ED_operator_scene_editable`
pub const SCENE_EDITABLE: Poll = Poll {
    requires: &[PollReq::SceneEditable],
    check: todo_poll,
};
/// `ED_operator_posemode`
pub const POSEMODE: Poll = Poll {
    requires: &[PollReq::ModePose],
    check: todo_poll,
};
/// `ED_operator_editarmature`
pub const EDITARMATURE: Poll = Poll {
    requires: &[PollReq::ModeEditArmature],
    check: todo_poll,
};
