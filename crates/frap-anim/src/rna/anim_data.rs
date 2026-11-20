// SPDX-License-Identifier: GPL-3.0-or-later
//! `AnimData`, `NlaTrack` and `NlaStrip`, as index proxies over the `AnimData`.
//!
//! The NLA strip setters port `rna_NlaStrip_*_set` of
//! blender/source/blender/makesrna/intern/rna_nla.cc and `BKE_nlastrip_recalculate_bounds` /
//! `nlastrip_fix_resize_overlaps` of blenkernel/intern/nla.cc (Blender 5.2.4,
//! GPL-2.0-or-later). Strip F-curves (animated influence and time) are not modelled: an
//! animated influence or strip time is its stored value.

use frap_rna::traits::{index_out_of_range, read_only, wrong_access};
use frap_rna::*;
use smol_str::SmolStr;

use super::{EmptyColl, as_bool, as_f32, as_str, boolean, float, int, string, unbound};
use crate::anim_data::{AnimData, NlaStrip, NlaTrack};
use crate::eval::curve::is_eqf;

/// `NLASTRIP_MIN_LEN_THRESH`
const NLASTRIP_MIN_LEN_THRESH: f32 = 0.1;
/// `MINAFRAMEF`
const MINAFRAMEF: f32 = -1_048_574.0;
/// `MAXFRAMEF`
const MAXFRAMEF: f32 = 1_048_574.0;

/// Where a struct lives inside an `AnimData`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AdtLoc {
    /// The `AnimData`.
    Root,
    /// `nla_tracks[t]`
    Track(usize),
    /// `nla_tracks[t].strips[s]`
    Strip(usize, usize),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AdtColl {
    Tracks,
    Strips(usize),
}

/// The struct definition of `loc`.
pub fn srna_of(loc: AdtLoc) -> &'static StructDef {
    match loc {
        AdtLoc::Root => &frap_spec_rna::ANIM_DATA,
        AdtLoc::Track(_) => &frap_spec_rna::NLA_TRACK,
        AdtLoc::Strip(..) => &frap_spec_rna::NLA_STRIP,
    }
}

fn coll_of(loc: AdtLoc, prop: &str) -> Option<AdtColl> {
    match (loc, prop) {
        (AdtLoc::Root, "nla_tracks") => Some(AdtColl::Tracks),
        (AdtLoc::Track(t), "strips") => Some(AdtColl::Strips(t)),
        _ => None,
    }
}

fn coll_len(adt: &AnimData, c: AdtColl) -> usize {
    match c {
        AdtColl::Tracks => adt.nla_tracks.len(),
        AdtColl::Strips(t) => adt.nla_tracks.get(t).map_or(0, |x| x.strips.len()),
    }
}

fn coll_key(adt: &AnimData, c: AdtColl, i: usize) -> Option<SmolStr> {
    match c {
        AdtColl::Tracks => adt.nla_tracks.get(i).map(|t| t.name.clone()),
        AdtColl::Strips(t) => adt.nla_tracks.get(t)?.strips.get(i).map(|s| s.name.clone()),
    }
}

fn item_loc(c: AdtColl, i: usize) -> AdtLoc {
    match c {
        AdtColl::Tracks => AdtLoc::Track(i),
        AdtColl::Strips(t) => AdtLoc::Strip(t, i),
    }
}

/// Locates an `RnaPtr` path below `animation_data` (`nla_tracks["NlaTrack"].strips["S"]`).
/// A trailing collection name is returned with its owner.
pub fn locate<'s>(adt: &AnimData, path: &'s [Seg]) -> Option<(AdtLoc, Option<&'s str>)> {
    let mut loc = AdtLoc::Root;
    let mut i = 0;
    while i < path.len() {
        let Seg::Prop(name) = &path[i] else {
            return None;
        };
        let c = coll_of(loc, name)?;
        let Some(sub) = path.get(i + 1) else {
            return Some((loc, Some(name)));
        };
        let n = coll_len(adt, c);
        let index = match sub {
            Seg::Index(x) => {
                let x = if *x < 0 { *x + n as i64 } else { *x };
                usize::try_from(x).ok().filter(|x| *x < n)?
            }
            Seg::Key(k) => (0..n).find(|&j| coll_key(adt, c, j).as_deref() == Some(k))?,
            Seg::Prop(_) => return Some((loc, Some(name))),
        };
        loc = item_loc(c, index);
        i += 2;
    }
    Some((loc, None))
}

/// A struct inside an `AnimData`, read-only.
pub struct Adt<'a> {
    /// The animation data.
    pub adt: &'a AnimData,
    /// Where.
    pub loc: AdtLoc,
}

/// A struct inside an `AnimData`, writable.
pub struct AdtMut<'a> {
    /// The animation data.
    pub adt: &'a mut AnimData,
    /// Where.
    pub loc: AdtLoc,
}

struct AdtCollView<'a> {
    adt: &'a AnimData,
    c: AdtColl,
}

struct AdtCollViewMut<'a> {
    adt: &'a mut AnimData,
    c: AdtColl,
}

impl Rna for AnimData {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::ANIM_DATA
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        get(self, AdtLoc::Root, prop)
    }
}

impl RnaMut for AnimData {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        set(self, AdtLoc::Root, prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(wrong_access(&frap_spec_rna::ANIM_DATA, prop.identifier, "a struct"))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        coll_mut(self, AdtLoc::Root, prop)
    }
}

impl Rna for Adt<'_> {
    fn srna(&self) -> &'static StructDef {
        srna_of(self.loc)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        get(self.adt, self.loc, prop)
    }
}

impl Rna for AdtMut<'_> {
    fn srna(&self) -> &'static StructDef {
        srna_of(self.loc)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        get(self.adt, self.loc, prop)
    }
}

impl RnaMut for AdtMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        set(self.adt, self.loc, prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(wrong_access(srna_of(self.loc), prop.identifier, "a struct"))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        coll_mut(self.adt, self.loc, prop)
    }
}

impl RnaColl for AdtCollView<'_> {
    fn item_type(&self) -> &'static StructDef {
        match self.c {
            AdtColl::Tracks => &frap_spec_rna::NLA_TRACK,
            AdtColl::Strips(_) => &frap_spec_rna::NLA_STRIP,
        }
    }
    fn len(&self) -> usize {
        coll_len(self.adt, self.c)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        coll_key(self.adt, self.c, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        if i >= self.len() {
            return Err(index_out_of_range(i, self.len()));
        }
        Ok(Item::Struct(StructRef::Owned(Box::new(Adt {
            adt: self.adt,
            loc: item_loc(self.c, i),
        }))))
    }
}

impl RnaColl for AdtCollViewMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        match self.c {
            AdtColl::Tracks => &frap_spec_rna::NLA_TRACK,
            AdtColl::Strips(_) => &frap_spec_rna::NLA_STRIP,
        }
    }
    fn len(&self) -> usize {
        coll_len(self.adt, self.c)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        coll_key(self.adt, self.c, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        if i >= self.len() {
            return Err(index_out_of_range(i, self.len()));
        }
        Ok(Item::Struct(StructRef::Owned(Box::new(Adt {
            adt: self.adt,
            loc: item_loc(self.c, i),
        }))))
    }
}

impl RnaCollMut for AdtCollViewMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        if i >= self.len() {
            return Err(index_out_of_range(i, self.len()));
        }
        let loc = item_loc(self.c, i);
        Ok(Box::new(AdtMut {
            adt: &mut *self.adt,
            loc,
        }))
    }
}

fn stale() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "StructRNA of type NlaStrip has been removed",
    )
}

/// Reads `prop` of the struct at `loc`.
pub fn get<'a>(adt: &'a AnimData, loc: AdtLoc, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    let id = prop.identifier;
    if let Some(c) = coll_of(loc, id) {
        return Ok(Field::Coll(CollRef::Owned(Box::new(AdtCollView { adt, c }))));
    }
    let srna = srna_of(loc);
    match loc {
        AdtLoc::Root => Ok(match id {
            "action" => Field::Id(adt.action.map(|a| a.any())),
            "action_extrapolation" => string(prop, &adt.action_extrapolation),
            "action_blend_type" => string(prop, &adt.action_blend_type),
            "action_influence" => float(prop, adt.action_influence),
            "action_tweak_storage" => Field::Id(None),
            "action_slot_handle_tweak_storage" => int(0),
            "drivers" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(&frap_spec_rna::F_CURVE)))),
            "use_nla" => boolean(adt.use_nla),
            "use_tweak_mode" => boolean(adt.use_tweak_mode),
            "use_pin" => boolean(adt.use_pin),
            "action_slot_handle" => int(i64::from(adt.action_slot_handle)),
            "last_slot_identifier" => string(prop, &adt.last_slot_identifier),
            // The slot lives in the action: the hook resolves it.
            "action_slot" | "action_suitable_slots" => Field::NeedsHook,
            _ => return Err(unbound(srna, prop)),
        }),
        AdtLoc::Track(t) => {
            let x = adt.nla_tracks.get(t).ok_or_else(stale)?;
            Ok(match id {
                "is_override_data" => boolean(false),
                "name" => string(prop, &x.name),
                "active" => boolean(x.active),
                "is_solo" => boolean(x.is_solo),
                "select" => boolean(x.select),
                "mute" => boolean(x.mute),
                "lock" => boolean(x.lock),
                _ => return Err(unbound(srna, prop)),
            })
        }
        AdtLoc::Strip(t, s) => {
            let x = adt
                .nla_tracks
                .get(t)
                .and_then(|t| t.strips.get(s))
                .ok_or_else(stale)?;
            Ok(match id {
                "name" => string(prop, &x.name),
                "type" => string(prop, &x.r#type),
                "extrapolation" => string(prop, &x.extrapolation),
                "blend_type" => string(prop, &x.blend_type),
                "frame_start" | "frame_start_raw" | "frame_start_ui" => float(prop, x.frame_start),
                "frame_end" | "frame_end_raw" | "frame_end_ui" => float(prop, x.frame_end),
                "blend_in" => float(prop, x.blend_in),
                "blend_out" => float(prop, x.blend_out),
                "use_auto_blend" => boolean(x.use_auto_blend),
                "action" => Field::Id(x.action.map(|a| a.any())),
                "action_slot_handle" => int(i64::from(x.action_slot_handle)),
                "last_slot_identifier" => string(prop, &x.last_slot_identifier),
                "action_slot" | "action_suitable_slots" => Field::NeedsHook,
                "action_frame_start" => float(prop, x.action_frame_start),
                "action_frame_end" => float(prop, x.action_frame_end),
                "repeat" => float(prop, x.repeat),
                "scale" => float(prop, x.scale),
                "fcurves" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(&frap_spec_rna::F_CURVE)))),
                "modifiers" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(
                    &frap_spec_rna::F_MODIFIER,
                )))),
                "strips" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(&frap_spec_rna::NLA_STRIP)))),
                "influence" => float(prop, x.influence),
                "strip_time" => float(prop, x.strip_time),
                "use_animated_influence" => boolean(x.use_animated_influence),
                "use_animated_time" => boolean(x.use_animated_time),
                "use_animated_time_cyclic" => boolean(x.use_animated_time_cyclic),
                "active" => boolean(x.active),
                "select" => boolean(x.select),
                "mute" => boolean(x.mute),
                "use_reverse" => boolean(x.use_reverse),
                "use_sync_length" => boolean(x.use_sync_length),
                _ => return Err(unbound(srna, prop)),
            })
        }
    }
}

/// `BKE_nlastrip_compute_frame_from_previous_strip`
fn limit_prev(track: &NlaTrack, s: usize) -> f32 {
    match s.checked_sub(1).map(|p| &track.strips[p]) {
        Some(p) if p.r#type == "TRANSITION" => p.frame_start + NLASTRIP_MIN_LEN_THRESH,
        Some(p) => p.frame_end,
        None => MINAFRAMEF,
    }
}

/// `BKE_nlastrip_compute_frame_to_next_strip`
fn limit_next(track: &NlaTrack, s: usize) -> f32 {
    match track.strips.get(s + 1) {
        Some(n) if n.r#type == "TRANSITION" => n.frame_end - NLASTRIP_MIN_LEN_THRESH,
        Some(n) => n.frame_start,
        None => MAXFRAMEF,
    }
}

/// `CLAMP(v, lo, hi)`
fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `BKE_nla_clip_length_get_nonzero`
fn clip_length(x: &NlaStrip) -> f32 {
    if x.action_frame_end <= x.action_frame_start {
        1.0
    } else {
        x.action_frame_end - x.action_frame_start
    }
}

/// `BKE_nlastrip_recalculate_bounds` with `nlastrip_fix_resize_overlaps`: the end follows the
/// action range, scale and repeat; a following strip that now overlaps is pushed later.
fn recalculate_bounds(track: &mut NlaTrack, s: usize) {
    let x = &mut track.strips[s];
    if x.r#type != "CLIP" {
        return;
    }
    let actlen = clip_length(x);
    let mapping = x.scale * x.repeat;
    if !is_eqf(mapping, 0.0) {
        x.frame_end = (actlen * mapping) + x.frame_start;
    }
    // `nlastrip_fix_resize_overlaps` (next strips; transitions are not modelled): every
    // following strip moves by the whole-frame overlap.
    let end = track.strips[s].frame_end;
    if let Some(n) = track.strips.get(s + 1)
        && end > n.frame_start
    {
        let offset = (end - n.frame_start).ceil();
        for n in track.strips.iter_mut().skip(s + 1) {
            n.frame_start += offset;
            n.frame_end += offset;
        }
    }
}

/// Writes `prop` of the struct at `loc`.
pub fn set(adt: &mut AnimData, loc: AdtLoc, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
    let id = prop.identifier;
    let srna = srna_of(loc);
    let done = Ok(SetOutcome::Done);
    match loc {
        AdtLoc::Root => {
            match id {
                // Assignment chooses a slot and needs the actions: hooks.
                "action" | "action_slot" | "action_slot_handle" => {
                    return Ok(SetOutcome::NeedsHook);
                }
                "action_extrapolation" => adt.action_extrapolation = as_str(prop, &v)?.into(),
                "action_blend_type" => adt.action_blend_type = as_str(prop, &v)?.into(),
                "action_influence" => adt.action_influence = as_f32(prop, &v)?,
                "use_nla" => adt.use_nla = as_bool(prop, &v)?,
                "use_pin" => adt.use_pin = as_bool(prop, &v)?,
                "last_slot_identifier" => adt.last_slot_identifier = as_str(prop, &v)?.into(),
                "use_tweak_mode" => {
                    if as_bool(prop, &v)? {
                        return Err(RnaError::new(
                            RnaErrorKind::NotImplemented,
                            PyExc::NotImplementedError,
                            "frap: NLA tweak mode (AnimData.use_tweak_mode = True) is not \
                             implemented yet",
                        ));
                    }
                    adt.use_tweak_mode = false;
                }
                _ if prop.is_readonly() => return Err(read_only(srna, id)),
                _ => return Err(unbound(srna, prop)),
            }
            done
        }
        AdtLoc::Track(t) => {
            if t >= adt.nla_tracks.len() {
                return Err(stale());
            }
            if id == "is_solo" {
                // `rna_NlaTrack_solo_set`: at most one soloed track.
                let on = as_bool(prop, &v)?;
                for (i, x) in adt.nla_tracks.iter_mut().enumerate() {
                    x.is_solo = on && i == t;
                }
                return done;
            }
            let x = &mut adt.nla_tracks[t];
            match id {
                "name" => x.name = as_str(prop, &v)?.into(),
                "select" => x.select = as_bool(prop, &v)?,
                "mute" => x.mute = as_bool(prop, &v)?,
                "lock" => x.lock = as_bool(prop, &v)?,
                _ if prop.is_readonly() => return Err(read_only(srna, id)),
                _ => return Err(unbound(srna, prop)),
            }
            done
        }
        AdtLoc::Strip(t, s) => {
            let track = adt.nla_tracks.get_mut(t).ok_or_else(stale)?;
            if s >= track.strips.len() {
                return Err(stale());
            }
            strip_set(track, s, prop, v)
        }
    }
}

fn strip_set(track: &mut NlaTrack, s: usize, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
    let id = prop.identifier;
    let done = Ok(SetOutcome::Done);
    let (lp, ln) = (limit_prev(track, s), limit_next(track, s));
    match id {
        "action" | "action_slot" | "action_slot_handle" => return Ok(SetOutcome::NeedsHook),
        "frame_start" | "frame_start_raw" => {
            // `rna_NlaStrip_start_frame_set`
            let x = &mut track.strips[s];
            x.frame_start = clamp(as_f32(prop, &v)?, lp, ln);
            if x.frame_start > x.frame_end - NLASTRIP_MIN_LEN_THRESH
                && x.frame_start + NLASTRIP_MIN_LEN_THRESH > ln
            {
                x.frame_end = ln;
                x.frame_start = x.frame_end - NLASTRIP_MIN_LEN_THRESH;
            }
        }
        "frame_end" | "frame_end_raw" => {
            // `rna_NlaStrip_end_frame_set`
            let x = &mut track.strips[s];
            x.frame_end = clamp(as_f32(prop, &v)?, lp, ln);
            if x.frame_end < x.frame_start + NLASTRIP_MIN_LEN_THRESH
                && x.frame_end - NLASTRIP_MIN_LEN_THRESH < lp
            {
                x.frame_start = lp;
                x.frame_end = x.frame_start + NLASTRIP_MIN_LEN_THRESH;
            }
        }
        "frame_start_ui" => {
            // `rna_NlaStrip_frame_start_ui_set`: moves the strip, keeping its length.
            let x = &mut track.strips[s];
            let len = x.frame_end - x.frame_start;
            x.frame_start = clamp(as_f32(prop, &v)?, lp, ln - len);
            if x.r#type != "TRANSITION" {
                x.frame_end = x.frame_start + len;
            }
        }
        "frame_end_ui" => {
            // `rna_NlaStrip_frame_end_ui_set`: changes the action range or the repeats.
            let x = &mut track.strips[s];
            let old_end = x.frame_end;
            x.frame_end = clamp(as_f32(prop, &v)?, x.frame_start + NLASTRIP_MIN_LEN_THRESH, ln);
            if x.r#type == "CLIP" || x.r#type == "SOUND" {
                let actlen = clip_length(x);
                let delta = (old_end - x.frame_end) / x.scale;
                if is_eqf(x.repeat, 1.0) {
                    if is_eqf(delta, actlen) {
                        x.action_frame_end = x.action_frame_start + 1.0;
                        x.frame_end += 1.0;
                    } else if delta < actlen {
                        x.action_frame_end -= delta;
                    }
                } else {
                    x.repeat -= delta / actlen;
                }
            }
        }
        "scale" => {
            track.strips[s].scale = clamp(as_f32(prop, &v)?, 0.0001, 1000.0);
            recalculate_bounds(track, s);
        }
        "repeat" => {
            track.strips[s].repeat = clamp(as_f32(prop, &v)?, 0.01, 1000.0);
            recalculate_bounds(track, s);
        }
        "action_frame_start" => {
            let x = &mut track.strips[s];
            x.action_frame_start = clamp(as_f32(prop, &v)?, MINAFRAMEF, x.action_frame_end);
            recalculate_bounds(track, s);
        }
        "action_frame_end" => {
            let x = &mut track.strips[s];
            x.action_frame_end = clamp(as_f32(prop, &v)?, x.action_frame_start, MAXFRAMEF);
            recalculate_bounds(track, s);
        }
        "blend_in" => {
            let x = &mut track.strips[s];
            let len = (x.frame_end - x.frame_start) - x.blend_out;
            x.blend_in = clamp(as_f32(prop, &v)?, 0.0, len);
        }
        "blend_out" => {
            let x = &mut track.strips[s];
            let len = x.frame_end - x.frame_start;
            let mut value = clamp(as_f32(prop, &v)?, 0.0, len);
            if (len - value) < x.blend_in {
                value = len - x.blend_in;
            }
            x.blend_out = value;
        }
        "use_auto_blend" => {
            let x = &mut track.strips[s];
            x.use_auto_blend = as_bool(prop, &v)?;
            if !x.use_auto_blend {
                x.blend_in = 0.0;
                x.blend_out = 0.0;
            }
        }
        _ => {
            let x = &mut track.strips[s];
            match id {
                "name" => x.name = as_str(prop, &v)?.into(),
                "extrapolation" => x.extrapolation = as_str(prop, &v)?.into(),
                "blend_type" => x.blend_type = as_str(prop, &v)?.into(),
                "last_slot_identifier" => x.last_slot_identifier = as_str(prop, &v)?.into(),
                "influence" => x.influence = as_f32(prop, &v)?,
                "strip_time" => x.strip_time = as_f32(prop, &v)?,
                "use_animated_influence" => x.use_animated_influence = as_bool(prop, &v)?,
                "use_animated_time" => x.use_animated_time = as_bool(prop, &v)?,
                "use_animated_time_cyclic" => x.use_animated_time_cyclic = as_bool(prop, &v)?,
                "select" => x.select = as_bool(prop, &v)?,
                "mute" => x.mute = as_bool(prop, &v)?,
                "use_reverse" => x.use_reverse = as_bool(prop, &v)?,
                "use_sync_length" => x.use_sync_length = as_bool(prop, &v)?,
                _ if prop.is_readonly() => return Err(read_only(&frap_spec_rna::NLA_STRIP, id)),
                _ => return Err(unbound(&frap_spec_rna::NLA_STRIP, prop)),
            }
        }
    }
    done
}

fn coll_mut<'a>(
    adt: &'a mut AnimData,
    loc: AdtLoc,
    prop: &'static PropDef,
) -> Result<RnaCollMutGuard<'a>, RnaError> {
    if let Some(c) = coll_of(loc, prop.identifier) {
        return Ok(Box::new(AdtCollViewMut { adt, c }));
    }
    match prop.identifier {
        "drivers" | "fcurves" => Ok(Box::new(EmptyColl(&frap_spec_rna::F_CURVE))),
        "modifiers" => Ok(Box::new(EmptyColl(&frap_spec_rna::F_MODIFIER))),
        "strips" => Ok(Box::new(EmptyColl(&frap_spec_rna::NLA_STRIP))),
        id => Err(wrong_access(srna_of(loc), id, "a collection")),
    }
}

/// Every property identifier these bindings serve, per struct.
pub const BOUND: &[(&str, &[&str])] = &[
    (
        "AnimData",
        &[
            "nla_tracks", "action", "action_extrapolation", "action_blend_type",
            "action_influence", "action_tweak_storage", "action_slot_handle_tweak_storage",
            "drivers", "use_nla", "use_tweak_mode", "use_pin", "action_slot_handle",
            "last_slot_identifier", "action_slot", "action_suitable_slots",
        ],
    ),
    (
        "NlaTrack",
        &["strips", "is_override_data", "name", "active", "is_solo", "select", "mute", "lock"],
    ),
    (
        "NlaStrip",
        &[
            "name", "type", "extrapolation", "blend_type", "frame_start", "frame_end",
            "frame_start_raw", "frame_end_raw", "blend_in", "blend_out", "use_auto_blend",
            "action", "action_slot_handle", "last_slot_identifier", "action_slot",
            "action_suitable_slots", "action_frame_start", "action_frame_end", "repeat", "scale",
            "fcurves", "modifiers", "strips", "influence", "strip_time", "use_animated_influence",
            "use_animated_time", "use_animated_time_cyclic", "active", "select", "mute",
            "use_reverse", "use_sync_length",
        ],
    ),
];

/// The writable properties of each struct that go through frap-scene's set hooks (the ID
/// bindings do not reach `animation_data`, so every write below it is a hook).
pub const WRITABLE: &[(&str, &[&str])] = &[
    (
        "AnimData",
        &[
            "action", "action_extrapolation", "action_blend_type", "action_influence",
            "use_nla", "use_tweak_mode", "use_pin", "action_slot_handle", "last_slot_identifier",
            "action_slot",
        ],
    ),
    ("NlaTrack", &["name", "is_solo", "select", "mute", "lock"]),
    (
        "NlaStrip",
        &[
            "name", "extrapolation", "blend_type", "frame_start", "frame_end", "frame_start_raw",
            "frame_end_raw", "blend_in", "blend_out", "use_auto_blend", "action",
            "action_slot_handle", "last_slot_identifier", "action_slot", "action_frame_start",
            "action_frame_end", "repeat", "scale", "influence", "strip_time",
            "use_animated_influence", "use_animated_time", "use_animated_time_cyclic", "select",
            "mute", "use_reverse", "use_sync_length",
        ],
    ),
];
