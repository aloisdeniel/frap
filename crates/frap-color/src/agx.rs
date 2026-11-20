// SPDX-License-Identifier: GPL-3.0-or-later
//! AgX (the factory view transform) and its looks: the baked LUTs (`assets/luts/agx*.cube`,
//! `tools/blender/bake_view_luts.py`). Owned by WP 5.3 (M5); the M5+M6 W0 integrator moved the
//! M1 entry here from `lut.rs`.

use crate::lut::BakedLut;

/// AgX LUTs: `(view transform, look, display, .cube)`.
pub const BAKED: &[BakedLut] = &[(
    "AgX",
    "None",
    "sRGB",
    include_str!("../../../assets/luts/agx.cube"),
)];
