// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-color` (layer L1): colour spaces, sRGB transfer, view transforms (3D LUTs over a log2
//! shaper baked from Blender), looks, exposure/gamma, display encode and dither. CPU only and
//! deterministic, so colour goldens need no GPU (ARCHITECTURE §19.5).
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.6.
#![deny(unsafe_code)]

pub mod display;
pub mod lut;
pub mod spaces;
