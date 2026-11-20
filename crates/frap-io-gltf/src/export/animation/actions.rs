// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_animation_mode` `ACTIONS` (one glTF animation per action, merged across objects by
//! `export_merge_animation`), `ACTIVE_ACTIONS` (every assigned action in one animation) and
//! `BROADCAST` (every action on every compatible object).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/animation/action.py` (Apache-2.0, The
//! glTF-Blender-IO authors): `ActionsData`, `ActionData`, `gather_actions_animations`,
//! `prepare_actions_range` (`_align_frame_start`), `gather_action_animations`,
//! `__get_blender_actions`, `__get_blender_actions_broadcast`, `__is_armature_slot`, and
//! `anim_utils.py` (`reset_bone_matrix`, `reset_sk_data`, `bake_animation`).
//!
//! The add-on assigns each action and slot to the object (and the shape keys) in turn, with
//! the object's NLA off, and samples the scene; `super::world::World` replays those
//! assignments on the export's working copy, and restores them afterwards as the add-on does,
//! so later objects see the same state they see in Blender.

use std::collections::HashMap;

use frap_anim::{Action, AnimData};
use frap_rna::{ActionId, IdRef, ObjectId};
use indexmap::IndexMap;
use smol_str::SmolStr;

use super::exp::{AKey, Exp};
use super::fcurves::{TType, channel_from_target, channel_groups, fcurve_channels, get_target};
use super::keys::Interp;
use super::world::exported_shape_keys;
use super::{GAnim, GChannel, Merge, Mode, Negative, Source};

/// `SlotData`.
#[derive(Clone, Debug)]
pub(crate) struct SlotData {
    pub handle: i32,
    pub identifier: SmolStr,
    pub target_id_type: SmolStr,
    pub track: Option<SmolStr>,
}

/// `ActionData`.
#[derive(Clone, Debug)]
pub(crate) struct ActionData {
    pub action: IdRef<ActionId>,
    pub action_name: SmolStr,
    pub slots: Vec<SlotData>,
    pub name: SmolStr,
    pub active: bool,
}

impl ActionData {
    fn new(action: IdRef<ActionId>, a: &Action) -> ActionData {
        ActionData {
            action,
            action_name: a.header.name.clone(),
            slots: Vec::new(),
            name: a.header.name.clone(),
            active: true,
        }
    }

    /// `add_slot`: an NLA slot replaces the same slot without track (the active one).
    fn add_slot(&mut self, s: SlotData) {
        if let Some(i) = self
            .slots
            .iter()
            .position(|x| x.handle == s.handle && x.track.is_none())
        {
            self.slots.remove(i);
        }
        self.slots.push(s);
    }
}

/// `ActionsData`.
#[derive(Clone, Debug, Default)]
pub(crate) struct ActionsData {
    actions: IndexMap<IdRef<ActionId>, Vec<ActionData>>,
}

impl ActionsData {
    fn add(&mut self, mut a: ActionData, force_new: bool) {
        if force_new {
            a.active = false;
            self.actions.entry(a.action).or_default().push(a);
            return;
        }
        match self.actions.get_mut(&a.action) {
            None => self.actions.insert(a.action, vec![a]).map_or((), |_| ()),
            Some(list) => {
                let idx = list.iter().position(|x| x.active).unwrap_or(list.len() - 1);
                for s in a.slots {
                    list[idx].add_slot(s);
                }
            }
        }
    }

    /// `get()`: every action data, by action name (case-insensitive, stable), slots OBJECT
    /// first.
    pub(crate) fn values(&self) -> Vec<ActionData> {
        let mut v: Vec<ActionData> = self.actions.values().flatten().cloned().collect();
        v.sort_by_key(|a| a.action_name.to_lowercase());
        for a in &mut v {
            a.slots.sort_by_key(|s| match s.target_id_type.as_str() {
                "OBJECT" => 1,
                "KEY" => 2,
                _ => 3,
            });
        }
        v
    }

    fn exists(&self, action: IdRef<ActionId>, handle: i32) -> bool {
        self.actions
            .get(&action)
            .is_some_and(|l| l.iter().any(|a| a.slots.iter().any(|s| s.handle == handle)))
    }

    fn exists_action_slot_target(&self, action: IdRef<ActionId>, handle: i32, ty: &str) -> bool {
        self.actions.get(&action).is_some_and(|l| {
            l.iter().any(|a| {
                a.slots
                    .iter()
                    .any(|s| s.handle != handle && s.target_id_type == ty)
            })
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    fn key_actions(&self) -> usize {
        self.actions
            .values()
            .flatten()
            .filter(|a| a.slots.iter().any(|s| s.target_id_type == "KEY"))
            .count()
    }
}

fn slot_data(a: &Action, handle: i32, track: Option<SmolStr>) -> Option<SlotData> {
    let s = a.slot(handle)?;
    Some(SlotData {
        handle,
        identifier: s.identifier.clone(),
        target_id_type: s.target_id_type.clone(),
        track,
    })
}

/// `slot.name_display`: the identifier without its two-letter ID-type prefix.
fn name_display(identifier: &str) -> &str {
    identifier.get(2..).unwrap_or("")
}

/// `__is_armature_slot`: the slot animates a pose bone.
fn is_armature_slot(a: &Action, handle: i32) -> bool {
    super::fcurves::slot_fcurves(a, handle)
        .iter()
        .any(|f| f.data_path.starts_with("pose.bones"))
}

/// The NLA strips of `adt` the `ACTIONS` mode collects (single non-muted strip per track).
fn nla_strips(
    exp: &mut Exp<'_>,
    adt: &AnimData,
    warn_multi: bool,
) -> Vec<(IdRef<ActionId>, i32, SmolStr)> {
    let mut out = Vec::new();
    for track in &adt.nla_tracks {
        let strips: Vec<_> = track
            .strips
            .iter()
            .filter(|s| s.action.is_some() && !s.mute)
            .collect();
        if warn_multi {
            if strips.len() > 1 {
                exp.warn(format!(
                    "NLA track '{}' has {} strips, but only single-strip tracks are supported in 'actions' mode.",
                    track.name,
                    track.strips.len()
                ));
                continue;
            }
        } else if strips.len() != 1 {
            continue;
        }
        for s in strips {
            let Some(a) = s.action else { continue };
            let has_slot = exp
                .world
                .main
                .actions
                .get(a)
                .is_some_and(|act| act.slot(s.action_slot_handle).is_some());
            if !has_slot {
                exp.warn(format!(
                    "Strip '{}' on track '{}' has no action slot assigned, and will be ignored.",
                    s.name, track.name
                ));
                continue;
            }
            out.push((a, s.action_slot_handle, track.name.clone()));
        }
    }
    out
}

/// `__get_blender_actions(obj)`.
pub(crate) fn blender_actions(exp: &mut Exp<'_>, ob: IdRef<ObjectId>) -> ActionsData {
    if exp.s.mode == Mode::Broadcast {
        return blender_actions_broadcast(exp, ob);
    }
    let mut actions = ActionsData::default();
    let Some(o) = exp.world.ob(ob).cloned() else {
        return actions;
    };
    let main_actions = |exp: &Exp<'_>, a: IdRef<ActionId>| exp.world.main.actions.get(a).cloned();
    if let Some(adt) = &o.animation_data {
        if let Some(a) = adt.action
            && let Some(act) = main_actions(exp, a)
            && let Some(sd) = slot_data(&act, adt.action_slot_handle, None)
        {
            let mut d = ActionData::new(a, &act);
            d.add_slot(sd);
            actions.add(d, false);
        }
        if exp.s.mode == Mode::Actions {
            for (a, handle, track) in nla_strips(exp, adt, true) {
                let Some(act) = main_actions(exp, a) else {
                    continue;
                };
                let Some(sd) = slot_data(&act, handle, Some(track)) else {
                    continue;
                };
                let mut d = ActionData::new(a, &act);
                let ty = sd.target_id_type.clone();
                let ident = sd.identifier.clone();
                d.add_slot(sd);
                if actions.exists_action_slot_target(a, handle, &ty) {
                    d.name = format!("{}-{}", act.header.name, name_display(&ident)).into();
                    actions.add(d, true);
                } else {
                    actions.add(d, false);
                }
            }
        }
    }
    // Shape-key actions come after the object's ones (cache order).
    if exp.s.morph_anim
        && o.object_type() == "MESH"
        && let Some(kadt) = exp.world.key(ob).and_then(|k| k.animation_data.clone())
    {
        if let Some(a) = kadt.action
            && let Some(act) = main_actions(exp, a)
            && let Some(sd) = slot_data(&act, kadt.action_slot_handle, None)
        {
            let mut d = ActionData::new(a, &act);
            d.add_slot(sd);
            actions.add(d, false);
        }
        if exp.s.mode == Mode::Actions {
            for (a, handle, track) in nla_strips(exp, &kadt, false) {
                let Some(act) = main_actions(exp, a) else {
                    continue;
                };
                let Some(sd) = slot_data(&act, handle, Some(track)) else {
                    continue;
                };
                let mut d = ActionData::new(a, &act);
                let ty = sd.target_id_type.clone();
                let ident = sd.identifier.clone();
                d.add_slot(sd);
                if actions.exists_action_slot_target(a, handle, &ty) {
                    d.name = format!("{}-{}", act.header.name, name_display(&ident)).into();
                    actions.add(d, true);
                } else {
                    actions.add(d, false);
                }
            }
        }
    }
    // `export_anim_single_armature`: the only armature gets every armature action.
    if exp.s.single_armature
        && o.object_type() == "ARMATURE"
        && o.animation_data.is_some()
        && exp
            .tree
            .nodes
            .iter()
            .filter(|n| {
                exp.world
                    .ob(n.object)
                    .is_some_and(|x| x.object_type() == "ARMATURE")
            })
            .count()
            == 1
    {
        let assigned = o.animation_data.as_ref().and_then(|a| a.action);
        let all: Vec<(IdRef<ActionId>, Action)> = exp
            .world
            .main
            .actions
            .iter()
            .map(|(r, a)| (r, a.clone()))
            .collect();
        for (r, act) in all {
            if Some(r) == assigned || actions.actions.contains_key(&r) {
                continue;
            }
            let mut added = false;
            for s in act.slots.iter().filter(|s| s.target_id_type == "OBJECT") {
                if !is_armature_slot(&act, s.handle) || actions.exists(r, s.handle) {
                    continue;
                }
                let Some(sd) = slot_data(&act, s.handle, None) else {
                    continue;
                };
                let mut d = ActionData::new(r, &act);
                d.add_slot(sd);
                if added {
                    d.name = format!("{}-{}", act.header.name, name_display(&s.identifier)).into();
                    actions.add(d, true);
                    continue;
                }
                actions.add(d, false);
                added = true;
            }
        }
    }
    actions
}

/// `__get_blender_actions_broadcast(obj)`.
fn blender_actions_broadcast(exp: &mut Exp<'_>, ob: IdRef<ObjectId>) -> ActionsData {
    let mut out = ActionsData::default();
    let Some(o) = exp.world.ob(ob).cloned() else {
        return out;
    };
    let key_adt = exp
        .world
        .key(ob)
        .is_some_and(|k| k.animation_data.is_some());
    let all: Vec<(IdRef<ActionId>, Action)> = exp
        .world
        .main
        .actions
        .iter()
        .map(|(r, a)| (r, a.clone()))
        .collect();
    for (r, act) in all {
        let mut d = ActionData::new(r, &act);
        let mut added_object = false;
        let mut added_sk = false;
        for s in &act.slots {
            let sd = || slot_data(&act, s.handle, None);
            match s.target_id_type.as_str() {
                "OBJECT" => {
                    if o.animation_data.is_none() {
                        continue;
                    }
                    let arm = is_armature_slot(&act, s.handle);
                    let fits = (o.object_type() == "ARMATURE" && arm)
                        || (o.object_type() == "MESH" && !arm);
                    if !fits {
                        continue;
                    }
                    let Some(sd) = sd() else { continue };
                    if added_object {
                        let mut f = ActionData::new(r, &act);
                        f.add_slot(sd);
                        f.name =
                            format!("{}-{}", act.header.name, name_display(&s.identifier)).into();
                        out.add(f, true);
                        continue;
                    }
                    d.add_slot(sd);
                    out.add(d.clone(), false);
                    added_object = true;
                }
                "KEY" => {
                    if o.object_type() != "MESH" || !key_adt {
                        continue;
                    }
                    let Some(sd) = sd() else { continue };
                    if added_sk {
                        let mut f = ActionData::new(r, &act);
                        f.add_slot(sd);
                        f.name =
                            format!("{}-{}", act.header.name, name_display(&s.identifier)).into();
                        out.add(f, true);
                        continue;
                    }
                    d.add_slot(sd);
                    out.add(d.clone(), false);
                    added_sk = true;
                }
                _ => {}
            }
        }
    }
    out
}

/// `_align_frame_start(reference, frame)`.
fn align(reference: Option<i32>, frame: i32, step: i32) -> i32 {
    match reference {
        Some(r) if step != 1 => r + step * ((f64::from(frame - r) / f64::from(step)).ceil() as i32),
        _ => frame,
    }
}

/// `prepare_actions_range`.
pub(crate) fn prepare_ranges(exp: &mut Exp<'_>) {
    let mut track_slide: IndexMap<Option<SmolStr>, i32> = IndexMap::new();
    let mut reference: Option<i32> = None;
    let step = exp.s.frame_step;
    let objs = exp.exported_objects();
    for &(_, ob, _) in &objs {
        exp.ranges.entry(ob).or_default();
        let actions = blender_actions(exp, ob);
        for ad in actions.values() {
            let Some(act) = exp.world.main.actions.get(ad.action).cloned() else {
                continue;
            };
            for slot in &ad.slots {
                let fr = act.frame_range_get();
                let mut start = fr[0] as i32;
                let mut end = fr[1] as i32;
                if end - start == 1 {
                    // Blender bug 107030 workaround: the slot's F-curve range.
                    let fcs = super::fcurves::slot_fcurves(&act, slot.handle);
                    let r: Vec<[f32; 2]> = fcs
                        .iter()
                        .filter_map(frap_anim::fcurve::keys::range)
                        .collect();
                    if !r.is_empty() {
                        start = r.iter().map(|x| x[0]).fold(f32::INFINITY, f32::min) as i32;
                        end = r.iter().map(|x| x[1]).fold(f32::NEG_INFINITY, f32::max) as i32;
                    }
                }
                if start < 0 && exp.s.negative == Negative::Crop {
                    start = 0;
                }
                if exp.s.frame_range {
                    start = start.max(exp.frame_start);
                    end = end.min(exp.frame_end);
                }
                let key = AKey::Name(act.header.name.clone());
                exp.ranges
                    .entry(ob)
                    .or_default()
                    .insert(key.clone(), (align(reference, start, step), end));
                if reference.is_none() {
                    reference = Some(start);
                    for r in exp.ranges.values_mut().flat_map(|m| m.values_mut()) {
                        r.0 = align(reference, r.0, step);
                    }
                }
                let default_track =
                    |t: &str| t.starts_with("NlaTrack") || t.starts_with("[Action Stash]");
                // `SLIDE` for negative frames, then `export_anim_slide_to_zero`.
                for (enabled, slide) in [
                    (exp.s.negative == Negative::Slide, start < 0),
                    (exp.s.slide_to_zero && start > 0, true),
                ] {
                    if !enabled {
                        continue;
                    }
                    let tracked = match &slot.track {
                        Some(t) if !default_track(t) => Some(Some(t.clone())),
                        None if exp.s.mode == Mode::ActiveActions => Some(None),
                        _ => None,
                    };
                    match tracked {
                        Some(t) => {
                            let e = track_slide.entry(t).or_insert(start);
                            if start < *e {
                                *e = start;
                            }
                        }
                        None if slide => exp.add_slide(start, ob, key.clone()),
                        None => {}
                    }
                }
                if slot.target_id_type == "KEY" && exp.s.bake {
                    let r = (align(reference, exp.frame_start, step), exp.frame_end);
                    exp.ranges.entry(ob).or_default().insert(AKey::Obj(ob), r);
                }
            }
        }
        if actions.is_empty() && exp.s.bake {
            let r = (align(reference, exp.frame_start, step), exp.frame_end);
            exp.ranges.entry(ob).or_default().insert(AKey::Obj(ob), r);
        }
    }
    if (exp.s.negative == Negative::Slide || exp.s.slide_to_zero) && !track_slide.is_empty() {
        for &(_, ob, _) in &objs {
            let actions = blender_actions(exp, ob);
            for ad in actions.values() {
                for slot in &ad.slots {
                    if let Some(&v) = track_slide.get(&slot.track) {
                        let key = AKey::Name(ad.action_name.clone());
                        if (exp.s.negative == Negative::Slide && v < 0) || exp.s.slide_to_zero {
                            exp.add_slide(v, ob, key);
                        }
                    }
                }
            }
        }
    }
}

/// `reset_sk_data` for the `ACTIONS` modes: shape keys back to 0 when more than one action
/// animates them.
fn reset_sk_data(exp: &mut Exp<'_>, ob: IdRef<ObjectId>, actions: &ActionsData) {
    if !exp.s.reset_sk || actions.key_actions() <= 1 {
        return;
    }
    if exp.world.ob(ob).map(|o| o.object_type()) != Some("MESH") {
        return;
    }
    if let Some(k) = exp.world.key_mut(ob) {
        for i in exported_shape_keys(k) {
            k.key_blocks[i].value = 0.0;
        }
    }
}

/// `reset_bone_matrix`: an armature loses its action and its pose goes back to rest.
pub(crate) fn reset_bone_matrix(exp: &mut Exp<'_>, ob: IdRef<ObjectId>) {
    if !exp.s.reset_pose_bones {
        return;
    }
    if exp.world.ob(ob).map(|o| o.object_type()) != Some("ARMATURE") {
        return;
    }
    if let Some(adt) = exp.world.adt_mut(ob)
        && adt.action.is_some()
    {
        adt.action = None;
        adt.action_slot_handle = 0;
    }
    exp.world.reset_pose(ob);
}

/// `gather_actions_animations`.
pub(crate) fn gather(exp: &mut Exp<'_>) -> Vec<GAnim> {
    prepare_ranges(exp);
    let mut animations: Vec<GAnim> = Vec::new();
    let mut merged: IndexMap<SmolStr, Vec<usize>> = IndexMap::new();
    for (_, ob, _) in exp.exported_objects() {
        let offset = animations.len();
        let new = gather_object(exp, ob, &mut merged, offset);
        animations.extend(new);
    }
    if exp.s.mode == Mode::ActiveActions {
        let name: SmolStr = if exp.s.merged_name.is_empty() {
            "Animation".into()
        } else {
            exp.s.merged_name.clone().into()
        };
        merged = IndexMap::from([(name, (0..animations.len()).collect())]);
    }
    if exp.s.merge == Merge::None {
        return animations;
    }
    super::merge_tracks(exp, merged, animations)
}

/// The keyed TRS channels of the object in a slot (`list_of_animated_channels`).
fn animated_object_channels(
    exp: &mut Exp<'_>,
    ob: IdRef<ObjectId>,
    act: &Action,
    handle: i32,
) -> HashMap<&'static str, Interp> {
    let groups = channel_groups(exp, ob, act, handle, false);
    let mut out = HashMap::new();
    for t in groups.targets.iter().filter(|t| t.bone.is_none()) {
        for (prop, chans) in &t.props {
            if let Some(c) = get_target(prop).as_ref().and_then(channel_from_target)
                && let Some(f) = chans.iter().flatten().next()
            {
                out.insert(
                    c,
                    Interp::from_blender(f.keyframe_points[0].interpolation, exp.s.fallback),
                );
            }
        }
    }
    for (ty, c, _) in &groups.to_be_sampled {
        if *ty == TType::Object {
            out.insert(*c, exp.s.fallback);
        }
    }
    out
}

/// The keyed channels of the bones and of the armature object in a slot.
#[allow(clippy::type_complexity)]
fn animated_armature_channels(
    exp: &mut Exp<'_>,
    ob: IdRef<ObjectId>,
    act: &Action,
    handle: i32,
) -> (
    HashMap<(SmolStr, &'static str), Interp>,
    Vec<(&'static str, Interp)>,
) {
    let groups = channel_groups(exp, ob, act, handle, false);
    let mut bones = HashMap::new();
    let mut object = Vec::new();
    for t in &groups.targets {
        for (prop, chans) in &t.props {
            let Some(f) = chans.iter().flatten().next() else {
                continue;
            };
            let interp = Interp::from_blender(f.keyframe_points[0].interpolation, exp.s.fallback);
            let Some(c) = get_target(prop).as_ref().and_then(channel_from_target) else {
                continue;
            };
            match (&t.bone, t.ty) {
                (Some(b), _) => {
                    bones.insert((b.clone(), c), interp);
                }
                (None, TType::Object) => object.push((c, interp)),
                _ => {}
            }
        }
    }
    for (ty, c, bone) in &groups.to_be_sampled {
        match (ty, bone) {
            (TType::Bone, Some(b)) => {
                bones.insert((b.clone(), *c), exp.s.fallback);
            }
            (TType::Object, _) => object.push((*c, exp.s.fallback)),
            _ => {}
        }
    }
    (bones, object)
}

/// `gather_action_animations(obj)`.
fn gather_object(
    exp: &mut Exp<'_>,
    ob: IdRef<ObjectId>,
    merged: &mut IndexMap<SmolStr, Vec<usize>>,
    offset: usize,
) -> Vec<GAnim> {
    let mut animations = Vec::new();
    let actions = blender_actions(exp, ob);
    if actions.is_empty() {
        if let Some(a) = super::bake_animation(exp, ob, AKey::Obj(ob), None) {
            animations.push(a);
        }
        return animations;
    }
    let Some(o) = exp.world.ob(ob).cloned() else {
        return animations;
    };
    let ob_type = o.object_type();
    // Keep the current state.
    let saved_adt = o.animation_data.clone();
    let saved_transform = (
        o.location,
        o.rotation_euler,
        o.rotation_quaternion,
        o.rotation_axis_angle,
        o.scale,
    );
    let restore_world = saved_adt.as_ref().is_some_and(|adt| {
        adt.action.is_none()
            || adt
                .action
                .and_then(|a| exp.world.main.actions.get(a))
                .is_none_or(|a| a.slot(adt.action_slot_handle).is_none())
    });
    let saved_key_adt = exp.world.key(ob).and_then(|k| k.animation_data.clone());
    if let Some(adt) = exp.world.adt_mut(ob) {
        for t in &mut adt.nla_tracks {
            if t.is_solo {
                t.is_solo = false;
                break;
            }
        }
        adt.use_nla = false;
    }
    let has_object_slot = actions
        .values()
        .iter()
        .any(|a| a.slots.iter().any(|s| s.target_id_type == "OBJECT"));
    let has_key_slot = actions
        .values()
        .iter()
        .any(|a| a.slots.iter().any(|s| s.target_id_type == "KEY"));
    for ad in actions.values() {
        let Some(act) = exp.world.main.actions.get(ad.action).cloned() else {
            continue;
        };
        let mut channels: Vec<GChannel> = Vec::new();
        let mut track_name: Option<SmolStr> = None;
        for slot in &ad.slots {
            track_name = slot.track.clone();
            let key = AKey::Name(act.header.name.clone());
            let sl = Some(slot.identifier.clone());
            match slot.target_id_type.as_str() {
                "OBJECT" => {
                    let current = exp
                        .world
                        .ob(ob)
                        .and_then(|o| o.animation_data.as_ref())
                        .map(|a| (a.action, a.action_slot_handle));
                    let switch = match current {
                        Some((Some(a), h)) => {
                            let same_name = exp
                                .world
                                .main
                                .actions
                                .get(a)
                                .is_some_and(|x| x.header.name == act.header.name);
                            let slot_ok = exp
                                .world
                                .main
                                .actions
                                .get(a)
                                .is_some_and(|x| x.slot(h).is_some());
                            !same_name || !slot_ok || h != slot.handle
                        }
                        _ => true,
                    };
                    if switch {
                        reset_bone_matrix(exp, ob);
                        let adt = exp
                            .world
                            .ob_mut(ob)
                            .map(|o| o.animation_data.get_or_insert_with(AnimData::default));
                        if let Some(adt) = adt {
                            adt.action = Some(ad.action);
                            adt.action_slot_handle = slot.handle;
                        }
                    }
                }
                "KEY" => {
                    let current = exp
                        .world
                        .key(ob)
                        .and_then(|k| k.animation_data.as_ref())
                        .map(|a| (a.action, a.action_slot_handle));
                    let switch = match current {
                        Some((Some(a), h)) => {
                            exp.world
                                .main
                                .actions
                                .get(a)
                                .is_none_or(|x| x.header.name != act.header.name)
                                || h != slot.handle
                        }
                        _ => true,
                    };
                    if switch {
                        reset_sk_data(exp, ob, &actions);
                        if let Some(k) = exp.world.key_mut(ob) {
                            let adt = k.animation_data.get_or_insert_with(AnimData::default);
                            adt.action = Some(ad.action);
                            adt.action_slot_handle = slot.handle;
                        }
                    }
                }
                _ => {}
            }
            if exp.s.force_sampling {
                if ob_type == "ARMATURE" {
                    let (bones, object) = animated_armature_channels(exp, ob, &act, slot.handle);
                    channels.extend(exp.armature_channels(ob, &key, &sl, &bones, Some(&object)));
                } else if slot.target_id_type == "OBJECT" {
                    let animated = animated_object_channels(exp, ob, &act, slot.handle);
                    channels.extend(exp.object_channels(ob, &key, &sl, &animated));
                } else if let Some(c) = exp.sk_channel(ob, &key, &sl) {
                    channels.push(c);
                }
            } else {
                let (chs, to_be_sampled) = fcurve_channels(exp, ob, &act, slot.handle);
                channels.extend(chs);
                for (ty, prop, bone) in to_be_sampled {
                    let c = match (ty, bone) {
                        (TType::Bone, Some(b)) => {
                            exp.bone_channel(ob, &b, prop, &key, &sl, Some(exp.s.fallback))
                        }
                        (TType::Object, _) => {
                            exp.object_channel(ob, prop, &key, &sl, Some(exp.s.fallback))
                        }
                        (TType::Sk, _) => exp.sk_channel(ob, &key, &sl),
                        _ => None,
                    };
                    channels.extend(c);
                }
            }
            // Bake the TRS of a shape-key-only animation, and the shape keys of a TRS-only
            // one, with `export_bake_animation`.
            if exp.s.bake && exp.s.force_sampling {
                if !has_object_slot && slot.target_id_type == "KEY" {
                    let r = exp.ranges.get(&ob).and_then(|m| m.get(&key)).copied();
                    if let Some(r) = r {
                        exp.ranges.entry(ob).or_default().insert(AKey::Obj(ob), r);
                    }
                    channels.extend(exp.object_channels(
                        ob,
                        &AKey::Obj(ob),
                        &None,
                        &HashMap::new(),
                    ));
                }
                if !has_key_slot
                    && exp.s.morph_anim
                    && ob_type == "MESH"
                    && exp.world.key(ob).is_some()
                {
                    let r = exp.ranges.get(&ob).and_then(|m| m.get(&key)).copied();
                    if let Some(r) = r {
                        exp.ranges.entry(ob).or_default().insert(AKey::Obj(ob), r);
                    }
                    channels.extend(exp.sk_channel(ob, &AKey::Obj(ob), &None));
                }
            }
        }
        if channels.is_empty() {
            continue;
        }
        animations.push(GAnim {
            name: act.header.name.to_string(),
            channels,
            sources: vec![Source::Action(ad.action)],
        });
        let idx = offset + animations.len() - 1;
        match exp.s.merge {
            Merge::NlaTrack => {
                if let Some(t) = track_name
                    && !(t.starts_with("NlaTrack") || t.starts_with("[Action Stash]"))
                {
                    merged.entry(t).or_default().push(idx);
                }
            }
            Merge::Action => merged.entry(ad.name.clone()).or_default().push(idx),
            Merge::None => {}
        }
    }
    // Restore the state.
    if let Some(saved) = saved_adt {
        let current_action = exp
            .world
            .ob(ob)
            .and_then(|o| o.animation_data.as_ref())
            .and_then(|a| a.action);
        if current_action.is_some() {
            let differs = match saved.action {
                None => true,
                Some(s) => {
                    let name = |r: IdRef<ActionId>| {
                        exp.world.main.actions.get(r).map(|a| a.header.name.clone())
                    };
                    name(s) != current_action.and_then(name)
                        || exp
                            .world
                            .ob(ob)
                            .and_then(|o| o.animation_data.as_ref())
                            .map(|a| a.action_slot_handle)
                            != Some(saved.action_slot_handle)
                }
            };
            if differs {
                reset_bone_matrix(exp, ob);
            }
        }
        if let Some(adt) = exp.world.adt_mut(ob) {
            adt.action = saved.action;
            adt.action_slot_handle = saved.action_slot_handle;
            adt.use_nla = saved.use_nla;
            adt.use_tweak_mode = saved.use_tweak_mode;
            for (t, st) in adt.nla_tracks.iter_mut().zip(&saved.nla_tracks) {
                t.is_solo = st.is_solo;
            }
        }
    }
    if ob_type == "MESH"
        && exp
            .world
            .key(ob)
            .is_some_and(|k| k.animation_data.is_some())
    {
        reset_sk_data(exp, ob, &actions);
        if let Some(k) = exp.world.key_mut(ob)
            && let Some(adt) = k.animation_data.as_mut()
        {
            let saved = saved_key_adt.as_ref();
            adt.action = saved.and_then(|s| s.action);
            if let Some(s) = saved
                && s.action.is_some()
            {
                adt.action_slot_handle = s.action_slot_handle;
            }
        }
    }
    if restore_world && let Some(o) = exp.world.ob_mut(ob) {
        (
            o.location,
            o.rotation_euler,
            o.rotation_quaternion,
            o.rotation_axis_angle,
            o.scale,
        ) = saved_transform;
    }
    animations
}
