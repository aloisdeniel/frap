// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.paste_transform(method='CURRENT', bake_step=1, use_mirror=False,
//! mirror_axis_loc='x', mirror_axis_rot='z', use_relative=False)`, tier T0
//! (`tests/golden/cases/object/transform_m8/copy_global_transform.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/copy_global_transform.py
//! (`OBJECT_OT_paste_transform.poll`, `execute`, `string_to_matrix`), Blender 5.2.4,
//! GPL-2.0-or-later. Poll: an active pose bone or object, else `Select an object or pose
//! bone`.
//!
//! **Clipboard semantics (verified headless on 5.2.4).** The operator first parses
//! `window_manager.clipboard` (a `Matrix((…))` literal, a `<Matrix 4x4 …>` repr or four lines
//! of `print_m4` floats). `blender -b` has no system clipboard: the clipboard always reads
//! `''`, so every call fails with `RuntimeError: Error: Clipboard does not contain a matrix`
//! before the method, mirror or relative options are looked at, and nothing changes. frap has
//! no clipboard either and answers the same, with a hint naming the direct assignment.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::copy_global_transform::has_active;

fn poll_active(c: &Context) -> Result<(), PollFail> {
    if has_active(c) {
        return Ok(());
    }
    Err(PollFail {
        message: Some(
            "Operator bpy.ops.object.paste_transform.poll() Select an object or pose bone".into(),
        ),
        unmet: smallvec::smallvec![PollReq::ActiveObject],
        hint: Some(super::copy_global_transform::make_active_hint(c)),
    })
}

/// `OBJECT_OT_paste_transform.poll`.
const ACTIVE_OR_POSE_BONE: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: poll_active,
};

/// The headless clipboard: always empty (`window_manager.clipboard == ''` in `blender -b`).
const CLIPBOARD: &str = "";

#[operator(
    poll = ACTIVE_OR_POSE_BONE,
    tier = T0,
    support = Partial(
        "headless: the clipboard is always empty (as in blender -b), so pasting always fails \
         with 'Clipboard does not contain a matrix'; assign matrix_world instead"
    )
)]
fn exec(_cx: &mut OpCtx, _p: object::PasteTransform) -> OpResult {
    // `string_to_matrix(clipboard.strip())` is `None` for the empty string.
    debug_assert!(CLIPBOARD.trim().is_empty());
    Err(OpError::Frap(Box::new(
        frap_rna::FrapError::new(
            frap_rna::PyExc::RuntimeError,
            frap_op::errors::OP_EXEC_ERROR,
            "Error: Clipboard does not contain a matrix",
        )
        .with_hint(
            "headless Blender (and frap) has no clipboard: set the world matrix directly, \
             e.g. bpy.data.objects['Cube'].matrix_world = bpy.data.objects['Cube.001']\
             .matrix_world, or copy channels (location, rotation_euler, scale)",
        ),
    )))
}
