// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-anim` (layer L3): actions, slots, layers, strips, channelbags, F-curves, NLA, and
//! F-curve evaluation; pure, no scene (ARCHITECTURE §18).
//!
//! Ownership (M6): WP 6.1 owns `crates/frap-anim/**` after the M5+M6 W0 integrator froze the
//! data model below (CONTRACT, additive from here on):
//!
//! - [`action`]: `Action { slots, layers: [{ strips: [{ channelbags: [{ slot_handle, groups,
//!   fcurves }] }] }] }` (Blender 5.x slotted, layered actions; `action.fcurves` is
//!   `RNA_REMOVED_IN_5X`);
//! - [`fcurve`]: `FCurve`, `Keyframe` and the interpolation / easing / handle / keyframe-type /
//!   extrapolation enums with Blender's identifiers;
//! - [`anim_data`]: `AnimData` with the NLA (`NlaTrack`, `NlaStrip`);
//! - [`eval`]: `evaluate_fcurve`, `evaluate_action`, `evaluate_anim` (NLA blending), the
//!   handle recalculation (`BKE_fcurve_handles_recalc`);
//! - [`rna`]: the RNA bindings of these structs (WP 6.1; the `Action` ID binding lives with
//!   the type).
#![deny(unsafe_code)]

pub mod action;
pub mod anim_data;
pub mod eval;
pub mod fcurve;
pub mod rna;

pub use action::{
    Action, ActionGroup, ActionLayer, ActionSlot, ActionStrip, Channelbag, KeyframeStrip,
};
pub use anim_data::{AnimData, NlaStrip, NlaTrack};
pub use eval::{ActionLookup, AnimChannel};
pub use fcurve::{
    Easing, Extrapolation, FCurve, HandleType, Interpolation, Keyframe, KeyframeType,
};
