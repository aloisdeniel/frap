// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-color` (layer L1): colour spaces, sRGB transfer, view transforms (3D LUTs over a log2
//! shaper baked from Blender), looks, exposure/gamma, display encode and dither. CPU only and
//! deterministic, so colour goldens need no GPU (ARCHITECTURE §19.5).
//!
//! Entry point: [`display_transform`] (scene-linear film → [`DisplayImage`]), then
//! [`DisplayImage::to_rgba8`] (with `render.dither_intensity`) or [`DisplayImage::to_rgba16`].
//! Standard and Raw are analytic; AgX is a LUT baked by `tools/blender/bake_view_luts.py` and
//! checked against Blender on a held-out colour set (`tests/heldout.rs`, CIEDE2000 < 1).
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.6. M5+M6 W0: one module per view
//! transform family (`agx`, `filmic`, `pbr_neutral`, `aces`, `false_color`) and `looks`, owned
//! by WP 5.3; `lut::baked` reads their `BAKED` lists.
#![deny(unsafe_code)]

pub mod aces;
pub mod agx;
pub mod delta_e;
pub mod display;
pub mod false_color;
pub mod filmic;
pub mod looks;
pub mod lut;
pub mod pbr_neutral;
pub mod spaces;

pub use display::{
    ColorError, DisplayDevice, DisplayImage, DisplayProcessor, DisplaySettings, ViewSettings,
    ViewTransform, display_transform,
};
pub use spaces::ColorSpace;
