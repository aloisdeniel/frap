// SPDX-License-Identifier: GPL-3.0-or-later
//! ACES 1.3 and ACES 2.0: the baked LUTs per look and display (`assets/luts/`, baked by
//! `tools/blender/bake_view_luts.py`, checked on held-out colours only, §19.5). Owned by WP 5.3
//! (M5); empty until it lands (the processor answers `ColorError::NotAvailable`).

use crate::lut::BakedLut;

/// ACES 1.3 and ACES 2.0 LUTs: `(view transform, look, display, .cube)`.
pub const BAKED: &[BakedLut] = &[];
