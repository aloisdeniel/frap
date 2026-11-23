// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.select_mirror(only_active=False, extend=False)`, tier T0 (WP 8.3.11). Port of
//! `pose_select_mirror_exec` (blender/source/blender/editors/armature/pose_select.cc) with
//! `BKE_pose_channel_get_mirrored` (blenkernel/intern/action.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! On every armature in Pose Mode (unique data): each selectable bone that has an X mirror
//! (`BLI_string_flip_side_name`, a different existing name) takes the selection its mirror had
//! before the operator (`select`, head and tail; an invisible mirror counts as unselected);
//! with `extend` the mirror's selection is added instead. Bones without a mirror keep theirs.
//! The active bone moves to its mirror. `only_active` touches only the active bone and its
//! mirror. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::names::flip_side_name;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::load_all;

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_SELECT_MIRROR)]
fn exec(cx: &mut OpCtx, p: pose::PoseSelectMirror) -> OpResult {
    let only_active = p.only_active.value;
    let extend = p.extend.value;
    for mut ch in load_all(cx, true) {
        let old: Vec<[bool; 3]> = (0..ch.len())
            .map(|c| {
                if ch.visible(c) {
                    let pb = ch.pb(c);
                    [pb.select, pb.select_head, pb.select_tail]
                } else {
                    [false; 3]
                }
            })
            .collect();
        let act = ch.arm.active_bone;
        let mut mirror_act = None;
        for c in 0..ch.len() {
            if ch.bone(c).is_none() || !ch.selectable(c) {
                continue;
            }
            let name = ch.pb(c).name.to_string();
            let flip = flip_side_name(&name, false);
            if flip == name {
                continue;
            }
            let Some(m) = ch.find(&flip) else {
                continue;
            };
            if ch.bone(c) == act && act.is_some() {
                mirror_act = Some(m);
            }
            if only_active && !(act.is_some() && (ch.bone(c) == act || ch.bone(m) == act)) {
                continue;
            }
            let f = old[m];
            let pb = ch.pb_mut(c);
            if extend {
                pb.select |= f[0];
                pb.select_head |= f[1];
                pb.select_tail |= f[2];
            } else {
                pb.select = f[0];
                pb.select_head = f[1];
                pb.select_tail = f[2];
            }
        }
        if let Some(m) = mirror_act
            && ch.bone(m).is_some()
        {
            ch.set_active(Some(m));
        }
        ch.store(cx)?;
    }
    Ok(OpStatus::Finished)
}
