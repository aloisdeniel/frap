// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF animation export (WP 6.7, `crates/frap-io-gltf/src/export/animation/**`): every
//! `export_animation_mode` (`ACTIONS`, `ACTIVE_ACTIONS`, `BROADCAST`, `NLA_TRACKS`, `SCENE`),
//! TRS and morph-weight channels, sampling (`export_force_sampling`, `export_frame_step`) vs
//! CUBICSPLINE vs STEP/LINEAR, the `fps_base` quirk, ported from io_scene_gltf2 5.2.41
//! `blender/exp/animation/**` (Apache-2.0, The glTF-Blender-IO authors). Channel targets
//! resolve through `export::node_map::NodeMap` (objects; bones once WP 6.8 records them).
//!
//! `gather` runs after the scenes (the add-on's `gather_animations` after each
//! `__gather_scene`): for every exported scene it rebuilds that scene's export tree, gathers the
//! animations of the mode on a working copy of the state (`world::World`, where the add-on's
//! action switches and `frame_set` calls are replayed), merges them
//! (`merge_tracks_perform`) and writes them after the scene's (`animations[]`, one sampler
//! per channel, accessors without buffer-view target, inputs with `min`/`max`).
//!
//! Times are `frame / (fps · fps_base)`, as the add-on computes them (Blender's own time is
//! `frame · fps_base / fps`); a scene with `fps_base ≠ 1` gets a WARNING naming the formula.
//!
//! Module layout (one file per add-on module): `keys` (`keyframes.py`, the value
//! transforms), `world` (the sampled state), `exp` (ranges, slides, `get_cache_data`, the
//! sampled object/bone/shape-key channels), `fcurves` (channel groups and the non-sampled
//! export), `actions` (`action.py`), `tracks` (`tracks.py`), `scene`
//! (`scene_animation.py`).

pub(crate) mod actions;
pub(crate) mod exp;
pub(crate) mod fcurves;
pub(crate) mod keys;
pub(crate) mod scene;
pub(crate) mod tracks;
pub(crate) mod world;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use frap_gltf::accessor::{AccessorData, Components};
use frap_gltf::builder::AccessorOpts;
use frap_gltf::json::accessor::AccessorType;
use frap_gltf::json::animation::{
    Animation, AnimationPath, AnimationSampler, Channel, ChannelTarget,
};
use frap_gltf::json::{Extras, Index};
use frap_rna::{ActionId, IdRef, ObjectId, SceneId};
use frap_scene::Context;
use frap_spec_ops_io::export_scene::gltf;
use indexmap::IndexMap;
use serde_json::{Map, Value as J};
use smol_str::SmolStr;

use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::node_map::NodeMap;
use crate::export::settings::ExportSettings;
use crate::export::{extras, scenes, vtree};
use exp::{AKey, Exp};
use keys::Interp;

/// The animation props honoured here (M5+M6 W0 extension point, WP 6.7).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_animations",
    "export_frame_range",
    "export_frame_step",
    "export_force_sampling",
    "export_sampling_interpolation_fallback",
    "export_animation_mode",
    "export_nla_strips_merged_animation_name",
    "export_optimize_animation_size",
    "export_optimize_animation_keep_anim_armature",
    "export_optimize_animation_keep_anim_object",
    "export_optimize_disable_viewport",
    "export_negative_frame",
    "export_anim_slide_to_zero",
    "export_bake_animation",
    "export_merge_animation",
    "export_anim_single_armature",
    "export_reset_pose_bones",
    "export_anim_scene_split_object",
    "export_morph_animation",
    "export_morph_reset_sk_data",
    "export_extra_animations",
    "export_nla_strips",
];

/// `export_animation_mode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Actions,
    ActiveActions,
    Broadcast,
    NlaTracks,
    Scene,
}

/// `export_merge_animation`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Merge {
    NlaTrack,
    Action,
    None,
}

/// `export_negative_frame`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Negative {
    Slide,
    Crop,
}

/// The animation settings after `ExportGLTF2.execute`'s coupling rules.
#[derive(Clone, Debug)]
pub(crate) struct AnimSettings {
    pub animations: bool,
    pub frame_range: bool,
    pub frame_step: i32,
    pub force_sampling: bool,
    pub fallback: Interp,
    pub mode: Mode,
    pub merged_name: String,
    pub optimize: bool,
    pub keep_armature: bool,
    pub keep_object: bool,
    pub negative: Negative,
    pub slide_to_zero: bool,
    pub bake: bool,
    pub merge: Merge,
    pub single_armature: bool,
    pub reset_pose_bones: bool,
    pub reset_sk: bool,
    pub split_object: bool,
    pub morph_anim: bool,
    pub yup: bool,
    pub cameras: bool,
    pub lights: bool,
}

impl AnimSettings {
    /// From the operator props (`execute()`'s `export_settings` for animations).
    pub(crate) fn from(s: &ExportSettings) -> AnimSettings {
        let p = &s.props;
        let animations = p.export_animations.value;
        let mode = match p.export_animation_mode.value {
            gltf::ExportAnimationMode::Actions => Mode::Actions,
            gltf::ExportAnimationMode::ActiveActions => Mode::ActiveActions,
            gltf::ExportAnimationMode::Broadcast => Mode::Broadcast,
            gltf::ExportAnimationMode::NlaTracks => Mode::NlaTracks,
            gltf::ExportAnimationMode::Scene => Mode::Scene,
        };
        let on = |b: bool| animations && b;
        let force_sampling = on(p.export_force_sampling.value || mode == Mode::NlaTracks);
        AnimSettings {
            animations,
            frame_range: on(p.export_frame_range.value),
            frame_step: p.export_frame_step.value.max(1),
            force_sampling,
            fallback: match p.export_sampling_interpolation_fallback.value {
                gltf::ExportSamplingInterpolationFallback::Linear => Interp::Linear,
                gltf::ExportSamplingInterpolationFallback::Step => Interp::Step,
            },
            mode,
            merged_name: p.export_nla_strips_merged_animation_name.value.to_string(),
            optimize: on(p.export_optimize_animation_size.value),
            keep_armature: on(p.export_optimize_animation_keep_anim_armature.value),
            keep_object: on(p.export_optimize_animation_keep_anim_object.value),
            negative: match p.export_negative_frame.value {
                gltf::ExportNegativeFrame::Slide => Negative::Slide,
                gltf::ExportNegativeFrame::Crop => Negative::Crop,
            },
            slide_to_zero: p.export_anim_slide_to_zero.value,
            // `export_force_sampling=False` turns baking off.
            bake: on(p.export_bake_animation.value) && force_sampling,
            merge: if mode != Mode::Actions {
                Merge::NlaTrack
            } else {
                match p.export_merge_animation.value {
                    gltf::ExportMergeAnimation::NlaTrack => Merge::NlaTrack,
                    gltf::ExportMergeAnimation::Action => Merge::Action,
                    gltf::ExportMergeAnimation::None => Merge::None,
                }
            },
            single_armature: on(mode == Mode::Actions && p.export_anim_single_armature.value),
            reset_pose_bones: on(p.export_reset_pose_bones.value),
            reset_sk: on(p.export_morph_reset_sk_data.value),
            split_object: mode == Mode::Scene && p.export_anim_scene_split_object.value,
            morph_anim: p.export_morph.value && p.export_morph_animation.value,
            yup: s.export_yup,
            cameras: crate::export::cameras::exported(s),
            lights: crate::export::lights::exported(s),
        }
    }
}

/// One sampler before writing (input seconds, flattened output).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GSampler {
    /// Key times in seconds as the add-on computes them (doubles; written as floats, the
    /// doubles in `min`/`max`).
    pub times: Vec<f64>,
    /// Output values.
    pub values: Vec<f32>,
    /// Output accessor type.
    pub ty: AccessorType,
    /// Interpolation.
    pub interp: Interp,
}

impl GSampler {
    pub(crate) fn new(times: Vec<f64>, values: Vec<f32>, ty: AccessorType, interp: Interp) -> Self {
        GSampler {
            times,
            values,
            ty,
            interp,
        }
    }
}

/// One channel before writing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GChannel {
    pub node: Index,
    pub path: AnimationPath,
    pub sampler: GSampler,
}

/// Where an animation's `extras` come from (`export_extras`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Action(IdRef<ActionId>),
    Object(IdRef<ObjectId>),
    Scene(IdRef<SceneId>),
}

/// One animation before writing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GAnim {
    pub name: String,
    pub channels: Vec<GChannel>,
    /// The datablocks whose custom properties become `extras` (merged animations add theirs).
    pub sources: Vec<Source>,
}

/// `merge_tracks_perform(merged_tracks, animations)`.
pub(crate) fn merge_tracks(
    exp: &mut Exp<'_>,
    merged: IndexMap<SmolStr, Vec<usize>>,
    mut animations: Vec<GAnim>,
) -> Vec<GAnim> {
    let mut deleted: Vec<usize> = Vec::new();
    for (name, list) in &merged {
        if list.len() < 2 {
            if let Some(&i) = list.first() {
                animations[i].name = name.to_string();
            }
            continue;
        }
        let base = list[0];
        animations[base].name = name.to_string();
        let mut done: Vec<(Index, AnimationPath)> = animations[base]
            .channels
            .iter()
            .map(|c| (c.node, c.path.clone()))
            .collect();
        for &i in &list[1..] {
            deleted.push(i);
            let other = std::mem::take(&mut animations[i].channels);
            let sources = animations[i].sources.clone();
            animations[base].sources.extend(sources);
            for c in other {
                if done.contains(&(c.node, c.path.clone())) {
                    exp.warn(format!(
                        "Some strips have same channel animation ({}), on node {} !",
                        c.path.as_str(),
                        c.node
                    ));
                    continue;
                }
                done.push((c.node, c.path.clone()));
                animations[base].channels.push(c);
            }
        }
    }
    animations
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !deleted.contains(i))
        .map(|(_, a)| a)
        .collect()
}

/// `bake_animation(obj, key, mode)`: the sampled TRS (and shape keys) of an object without an
/// action (`export_bake_animation`) or of an NLA track group (`mode` `OBJECT` / `KEY`).
pub(crate) fn bake_animation(
    exp: &mut Exp<'_>,
    ob: IdRef<ObjectId>,
    key: AKey,
    mode: Option<&str>,
) -> Option<GAnim> {
    if !exp.has_actions {
        return None;
    }
    let o = exp.world.ob(ob)?.clone();
    let baking = exp.s.bake || exp.s.mode == Mode::NlaTracks;
    let name = match &key {
        AKey::Name(n) => n.to_string(),
        AKey::Obj(_) => o.header.name.to_string(),
    };
    if baking && o.object_type() != "ARMATURE" && exp.s.force_sampling {
        let mut channels = Vec::new();
        let mut anim_name = None;
        if mode.is_none_or(|m| m == "OBJECT") {
            let c = exp.object_channels(ob, &key, &None, &HashMap::new());
            if !c.is_empty() {
                channels.extend(c);
                anim_name = Some(name.clone());
            }
        }
        if exp.s.morph_anim
            && o.object_type() == "MESH"
            && exp.world.key(ob).is_some()
            && mode != Some("OBJECT")
            && let Some(c) = exp.sk_channel(ob, &key, &None)
        {
            channels.push(c);
            anim_name.get_or_insert_with(|| o.header.name.to_string());
        }
        if channels.is_empty() {
            return None;
        }
        return Some(GAnim {
            name: anim_name.unwrap_or(name),
            channels,
            sources: Vec::new(),
        });
    }
    if baking && o.object_type() == "ARMATURE" && mode.is_none_or(|m| m == "OBJECT") {
        let channels = exp.armature_channels(ob, &key, &None, &HashMap::new(), None);
        if channels.is_empty() {
            return None;
        }
        return Some(GAnim {
            name,
            channels,
            sources: Vec::new(),
        });
    }
    None
}

/// Gathers `doc.animations` for the export (after `scenes::gather`).
pub fn gather(g: &mut Gather) -> Result<(), ExportError> {
    let s = AnimSettings::from(g.settings);
    if !s.animations {
        return Ok(());
    }
    let nodes: NodeMap = g.state::<NodeMap>().clone();
    // Nodes whose mesh has morph targets: `weights` channels need them (WP 6.8 writes them).
    let morph_nodes: HashSet<Index> = g
        .doc
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            n.mesh
                .and_then(|m| g.doc.meshes.get(m as usize))
                .is_some_and(|m| m.primitives.iter().any(|p| !p.targets.is_empty()))
        })
        .map(|(i, _)| i as Index)
        .collect();
    let user = g.ctx.win.scene;
    for scene in scenes::exported_scenes(g.main, user, g.settings) {
        let mut win = g.ctx.win.clone();
        frap_scene::window::set_scene(g.main, &mut win, scene).map_err(ExportError::Invalid)?;
        let cx = Context {
            main: g.main,
            win: &win,
            prefs: g.ctx.prefs,
        };
        let tree = vtree::build(g.main, &cx, g.settings);
        let mut exp = Exp::new(&s, &tree, scene, &nodes, g.main, &morph_nodes);
        if let Some(sc) = g.main.scenes.get(scene)
            && sc.render.fps_base != 1.0
            && g.main.actions.iter().next().is_some()
        {
            exp.warn(format!(
                "glTF export: animation times of scene '{}' use the add-on's frame / (fps × fps_base) = frame / {} (Blender's own time is frame × fps_base / fps)",
                sc.header.name, exp.fps
            ));
        }
        let anims = match s.mode {
            Mode::Actions | Mode::ActiveActions | Mode::Broadcast => actions::gather(&mut exp),
            Mode::NlaTracks => tracks::gather(&mut exp),
            Mode::Scene => scene::gather(&mut exp),
        };
        for w in std::mem::take(&mut exp.warnings) {
            g.warn(w);
        }
        drop(exp);
        write(g, anims);
    }
    Ok(())
}

/// The merged `extras` of an animation's sources.
fn anim_extras(g: &mut Gather, sources: &[Source]) -> Option<Extras> {
    if !g.settings.props.export_extras.value {
        return None;
    }
    let main = g.main;
    let mut map = Map::new();
    let mut any = false;
    for src in sources {
        let header = match *src {
            Source::Action(a) => main.actions.get(a).map(|a| &a.header),
            Source::Object(o) => main.objects.get(o).map(|o| &o.header),
            Source::Scene(s) => main.scenes.get(s).map(|s| &s.header),
        };
        let Some(e) = header.and_then(|h| extras::id(g, h)) else {
            continue;
        };
        if let Ok(J::Object(m)) = serde_json::from_str::<J>(e.0.get()) {
            any = true;
            map.extend(m);
        }
    }
    any.then(|| Extras::from_value(&J::Object(map)))
}

/// Writes `anims` into `g.doc.animations` with their accessors (inputs and outputs shared by
/// content, as the add-on's `@cached gather_accessor` shares them).
fn write(g: &mut Gather, anims: Vec<GAnim>) {
    for a in anims {
        let extras = anim_extras(g, &a.sources);
        let mut channels = Vec::with_capacity(a.channels.len());
        let mut samplers = Vec::with_capacity(a.channels.len());
        for c in a.channels {
            let times: Vec<f32> = c.sampler.times.iter().map(|t| *t as f32).collect();
            let input = accessor(g, &times, AccessorType::Scalar, Some(&c.sampler.times));
            let output = accessor(g, &c.sampler.values, c.sampler.ty, None);
            samplers.push(AnimationSampler {
                extensions: Default::default(),
                extras: None,
                input,
                interpolation: Some(c.sampler.interp.gltf()),
                output,
            });
            channels.push(Channel {
                extensions: Default::default(),
                extras: None,
                sampler: (samplers.len() - 1) as Index,
                target: ChannelTarget {
                    extensions: Default::default(),
                    extras: None,
                    node: Some(c.node),
                    path: c.path,
                },
            });
        }
        g.doc.animations.push(Animation {
            channels,
            extensions: Default::default(),
            extras,
            name: Some(a.name),
            samplers,
        });
    }
}

/// The animation accessors written so far, by content: the add-on's `@cached
/// gather_accessor` is export-wide, so the object, bone and shape-key channels and the
/// `KHR_animation_pointer` channels ([`crate::export::pointer`]) share one input accessor
/// when they sample the same times.
#[derive(Default)]
struct SharedAccessors(HashMap<(Vec<u8>, bool), Index>);

/// One float accessor without buffer-view target, shared by content with every animation
/// accessor written before it; inputs carry `min`/`max` (the add-on writes the doubles it
/// computed).
pub(crate) fn accessor(
    g: &mut Gather,
    data: &[f32],
    ty: AccessorType,
    min_max: Option<&[f64]>,
) -> Index {
    let comps = ty.components().max(1);
    let mut bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes.extend_from_slice(&(comps as u32).to_le_bytes());
    let k = (bytes, min_max.is_some());
    if let Some(i) = g.state::<SharedAccessors>().0.get(&k) {
        return *i;
    }
    let ad = AccessorData::new(ty, false, Components::F32(data.to_vec()))
        .expect("invariant: whole elements");
    let i = g.buf.push_accessor(&ad, &AccessorOpts::default());
    if let Some(t) = min_max
        && let Some(acc) = g.buf.accessor_mut(i)
    {
        let lo = t.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = t.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        acc.min = Some(vec![lo]);
        acc.max = Some(vec![hi]);
    }
    g.state::<SharedAccessors>().0.insert(k, i);
    i
}
