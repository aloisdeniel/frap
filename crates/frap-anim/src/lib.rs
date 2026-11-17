// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-anim` (layer L3): actions, slots, layers, strips, channelbags, F-curves, NLA, and
//! F-curve evaluation; pure, no scene (ARCHITECTURE §18).
//!
//! M1 W0: only the `Action` datablock type (so `Main` has its final shape, §5.2) and the
//! `AnimData` block that `Object`, `NodeTree` and other IDs embed. Both are placeholders whose
//! fields are completed by the M6 W0 integrator; generated plain structs (`Keyframe`,
//! `NlaStrip` fields) go to `src/generated/` then.
#![deny(unsafe_code)]

mod action;

pub use action::{Action, AnimData};
