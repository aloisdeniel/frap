// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.copy_global_transform()`, tier T0
//! (`tests/golden/cases/object/transform_m8/copy_global_transform.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/copy_global_transform.py
//! (`OBJECT_OT_copy_global_transform`, `get_matrix`, `_copy_matrix_to_clipboard`), Blender
//! 5.2.4, GPL-2.0-or-later. Poll: an active pose bone or an active object.
//!
//! **Clipboard semantics (verified headless on 5.2.4).** The add-on writes the world matrix of
//! the active pose bone (`armature.matrix_world @ bone.matrix`) or of the active object to
//! `window_manager.clipboard` as `Matrix((\n    (…),\n    …\n))`. `blender -b` has no system
//! clipboard: the write is dropped and `window_manager.clipboard` always reads `''`, so the
//! operator finishes without any visible change and a later `object.paste_transform` fails
//! with `Clipboard does not contain a matrix`. frap behaves the same (it has no
//! `window_manager`), and gives the text the add-on would have copied as the response's
//! `result.clipboard`, with a hint on how to apply it.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::value::py_float_repr;
use frap_scene::Context;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_object::object;
use glam::Mat4;

/// `bool(context.active_pose_bone) or bool(context.active_object)`.
pub(crate) fn has_active(c: &Context) -> bool {
    c.active_pose_bone().is_some()
        || c.active_object()
            .is_some_and(|a| c.main.objects.contains(a))
}

/// A Python poll that returns `False` (`failed, context is incorrect`).
pub(crate) fn context_incorrect(hint: String) -> PollFail {
    PollFail {
        message: None,
        unmet: smallvec::smallvec![PollReq::ActiveObject],
        hint: Some(hint),
    }
}

/// The hint when there is no active object.
pub(crate) fn make_active_hint(c: &Context) -> String {
    match c
        .view_layer()
        .bases
        .first()
        .and_then(|b| c.main.objects.get(b.object))
    {
        Some(o) => format!(
            "bpy.context.view_layer.objects.active = bpy.data.objects[{}]",
            poll::py_name(&o.header.name)
        ),
        None => "add an object first, e.g. bpy.ops.mesh.primitive_cube_add()".into(),
    }
}

fn poll_active(c: &Context) -> Result<(), PollFail> {
    if has_active(c) {
        Ok(())
    } else {
        Err(context_incorrect(make_active_hint(c)))
    }
}

/// `OBJECT_OT_copy_global_transform.poll`.
const ACTIVE_OR_POSE_BONE: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: poll_active,
};

/// `get_matrix`: the active pose bone's world matrix (`armature.matrix_world @ bone.matrix`)
/// or the active object's `matrix_world`.
pub(crate) fn get_matrix(cx: &mut OpCtx) -> Result<Option<Mat4>, OpError> {
    let (bone, active, arm_world) = {
        let c = cx.ctx();
        let bone = c.active_pose_bone();
        let arm_world = bone.as_ref().map(|b| world_matrix_in(c.main, b.object));
        let active = c.active_object().filter(|a| c.main.objects.contains(*a));
        (bone, active, arm_world)
    };
    if let (Some(b), Some(aw)) = (bone, arm_world) {
        let ev = cx.evaluated(b.object)?;
        let pose = ev
            .pose
            .as_ref()
            .and_then(|p| p.bones.iter().find(|(n, _)| *n == b.bone).map(|(_, m)| *m))
            .unwrap_or(Mat4::IDENTITY);
        return Ok(Some(aw * pose));
    }
    Ok(active.map(|a| world_matrix_in(cx.main(), a)))
}

/// `_copy_matrix_to_clipboard`'s text: `Matrix((\n    (a, b, c, d),\n …\n))`, rows as
/// Python tuples of floats.
pub(crate) fn matrix_text(m: &Mat4) -> String {
    let rows: Vec<String> = (0..4)
        .map(|r| {
            let row = m.row(r);
            let items: Vec<String> = row
                .to_array()
                .iter()
                .map(|v| py_float_repr(f64::from(*v)))
                .collect();
            format!("    ({}),", items.join(", "))
        })
        .collect();
    format!("Matrix((\n{}\n))", rows.join("\n"))
}

/// The frap side of a copy: no clipboard headless, the text in `result.clipboard`.
pub(crate) fn copied(cx: &mut OpCtx, m: &Mat4, what: &str) -> OpResult {
    let text = matrix_text(m);
    cx.result("clipboard", text);
    cx.hint(
        "HEADLESS_CLIPBOARD",
        format!(
            "headless Blender (and frap) has no clipboard: object.paste_transform cannot read \
             this {what} matrix back; result.clipboard holds the text the add-on would copy; \
             apply it with bpy.data.objects['<name>'].matrix_world = <the matrix>"
        ),
    );
    Ok(OpStatus::Finished)
}

#[operator(
    poll = ACTIVE_OR_POSE_BONE,
    tier = T0,
    support = Partial(
        "headless: nothing reaches a clipboard (as in blender -b); the copied text is in \
         result.clipboard"
    )
)]
fn exec(cx: &mut OpCtx, _p: object::CopyGlobalTransform) -> OpResult {
    let Some(m) = get_matrix(cx)? else {
        return Ok(OpStatus::Cancelled);
    };
    copied(cx, &m, "world")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_text_is_the_add_on_format() {
        let m = Mat4::from_translation(glam::Vec3::new(1.5, 0.0, -2.0));
        assert_eq!(
            matrix_text(&m),
            "Matrix((\n    (1.0, 0.0, 0.0, 1.5),\n    (0.0, 1.0, 0.0, 0.0),\n    (0.0, 0.0, \
             1.0, -2.0),\n    (0.0, 0.0, 0.0, 1.0),\n))"
        );
    }
}
