// SPDX-License-Identifier: GPL-3.0-or-later
//! False Color: the baked LUTs per look and display (`assets/luts/`, baked by
//! `tools/blender/bake_view_luts.py`, checked on held-out colours only, §19.5). Owned by WP 5.3
//! (M5); empty until it lands (the processor answers `ColorError::NotAvailable`).

use crate::lut::BakedLut;

/// False Color LUTs: `(view transform, look, display, .cube)`.
pub const BAKED: &[BakedLut] = &[];
