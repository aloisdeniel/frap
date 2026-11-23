// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.channels_bake(use_scene_range=True, range=(0, 0), step=1.0,
//! remove_outside_range=False, interpolation_type='BEZIER', bake_modifiers=True)`, tier T1
//! (WP 8.3.11), headless-extended. Port of `channels_bake_exec`
//! (blender/source/blender/editors/animation/anim_channels_edit.cc) with the parts of
//! `ANIM_animdata_filter` (anim_filter.cc: `animdata_filter_dopesheet`,
//! `animdata_filter_base_is_ok`, `animfilter_act_group`, `animfilter_fcurve_next`,
//! `skip_fcurve_selected_data`) that decide which F-curves a Graph Editor lists, Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Blender bakes the selected F-curves listed in an animation editor (its poll needs one);
//! frap has no editors and bakes what the factory Graph Editor would list ("Only Show
//! Selected" on, "Show Hidden" off; deviation D-163, the unlisted data in deviations §2):
//! - the scene's and its world's animation, then every visible object (base order) that is
//!   selected (in Pose Mode: every object in Pose Mode) or whose animation data is pinned
//!   (`use_pin`), with its object data (armature, camera, light, …);
//! - of each, the F-curves of the assigned action slot: an ungrouped F-curve when it is
//!   selected (`FCurve.select`); in a group collapsed in the Graph Editor
//!   (`show_expanded_graph` off) every F-curve when the group is selected and none otherwise;
//!   in an expanded group the selected ones;
//! - a `pose.bones["…"]` F-curve of an object only while that bone is visible and selected
//!   (`PoseBone.select`), unless its group is pinned;
//! - each F-curve once (`ANIMFILTER_NODUPLIS`); one without keys is listed but not baked.
//!
//! The frame range is the scene's (`frame_start`..`frame_end`; frap has no preview range) or,
//! with `use_scene_range=False`, `range` as given (Blender's default `(0, 0)` bakes frame 0
//! only), its end raised to its start. Each F-curve is sampled every `step` frames over the
//! range (`bake_fcurve`), the keys in the range are removed (`remove_outside_range`: every
//! key), the samples merged in as `AUTO_CLAMPED` keys with `interpolation_type`, the key ending
//! the baked segment keeping the interpolation the curve had there, and the handles
//! recalculated (the sample count is Blender's wrapping `int` arithmetic: `range=(INT_MIN,
//! INT_MAX)` samples nothing and only removes the keys). The baked keys' memory is checked
//! against `--max-elements` first (`LIMIT_EXCEEDED`, D-52). frap's F-curves have no
//! F-modifiers, so `bake_modifiers` changes nothing.
//! Without any listed F-curve: WARNING `No channels to operate on`, `CANCELLED`. Blender does not
//! tag the baked actions for the depsgraph (a later `frame_set` evaluates the old curves); frap's
//! evaluation always sees the baked ones (D-163, D-4).

mod bake;

use frap_anim::{AnimData, Interpolation};
use frap_eval::animate::anim_data;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{ActionId, IdRef, ObjectId, Root};
use frap_scene::{Main, ObjectMode};
use frap_spec_ops_anim::anim::{self, channels_bake::InterpolationType};

/// One listed F-curve: the action, the slot's channelbag and the F-curve's index in it.
type Listed = (IdRef<ActionId>, i32, usize);

/// `BLI_str_quoted_substr(path, "pose.bones[", …)`: the bone name of a pose-bone path.
fn pose_bone_name(path: &str) -> Option<String> {
    let at = path.find("pose.bones[")? + "pose.bones[".len();
    let rest = &path[at..];
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let mut out = String::new();
    let mut escaped = false;
    for ch in rest[1..].chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

/// `skip_fcurve_selected_data` for an F-curve of object `ob`.
fn skip_for_bone(main: &Main, ob: Option<IdRef<ObjectId>>, path: &str) -> bool {
    let Some(o) = ob.and_then(|ob| main.objects.get(ob)) else {
        return false;
    };
    let Some(name) = pose_bone_name(path) else {
        return false;
    };
    let Some(pb) = o.pose.as_ref().and_then(|p| p.bone(&name)) else {
        return false;
    };
    let Some(arm) = o
        .data
        .and_then(|d| d.typed::<frap_rna::ArmatureId>())
        .and_then(|a| main.armatures.get(a))
    else {
        return false;
    };
    let Some(b) = arm.bone(&name) else {
        return false;
    };
    let visible = !pb.hide && arm.bone_collections_visible(b);
    !visible || !pb.select
}

/// The F-curves of `adt`'s assigned slot that the Graph Editor lists, appended to `out`.
fn list_adt(main: &Main, adt: &AnimData, ob: Option<IdRef<ObjectId>>, out: &mut Vec<Listed>) {
    let Some(a) = adt.action else {
        return;
    };
    let Some(act) = main.actions.get(a) else {
        return;
    };
    let handle = adt.action_slot_handle;
    if act.slot(handle).is_none() {
        return;
    }
    let Some(bag) = act.channelbag(handle) else {
        return;
    };
    for (i, fc) in bag.fcurves.iter().enumerate() {
        let group = fc
            .group
            .as_ref()
            .and_then(|g| bag.groups.iter().find(|x| &x.name == g));
        let pinned_group = group.is_some_and(|g| g.use_pin);
        let sel_ok = match group {
            // `animfilter_act_group`: a collapsed group stands for its F-curves.
            Some(g) if !g.show_expanded_graph => g.select,
            _ => fc.select,
        };
        if !sel_ok {
            continue;
        }
        if !pinned_group && skip_for_bone(main, ob, &fc.data_path) {
            continue;
        }
        let item = (a, handle, i);
        if !out.contains(&item) {
            out.push(item);
        }
    }
}

/// The F-curves of the factory Graph Editor's channel list (see the module notes).
fn listed_fcurves(cx: &OpCtx) -> Vec<Listed> {
    let c = cx.ctx();
    let main = c.main;
    let mut out = Vec::new();
    // `animdata_filter_dopesheet_scene`: the scene, then its world.
    let scene = c.scene();
    if let Some(adt) = &scene.animation_data {
        list_adt(main, adt, None, &mut out);
    }
    if let Some(adt) = scene.world.and_then(|w| anim_data(main, Root::Id(w.any()))) {
        list_adt(main, adt, None, &mut out);
    }
    let pose_mode = c
        .active_object()
        .and_then(|a| main.objects.get(a))
        .is_some_and(|o| o.mode == ObjectMode::Pose);
    let selected = c.selected_objects();
    for ob in c.visible_objects() {
        let Some(o) = main.objects.get(ob) else {
            continue;
        };
        let data_adt = o.data.and_then(|d| anim_data(main, Root::Id(d)));
        let pinned = o.animation_data.as_ref().is_some_and(|a| a.use_pin)
            || data_adt.is_some_and(|a| a.use_pin);
        if !pinned {
            let ok = if pose_mode {
                o.mode == ObjectMode::Pose
            } else {
                selected.contains(&ob)
            };
            if !ok {
                continue;
            }
        }
        if let Some(adt) = &o.animation_data {
            list_adt(main, adt, Some(ob), &mut out);
        }
        if let Some(adt) = data_adt {
            list_adt(main, adt, None, &mut out);
        }
    }
    out
}

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = HeadlessExtended("bakes the F-curves the factory Graph Editor lists (selected objects)")
)]
fn exec(cx: &mut OpCtx, p: anim::ChannelsBake) -> OpResult {
    let listed = listed_fcurves(cx);
    if listed.is_empty() {
        cx.report(ReportType::Warning, "No channels to operate on");
        return Ok(OpStatus::Cancelled);
    }
    let scene = cx.ctx().scene();
    let mut range = [scene.frame_start, scene.frame_end];
    if !p.use_scene_range.value {
        range = p.range.value;
    }
    range[1] = range[1].max(range[0]);
    let remove = if p.remove_outside_range.value {
        bake::Remove::All
    } else {
        bake::Remove::InRange
    };
    let ipo = match p.interpolation_type.value {
        InterpolationType::Bezier => Interpolation::Bezier,
        InterpolationType::Lin => Interpolation::Linear,
        InterpolationType::Const => Interpolation::Constant,
    };
    let step = p.step.value;
    // The baked keys' memory against the `--max-elements` budget before sampling (D-52, as
    // `keyframe_points.add`): a huge range answers `LIMIT_EXCEEDED` instead of exhausting the
    // process. Blender bakes any range.
    let baked_curves = listed
        .iter()
        .filter(|&&(a, handle, i)| {
            cx.main()
                .actions
                .get(a)
                .and_then(|act| act.channelbag(handle))
                .and_then(|bag| bag.fcurves.get(i))
                .is_some_and(|fc| !fc.keyframe_points.is_empty())
        })
        .count() as u64;
    let samples = bake::sample_count(range, step) as u64;
    frap_math::budget::check_buffer(
        "keyframe_points",
        samples
            .checked_mul(std::mem::size_of::<frap_anim::Keyframe>() as u64)
            .and_then(|b| b.checked_mul(baked_curves)),
    );
    let (main, log) = cx.main_mut();
    for (a, handle, i) in listed {
        let Some(act) = main.actions.get_mut(a, log) else {
            continue;
        };
        let Some(frap_anim::ActionStrip::Keyframe(strip)) =
            act.layers.first_mut().and_then(|l| l.strips.first_mut())
        else {
            continue;
        };
        let Some(bag) = strip
            .channelbags
            .iter_mut()
            .find(|b| b.slot_handle == handle)
        else {
            continue;
        };
        if let Some(fc) = bag.fcurves.get_mut(i) {
            bake::bake(fc, range, step, remove, ipo);
        }
    }
    Ok(OpStatus::Finished)
}
