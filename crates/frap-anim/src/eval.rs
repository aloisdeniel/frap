// SPDX-License-Identifier: GPL-3.0-or-later
//! Evaluation (WP 6.1): F-curve sampling (`BKE_fcurve_evaluate`: Bézier through
//! `BKE_fcurve_correct_bezpart`/`findzero`, the 13 interpolations with Robert Penner's easing
//! equations, extrapolation), handle recalculation (`BKE_fcurve_handles_recalc` with
//! `BKE_nurb_handle_calc` semantics), action-slot evaluation and NLA blending
//! (`animsys_evaluate_nla_*`). M5+M6 W0 contract: the signatures; WP 6.1 implements them.

use frap_rna::{ActionId, IdRef};
use smol_str::SmolStr;

use crate::action::Action;
use crate::anim_data::AnimData;
use crate::fcurve::FCurve;

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
    let _ = (fc, frame);
    todo!("M6 WP 6.1: F-curve evaluation")
}

/// `BKE_fcurve_handles_recalc`: recomputes AUTO / AUTO_CLAMPED / VECTOR handles (and ALIGNED
/// partners) after keys moved or were inserted.
pub fn recalc_handles(fc: &mut FCurve) {
    let _ = fc;
    todo!("M6 WP 6.1: handle recalculation")
}

/// Every channel of slot `slot_handle` of `action` at `frame` (layer influence and mix modes
/// as Blender 5.2 applies them; muted F-curves skipped).
pub fn evaluate_action(action: &Action, slot_handle: i32, frame: f32) -> Vec<AnimChannel> {
    let _ = (action, slot_handle, frame);
    todo!("M6 WP 6.1: action evaluation")
}

/// Every channel `adt` writes at `frame`: the NLA tracks bottom to top (strip time mapping,
/// blend in/out, extrapolation, blend types, influence), then the active action with
/// `action_blend_type`, `action_influence` and `action_extrapolation`
/// (`BKE_animsys_evaluate_animdata` with `ADT_RECALC_ANIM`).
pub fn evaluate_anim(adt: &AnimData, actions: &dyn ActionLookup, frame: f32) -> Vec<AnimChannel> {
    let _ = (adt, actions, frame);
    todo!("M6 WP 6.1: AnimData / NLA evaluation")
}
