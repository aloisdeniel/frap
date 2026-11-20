// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_insert(type='DEFAULT')`, tier T0 (WP 6.4). Port of
//! `insert_key_exec` / `insert_key` (blender/source/blender/editors/animation/keyframing.cc),
//! Blender 5.2.4, GPL-2.0-or-later: with no active keying set, the channels of the preference
//! "Default Key Channels" (factory: location, rotation, scale; custom properties are not
//! keyed by frap, deviations §2) are keyed on every selected object (Object Mode) or selected
//! pose bone (Pose Mode) at the current frame, with the scene's keyframe type. The rotation
//! channel follows each item's `rotation_mode`. Another mode: "Unsupported context mode";
//! nothing selected: WARNING "Nothing selected to key" and CANCELLED. Keyed values are the
//! originals as they are (the operator pipeline applied pending animation first, §7.3).
//!
//! [`common`] holds what the `anim.keyframe_*` operators share (keying sets, the keyframe type).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::anim;

use crate::keying::{CombinedKeyingResult, KeyFlags, KeyPath, KeyingResult, can_have_animdata,
    id_code_name, insert_keyframes, targets};

/// Shared by the `anim.keyframe_*` operators.
pub(crate) mod common {
    use frap_anim::KeyframeType;
    use frap_op::prelude::*;

    use crate::keying::keyingsets::{self, KeyingSetInfo};
    use crate::keying::{KeyFlags, KeyPath, KeyingResult, delete_keyframe, insert_keyframes};

    /// `scene->toolsettings->keyframe_type`
    pub(crate) fn keytype(cx: &OpCtx) -> KeyframeType {
        KeyframeType::from_identifier(&cx.ctx().scene().tool_settings.keyframe_type)
            .unwrap_or(KeyframeType::Keyframe)
    }

    /// The context scene's frame.
    pub(crate) fn frame(cx: &OpCtx) -> f32 {
        cx.ctx().scene().frame_current as f32
    }

    /// `keyingset_get_from_op_with_error` for a keying-set idname (`…_by_name`).
    pub(crate) fn find(idname: &str) -> Result<&'static KeyingSetInfo, OpError> {
        if keyingsets::UNSUPPORTED.contains(&idname) {
            return Err(OpError::NotImplemented(format!(
                "frap: the keying set '{idname}' is not implemented (transform keying sets only)"
            )));
        }
        keyingsets::find(idname)
            .ok_or_else(|| OpError::Runtime(format!("Keying set '{idname}' not found")))
    }

    /// `insert_key_with_keyingset` (exec, no confirmation reports).
    pub(crate) fn insert_with_keyingset(cx: &mut OpCtx, ks: &KeyingSetInfo) -> OpResult {
        let Some(paths) = keyingsets::paths(&cx.ctx(), ks) else {
            return Err(OpError::Runtime(
                "No suitable context info for active keying set".into(),
            ));
        };
        let (frame, keytype) = (frame(cx), keytype(cx));
        let flags = KeyFlags {
            visual: ks.visual,
            ..KeyFlags::default()
        };
        for p in paths {
            // `insert_key_to_keying_set_path`: one call per array index of a whole array.
            let result = insert_keyframes(
                cx,
                p.root,
                p.group.as_deref(),
                &[KeyPath {
                    path: p.path.clone(),
                    index: p.index,
                }],
                frame,
                keytype,
                flags,
            );
            if result.count(KeyingResult::Success) == 0 {
                // Blender reports these to the window manager (no exception).
                let (_, msg) = result.report(ReportType::Error);
                cx.report(ReportType::Warning, msg);
            }
        }
        Ok(OpStatus::Finished)
    }

    /// `delete_key_using_keying_set` (exec).
    pub(crate) fn delete_with_keyingset(cx: &mut OpCtx, ks: &KeyingSetInfo) -> OpResult {
        let Some(paths) = keyingsets::paths(&cx.ctx(), ks) else {
            return Err(OpError::Runtime(
                "No suitable context info for active keying set".into(),
            ));
        };
        let frame = frame(cx);
        for p in paths {
            if let Err(msg) = delete_keyframe(cx, p.root, &p.path, p.index, frame) {
                cx.report(ReportType::Warning, msg);
            }
        }
        Ok(OpStatus::Finished)
    }
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: anim::KeyframeInsert) -> OpResult {
    // `type='DEFAULT'` with no active keying set (frap scenes have none): `insert_key`.
    let Some(selection) = targets::selection(&cx.ctx()) else {
        return Err(OpError::Runtime("Unsupported context mode".into()));
    };
    if selection.is_empty() {
        cx.report(ReportType::Warning, "Nothing selected to key");
        return Ok(OpStatus::Cancelled);
    }
    let (frame, keytype) = (common::frame(cx), common::keytype(cx));
    let mut combined = CombinedKeyingResult::default();
    for t in &selection {
        if !can_have_animdata(cx.main(), t.root) {
            let name = id_code_name(cx.main(), t.root);
            cx.report(
                ReportType::Error,
                format!(
                    "Could not insert keyframe, as this type does not support animation data (ID = {name})"
                ),
            );
            continue;
        }
        // `construct_rna_paths` with the factory "Default Key Channels".
        let paths = [
            KeyPath::all(t.path("location")),
            KeyPath::all(t.rotation_path()),
            KeyPath::all(t.path("scale")),
        ];
        let r = insert_keyframes(cx, t.root, None, &paths, frame, keytype, KeyFlags::default());
        combined.merge(&r);
    }
    if combined.count(KeyingResult::Success) == 0 {
        let (_, msg) = combined.report(ReportType::Error);
        return Err(OpError::Runtime(msg));
    }
    Ok(OpStatus::Finished)
}
