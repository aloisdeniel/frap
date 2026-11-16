// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-render-scene` (layer L8): `types` (plain-data `RenderScene`, `Film`, `RenderError`,
//! the `RenderService` trait; no wgpu) and `build` (evaluated scene → `RenderScene` through the
//! `Evaluator` trait) (ARCHITECTURE §19.1–§19.2).
//!
//! Ownership (M1): `src/build/**` is WP 1.15's; `src/types.rs` is the CONTRACT frozen at M1 W0.
#![deny(unsafe_code)]

pub mod build;
pub mod types;

pub use build::build;
pub use types::*;
