// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.flip_names(do_strip_numbers=False)`, tier T0 (WP 8.3.11). Port of
//! `pose_flip_names_exec` (blender/source/blender/editors/armature/pose_edit.cc) and
//! `ED_armature_bones_flip_names` (armature_naming.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature object in Pose Mode, the selected visible pose bones are renamed to their
//! flipped side name (`BLI_string_flip_side_name`; `do_strip_numbers` drops a trailing
//! `.NNN`) through `ED_armature_bone_rename` (the bone, the pose channels, bone-parented
//! objects, vertex groups and the `pose.bones["…"]` animation paths follow). A bone whose
//! flipped name was taken got a unique name in the first pass and is renamed again once every
//! bone was renamed, so `.L`/`.R` pairs swap. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::names::flip_side_name;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::load_all;
use super::armature__flip_names::rename_cx;

/// The current name of channel `c` of `ob`.
fn chan_name(cx: &OpCtx, ob: frap_rna::IdRef<frap_rna::ObjectId>, c: usize) -> Option<String> {
    cx.main()
        .objects
        .get(ob)?
        .pose
        .as_ref()?
        .bones
        .get(c)
        .map(|p| p.name.to_string())
}

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_FLIP_NAMES)]
fn exec(cx: &mut OpCtx, p: pose::PoseFlipNames) -> OpResult {
    let strip = p.do_strip_numbers.value;
    for ch in load_all(cx, false) {
        let (ob, arm) = (ch.ob, ch.arm_ref);
        let chans = ch.selected_chans();
        // First pass: rename blindly; keep those that did not get their flipped name.
        let mut conflicts: Vec<(usize, String)> = Vec::new();
        for c in chans {
            let Some(name) = chan_name(cx, ob, c) else {
                continue;
            };
            let flip = flip_side_name(&name, strip);
            rename_cx::rename(cx, arm, &name, &flip);
            if chan_name(cx, ob, c).as_deref() != Some(flip.as_str()) {
                conflicts.push((c, flip));
            }
        }
        // Second pass: the other bone of a swapped pair has moved away by now.
        for (c, flip) in conflicts {
            if let Some(name) = chan_name(cx, ob, c) {
                rename_cx::rename(cx, arm, &name, &flip);
            }
        }
    }
    Ok(OpStatus::Finished)
}
