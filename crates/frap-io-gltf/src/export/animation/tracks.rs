// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_animation_mode='NLA_TRACKS'`: one glTF animation per NLA track (a REPLACE track
//! with the non-REPLACE tracks above it), baked with only that track unmuted, merged across
//! objects by track name. An object without NLA tracks exports no animation.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/animation/tracks.py` (Apache-2.0, The
//! glTF-Blender-IO authors): `gather_tracks_animations`, `gather_track_animations`,
//! `__get_nla_tracks_obj`, `__get_nla_tracks_sk`, `prepare_tracks_range`.

use frap_anim::AnimData;
use frap_rna::{IdRef, ObjectId};
use indexmap::IndexMap;
use smol_str::SmolStr;

use super::actions::reset_bone_matrix;
use super::exp::{AKey, Exp};
use super::world::exported_shape_keys;
use super::{GAnim, Negative};

/// A group of NLA tracks exported as one animation (`TrackData`).
#[derive(Clone, Debug)]
struct TrackGroup {
    /// Indices into `nla_tracks`.
    tracks: Vec<usize>,
    /// (frame_start of the first strip, frame_end of the last) per track.
    ranges: Vec<(f32, f32)>,
    /// The first track's name.
    name: SmolStr,
    /// `OBJECT` or `KEY`.
    on_key: bool,
}

/// `__get_nla_tracks_obj` / `__get_nla_tracks_sk`.
fn groups_of(adt: &AnimData, on_key: bool) -> Vec<TrackGroup> {
    let mut out = Vec::new();
    let mut current: Vec<(usize, (f32, f32))> = Vec::new();
    let flush = |cur: &mut Vec<(usize, (f32, f32))>, out: &mut Vec<TrackGroup>| {
        if let Some(&(first, _)) = cur.first() {
            out.push(TrackGroup {
                tracks: cur.iter().map(|c| c.0).collect(),
                ranges: cur.iter().map(|c| c.1).collect(),
                name: adt.nla_tracks[first].name.clone(),
                on_key,
            });
        }
        cur.clear();
    };
    for (i, t) in adt.nla_tracks.iter().enumerate() {
        let (Some(first), Some(last)) = (t.strips.first(), t.strips.last()) else {
            continue;
        };
        if t.strips.iter().all(|s| s.blend_type == "REPLACE") {
            flush(&mut current, &mut out);
        }
        current.push((i, (first.frame_start, last.frame_end)));
    }
    flush(&mut current, &mut out);
    out
}

/// `prepare_tracks_range`: the range (and slide) of one track group.
fn prepare_range(exp: &mut Exp<'_>, ob: IdRef<ObjectId>, g: &TrackGroup) {
    let mut start = 0.0f32;
    let mut end = 0.0f32;
    for (i, &(a, b)) in g.ranges.iter().enumerate() {
        start = if i == 0 { a } else { start.min(a) };
        end = if i == 0 { b } else { end.max(b) };
    }
    if start < 0.0 && exp.s.negative == Negative::Crop {
        start = 0.0;
    }
    if exp.s.frame_range {
        start = start.max(exp.frame_start as f32);
        end = end.min(exp.frame_end as f32);
    }
    let key = AKey::Name(g.name.clone());
    let mut r = IndexMap::new();
    r.insert(key.clone(), (start as i32, end as i32));
    exp.ranges.insert(ob, r);
    let default_track = g.name.starts_with("NlaTrack") || g.name.starts_with("[Action Stash]");
    // A named track slides by its own start; a default-named one only when negative (SLIDE)
    // or positive (`export_anim_slide_to_zero`).
    let mut slide: Option<f32> = None;
    if exp.s.negative == Negative::Slide {
        if !default_track {
            slide = Some(start);
        } else if start < 0.0 {
            exp.add_slide(start as i32, ob, key.clone());
        }
    }
    if exp.s.slide_to_zero && start > 0.0 {
        if !default_track {
            slide = Some(slide.map_or(start, |s| s.min(start)));
        } else {
            exp.add_slide(start as i32, ob, key.clone());
        }
    }
    if let Some(v) = slide
        && ((exp.s.negative == Negative::Slide && v < 0.0) || exp.s.slide_to_zero)
    {
        exp.add_slide(v as i32, ob, key);
    }
}

/// `gather_tracks_animations`.
pub(crate) fn gather(exp: &mut Exp<'_>) -> Vec<GAnim> {
    let mut animations: Vec<GAnim> = Vec::new();
    let mut merged: IndexMap<SmolStr, Vec<usize>> = IndexMap::new();
    for (_, ob, _) in exp.exported_objects() {
        let offset = animations.len();
        let new = gather_object(exp, ob, &mut merged, offset);
        animations.extend(new);
    }
    super::merge_tracks(exp, merged, animations)
}

/// `gather_track_animations(obj)`.
fn gather_object(
    exp: &mut Exp<'_>,
    ob: IdRef<ObjectId>,
    merged: &mut IndexMap<SmolStr, Vec<usize>>,
    offset: usize,
) -> Vec<GAnim> {
    let mut animations = Vec::new();
    let Some(o) = exp.world.ob(ob).cloned() else {
        return animations;
    };
    let is_mesh = o.object_type() == "MESH";
    let mut groups: Vec<TrackGroup> = o
        .animation_data
        .as_ref()
        .map(|a| groups_of(a, false))
        .unwrap_or_default();
    let key_adt = if is_mesh {
        exp.world.key(ob).and_then(|k| k.animation_data.clone())
    } else {
        None
    };
    if let Some(kadt) = &key_adt {
        groups.extend(groups_of(kadt, true));
    }
    if groups.is_empty() {
        return animations;
    }
    let saved_adt = o.animation_data.clone();
    let saved_transform = (
        o.location,
        o.rotation_euler,
        o.rotation_quaternion,
        o.rotation_axis_angle,
        o.scale,
    );
    // No active action, NLA on, nothing soloed, every exported track muted.
    if let Some(adt) = exp.world.adt_mut(ob) {
        adt.action = None;
        adt.action_slot_handle = 0;
        adt.use_nla = true;
        if let Some(t) = adt.nla_tracks.iter_mut().find(|t| t.is_solo) {
            t.is_solo = false;
        }
    }
    if let Some(adt) = exp.world.key_adt_mut(ob)
        && let Some(t) = adt.nla_tracks.iter_mut().find(|t| t.is_solo)
    {
        t.is_solo = false;
    }
    let mute = |exp: &mut Exp<'_>, g: &TrackGroup, m: bool| {
        let adt = if g.on_key {
            exp.world.key_adt_mut(ob)
        } else {
            exp.world.adt_mut(ob)
        };
        if let Some(adt) = adt {
            for &i in &g.tracks {
                if let Some(t) = adt.nla_tracks.get_mut(i) {
                    t.mute = m;
                }
            }
        }
    };
    for g in &groups {
        mute(exp, g, true);
    }
    let key_groups = groups.iter().filter(|g| g.on_key).count();
    for g in &groups {
        prepare_range(exp, ob, g);
        mute(exp, g, false);
        reset_bone_matrix(exp, ob);
        if g.on_key
            && exp.s.reset_sk
            && key_groups > 1
            && is_mesh
            && let Some(k) = exp.world.key_mut(ob)
        {
            for i in exported_shape_keys(k) {
                k.key_blocks[i].value = 0.0;
            }
        }
        let anim = super::bake_animation(
            exp,
            ob,
            AKey::Name(g.name.clone()),
            Some(if g.on_key { "KEY" } else { "OBJECT" }),
        );
        exp.reset_cache();
        if let Some(a) = anim {
            animations.push(a);
            if !(g.name.starts_with("NlaTrack") || g.name.starts_with("[Action Stash]")) {
                merged
                    .entry(g.name.clone())
                    .or_default()
                    .push(offset + animations.len() - 1);
            }
        }
        mute(exp, g, true);
    }
    // Restore.
    if let Some(saved) = &saved_adt
        && let Some(adt) = exp.world.adt_mut(ob)
    {
        if saved.action.is_some() {
            adt.action = saved.action;
            adt.action_slot_handle = saved.action_slot_handle;
        }
        adt.use_nla = saved.use_nla;
        adt.use_tweak_mode = saved.use_tweak_mode;
        for (t, st) in adt.nla_tracks.iter_mut().zip(&saved.nla_tracks) {
            t.is_solo = st.is_solo;
            t.mute = st.mute;
        }
    }
    if let Some(saved) = &key_adt
        && let Some(adt) = exp.world.key_adt_mut(ob)
    {
        if saved.action.is_some() {
            adt.action = saved.action;
            adt.action_slot_handle = saved.action_slot_handle;
        }
        adt.use_nla = saved.use_nla;
        for (t, st) in adt.nla_tracks.iter_mut().zip(&saved.nla_tracks) {
            t.is_solo = st.is_solo;
            t.mute = st.mute;
        }
    }
    if let Some(o) = exp.world.ob_mut(ob) {
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

#[cfg(test)]
mod tests {
    use frap_anim::{NlaStrip, NlaTrack};

    use super::*;

    fn track(name: &str, blend: &str, start: f32, end: f32) -> NlaTrack {
        let mut t = NlaTrack::new(name);
        let mut s = NlaStrip::new_clip("s", None, start, [start, end]);
        s.blend_type = blend.into();
        t.strips.push(s);
        t
    }

    #[test]
    fn non_replace_tracks_join_the_track_below() {
        let mut adt = AnimData::default();
        adt.nla_tracks.push(track("Walk", "REPLACE", 1.0, 10.0));
        adt.nla_tracks.push(track("Add", "ADD", 5.0, 20.0));
        adt.nla_tracks.push(NlaTrack::new("Empty"));
        adt.nla_tracks.push(track("Run", "REPLACE", 3.0, 8.0));
        let g = groups_of(&adt, false);
        assert_eq!(g.len(), 2);
        assert_eq!(
            (g[0].name.as_str(), g[0].tracks.clone()),
            ("Walk", vec![0, 1])
        );
        assert_eq!(g[0].ranges, vec![(1.0, 10.0), (5.0, 20.0)]);
        assert_eq!((g[1].name.as_str(), g[1].tracks.clone()), ("Run", vec![3]));
    }
}
