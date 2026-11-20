// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-anim` (layer L10): `anim.*`, `screen.frame_*`/`keyframe_jump`, `armature.*`,
//! `pose.*`, `nla.*` operators, keying (`keying.rs`) and the animation RNA functions
//! (ARCHITECTURE §3.2, §18). Primary namespaces: `anim`, `screen`, `armature`, `pose`, `nla`.
//!
//! One file per operator in `src/ops/` (`ops/<ns>__<op>.rs`, §21.1) and one per RNA function in
//! `src/funcs/` (`funcs/<struct>_<func>.rs`), both included with `automod` and registered in
//! the operator / RNA function slices: adding one needs no `mod` edit. Ownership (M6, M5+M6
//! W0 decision log): `ops/{armature__*,pose__*}.rs` WP 6.2; `keying.rs`,
//! `ops/{keyframe_*,anim__*,screen__*,nla__*}.rs` and the keying/frame functions in `funcs/`
//! WP 6.4; the Action data-API functions in `funcs/` (`action_*`, `action_slots_*`,
//! `action_layers_*`, `action_strips_*`, `action_channelbag*`, `channelbag_*`, `fcurve_*`,
//! `fcurve_keyframe_points_*`, `anim_data_*`, `id_animation_data_*`, `blend_data_actions_*`)
//! WP 6.1. Only pure unit tests live here; behaviour tests go to `frap-golden` cases.
#![deny(unsafe_code)]

pub mod keying;

#[allow(non_snake_case)]
mod ops {
    automod::dir!("src/ops");
}

/// RNA function implementations, one file each (`funcs/<struct>_<func>.rs`).
mod funcs {
    automod::dir!("src/funcs");
}

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
