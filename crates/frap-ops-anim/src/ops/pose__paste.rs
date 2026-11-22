// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.paste(flipped=False, selected_mask=False)`, tier T0 (WP 8.3.11). Port of
//! `pose_paste_exec` and `pose_bone_do_paste`
//! (blender/source/blender/editors/armature/pose_transform.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Reads the copy buffer of `pose.copy` (`Internal clipboard is empty`, `RuntimeError`, when
//! nothing was copied in this process, deviation D-158). Each channel that was selected at copy
//! time is pasted onto the pose object's channel of the same name (`flipped`: of the X-flipped
//! name, `BLI_string_flip_side_name`): location and scale, and the rotation converted to the
//! target's rotation mode (an axis-angle source pasted onto a quaternion bone takes the
//! **target's** angle, as Blender's code does). `flipped` mirrors the pose on X: the location's
//! X is negated, Euler Y and Z are negated (axis-angle and quaternion rotations through an XYZ
//! Euler). `selected_mask` pastes only onto selected bones (`PoseBone.select`, hidden ones
//! included), unless no pose bone is selected at all. Reports as Blender: INFO `Pasted all N
//! bones`, `Pasted N bones, and skipped M unselected bones`, `Pasted only N of the C copied
//! bones, and skipped M unselected bones`, WARNING `Pasted only N of the C copied bones`; with
//! nothing pasted a WARNING `None of the C copied bones are selected now` / `could be pasted`.
//! Always `FINISHED` once the buffer was read. Auto-keying is not done (deviations §2).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__copy::buffer;
use super::pose__select_hierarchy::chans::Chans;

/// `pose_bone_do_paste`, shared with `pose.user_transforms_clear`.
pub(crate) mod paste {
    use frap_math::EulerOrder;
    use frap_math::euler::{eul_o_to_quat, quat_to_eul_o};
    use frap_math::matrix::RotationMode;
    use frap_math::quat::{
        axis_angle_to_eul_o, axis_angle_to_quat, eul_o_to_axis_angle, normalize_qt,
        quat_to_axis_angle,
    };
    use frap_scene::edit_armature::names::flip_side_name;
    use frap_scene::id::{Pose, PoseBone};

    fn mode(pb: &PoseBone) -> RotationMode {
        RotationMode::from_blender(&pb.rotation_mode).unwrap_or(RotationMode::Quaternion)
    }

    /// `eul_to_quat` (the XYZ-only single-precision version, unlike `eulO_to_quat`).
    ///
    /// Ported from blender/source/blender/blenlib/intern/math_rotation_c.cc, Blender 5.2.4,
    /// GPL-2.0-or-later.
    pub(crate) fn eul_to_quat(eul: [f32; 3]) -> [f32; 4] {
        let ti = eul[0] * 0.5;
        let tj = eul[1] * 0.5;
        let th = eul[2] * 0.5;
        let ci = libm::cosf(ti);
        let cj = libm::cosf(tj);
        let ch = libm::cosf(th);
        let si = libm::sinf(ti);
        let sj = libm::sinf(tj);
        let sh = libm::sinf(th);
        let cc = ci * ch;
        let cs = ci * sh;
        let sc = si * ch;
        let ss = si * sh;
        [
            cj * cc + sj * ss,
            cj * sc - sj * cs,
            cj * ss + sj * cc,
            cj * cs - sj * sc,
        ]
    }

    /// What one paste did.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub(crate) enum Outcome {
        /// Pasted onto this channel.
        Pasted(usize),
        /// The channel exists but is not selected (`selected_mask`).
        Skipped,
        /// No channel of that name.
        NotFound,
    }

    /// `pose_bone_do_paste(ob, chan, sel_only, flip)` onto `pose`.
    pub(crate) fn do_paste(pose: &mut Pose, chan: &PoseBone, sel_only: bool, flip: bool) -> Outcome {
        let name = if flip {
            flip_side_name(&chan.name, false)
        } else {
            chan.name.to_string()
        };
        let Some(i) = pose.bones.iter().position(|p| p.name == name.as_str()) else {
            return Outcome::NotFound;
        };
        let pchan = &mut pose.bones[i];
        if sel_only && !pchan.select {
            return Outcome::Skipped;
        }
        pchan.location = chan.location;
        pchan.scale = chan.scale;
        let (to, from) = (mode(pchan), mode(chan));
        let from_aa = (
            [
                chan.rotation_axis_angle[1],
                chan.rotation_axis_angle[2],
                chan.rotation_axis_angle[3],
            ],
            chan.rotation_axis_angle[0],
        );
        if to == from {
            match to {
                RotationMode::Euler(_) => pchan.rotation_euler = chan.rotation_euler,
                RotationMode::AxisAngle => pchan.rotation_axis_angle = chan.rotation_axis_angle,
                RotationMode::Quaternion => pchan.rotation_quaternion = chan.rotation_quaternion,
            }
        } else {
            match to {
                RotationMode::Euler(order) => {
                    pchan.rotation_euler = match from {
                        RotationMode::AxisAngle => axis_angle_to_eul_o(from_aa.0, from_aa.1, order),
                        _ => quat_to_eul_o(chan.rotation_quaternion, order),
                    };
                }
                RotationMode::AxisAngle => {
                    let (axis, angle) = match from {
                        RotationMode::Euler(order) => {
                            eul_o_to_axis_angle(chan.rotation_euler, order)
                        }
                        _ => quat_to_axis_angle(chan.rotation_quaternion),
                    };
                    pchan.rotation_axis_angle = [angle, axis[0], axis[1], axis[2]];
                }
                RotationMode::Quaternion => {
                    pchan.rotation_quaternion = match from {
                        RotationMode::Euler(order) => eul_o_to_quat(chan.rotation_euler, order),
                        // `axis_angle_to_quat(pchan->quat, chan->rotAxis, pchan->rotAngle)`.
                        _ => axis_angle_to_quat(from_aa.0, pchan.rotation_axis_angle[0]),
                    };
                }
            }
        }
        if flip {
            pchan.location[0] *= -1.0;
            match to {
                RotationMode::Euler(_) => {
                    pchan.rotation_euler[1] *= -1.0;
                    pchan.rotation_euler[2] *= -1.0;
                }
                RotationMode::AxisAngle => {
                    let a = pchan.rotation_axis_angle;
                    let mut eul = axis_angle_to_eul_o([a[1], a[2], a[3]], a[0], EulerOrder::XYZ);
                    eul[1] *= -1.0;
                    eul[2] *= -1.0;
                    let (axis, angle) = eul_o_to_axis_angle(eul, EulerOrder::XYZ);
                    pchan.rotation_axis_angle = [angle, axis[0], axis[1], axis[2]];
                }
                RotationMode::Quaternion => {
                    let (q, _) = normalize_qt(pchan.rotation_quaternion);
                    let mut eul = quat_to_eul_o(q, EulerOrder::XYZ);
                    eul[1] *= -1.0;
                    eul[2] *= -1.0;
                    pchan.rotation_quaternion = eul_to_quat(eul);
                }
            }
        }
        Outcome::Pasted(i)
    }
}

#[operator(
    poll = poll::POSEMODE,
    tier = T0,
    notes = [
        "the copy buffer lives in the frap process (D-158), not in <tempdir>/copybuffer_pose.blend",
    ]
)]
fn exec(cx: &mut OpCtx, p: pose::Paste) -> OpResult {
    let flip = p.flipped.value;
    let mut sel_only = p.selected_mask.value;
    let found = {
        let c = cx.ctx();
        c.pose_object().and_then(|ob| {
            let arm = c.main.objects.get(ob)?.data?.typed()?;
            Some((ob, arm))
        })
    };
    let Some(mut ch) = found.and_then(|(ob, arm)| Chans::load(cx, ob, arm)) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(from) = buffer::read() else {
        return Err(OpError::Runtime("Internal clipboard is empty".into()));
    };
    if sel_only && cx.ctx().selected_pose_bones().is_empty() {
        sel_only = false;
    }
    let (mut pasted, mut skipped, mut copied) = (0usize, 0usize, 0usize);
    for chan in from.bones.iter().filter(|b| b.select) {
        copied += 1;
        match paste::do_paste(&mut ch.pose, chan, sel_only, flip) {
            paste::Outcome::Pasted(_) => pasted += 1,
            paste::Outcome::Skipped => skipped += 1,
            paste::Outcome::NotFound => {}
        }
    }
    ch.store(cx)?;
    if pasted == 0 {
        let msg = if sel_only {
            format!("None of the {copied} copied bones are selected now")
        } else {
            format!("None of the {copied} copied bones could be pasted")
        };
        cx.report(ReportType::Warning, msg);
        return Ok(OpStatus::Finished);
    }
    if pasted + skipped == copied {
        if skipped > 0 {
            cx.report(
                ReportType::Info,
                format!("Pasted {pasted} bones, and skipped {skipped} unselected bones"),
            );
        } else {
            cx.report(ReportType::Info, format!("Pasted all {pasted} bones"));
        }
    } else if skipped > 0 {
        cx.report(
            ReportType::Info,
            format!(
                "Pasted only {pasted} of the {copied} copied bones, and skipped {skipped} unselected bones"
            ),
        );
    } else {
        cx.report(
            ReportType::Warning,
            format!("Pasted only {pasted} of the {copied} copied bones"),
        );
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_math::EulerOrder;
    use frap_math::euler::eul_o_to_quat;
    use frap_scene::id::{Pose, PoseBone};

    use super::paste::{Outcome, do_paste, eul_to_quat};

    fn pose(bones: Vec<PoseBone>) -> Pose {
        let mut p = Pose::default();
        p.bones = bones;
        p
    }

    #[test]
    fn single_precision_eul_to_quat_matches_eulo_xyz() {
        for e in [[0.3f32, -0.2, 1.1], [3.0, 0.0, -2.5], [0.0, 1.5, 0.0]] {
            let (a, b) = (eul_to_quat(e), eul_o_to_quat(e, EulerOrder::XYZ));
            for i in 0..4 {
                assert!((a[i] - b[i]).abs() < 1e-6, "{e:?}: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn axis_angle_onto_quaternion_takes_the_target_angle() {
        let mut src = PoseBone::new("a");
        src.rotation_mode = "AXIS_ANGLE".into();
        src.rotation_axis_angle = [0.5, 1.0, 0.0, 0.0];
        let mut dst = PoseBone::new("a");
        dst.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0];
        let mut p = pose(vec![dst]);
        assert_eq!(do_paste(&mut p, &src, false, false), Outcome::Pasted(0));
        // The target's angle is 0: the identity, whatever the source angle.
        assert_eq!(p.bones[0].rotation_quaternion, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn flipped_paste_mirrors_onto_the_other_side() {
        let mut src = PoseBone::new("arm.L");
        src.location = [1.0, 2.0, 3.0];
        src.rotation_mode = "XYZ".into();
        src.rotation_euler = [0.1, 0.2, 0.3];
        let mut r = PoseBone::new("arm.R");
        r.rotation_mode = "XYZ".into();
        let mut p = pose(vec![PoseBone::new("arm.L"), r]);
        assert_eq!(do_paste(&mut p, &src, true, true), Outcome::Skipped);
        assert_eq!(do_paste(&mut p, &src, false, true), Outcome::Pasted(1));
        assert_eq!(p.bones[1].location, [-1.0, 2.0, 3.0]);
        assert_eq!(p.bones[1].rotation_euler, [0.1, -0.2, -0.3]);
        let missing = PoseBone::new("leg.L");
        assert_eq!(do_paste(&mut p, &missing, false, true), Outcome::NotFound);
    }
}
