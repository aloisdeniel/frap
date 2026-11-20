// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.loc_clear()`, tier T0 (WP 6.2). Port of `pchan_clear_loc` and
//! `pose_clear_transform_generic_exec`
//! (blender/source/blender/editors/armature/pose_transform.cc), Blender 5.2.4,
//! GPL-2.0-or-later: the selected, visible pose bones of every armature in Pose Mode get
//! location 0 on their unlocked axes (with `Pose.use_mirror_x` their mirror bones too).
//! `CANCELLED` when no bone is selected. Auto-keying is WP 6.4's.
//!
//! [`clear`] holds the clearing functions the four `pose.*_clear` operators share.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

/// The clearing functions of `pose.loc_clear`, `rot_clear`, `scale_clear`,
/// `transforms_clear` (`pchan_clear_*`, `pchan_clear_*_with_mirrored`).
pub(crate) mod clear {
    use frap_math::EulerOrder;
    use frap_math::euler::{eul_o_to_quat, quat_to_eul_o};
    use frap_math::quat::{axis_angle_to_eul_o, eul_o_to_axis_angle, mul_qt_fl, normalize_qt};
    use frap_op::prelude::*;
    use frap_scene::edit_armature::names::flip_side_name;
    use frap_scene::id::PoseBone;

    use super::super::pose__select_all::common;

    /// What a clear operator resets.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub(crate) struct What {
        pub(crate) loc: bool,
        pub(crate) rot: bool,
        pub(crate) scale: bool,
    }

    /// `pchan_clear_loc`.
    fn clear_loc(pb: &mut PoseBone) {
        for i in 0..3 {
            if !pb.lock_location[i] {
                pb.location[i] = 0.0;
            }
        }
    }

    /// `pchan_clear_scale` (the bendy-bone ease and scale are not modelled).
    fn clear_scale(pb: &mut PoseBone) {
        for i in 0..3 {
            if !pb.lock_scale[i] {
                pb.scale[i] = 1.0;
            }
        }
    }

    /// `pchan_clear_rot`.
    fn clear_rot(pb: &mut PoseBone) {
        let any_lock = pb.lock_rotation.iter().any(|b| *b) || pb.lock_rotation_w;
        let mode = pb.rotation_mode.as_str();
        if !any_lock {
            match mode {
                "QUATERNION" => pb.rotation_quaternion = [1.0, 0.0, 0.0, 0.0],
                "AXIS_ANGLE" => pb.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0],
                _ => pb.rotation_euler = [0.0; 3],
            }
            return;
        }
        if pb.lock_rotations_4d {
            match mode {
                "AXIS_ANGLE" => {
                    let a = &mut pb.rotation_axis_angle;
                    if !pb.lock_rotation_w {
                        a[0] = 0.0;
                    }
                    for i in 0..3 {
                        if !pb.lock_rotation[i] {
                            a[i + 1] = 0.0;
                        }
                    }
                    if (a[1] - a[2]).abs() < f32::EPSILON && (a[2] - a[3]).abs() < f32::EPSILON {
                        a[2] = 1.0;
                    }
                }
                "QUATERNION" => {
                    let q = &mut pb.rotation_quaternion;
                    if !pb.lock_rotation_w {
                        q[0] = 1.0;
                    }
                    for i in 0..3 {
                        if !pb.lock_rotation[i] {
                            q[i + 1] = 0.0;
                        }
                    }
                }
                _ => {
                    for i in 0..3 {
                        if !pb.lock_rotation[i] {
                            pb.rotation_euler[i] = 0.0;
                        }
                    }
                }
            }
            return;
        }
        let mut quat1 = [0.0f32; 4];
        let mut qlen = 0.0f32;
        let oldeul = match mode {
            "QUATERNION" => {
                let (q, l) = normalize_qt(pb.rotation_quaternion);
                quat1 = q;
                qlen = l;
                quat_to_eul_o(q, EulerOrder::XYZ)
            }
            "AXIS_ANGLE" => {
                let a = pb.rotation_axis_angle;
                axis_angle_to_eul_o([a[1], a[2], a[3]], a[0], EulerOrder::XYZ)
            }
            _ => pb.rotation_euler,
        };
        let mut eul = [0.0f32; 3];
        for i in 0..3 {
            if pb.lock_rotation[i] {
                eul[i] = oldeul[i];
            }
        }
        match mode {
            "QUATERNION" => {
                let mut q = mul_qt_fl(eul_o_to_quat(eul, EulerOrder::XYZ), qlen);
                if (quat1[0] < 0.0 && q[0] > 0.0) || (quat1[0] > 0.0 && q[0] < 0.0) {
                    q = mul_qt_fl(q, -1.0);
                }
                pb.rotation_quaternion = q;
            }
            "AXIS_ANGLE" => {
                let (axis, angle) = eul_o_to_axis_angle(eul, EulerOrder::XYZ);
                pb.rotation_axis_angle = [angle, axis[0], axis[1], axis[2]];
            }
            _ => pb.rotation_euler = eul,
        }
    }

    fn clear_one(pb: &mut PoseBone, what: What) {
        if what.loc {
            clear_loc(pb);
        }
        if what.rot {
            clear_rot(pb);
        }
        if what.scale {
            clear_scale(pb);
        }
    }

    /// `pose_clear_transform_generic_exec`.
    pub(crate) fn exec(cx: &mut OpCtx, what: What) -> OpResult {
        let mut changed_multi = false;
        for (ob, arm) in common::pose_objects(cx) {
            let names = common::channels(cx, ob, arm, true);
            if names.is_empty() {
                continue;
            }
            changed_multi = true;
            let o = cx.object_mut(ob)?;
            let Some(pose) = o.pose.as_mut() else {
                continue;
            };
            let mirror = pose.use_mirror_x;
            for n in &names {
                if mirror {
                    let flip = flip_side_name(n, false);
                    if flip != *n
                        && let Some(m) = pose.bone_mut(&flip)
                    {
                        clear_one(m, what);
                    }
                }
                if let Some(pb) = pose.bone_mut(n) {
                    clear_one(pb, what);
                }
            }
        }
        Ok(if changed_multi {
            OpStatus::Finished
        } else {
            OpStatus::Cancelled
        })
    }
}

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: pose::LocClear) -> OpResult {
    clear::exec(
        cx,
        clear::What {
            loc: true,
            rot: false,
            scale: false,
        },
    )
}
