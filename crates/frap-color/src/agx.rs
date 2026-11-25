// SPDX-License-Identifier: GPL-3.0-or-later
//! AgX (the factory view transform) and its looks. Owned by WP 5.3 (M5); the M5+M6 W0
//! integrator moved the M1 entry here from `lut.rs`.
//!
//! The AgX view is an E-Gamut matrix, a log2 shaper and Blender's base LUT for the display
//! (sRGB/Rec.1886, Display P3 or Rec.2020), then the display encoding. Its looks are applied in
//! "AgX Log" (the Rec.2020 luminance-compensation LUT, an inset matrix and the log2 shaper)
//! before the view. `assets/luts/agx.ops` holds every display × look pipeline
//! (`tools/blender/bake_view_luts.py`); `agx.cube` doubles as the single-LUT `lut::baked`
//! entry for look None on sRGB.

use crate::display::ocio::{DataFile, OpsFile};
use crate::lut::BakedLut;

/// AgX single-LUT views: `(view transform, look, display, .cube)`.
pub const BAKED: &[BakedLut] = &[(
    "AgX",
    "None",
    "sRGB",
    include_bytes!("generated/agx.cube.deflate"),
)];

/// The AgX op pipelines: every display × AgX look.
pub(crate) const PIPELINES: OpsFile = include_str!("../../../assets/luts/agx.ops");

/// LUT data of the AgX (and False Color) pipelines: the base LUTs per display and the
/// luminance compensation of "AgX Log".
pub(crate) const DATA: &[DataFile] = &[
    ("agx", BAKED[0].3),
    (
        "agx_base_p3",
        include_bytes!("generated/agx_base_p3.cube.deflate"),
    ),
    (
        "agx_base_rec2020",
        include_bytes!("generated/agx_base_rec2020.cube.deflate"),
    ),
    (
        "luminance_compensation_bt2020",
        include_bytes!("generated/luminance_compensation_bt2020.cube.deflate"),
    ),
];
