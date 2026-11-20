// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_delete_v3d(confirm=True)`, tier T0 (WP 6.4). Port of
//! `delete_key_v3d_without_keying_set` with `can_delete_key`
//! (blender/source/blender/editors/animation/keyframing.cc), Blender 5.2.4, GPL-2.0-or-later
//! (frap scenes have no active keying set): for every selected object with an action, the
//! keys at the current frame are removed from the F-curves of its assigned slot; F-curves left
//! empty are removed. Locked F-curves are skipped with a WARNING; on an object in Pose Mode only
//! the channels of selected pose bones are touched. Always `FINISHED` (`confirm` only matters
//! for the interactive invoke).
//!
//! [`v3d`] holds the per-object F-curve filter `keyframe_clear_v3d` shares.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::IdRef;
use frap_spec_ops_anim::anim;

use super::anim__keyframe_insert::common;

/// Shared by `keyframe_delete_v3d` and `keyframe_clear_v3d`.
pub(crate) mod v3d {
    use frap_anim::FCurve;
    use frap_scene::id::Object;

    /// `BLI_str_quoted_substr(path, "pose.bones[")`
    pub(crate) fn bone_of(path: &str) -> Option<&str> {
        let rest = path.strip_prefix("pose.bones[\"")?;
        rest.split("\"]").next()
    }

    /// Whether the object is in Pose Mode (`ob->mode & OB_MODE_POSE`).
    pub(crate) fn posed(ob: &Object) -> bool {
        ob.mode.as_str() == "POSE"
    }

    /// The selection part of `can_delete_key` (`missing_bone_ok`) / `can_delete_fcurve`: in
    /// Pose Mode only the channels of selected bones; a channel that is no bone's is refused,
    /// one of a bone the pose lacks is accepted by `can_delete_key` only.
    pub(crate) fn bone_allows(ob: &Object, fc: &FCurve, missing_bone_ok: bool) -> bool {
        if !posed(ob) {
            return true;
        }
        let Some(name) = bone_of(&fc.data_path) else {
            return false;
        };
        match ob
            .pose
            .as_ref()
            .and_then(|p| p.bones.iter().find(|b| b.name == name))
        {
            Some(pb) => pb.select,
            None => missing_bone_ok,
        }
    }
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: anim::KeyframeDeleteV3d) -> OpResult {
    let frame = common::frame(cx);
    let objects: Vec<IdRef<frap_rna::ObjectId>> = cx.ctx().selected_objects();
    for o in objects {
        let Some(ob) = cx.main().objects.get(o).cloned() else {
            continue;
        };
        let Some(adt) = ob.animation_data.clone() else {
            continue;
        };
        let Some(a) = adt.action else {
            continue;
        };
        let mut warnings = Vec::new();
        let (main, log) = cx.main_mut();
        let Some(act) = main.actions.get_mut(a, log) else {
            continue;
        };
        for layer in &mut act.layers {
            for strip in &mut layer.strips {
                let Some(k) = strip.keyframe_mut() else { continue };
                for bag in k
                    .channelbags
                    .iter_mut()
                    .filter(|b| b.slot_handle == adt.action_slot_handle)
                {
                    let mut f = 0;
                    while f < bag.fcurves.len() {
                        let fc = &bag.fcurves[f];
                        let locked = fc.lock
                            || fc
                                .group
                                .as_ref()
                                .and_then(|g| bag.groups.iter().find(|x| &x.name == g))
                                .is_some_and(|g| g.lock);
                        if locked {
                            warnings.push(format!(
                                "Not deleting keyframe for locked F-Curve '{}', object '{}'",
                                fc.data_path, ob.header.name
                            ));
                            f += 1;
                            continue;
                        }
                        if !v3d::bone_allows(&ob, fc, true) {
                            f += 1;
                            continue;
                        }
                        let fc = &mut bag.fcurves[f];
                        let (i, found) =
                            frap_anim::eval::curve::binarysearch_index(&fc.keyframe_points, frame);
                        if found {
                            fc.keyframe_points.remove(i);
                            frap_anim::eval::recalc_handles(fc);
                            if fc.is_empty() {
                                bag.fcurve_remove(f);
                                continue;
                            }
                        }
                        f += 1;
                    }
                }
            }
        }
        for w in warnings {
            cx.report(ReportType::Warning, w);
        }
    }
    Ok(OpStatus::Finished)
}
