// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.copy()`, tier T0 (WP 8.3.11). Port of `pose_copy_exec`
//! (blender/source/blender/editors/armature/pose_transform.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! The pose object's bones that are not visible (hidden in Pose Mode or in a hidden bone
//! collection) lose their selection **in the scene** (Blender does this so they are not pasted),
//! then the whole pose is stored in the copy buffer ([`buffer`]) and an INFO `Copied pose to
//! internal clipboard` is reported. `pose.paste` reads the channels that were selected at copy
//! time.
//!
//! **Copy buffer.** Blender writes the object and its armature to
//! `<tempdir>/copybuffer_pose.blend`, which survives file loads and is shared by every Blender
//! process of the user. frap keeps the copied channels in the process (deviation D-158): they
//! survive `wm.open_mainfile` / `read_homefile` and undo, and a new frap process starts with
//! an empty buffer (`pose.paste` then fails with `Internal clipboard is empty`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::Chans;

/// The pose copy buffer (`copybuffer_pose.blend`).
pub(crate) mod buffer {
    use std::sync::Mutex;

    use frap_scene::id::PoseBone;

    /// What `pose.copy` stored: the pose channels of the copied object, in channel order, with
    /// their selection at copy time.
    #[derive(Clone, Debug, PartialEq)]
    pub(crate) struct CopiedPose {
        /// The channels (`object_from->pose->chanbase`).
        pub(crate) bones: Vec<PoseBone>,
    }

    static BUFFER: Mutex<Option<CopiedPose>> = Mutex::new(None);

    /// Replaces the buffer (`write_as_copypaste_buffer`).
    pub(crate) fn store(p: CopiedPose) {
        let mut g = BUFFER.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(p);
    }

    /// The buffer's content (`BKE_copybuffer_read`); `None` when nothing was copied yet.
    pub(crate) fn read() -> Option<CopiedPose> {
        BUFFER.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: pose::Copy) -> OpResult {
    let found = {
        let c = cx.ctx();
        c.pose_object().and_then(|ob| {
            let arm = c.main.objects.get(ob)?.data?.typed()?;
            Some((ob, arm))
        })
    };
    let Some(mut ch) = found.and_then(|(ob, arm)| Chans::load(cx, ob, arm)) else {
        return Err(OpError::Runtime("No pose to copy".into()));
    };
    // Taking off the selection flag in case bones are hidden so they are not applied when
    // pasting.
    for c in 0..ch.len() {
        if !ch.visible(c) {
            ch.deselect(c);
        }
    }
    buffer::store(buffer::CopiedPose {
        bones: ch.pose.bones.clone(),
    });
    ch.store(cx)?;
    cx.report(ReportType::Info, "Copied pose to internal clipboard");
    Ok(OpStatus::Finished)
}
