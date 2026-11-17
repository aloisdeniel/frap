// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-anim`: `anim.*`, `screen.frame_*`, `armature.*`, `pose.*`, `nla.*` operators and keying. Empty until M6 W0 (ARCHITECTURE §3.2).
#![deny(unsafe_code)]

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
