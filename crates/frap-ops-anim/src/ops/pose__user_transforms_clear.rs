// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.user_transforms_clear(only_selected=True)`, tier T0 (WP 8.3.11). Port of
//! `pose_clear_user_transforms_exec` (blender/source/blender/editors/armature/pose_transform.cc)
//! and `BKE_pose_rest` (blenkernel/intern/action.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature object in Pose Mode:
//! - with an assigned action (`animation_data.action`), the object's animation data (NLA
//!   included) is evaluated at `scene.frame_current` on a copy of the pose, and every channel of
//!   that copy is pasted back (`pose_bone_do_paste` without flipping, so un-animated channels
//!   keep their values): the keyed location, rotation and scale replace the user's edits;
//! - without an action (NLA tracks alone do not count, as in Blender), the bones are reset to
//!   the rest pose (location 0, identity rotation in every mode, axis-angle `(0, (0, 1, 0))`,
//!   scale 1).
//!
//! `only_selected` restricts both to bones whose `PoseBone.select` is set (hidden ones
//! included, as Blender tests the flag only). Always `FINISHED`.

use frap_anim::eval::evaluate_anim_refined;
use frap_anim::eval::props::{PathSeg, parse_path};
use frap_eval::animate::MainActions;
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::{Pose, PoseBone};
use frap_spec_ops_anim::pose;

use super::pose__paste::paste;
use super::pose__select_hierarchy::chans::load_all;

/// Writes an evaluated `pose.bones["name"].<prop>[index]` channel into `pose`.
fn write_channel(pose: &mut Pose, path: &str, index: i32, value: f32) {
    let Some(segs) = parse_path(path) else {
        return;
    };
    let [
        PathSeg::Prop(p),
        PathSeg::Prop(b),
        PathSeg::Key(name),
        PathSeg::Prop(prop),
    ] = segs.as_slice()
    else {
        return;
    };
    if p != "pose" || b != "bones" {
        return;
    }
    let Some(pb) = pose.bone_mut(name) else {
        return;
    };
    let Ok(i) = usize::try_from(index) else {
        return;
    };
    let slot: Option<&mut f32> = match prop.as_str() {
        "location" => pb.location.get_mut(i),
        "scale" => pb.scale.get_mut(i),
        "rotation_euler" => pb.rotation_euler.get_mut(i),
        "rotation_quaternion" => pb.rotation_quaternion.get_mut(i),
        "rotation_axis_angle" => pb.rotation_axis_angle.get_mut(i),
        _ => None,
    };
    if let Some(s) = slot {
        *s = value;
    }
}

/// `BKE_pose_rest` on one channel.
fn rest(pb: &mut PoseBone) {
    pb.location = [0.0; 3];
    pb.rotation_euler = [0.0; 3];
    pb.rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
    pb.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0];
    pb.scale = [1.0; 3];
}

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: pose::UserTransformsClear) -> OpResult {
    let only_select = p.only_selected.value;
    let frame = cx.ctx().scene().frame_current as f32;
    for mut ch in load_all(cx, false) {
        let adt = cx
            .main()
            .objects
            .get(ch.ob)
            .and_then(|o| o.animation_data.clone());
        match adt {
            Some(adt) if adt.action.is_some() => {
                let channels = evaluate_anim_refined(
                    &adt,
                    &MainActions(cx.main()),
                    frame,
                    Some(&frap_spec_rna::OBJECT),
                    None,
                );
                let mut dummy = ch.pose.clone();
                for c in channels {
                    write_channel(&mut dummy, &c.data_path, c.array_index, c.value);
                }
                for chan in &dummy.bones {
                    paste::do_paste(&mut ch.pose, chan, only_select, false);
                }
            }
            _ => {
                for c in 0..ch.len() {
                    if only_select && ch.bone(c).is_some() && !ch.pb(c).select {
                        continue;
                    }
                    rest(ch.pb_mut(c));
                }
            }
        }
        ch.store(cx)?;
    }
    Ok(OpStatus::Finished)
}
