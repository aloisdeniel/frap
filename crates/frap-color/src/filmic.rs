// SPDX-License-Identifier: GPL-3.0-or-later
//! Filmic and Filmic Log, and the generic contrast looks built on them. Owned by WP 5.3 (M5).
//!
//! "Filmic Log" is a log2 shaper and Blender's 33³ desaturation LUT (`filmic_desat_33`);
//! "Filmic" adds the base-contrast 1D curve (`filmic_to_0-70_1-03`). The generic looks ("Very
//! High Contrast" … "Very Low Contrast") work in Filmic Log: their 1D curve, then the inverse of
//! the base curve, back to scene linear and into the view. `assets/luts/filmic.ops` holds the
//! Filmic pipelines (`tools/blender/bake_view_luts.py`); the Standard/Raw/Khronos PBR Neutral
//! pipelines with these looks use the same data. Neither view is a single LUT, so [`BAKED`]
//! stays empty.

use crate::display::ocio::{DataFile, OpsFile};
use crate::lut::BakedLut;

/// Filmic and Filmic Log single-LUT views: none (see the module docs).
pub const BAKED: &[BakedLut] = &[];

/// The Filmic op pipelines: every display × view × look.
pub(crate) const PIPELINES: OpsFile = include_str!("../../../assets/luts/filmic.ops");

/// LUT data of the Filmic views and the generic contrast looks.
pub(crate) const DATA: &[DataFile] = &[
    (
        "filmic_desat_33",
        include_str!("../../../assets/luts/filmic_desat_33.cube"),
    ),
    (
        "filmic_to_0_35_1_30",
        include_str!("../../../assets/luts/filmic_to_0_35_1_30.lut1d"),
    ),
    (
        "filmic_to_0_48_1_09",
        include_str!("../../../assets/luts/filmic_to_0_48_1_09.lut1d"),
    ),
    (
        "filmic_to_0_60_1_04",
        include_str!("../../../assets/luts/filmic_to_0_60_1_04.lut1d"),
    ),
    (
        "filmic_to_0_70_1_03",
        include_str!("../../../assets/luts/filmic_to_0_70_1_03.lut1d"),
    ),
    (
        "filmic_to_0_85_1_011",
        include_str!("../../../assets/luts/filmic_to_0_85_1_011.lut1d"),
    ),
    (
        "filmic_to_0_99_1_0075",
        include_str!("../../../assets/luts/filmic_to_0_99_1_0075.lut1d"),
    ),
    (
        "filmic_to_1_20_1_00",
        include_str!("../../../assets/luts/filmic_to_1_20_1_00.lut1d"),
    ),
];
