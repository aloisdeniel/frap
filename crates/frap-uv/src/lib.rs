// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-uv` (layer L3): Blender's UV parametrizer on its own [`ParamMesh`] (the port of
//! `ParamHandle`, `blender/source/blender/geometry/GEO_uv_parametrizer.hh`): chart construction
//! from faces and seams, LSCM (CONFORMAL), ABF++ and SLIM (M8, WP 8.1), stretch minimisation,
//! averaging of island scales, the island packer (`GEO_uv_pack.hh`), and the projection
//! helpers of `uvedit_unwrap_ops.cc` (cube, cylinder, sphere, smart project) on plain arrays.
//!
//! The crate sits below `frap-mesh` and `frap-bmesh`; `frap-ops-uv` (WP 3.8) feeds it from the
//! edit BMesh and writes the UVs back (`uv` slots are indices into the caller's corner UV
//! array, Blender's `float **uv` pointers).
//!
//! Ownership (M3): WP 3.3 owns this crate (`crates/frap-uv/**`) after W0. The [`ParamMesh`]
//! API and the [`pack`]/[`project`]/[`smart`] signatures are the contract WP 3.8 builds on;
//! WP 3.3 may add items and fields to `#[non_exhaustive]` structs.
#![deny(unsafe_code)]

pub mod abf;
pub mod average;
pub mod lscm;
pub mod pack;
pub mod param;
pub mod project;
pub mod slim;
pub mod smart;
pub mod stretch;

pub use pack::{MarginMethod, PackIsland, PackParams, PinMethod, RotationMethod, ShapeMethod};
pub use param::{ConstructOpts, ParamFace, ParamKey, ParamMesh, PhState, SolveCount};
