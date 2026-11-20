// SPDX-License-Identifier: GPL-3.0-or-later
//! NLA evaluation: `animsys_calculate_nla` → `animsys_evaluate_nla_for_flush` (one strip per
//! enabled track at the current time, then the action track as a last strip), the snapshot
//! blending (`nlasnapshot_blend`: REPLACE / ADD / SUBTRACT / MULTIPLY, and COMBINE with the
//! per-property mix modes, quaternions combined as rotations), `animsys_evaluate_nla_domain`
//! (channels touched by any enabled track but no current strip are reset to their defaults)
//! and `nladata_flush_channels`.
//!
//! Ported from blender/source/blender/blenkernel/intern/anim_sys.cc and nla.cc (Blender 5.2.4,
//! GPL-2.0-or-later). frap's NLA has no strip F-curves, F-modifiers, transition or meta strips
//! (P2): a transition or meta strip contributes nothing. NLA tweak mode (`use_tweak_mode`) is
//! evaluated as if it were off.

use frap_rna::{ActionId, IdRef, StructDef};
use smol_str::SmolStr;

use super::curve::is_eqf;
use super::props::{ChannelProp, MixMode, channel_prop};
use super::{ActionLookup, AnimChannel};
use crate::action::Action;
use crate::anim_data::{AnimData, NlaStrip, NlaTrack};
use crate::fcurve::FCurve;

/// `MINAFRAMEF`
const MINAFRAMEF: f32 = -1_048_574.0;
/// `MAXFRAMEF`
const MAXFRAMEF: f32 = 1_048_574.0;

/// `eNlaStrip_Blend_Mode`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Blend {
    Replace,
    Add,
    Subtract,
    Multiply,
    Combine,
}

impl Blend {
    fn from_identifier(s: &str) -> Blend {
        match s {
            "ADD" => Blend::Add,
            "SUBTRACT" => Blend::Subtract,
            "MULTIPLY" => Blend::Multiply,
            "COMBINE" => Blend::Combine,
            _ => Blend::Replace,
        }
    }
}

/// `eNlaStrip_Extrapolate_Mode`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Extend {
    Hold,
    HoldForward,
    Nothing,
}

impl Extend {
    fn from_identifier(s: &str) -> Extend {
        match s {
            "HOLD_FORWARD" => Extend::HoldForward,
            "NOTHING" => Extend::Nothing,
            _ => Extend::Hold,
        }
    }
}

/// The evaluation copy of an `NlaStrip` (its `NLASTRIP_FLAG_*` spelled out).
#[derive(Clone, Debug)]
struct Strip {
    clip: bool,
    action: Option<IdRef<ActionId>>,
    slot: i32,
    start: f32,
    end: f32,
    actstart: f32,
    actend: f32,
    scale: f32,
    repeat: f32,
    blend: Blend,
    extend: Extend,
    blendin: f32,
    blendout: f32,
    influence: f32,
    strip_time: f32,
    user_influence: bool,
    user_time: bool,
    no_time_map: bool,
    muted: bool,
    reverse: bool,
}

impl Strip {
    fn of(s: &NlaStrip) -> Strip {
        Strip {
            clip: s.r#type == "CLIP",
            action: s.action,
            slot: s.action_slot_handle,
            start: s.frame_start,
            end: s.frame_end,
            actstart: s.action_frame_start,
            actend: s.action_frame_end,
            scale: s.scale,
            repeat: s.repeat,
            blend: Blend::from_identifier(&s.blend_type),
            extend: Extend::from_identifier(&s.extrapolation),
            blendin: s.blend_in,
            blendout: s.blend_out,
            influence: s.influence,
            strip_time: s.strip_time,
            user_influence: s.use_animated_influence,
            user_time: s.use_animated_time,
            no_time_map: false,
            muted: s.mute,
            reverse: s.use_reverse,
        }
    }

    /// `nlastrip_get_influence`
    fn influence_at(&mut self, cframe: f32) -> f32 {
        self.blendin = self.blendin.abs();
        self.blendout = self.blendout.abs();
        if !is_eqf(self.blendin, 0.0) && cframe <= self.start + self.blendin {
            return (cframe - self.start).abs() / self.blendin;
        }
        if !is_eqf(self.blendout, 0.0) && cframe >= self.end - self.blendout {
            return (self.end - cframe).abs() / self.blendout;
        }
        1.0
    }

    /// `nlastrip_get_frame_actionclip(strip, cframe, NLATIME_CONVERT_EVAL)`
    fn frame_eval(&mut self, cframe: f32) -> f32 {
        if is_eqf(self.repeat, 0.0) {
            self.repeat = 1.0;
        }
        if is_eqf(self.scale, 0.0) {
            self.scale = 1.0;
        }
        let scale = self.scale.abs();
        let actlength = if self.actend <= self.actstart {
            1.0
        } else {
            self.actend - self.actstart
        };
        let whole_repeats = is_eqf(self.repeat, self.repeat.floor());
        if self.reverse {
            if is_eqf(cframe, self.end) && whole_repeats {
                return self.actstart;
            }
            return self.actend - ((cframe - self.start) % (actlength * scale)) / scale;
        }
        if is_eqf(cframe, self.end) && whole_repeats {
            return self.actend;
        }
        self.actstart + ((cframe - self.start) % (actlength * scale)) / scale
    }

    /// `nlastrip_evaluate_controls` (no strip F-curves).
    fn evaluate_controls(&mut self, time: f32) {
        if !self.user_influence {
            self.influence = self.influence_at(time);
        }
        if self.no_time_map {
            self.strip_time = time;
            return;
        }
        if !self.user_time {
            self.strip_time = self.frame_eval(time);
        }
    }
}

/// `nlastrips_ctime_get_strip`: the strip of a track that acts at `ctime`, its controls
/// evaluated (at the clamped time when holding).
fn ctime_get_strip(strips: &[Strip], ctime: f32) -> Option<Strip> {
    let mut estrip: Option<usize> = None;
    #[derive(PartialEq)]
    enum Side {
        Within,
        Before,
        After,
    }
    let mut side = Side::Within;
    let n = strips.len();
    for (i, strip) in strips.iter().enumerate() {
        let mut in_range = strip.start <= ctime && ctime <= strip.end;
        if strip.start > strip.end {
            in_range = strip.end <= ctime && ctime <= strip.start;
        }
        if strip.no_time_map {
            match strip.extend {
                Extend::Hold => in_range = true,
                Extend::HoldForward => in_range = ctime >= strip.start,
                Extend::Nothing => {}
            }
        }
        if in_range {
            estrip = Some(i);
            side = Side::Within;
            break;
        }
        if ctime < strip.start {
            if i == 0 {
                // Before the first strip: only when it extends backwards in time too.
                if strip.extend == Extend::Hold {
                    estrip = Some(i);
                }
                side = Side::Before;
            } else {
                // Between strips: the previous one holds unless it extends NOTHING.
                if strips[i - 1].extend != Extend::Nothing {
                    estrip = Some(i - 1);
                }
                side = Side::After;
            }
            break;
        }
        if ctime > strip.end && i + 1 == n {
            if strip.extend != Extend::Nothing {
                estrip = Some(i);
            }
            side = Side::After;
            break;
        }
    }
    let mut s = strips[estrip?].clone();
    if s.muted {
        return None;
    }
    let time = match side {
        Side::Before => s.start,
        Side::After => s.end,
        Side::Within => ctime,
    };
    s.evaluate_controls(time);
    if s.influence <= 0.0 {
        return None;
    }
    if !s.clip || s.action.is_none() {
        // Transition and meta strips are not supported (P2).
        return None;
    }
    Some(s)
}

/// `BKE_nlatrack_is_enabled`
fn track_enabled(solo: bool, t: &NlaTrack) -> bool {
    if solo { t.is_solo } else { !t.mute }
}

/// `is_fcurve_evaluatable` of `anim_sys.cc` (group mute included).
fn evaluatable(fc: &FCurve, group_muted: bool) -> bool {
    !fc.mute && !group_muted && !fc.is_empty() && !fc.data_path.is_empty()
}

/// Every evaluatable F-curve of `slot` in `action` (`fcurves_for_action_slot`).
fn slot_fcurves(action: &Action, slot: i32) -> impl Iterator<Item = &FCurve> {
    action
        .channelbag(slot)
        .into_iter()
        .flat_map(|bag| {
            bag.fcurves.iter().filter(move |fc| {
                let group_muted = fc
                    .group
                    .as_ref()
                    .and_then(|g| bag.groups.iter().find(|x| &x.name == g))
                    .is_some_and(|g| g.mute);
                evaluatable(fc, group_muted)
            })
        })
}

/// One NLA channel (`NlaEvalChannel`): a property, or one array index of a property the RNA
/// definitions cannot resolve.
struct Channel {
    path: SmolStr,
    /// `Some(index)` for an unresolved path (one scalar channel per index).
    fallback_index: Option<i32>,
    prop: ChannelProp,
    /// `NlaEvalChannel::domain`: the components the NLA writes.
    domain: Vec<bool>,
}

/// `NlaEvalChannelSnapshot`
#[derive(Clone)]
struct ChanSnap {
    values: Vec<f32>,
    blend_domain: Vec<bool>,
}

/// `NlaEvalData` plus the snapshots.
struct Nla {
    root: Option<&'static StructDef>,
    channels: Vec<Channel>,
}

/// `NlaEvalSnapshot` (indexed by channel; `None` falls back to the channel defaults).
type Snapshot = Vec<Option<ChanSnap>>;

impl Nla {
    /// `nlaevalchan_verify` + `nlaevalchan_validate_index`: the channel and component an
    /// F-curve writes, `None` when it is skipped.
    fn verify(&mut self, path: &str, array_index: i32) -> Option<(usize, usize)> {
        match channel_prop(self.root, path) {
            Some(prop) => {
                if !prop.animatable {
                    return None;
                }
                let comp = if prop.is_array {
                    usize::try_from(array_index)
                        .ok()
                        .filter(|i| *i < prop.length)?
                } else {
                    0
                };
                let i = match self
                    .channels
                    .iter()
                    .position(|c| c.fallback_index.is_none() && c.path == path)
                {
                    Some(i) => i,
                    None => self.push(path, None, prop),
                };
                Some((i, comp))
            }
            None => {
                let i = match self
                    .channels
                    .iter()
                    .position(|c| c.fallback_index == Some(array_index) && c.path == path)
                {
                    Some(i) => i,
                    None => self.push(
                        path,
                        Some(array_index),
                        ChannelProp {
                            is_array: false,
                            length: 1,
                            mix_mode: MixMode::Add,
                            defaults: vec![0.0],
                            animatable: true,
                        },
                    ),
                };
                Some((i, 0))
            }
        }
    }

    fn push(&mut self, path: &str, fallback_index: Option<i32>, prop: ChannelProp) -> usize {
        let n = prop.length;
        self.channels.push(Channel {
            path: path.into(),
            fallback_index,
            prop,
            domain: vec![false; n],
        });
        self.channels.len() - 1
    }

    /// The defaults snapshot of channel `i`.
    fn base(&self, i: usize) -> ChanSnap {
        let c = &self.channels[i];
        ChanSnap {
            values: c.prop.defaults.clone(),
            blend_domain: vec![false; c.prop.length],
        }
    }

    /// `nlasnapshot_from_action`
    fn snapshot_from_action(&mut self, action: &Action, slot: i32, time: f32) -> Snapshot {
        let mut snap: Snapshot = Vec::new();
        for fc in slot_fcurves(action, slot) {
            let Some((ci, comp)) = self.verify(&fc.data_path, fc.array_index) else {
                continue;
            };
            if snap.len() <= ci {
                snap.resize(ci + 1, None);
            }
            let quat = self.channels[ci].prop.mix_mode == MixMode::Quaternion;
            let base = self.base(ci);
            let s = snap[ci].get_or_insert(base);
            s.values[comp] = super::curve::evaluate(fc, time);
            if quat {
                s.blend_domain.iter_mut().for_each(|b| *b = true);
            } else {
                s.blend_domain[comp] = true;
            }
        }
        snap
    }

    /// `nlasnapshot_blend(eval_data, lower, upper, blendmode, influence, lower)`
    fn blend(&self, lower: &mut Snapshot, upper: &Snapshot, mode: Blend, influence: f32) {
        if lower.len() < self.channels.len() {
            lower.resize(self.channels.len(), None);
        }
        for i in 0..self.channels.len() {
            let up = upper.get(i).and_then(Option::as_ref);
            if up.is_none() && lower[i].is_none() {
                continue;
            }
            let low = lower[i].clone().unwrap_or_else(|| self.base(i));
            let out = self.blend_or_combine(i, &low, up, mode, influence);
            lower[i] = Some(out);
        }
    }

    /// `nlaevalchan_blendOrcombine`
    fn blend_or_combine(
        &self,
        ch: usize,
        lower: &ChanSnap,
        upper: Option<&ChanSnap>,
        mode: Blend,
        influence: f32,
    ) -> ChanSnap {
        let mut out = lower.clone();
        // `nlaevalchan_blendOrcombine_try_copy_from_lower`
        let Some(upper) = upper.filter(|_| !is_eqf(influence, 0.0)) else {
            return out;
        };
        let channel = &self.channels[ch];
        if mode == Blend::Combine && channel.prop.mix_mode == MixMode::Quaternion {
            // All or nothing for quaternions.
            if !upper.blend_domain[0] {
                return out;
            }
            out.values = combine_quaternion(&lower.values, &upper.values, influence);
            return out;
        }
        for j in 0..lower.values.len() {
            if !upper.blend_domain[j] {
                continue;
            }
            out.values[j] = if mode == Blend::Combine {
                combine_value(
                    channel.prop.mix_mode,
                    channel.prop.defaults[j],
                    lower.values[j],
                    upper.values[j],
                    influence,
                )
            } else {
                blend_value(mode, lower.values[j], upper.values[j], influence)
            };
        }
        out
    }

    /// `nla_eval_domain_action`
    fn domain_action(&mut self, action: &Action, slot: i32) {
        for fc in slot_fcurves(action, slot) {
            let Some((ci, comp)) = self.verify(&fc.data_path, fc.array_index) else {
                continue;
            };
            let c = &mut self.channels[ci];
            if c.prop.mix_mode == MixMode::Quaternion {
                c.domain.iter_mut().for_each(|b| *b = true);
            } else {
                c.domain[comp] = true;
            }
        }
    }
}

/// `nla_blend_value`
fn blend_value(mode: Blend, lower: f32, strip: f32, influence: f32) -> f32 {
    if is_eqf(influence, 0.0) {
        return lower;
    }
    match mode {
        Blend::Add => lower + (strip * influence),
        Blend::Subtract => lower - (strip * influence),
        Blend::Multiply => influence * (lower * strip) + (1.0 - influence) * lower,
        Blend::Replace | Blend::Combine => lower * (1.0 - influence) + (strip * influence),
    }
}

/// `nla_combine_value`
fn combine_value(mix: MixMode, mut base: f32, lower: f32, strip: f32, influence: f32) -> f32 {
    if is_eqf(influence, 0.0) {
        return lower;
    }
    match mix {
        MixMode::Add | MixMode::AxisAngle => lower + (strip - base) * influence,
        MixMode::Multiply => {
            if is_eqf(base, 0.0) {
                base = 1.0;
            }
            lower * libm::powf(strip / base, influence)
        }
        MixMode::Quaternion => lower,
    }
}

/// `normalize_qt_qt`
fn normalize_qt(q: &[f32]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len != 0.0 {
        let f = 1.0 / len;
        [q[0] * f, q[1] * f, q[2] * f, q[3] * f]
    } else {
        [0.0, 1.0, 0.0, 0.0]
    }
}

/// `nla_combine_quaternion`: `lower * strip^influence` (both normalized).
fn combine_quaternion(lower: &[f32], strip: &[f32], influence: f32) -> Vec<f32> {
    let a = normalize_qt(lower);
    let mut b = normalize_qt(strip);
    // `pow_qt_fl_normalized`
    let angle = influence * libm::acosf(b[0].clamp(-1.0, 1.0));
    let co = libm::cosf(angle);
    let si = libm::sinf(angle);
    b[0] = co;
    let d = b[1] * b[1] + b[2] * b[2] + b[3] * b[3];
    if d > 1.0e-35 {
        let f = si / d.sqrt();
        b[1] *= f;
        b[2] *= f;
        b[3] *= f;
    } else {
        b[1] = 0.0;
        b[2] = 0.0;
        b[3] = 0.0;
    }
    // `mul_qt_qtqt`
    vec![
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] + a[2] * b[0] + a[3] * b[1] - a[1] * b[3],
        a[0] * b[3] + a[3] * b[0] + a[1] * b[2] - a[2] * b[1],
    ]
}

/// The time range of every key of `action` (`Action::get_frame_range_of_keys`).
pub(crate) fn frame_range_of_keys(action: &Action) -> [f32; 2] {
    let (mut min, mut max) = (999_999_999.0_f32, -999_999_999.0_f32);
    let mut found = false;
    for layer in &action.layers {
        for strip in &layer.strips {
            let crate::action::ActionStrip::Keyframe(k) = strip;
            for bag in &k.channelbags {
                for fc in &bag.fcurves {
                    if let (Some(f), Some(l)) = (fc.keyframe_points.first(), fc.keyframe_points.last()) {
                        min = min.min(f.co[0]);
                        max = max.max(l.co[0]);
                        found = true;
                    }
                }
            }
        }
    }
    if found {
        [min.max(MINAFRAMEF), max.min(MAXFRAMEF)]
    } else {
        [0.0, 0.0]
    }
}

/// `animsys_calculate_nla`: `None` when the NLA evaluated nothing (the caller then evaluates
/// the active action on its own, as Blender does).
pub(super) fn calculate(
    adt: &AnimData,
    actions: &dyn ActionLookup,
    frame: f32,
    root: Option<&'static StructDef>,
) -> Option<Vec<AnimChannel>> {
    let solo = adt.nla_tracks.iter().any(|t| t.is_solo);
    let mut nla = Nla {
        root,
        channels: Vec::new(),
    };
    // `animsys_evaluate_nla_for_flush`
    let mut estrips: Vec<Strip> = Vec::new();
    let mut has_strips = false;
    for track in &adt.nla_tracks {
        if !track_enabled(solo, track) {
            continue;
        }
        if !track.strips.is_empty() {
            has_strips = true;
        }
        let strips: Vec<Strip> = track.strips.iter().map(Strip::of).collect();
        if strips.is_empty() {
            continue;
        }
        if let Some(s) = ctime_get_strip(&strips, frame) {
            estrips.push(s);
        }
    }
    // `is_action_track_evaluated_without_nla`
    if adt.action.is_some() && !has_strips && !solo {
        return None;
    }
    // `animsys_create_action_track_strip`
    let act = adt.action.and_then(|a| actions.action(a));
    let range = act.map_or([0.0, 0.0], frame_range_of_keys);
    let mut action_strip = Strip {
        clip: true,
        action: adt.action,
        slot: adt.action_slot_handle,
        start: range[0],
        end: if range[1] <= range[0] {
            range[0] + 1.0
        } else {
            range[1]
        },
        actstart: range[0],
        actend: if range[1] <= range[0] {
            range[0] + 1.0
        } else {
            range[1]
        },
        scale: 1.0,
        repeat: 1.0,
        blend: Blend::from_identifier(&adt.action_blend_type),
        extend: Extend::from_identifier(&adt.action_extrapolation),
        blendin: 0.0,
        blendout: 0.0,
        influence: adt.action_influence,
        strip_time: 0.0,
        user_influence: true,
        user_time: false,
        no_time_map: true,
        muted: false,
        reverse: false,
    };
    action_strip.muted = !(adt.action.is_some() && !solo);
    if let Some(s) = ctime_get_strip(std::slice::from_ref(&action_strip), frame) {
        estrips.push(s);
    }
    // Per strip, evaluate and accumulate on top of the existing channels.
    let mut eval: Snapshot = Vec::new();
    for s in &estrips {
        let Some(action) = s.action.and_then(|a| actions.action(a)) else {
            continue;
        };
        let strip_snap = nla.snapshot_from_action(action, s.slot, s.strip_time);
        nla.blend(&mut eval, &strip_snap, s.blend, s.influence);
    }
    // `animsys_evaluate_nla_domain`: the action track (not tweaking) and every enabled track.
    let mut touched: Vec<(IdRef<ActionId>, i32)> = Vec::new();
    let mut domain = |nla: &mut Nla, a: IdRef<ActionId>, slot: i32| {
        if touched.contains(&(a, slot)) {
            return;
        }
        touched.push((a, slot));
        if let Some(action) = actions.action(a) {
            nla.domain_action(action, slot);
        }
    };
    if let Some(a) = adt.action {
        domain(&mut nla, a, adt.action_slot_handle);
    }
    for track in &adt.nla_tracks {
        if !track_enabled(solo, track) {
            continue;
        }
        for s in &track.strips {
            if let Some(a) = s.action {
                domain(&mut nla, a, s.action_slot_handle);
            }
        }
    }
    // `nladata_flush_channels`
    let mut out = Vec::new();
    for (i, c) in nla.channels.iter().enumerate() {
        let snap = eval
            .get(i)
            .and_then(Option::as_ref)
            .cloned()
            .unwrap_or_else(|| nla.base(i));
        for (j, on) in c.domain.iter().enumerate() {
            if !*on {
                continue;
            }
            let array_index = match c.fallback_index {
                Some(ix) => ix,
                None if c.prop.is_array => j as i32,
                None => 0,
            };
            out.push(AnimChannel {
                data_path: c.path.clone(),
                array_index,
                value: snap.values[j],
            });
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_modes() {
        assert_eq!(blend_value(Blend::Replace, 1.0, 3.0, 0.5), 2.0);
        assert_eq!(blend_value(Blend::Add, 1.0, 3.0, 0.5), 2.5);
        assert_eq!(blend_value(Blend::Subtract, 1.0, 3.0, 0.5), -0.5);
        assert_eq!(blend_value(Blend::Multiply, 2.0, 3.0, 0.5), 4.0);
        assert_eq!(combine_value(MixMode::Add, 1.0, 2.0, 3.0, 0.5), 3.0);
        assert_eq!(combine_value(MixMode::Multiply, 1.0, 2.0, 4.0, 0.5), 4.0);
        let q = combine_quaternion(&[1.0, 0.0, 0.0, 0.0], &[0.0, 0.0, 0.0, 1.0], 0.5);
        let h = core::f32::consts::FRAC_1_SQRT_2;
        assert!((q[0] - h).abs() < 1e-6 && (q[3] - h).abs() < 1e-6);
    }

    #[test]
    fn strip_time_mapping() {
        let mut s = Strip::of(&NlaStrip::new_clip("S", None, 10.0, [1.0, 11.0]));
        assert_eq!(s.frame_eval(10.0), 1.0);
        assert_eq!(s.frame_eval(15.0), 6.0);
        assert_eq!(s.frame_eval(20.0), 11.0);
        s.reverse = true;
        assert_eq!(s.frame_eval(15.0), 6.0);
        assert_eq!(s.frame_eval(12.0), 9.0);
        s.reverse = false;
        s.scale = 2.0;
        assert_eq!(s.frame_eval(14.0), 3.0);
        s.blendin = 2.0;
        s.blendout = 4.0;
        assert_eq!(s.influence_at(11.0), 0.5);
        assert_eq!(s.influence_at(19.0), 0.25);
    }
}
