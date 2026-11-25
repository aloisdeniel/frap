// SPDX-License-Identifier: GPL-3.0-or-later
//! Khronos PBR Neutral. Owned by WP 5.3 (M5).
//!
//! A log2 shaper over `[-9, 10]` stops, Blender's 57³ `pbrNeutral` LUT (display-linear output)
//! and the sRGB encoding; the generic contrast looks apply before it. `assets/luts/
//! pbr_neutral.ops` holds the pipelines (`tools/blender/bake_view_luts.py`). The LUT output is
//! linear and encoded after interpolation, which `lut::ViewLut` cannot express, so [`BAKED`]
//! stays empty.

use crate::display::ocio::{DataFile, OpsFile};
use crate::lut::BakedLut;

/// Khronos PBR Neutral single-LUT views: none (see the module docs).
pub const BAKED: &[BakedLut] = &[];

/// The Khronos PBR Neutral op pipelines: every look (the view exists on sRGB only).
pub(crate) const PIPELINES: OpsFile = include_str!("../../../assets/luts/pbr_neutral.ops");

/// LUT data of the Khronos PBR Neutral pipelines.
pub(crate) const DATA: &[DataFile] = &[(
    "pbr_neutral",
    include_bytes!("generated/pbr_neutral.cube.deflate"),
)];
