// SPDX-License-Identifier: GPL-3.0-or-later
//! Evaluation (WP 6.1): F-curve sampling (`BKE_fcurve_evaluate`: Bézier through
//! `BKE_fcurve_correct_bezpart`/`findzero`, the 13 interpolations with Robert Penner's easing
//! equations, extrapolation), handle recalculation (`BKE_fcurve_handles_recalc` with
//! `BKE_nurb_handle_calc` semantics and continuous-acceleration smoothing), action-slot
//! evaluation (`animrig::evaluate_action`) and NLA blending (`animsys_evaluate_nla_*`).
//! M5+M6 W0 contract: the signatures; WP 6.1 implements them in the submodules, ported from
//! Blender 5.2.4 (provenance in each file).

use frap_rna::{ActionId, IdRef, StructDef};
use smol_str::SmolStr;

use crate::action::{Action, ActionStrip};
use crate::anim_data::AnimData;
use crate::fcurve::FCurve;

pub mod curve;
pub mod easing;
pub mod handles;
pub mod nla;
pub mod props;

/// One evaluated channel: write `value` to `data_path[array_index]` of the animated ID
/// (`array_index` is 0 for a scalar property).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimChannel {
    /// ID-relative RNA path (`location`, `pose.bones["Bone"].rotation_quaternion`,
    /// `key_blocks["Key 1"].value`, `nodes["Principled BSDF"].inputs[0].default_value`).
    pub data_path: SmolStr,
    /// Array index.
    pub array_index: i32,
    /// Value (integers, booleans and enums are rounded by the writer, as Blender's
    /// `animsys_write_rna_setting` does).
    pub value: f32,
}

/// Resolves action pointers (the caller's `bpy.data.actions`; `frap-anim` sits below
/// `frap-scene`).
pub trait ActionLookup {
    /// The action `r` points to.
    fn action(&self, r: IdRef<ActionId>) -> Option<&Action>;
}

/// `BKE_fcurve_evaluate` / `evaluate_fcurve(fcu, evaltime)` (no F-modifiers, no drivers).
pub fn evaluate_fcurve(fc: &FCurve, frame: f32) -> f32 {
    curve::evaluate(fc, frame)
}

/// `BKE_fcurve_handles_recalc`: recomputes AUTO / AUTO_CLAMPED / VECTOR handles (and ALIGNED
/// partners) after keys moved or were inserted.
pub fn recalc_handles(fc: &mut FCurve) {
    handles::recalc(fc)
}

/// `is_fcurve_evaluatable` of `animrig/intern/evaluation.cc` (no check on group mute there).
fn layered_evaluatable(fc: &FCurve) -> bool {
    !fc.data_path.is_empty() && !fc.mute && !fc.is_empty()
}

/// Stores `(path, index) = value`, replacing an earlier result for the same channel
/// (`EvaluationResult::store`).
fn store(out: &mut Vec<AnimChannel>, data_path: &SmolStr, array_index: i32, value: f32) {
    match out
        .iter_mut()
        .find(|c| c.array_index == array_index && &c.data_path == data_path)
    {
        Some(c) => c.value = value,
        None => out.push(AnimChannel {
            data_path: data_path.clone(),
            array_index,
            value,
        }),
    }
}

/// Every channel of slot `slot_handle` of `action` at `frame` (layer influence and mix modes
/// as Blender 5.2 applies them; muted F-curves skipped).
///
/// Ported from blender/source/blender/animrig/intern/evaluation.cc (`evaluate_action`,
/// `evaluate_layer`, `blend_layer_results`): a layer without influence is skipped, the first
/// layer with results is taken as-is, later ones blend in by their mix mode. Keyframe strips
/// are infinite, so the first strip of a layer gives its result.
pub fn evaluate_action(action: &Action, slot_handle: i32, frame: f32) -> Vec<AnimChannel> {
    let mut result: Vec<AnimChannel> = Vec::new();
    for layer in &action.layers {
        if layer.influence <= 0.0 {
            continue;
        }
        let mut layer_result: Vec<AnimChannel> = Vec::new();
        if let Some(ActionStrip::Keyframe(strip)) = layer.strips.first()
            && let Some(bag) = strip
                .channelbags
                .iter()
                .find(|c| c.slot_handle == slot_handle)
        {
            for fc in bag.fcurves.iter().filter(|f| layered_evaluatable(f)) {
                let v = evaluate_fcurve(fc, frame);
                store(&mut layer_result, &fc.data_path, fc.array_index, v);
            }
        }
        if layer_result.is_empty() {
            continue;
        }
        if result.is_empty() {
            result = layer_result;
            continue;
        }
        for ch in layer_result {
            let Some(last) = result
                .iter_mut()
                .find(|c| c.array_index == ch.array_index && c.data_path == ch.data_path)
            else {
                result.push(AnimChannel {
                    value: ch.value * layer.influence,
                    ..ch
                });
                continue;
            };
            let inf = layer.influence;
            match layer.mix_mode.as_str() {
                // `math::interpolate(influence, last, value)`, argument order as in Blender.
                "OFFSET" => last.value = inf * (1.0 - ch.value) + last.value * ch.value,
                "ADD" => last.value += ch.value * inf,
                "SUBTRACT" => last.value -= ch.value * inf,
                "MULTIPLY" => last.value *= ch.value * inf,
                // REPLACE
                _ => last.value = ch.value * inf,
            }
        }
    }
    result
}

/// Every channel `adt` writes at `frame`: the NLA tracks bottom to top (strip time mapping,
/// blend in/out, extrapolation, blend types, influence), then the active action with
/// `action_blend_type`, `action_influence` and `action_extrapolation`
/// (`BKE_animsys_evaluate_animdata` with `ADT_RECALC_ANIM`).
///
/// The ID type the paths start from (for the NLA's channel defaults and mix modes) is taken
/// from the action slots; [`evaluate_anim_for`] names it explicitly.
pub fn evaluate_anim(adt: &AnimData, actions: &dyn ActionLookup, frame: f32) -> Vec<AnimChannel> {
    let id_type = slot_id_type(adt, actions);
    evaluate_anim_for(adt, actions, frame, id_type.and_then(props::root_struct))
}

/// [`evaluate_anim`] for an animated ID whose RNA struct is `root` (`frap_spec_rna::OBJECT`,
/// …; `None` when unknown: every channel then blends as an additive scalar with default 0).
pub fn evaluate_anim_for(
    adt: &AnimData,
    actions: &dyn ActionLookup,
    frame: f32,
    root: Option<&'static StructDef>,
) -> Vec<AnimChannel> {
    evaluate_anim_refined(adt, actions, frame, root, None)
}

/// [`evaluate_anim_for`] with the refined structs of collection items and pointers taken from
/// the data through `refine` (a modifier's settings struct, a node's or socket's type), so
/// that NLA channels under them get the real defaults and mix modes, as in Blender.
pub fn evaluate_anim_refined(
    adt: &AnimData,
    actions: &dyn ActionLookup,
    frame: f32,
    root: Option<&'static StructDef>,
    refine: Option<props::Refine<'_>>,
) -> Vec<AnimChannel> {
    // NLA first; the active action is its top strip (`animsys_calculate_nla`).
    if !adt.nla_tracks.is_empty()
        && adt.use_nla
        && let Some(out) = nla::calculate(adt, actions, frame, root, refine)
    {
        return out;
    }
    match adt.action.and_then(|a| actions.action(a)) {
        Some(action) => evaluate_action(action, adt.action_slot_handle, frame),
        None => Vec::new(),
    }
}

/// The `target_id_type` of the first typed slot `adt` uses (its action's, then its strips').
fn slot_id_type<'a>(adt: &AnimData, actions: &'a dyn ActionLookup) -> Option<&'a str> {
    let typed = |a: Option<IdRef<ActionId>>, h: i32| {
        let s = actions.action(a?)?.slot(h)?;
        (s.target_id_type != "UNSPECIFIED").then_some(s.target_id_type.as_str())
    };
    typed(adt.action, adt.action_slot_handle).or_else(|| {
        adt.nla_tracks
            .iter()
            .flat_map(|t| &t.strips)
            .find_map(|s| typed(s.action, s.action_slot_handle))
    })
}
