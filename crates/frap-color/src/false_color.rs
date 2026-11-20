// SPDX-License-Identifier: GPL-3.0-or-later
//! False Color. Owned by WP 5.3 (M5).
//!
//! AgX Base Rec.2020, decoded to linear, reduced to Rec.2020 luminance, encoded with a 2.5
//! exponent and mapped through Blender's `AgX_False_Color` 1D LUT (a heat map), then the
//! display encoding; the False Color looks are the AgX looks. `assets/luts/false_color.ops`
//! holds every display × look pipeline (`tools/blender/bake_view_luts.py`; the Rec.2020 base
//! LUT is in [`crate::agx`]). The view is not a single LUT, so [`BAKED`] stays empty.

use crate::display::ocio::{DataFile, OpsFile};
use crate::lut::BakedLut;

/// False Color single-LUT views: none (see the module docs).
pub const BAKED: &[BakedLut] = &[];

/// The False Color op pipelines: every display × look.
pub(crate) const PIPELINES: OpsFile = include_str!("../../../assets/luts/false_color.ops");

/// LUT data of the False Color pipelines (besides the AgX base LUTs).
pub(crate) const DATA: &[DataFile] = &[(
    "agx_false_color",
    include_str!("../../../assets/luts/agx_false_color.lut1d"),
)];
