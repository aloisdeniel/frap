// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-render` (layer L9): wgpu device (lazy), offscreen targets, readback, the Workbench and
//! EEVEE-like engines behind `trait Engine`, GPU cache, contact-sheet compositor; implements
//! `frap_render_scene::RenderService`. The only crate that depends on `wgpu`; it never names a
//! `frap-scene` type (ARCHITECTURE §19).
//!
//! Ownership (M1): WP 1.7 owns everything except this file. CONTRACT (frozen at M1 W0):
//! `new_service`, `Engine`, `Gpu`, `Gpu::new` (in `gpu.rs`, `engine.rs`, `service.rs`).
#![deny(unsafe_code)]

pub mod composite;
mod engine;
mod gpu;
pub mod readback;
mod service;
pub mod workbench;

pub use engine::{Engine, GpuCache};
pub use frap_render_scene::GpuOptions;
pub use gpu::Gpu;
pub use service::{Renderer, new_service};
