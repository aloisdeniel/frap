// SPDX-License-Identifier: GPL-3.0-or-later
//! Baking the registered assets into `pointer` channels, for the two animation modes that
//! export them, and writing the channels into the document.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! - `blender/exp/animation/scene_animation.py` (`gather_scene_animations`, `SCENE` mode): the
//!   scene's frame range at `export_frame_step`, materials, then lights, then cameras, all in
//!   the scene's animation, or with `export_anim_scene_split_object` one animation per light
//!   and per camera named after it, and **one** for every material together, named after the
//!   last one (the add-on builds it after its material loop);
//! - `blender/exp/animation/tracks.py` (`gather_data_track_animations`,
//!   `__get_data_blender_tracks`, `prepare_tracks_range`, `NLA_TRACKS` mode): materials, then
//!   cameras, then lights; per datablock its own NLA tracks then (materials) its node tree's,
//!   consecutive non-`REPLACE` tracks grouped with the one below; each group baked over its
//!   strips' range with every other track muted (the datablock's active action removed and
//!   its NLA forced on, the node tree's active action kept), then `merge_tracks_perform`: the
//!   animations of a track name that is not a default one (`NlaTrack…`, `[Action Stash]…`)
//!   are merged into the first one and take the track's name;
//! - `blender/exp/animation/sampled/data/{channels,sampler,keyframes}.py`: one channel per
//!   registered path (the base colour and its alpha once), constant channels dropped
//!   (`fcurve_is_constant`; under `frap_compat='SPEC'`, Blender's clock `(frame − slide) ×
//!   fps_base / fps`, WP 8.12), times `frame / (fps · fps_base)` (or `(frame − slide) / fps` when
//!   slid), input min/max, `export_sampling_interpolation_fallback`;
//! - `blender/exp/exporter.py` (`__traverse_property`): the target becomes
//!   `{path: "pointer", extensions: {KHR_animation_pointer: {pointer}}}`, a material whose
//!   alpha is animated gets `alphaMode: BLEND`.

use frap_anim::AnimData;
use frap_gltf::accessor::{AccessorData, Components};
use frap_gltf::builder::AccessorOpts;
use frap_gltf::ext::khr_animation_pointer::KhrAnimationPointer;
use frap_gltf::json::{
    AccessorType, AlphaMode, Animation, AnimationSampler, Channel, Extras, Index, Interpolation,
};
use frap_spec_ops_io::export_scene::gltf::{
    ExportImportConvertLightingMode, ExportNegativeFrame, ExportSamplingInterpolationFallback,
};

use super::registry::{Asset, DataKind, Registry};
use super::sample::{
    AssetAnim, FrameData, channel_is_constant, frame_data, is_constant, key_value,
};
use crate::export::gather::Gather;

const BASE_COLOR: &str = "/materials/XXX/pbrMetallicRoughness/baseColorFactor";

/// One baked channel.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerChannel {
    /// The resolved JSON pointer.
    pub pointer: String,
    /// Key times in seconds (double precision; written as floats).
    pub times: Vec<f64>,
    /// Key values (one element per key).
    pub values: Vec<Vec<f64>>,
    /// The glTF material whose base-colour alpha this channel animates (`tmp_alpha_cst is
    /// False`: the material becomes `BLEND`).
    pub blend_material: Option<Index>,
    /// A copy for an `additional` pointer: `deepcopy` of the sampler's accessors, so it gets
    /// its own input and output accessors (never shared by content).
    pub copy: bool,
}

/// One baked animation, before it is merged into the document.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerAnimation {
    /// `animation.name`
    pub name: String,
    /// `animation.extras`
    pub extras: Option<Extras>,
    /// Channels, in gather order.
    pub channels: Vec<PointerChannel>,
    /// `NLA_TRACKS`: the track name it merges under (`None` for a default track name).
    pub merge: Option<String>,
}

/// The frames and slide of one bake.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Range {
    /// First frame.
    pub start: i32,
    /// Last frame (included).
    pub end: i32,
    /// `export_frame_step`
    pub step: i32,
    /// `export_settings['slide']` for this asset and animation.
    pub slide: Option<i32>,
}

impl Range {
    fn frames(&self) -> Vec<i32> {
        let mut out = Vec::new();
        let mut f = self.start;
        while f <= self.end {
            out.push(f);
            f += self.step.max(1);
        }
        out
    }
}

/// What the bake reads besides the registry.
pub struct BakeCx<'g, 'a> {
    /// The export state.
    pub g: &'g Gather<'a>,
    /// `fps · fps_base` and `fps` of the context scene.
    pub fps: (f64, f64),
    /// `export_import_convert_lighting_mode == 'SPEC'`
    pub spec_lighting: bool,
}

impl BakeCx<'_, '_> {
    fn seconds(&self, frame: i32, slide: Option<i32>) -> f64 {
        // `frap_compat='SPEC'` (WP 8.12): Blender's clock, `(frame − slide) × fps_base / fps`,
        // the time base `spec_complete::compat::animation_times` gives the other samplers.
        if crate::export::spec_complete::spec(self.g) {
            let base = f64::from(self.g.ctx.scene().render.fps_base);
            return f64::from(frame - slide.unwrap_or(0)) * base / self.fps.1;
        }
        match slide {
            Some(s) => f64::from(frame - s) / self.fps.1,
            None => f64::from(frame) / self.fps.0,
        }
    }
}

/// `gather_data_sampled_channels` for one asset over `range` with animation `anim`.
pub fn asset_channels(
    cx: &BakeCx,
    kind: DataKind,
    asset: &Asset,
    anim: &AssetAnim,
    range: Range,
) -> Vec<PointerChannel> {
    let frames = range.frames();
    if frames.is_empty() {
        return Vec::new();
    }
    let render = &cx.g.ctx.scene().render;
    let data: Vec<FrameData> = frames
        .iter()
        .map(|&f| frame_data(cx.g.main, render, kind, asset, anim, f))
        .collect();
    let mut out = Vec::new();
    let mut base_color_done = false;
    for (key, info) in &asset.paths {
        if info.pointer == BASE_COLOR && base_color_done {
            continue;
        }
        if info.pointer == BASE_COLOR {
            base_color_done = true;
        }
        let values: Option<Vec<Vec<f64>>> = data
            .iter()
            .map(|d| key_value(kind, asset, key, d, cx.spec_lighting))
            .collect();
        let Some(values) = values else { continue };
        if is_constant(&values) {
            continue;
        }
        let blend_material = (kind == DataKind::Materials
            && info.pointer == BASE_COLOR
            && !channel_is_constant(&values, 3))
        .then_some(asset.index);
        let times: Vec<f64> = frames.iter().map(|&f| cx.seconds(f, range.slide)).collect();
        // `Manage multiple Texture Transform for the same path`: one copy per additional
        // pointer, right after the channel.
        let copies: Vec<PointerChannel> = info
            .resolved_additional(asset.index)
            .into_iter()
            .map(|pointer| PointerChannel {
                pointer,
                times: times.clone(),
                values: values.clone(),
                blend_material: None,
                copy: true,
            })
            .collect();
        out.push(PointerChannel {
            pointer: info.resolved(asset.index),
            times,
            values,
            blend_material,
            copy: false,
        });
        out.extend(copies);
    }
    out
}

/// The `extras` of a datablock's animation (`__gather_extras` with `export_extras`).
fn data_extras(g: &mut Gather, asset: &Asset) -> Option<Extras> {
    let header = g.main.header(asset.id)?.clone();
    crate::export::extras::id(g, &header)
}

/// The name of a datablock.
fn data_name(g: &Gather, asset: &Asset) -> String {
    g.main
        .header(asset.id)
        .map(|h| h.name.to_string())
        .unwrap_or_default()
}

/// `gather_scene_animations`' pointer part (`SCENE` mode).
pub fn scene(
    g: &mut Gather,
    reg: &Registry,
    cx_fps: (f64, f64),
    spec: bool,
) -> Vec<PointerAnimation> {
    let s = g.ctx.scene();
    let (start, end) = (s.frame_start, s.frame_end);
    let p = g.settings.props.clone();
    let split = p.export_anim_scene_split_object.value;
    let range = Range {
        start,
        end,
        step: p.export_frame_step.value,
        slide: (p.export_anim_slide_to_zero.value && start > 0).then_some(start),
    };
    let mut out = Vec::new();
    let mut total: Vec<PointerChannel> = Vec::new();
    let mut last_material = None;
    for kind in [DataKind::Materials, DataKind::Lights, DataKind::Cameras] {
        for asset in reg.of(kind) {
            if asset.paths.is_empty() {
                continue;
            }
            let anim = AssetAnim::of(g.main, kind, asset);
            let cx = BakeCx {
                g,
                fps: cx_fps,
                spec_lighting: spec,
            };
            total.extend(asset_channels(&cx, kind, asset, &anim, range));
            if kind == DataKind::Materials {
                last_material = Some(asset);
            } else if split && !total.is_empty() {
                out.push(PointerAnimation {
                    name: data_name(g, asset),
                    extras: data_extras(g, asset),
                    channels: std::mem::take(&mut total),
                    merge: None,
                });
            }
        }
        if kind == DataKind::Materials
            && split
            && !total.is_empty()
            && let Some(asset) = last_material
        {
            out.push(PointerAnimation {
                name: data_name(g, asset),
                extras: data_extras(g, asset),
                channels: std::mem::take(&mut total),
                merge: None,
            });
        }
    }
    if !split && !total.is_empty() {
        let scene = g.ctx.scene();
        let extras = crate::export::extras::scene(g, scene);
        out.push(PointerAnimation {
            name: scene.header.name.to_string(),
            extras,
            channels: total,
            // The scene's animation: object channels (WP 6.7) come first in it.
            merge: Some(scene.header.name.to_string()),
        });
    }
    out
}

/// One group of NLA tracks baked together (`TrackData`).
#[derive(Clone, Debug, PartialEq)]
pub struct TrackGroup {
    /// The name of its first track.
    pub name: String,
    /// The tracks (indices in the animation data's `nla_tracks`).
    pub tracks: Vec<usize>,
    /// Tracks of the node tree (`NODETREE`) rather than of the datablock.
    pub tree: bool,
}

/// `__get_nla_tracks_data` / `__get_nla_tracks_material_node_tree`: tracks with strips, a
/// track blending (any strip not `REPLACE`) joining the group below it.
pub fn track_groups(adt: Option<&AnimData>, tree: bool) -> Vec<TrackGroup> {
    let Some(adt) = adt else { return Vec::new() };
    let mut out = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (i, t) in adt.nla_tracks.iter().enumerate() {
        if t.strips.is_empty() {
            continue;
        }
        let blends = t.strips.iter().any(|s| s.blend_type != "REPLACE");
        if !blends && !current.is_empty() {
            out.push(TrackGroup {
                name: adt.nla_tracks[current[0]].name.to_string(),
                tracks: std::mem::take(&mut current),
                tree,
            });
        }
        current.push(i);
    }
    if !current.is_empty() {
        out.push(TrackGroup {
            name: adt.nla_tracks[current[0]].name.to_string(),
            tracks: current,
            tree,
        });
    }
    out
}

/// Whether a track name is one of the defaults the merge ignores.
pub fn is_default_track_name(name: &str) -> bool {
    name.starts_with("NlaTrack") || name.starts_with("[Action Stash]")
}

/// Mutes every track with strips of `adt` except `keep`.
fn mute_others(adt: &mut AnimData, keep: &[usize]) {
    for (i, t) in adt.nla_tracks.iter_mut().enumerate() {
        if !t.strips.is_empty() {
            t.mute = !keep.contains(&i);
        }
    }
}

/// The animation data a track group is baked with (see the module documentation).
pub fn track_anim(stored: &AssetAnim, group: &TrackGroup) -> AssetAnim {
    let mut a = stored.clone();
    if let Some(adt) = a.id.as_mut() {
        adt.action = None;
        adt.action_slot_handle = 0;
        adt.use_nla = true;
        if let Some(t) = adt.nla_tracks.iter_mut().find(|t| t.is_solo) {
            t.is_solo = false;
        }
        mute_others(adt, if group.tree { &[] } else { &group.tracks });
    }
    if let Some(tree) = a.tree.as_mut() {
        mute_others(tree, if group.tree { &group.tracks } else { &[] });
    }
    a
}

/// `prepare_tracks_range`: the strips' frame range (cropped or clamped by the settings) and
/// the slide.
pub fn track_range(g: &Gather, adt: &AnimData, group: &TrackGroup) -> Range {
    let p = &g.settings.props;
    let mut start = f32::INFINITY;
    let mut end = f32::NEG_INFINITY;
    for &t in &group.tracks {
        let strips = &adt.nla_tracks[t].strips;
        if let (Some(f), Some(l)) = (strips.first(), strips.last()) {
            start = start.min(f.frame_start);
            end = end.max(l.frame_end);
        }
    }
    let negative = p.export_negative_frame.value;
    if start < 0.0 && negative == ExportNegativeFrame::Crop {
        start = 0.0;
    }
    if p.export_frame_range.value {
        let s = g.ctx.scene();
        start = start.max(s.frame_start as f32);
        end = end.min(s.frame_end as f32);
    }
    // `int(frame)` truncates toward zero.
    let (start, end) = (start as i32, end as i32);
    let slide = ((negative == ExportNegativeFrame::Slide && start < 0)
        || (p.export_anim_slide_to_zero.value && start > 0))
        .then_some(start);
    Range {
        start,
        end,
        step: p.export_frame_step.value,
        slide,
    }
}

/// `gather_tracks_animations`' pointer part (`NLA_TRACKS` mode).
pub fn tracks(
    g: &mut Gather,
    reg: &Registry,
    fps: (f64, f64),
    spec: bool,
) -> Vec<PointerAnimation> {
    // `bake_data_animation`: nothing without any action in the file.
    if g.main.actions.iter().next().is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for kind in [DataKind::Materials, DataKind::Cameras, DataKind::Lights] {
        for asset in reg.of(kind) {
            let stored = AssetAnim::of(g.main, kind, asset);
            let mut groups = track_groups(stored.id.as_ref(), false);
            if kind == DataKind::Materials {
                groups.extend(track_groups(stored.tree.as_ref(), true));
            }
            for group in groups {
                let adt = if group.tree {
                    stored.tree.as_ref()
                } else {
                    stored.id.as_ref()
                };
                let Some(adt) = adt else { continue };
                let range = track_range(g, adt, &group);
                let anim = track_anim(&stored, &group);
                let cx = BakeCx {
                    g,
                    fps,
                    spec_lighting: spec,
                };
                let channels = asset_channels(&cx, kind, asset, &anim, range);
                if channels.is_empty() {
                    continue;
                }
                out.push(PointerAnimation {
                    name: data_name(g, asset),
                    extras: None,
                    channels,
                    merge: (!is_default_track_name(&group.name)).then(|| group.name.clone()),
                });
            }
        }
    }
    out
}

/// The interpolation of baked samplers (`export_sampling_interpolation_fallback`).
fn interpolation(g: &Gather) -> Interpolation {
    match g
        .settings
        .props
        .export_sampling_interpolation_fallback
        .value
    {
        ExportSamplingInterpolationFallback::Step => Interpolation::Step,
        ExportSamplingInterpolationFallback::Linear => Interpolation::Linear,
    }
}

/// Writes one channel's accessors, sampler and channel into `anim`.
fn write_channel(g: &mut Gather, anim: &mut Animation, ch: &PointerChannel, interp: Interpolation) {
    let times: Vec<f32> = ch.times.iter().map(|&t| t as f32).collect();
    // `gather_accessor(…, tuple([max(times)]), tuple([min(times)]), …)` is `@cached`: a
    // pointer channel sampling the object channels' times reuses their input accessor (a
    // copy has its own).
    let input = if ch.copy {
        own_accessor(g, &times, AccessorType::Scalar, Some(&ch.times))
    } else {
        crate::export::animation::accessor(g, &times, AccessorType::Scalar, Some(&ch.times))
    };
    let n = ch.values.first().map_or(1, Vec::len);
    let ty = match n {
        2 => AccessorType::Vec2,
        3 => AccessorType::Vec3,
        4 => AccessorType::Vec4,
        _ => AccessorType::Scalar,
    };
    let flat: Vec<f32> = ch
        .values
        .iter()
        .flat_map(|v| (0..n).map(move |i| v.get(i).copied().unwrap_or(0.0) as f32))
        .collect();
    let output = if ch.copy {
        own_accessor(g, &flat, ty, None)
    } else {
        crate::export::animation::accessor(g, &flat, ty, None)
    };
    anim.samplers.push(AnimationSampler {
        extensions: Default::default(),
        extras: None,
        input,
        interpolation: Some(interp),
        output,
    });
    anim.channels.push(Channel {
        extensions: Default::default(),
        extras: None,
        sampler: (anim.samplers.len() - 1) as Index,
        target: KhrAnimationPointer::target(ch.pointer.clone()),
    });
    if let Some(m) = ch.blend_material
        && let Some(mat) = g.doc.materials.get_mut(m as usize)
    {
        mat.alpha_mode = Some(AlphaMode::Blend);
    }
}

/// A float accessor of its own (the deep copy of an `additional` channel's accessor), with
/// the input's double-precision `min` / `max`.
fn own_accessor(g: &mut Gather, data: &[f32], ty: AccessorType, min_max: Option<&[f64]>) -> Index {
    let ad = AccessorData::new(ty, false, Components::F32(data.to_vec()))
        .expect("invariant: whole elements");
    let i = g.buf.push_accessor(&ad, &AccessorOpts::default());
    if let Some(t) = min_max
        && let Some(acc) = g.buf.accessor_mut(i)
    {
        acc.min = Some(vec![t.iter().copied().fold(f64::INFINITY, f64::min)]);
        acc.max = Some(vec![t.iter().copied().fold(f64::NEG_INFINITY, f64::max)]);
    }
    i
}

/// Adds the baked animations to the document: an animation merging under a name (the scene's
/// animation, an NLA track's) is appended to the document's animation of that name when there
/// is one (the object channels WP 6.7 gathered first), else created; the others are appended.
/// Returns the number of channels written.
pub fn write(g: &mut Gather, anims: Vec<PointerAnimation>) -> usize {
    let interp = interpolation(g);
    let mut written = 0;
    for pa in anims {
        let target = pa.merge.as_deref().and_then(|m| {
            g.doc
                .animations
                .iter()
                .position(|a| a.name.as_deref() == Some(m))
        });
        let mut anim = match target {
            Some(i) => std::mem::take(&mut g.doc.animations[i]),
            None => Animation {
                name: Some(pa.merge.clone().unwrap_or(pa.name.clone())),
                extras: pa.extras.clone(),
                ..Animation::default()
            },
        };
        for ch in &pa.channels {
            // `merge_tracks_perform`: a target already animated in the base is skipped.
            let dup = target.is_some()
                && anim
                    .channels
                    .iter()
                    .any(|c| KhrAnimationPointer::of(&c.target).as_deref() == Some(&ch.pointer));
            if dup {
                g.warn(format!(
                    "Some strips have same channel animation (pointer), on {} !",
                    ch.pointer
                ));
                continue;
            }
            write_channel(g, &mut anim, ch, interp);
            written += 1;
        }
        match target {
            Some(i) => g.doc.animations[i] = anim,
            None => g.doc.animations.push(anim),
        }
    }
    written
}

/// The scene's `fps · fps_base` and `fps`, and whether the lighting mode is `SPEC`.
pub fn timing(g: &Gather) -> ((f64, f64), bool) {
    let r = &g.ctx.scene().render;
    let fps = f64::from(r.fps);
    let spec = g.settings.props.export_import_convert_lighting_mode.value
        == ExportImportConvertLightingMode::Spec;
    ((fps * f64::from(r.fps_base), fps), spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_anim::{NlaStrip, NlaTrack};

    fn track(name: &str, blends: &[&str]) -> NlaTrack {
        let mut t = NlaTrack::new(name);
        for (i, b) in blends.iter().enumerate() {
            let mut s = NlaStrip::new_clip("s", None, i as f32 * 10.0, [0.0, 5.0]);
            s.blend_type = (*b).into();
            t.strips.push(s);
        }
        t
    }

    #[test]
    fn tracks_group_blending_ones_with_the_track_below() {
        let adt = AnimData {
            nla_tracks: vec![
                track("A", &["REPLACE"]),
                track("Empty", &[]),
                track("B", &["ADD"]),
                track("C", &["REPLACE"]),
                track("D", &["REPLACE", "MULTIPLY"]),
            ],
            ..AnimData::default()
        };
        let g = track_groups(Some(&adt), false);
        let got: Vec<(&str, Vec<usize>)> = g
            .iter()
            .map(|t| (t.name.as_str(), t.tracks.clone()))
            .collect();
        assert_eq!(got, [("A", vec![0, 2]), ("C", vec![3, 4])]);
        assert!(track_groups(None, false).is_empty());
    }

    #[test]
    fn track_bake_mutes_the_other_tracks() {
        let stored = AssetAnim {
            id: Some(AnimData {
                nla_tracks: vec![track("A", &["REPLACE"]), track("B", &["REPLACE"])],
                ..AnimData::default()
            }),
            tree: Some(AnimData {
                nla_tracks: vec![track("T", &["REPLACE"])],
                ..AnimData::default()
            }),
        };
        let g = TrackGroup {
            name: "B".into(),
            tracks: vec![1],
            tree: false,
        };
        let a = track_anim(&stored, &g);
        let id = a.id.unwrap();
        assert!(id.nla_tracks[0].mute && !id.nla_tracks[1].mute);
        assert!(id.use_nla && id.action.is_none());
        assert!(a.tree.unwrap().nla_tracks[0].mute);
    }

    #[test]
    fn frames_step_and_default_names() {
        let r = Range {
            start: 1,
            end: 6,
            step: 2,
            slide: None,
        };
        assert_eq!(r.frames(), [1, 3, 5]);
        assert!(is_default_track_name("NlaTrack.001"));
        assert!(is_default_track_name("[Action Stash]"));
        assert!(!is_default_track_name("Walk"));
    }
}
