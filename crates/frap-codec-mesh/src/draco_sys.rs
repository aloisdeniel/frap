// SPDX-License-Identifier: GPL-3.0-or-later
//! The `frap-draco-sys` backend (feature `draco-sys`, WP 7.2; M7 W0 stub).

use crate::draco::DracoCodec;

/// The Google Draco codec, `None` until WP 7.2 implements it (or without `draco-sys`).
pub fn codec() -> Option<&'static dyn DracoCodec> {
    None
}
