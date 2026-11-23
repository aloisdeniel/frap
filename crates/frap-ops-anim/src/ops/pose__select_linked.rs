// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.select_linked()`, tier T0 (WP 8.3.11). Port of `pose_select_linked_exec` and
//! `selectconnected_posebonechildren`
//! (blender/source/blender/editors/armature/pose_select.cc) with
//! `pose_bone_descendent_depth_iterator` (blender/source/blender/animrig/intern/armature.cc),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! For every visible pose bone of every armature in Pose Mode (`context.visible_pose_bones`,
//! in channel order) that is selected **when it is reached**: its chain of connected parents
//! is selected up to the first unconnected or unselectable bone, then its connected
//! descendants, depth first, each branch stopping at the first unconnected or `hide_select`
//! bone (hidden descendants are selected too, as in Blender). The selection test reads the
//! flags as they change, so a parent selected by a later bone is not revisited. Always
//! `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::{Chans, load_all};

/// `selectconnected_posebonechildren(ob, pchan, extend=false)`.
fn select_connected_children(ch: &mut Chans, c: usize, input: bool, depth: usize) {
    let ok = !ch.unselectable(c) && (input || ch.connected(c));
    if !ok {
        return;
    }
    ch.select(c);
    if depth > ch.len() {
        return;
    }
    for k in ch.children(c) {
        select_connected_children(ch, k, false, depth + 1);
    }
}

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_SELECT_LINKED)]
fn exec(cx: &mut OpCtx, _p: pose::PoseSelectLinked) -> OpResult {
    for mut ch in load_all(cx, false) {
        for c in ch.visible_chans() {
            if !ch.pb(c).select {
                continue;
            }
            // Select parents.
            let mut cur = Some(c);
            let mut steps = 0;
            while let Some(b) = cur {
                if ch.selectable(b) {
                    ch.select(b);
                    cur = if ch.connected(b) { ch.parent(b) } else { None };
                } else {
                    cur = None;
                }
                steps += 1;
                if steps > ch.len() {
                    break;
                }
            }
            select_connected_children(&mut ch, c, true, 0);
        }
        ch.store(cx)?;
    }
    Ok(OpStatus::Finished)
}
