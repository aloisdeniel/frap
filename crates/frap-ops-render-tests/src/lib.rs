// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-render-tests`: a test-only crate (ARCHITECTURE rule 2.2.11, §3.1). Behaviour tests
//! of `render.render` that need a full `Session` (and a GPU) live in `tests/*.rs` here:
//!
//! - `render_goldens.rs`: the Workbench self-goldens of `tests/golden/cases/render/m1/` (SSIM
//!   ≥ 0.995 against the frap-blessed PNG, gating; SSIM against Blender's Workbench reported);
//! - `render_op.rs`: errors, `GPU_UNAVAILABLE`, `Render Result`, output paths and formats,
//!   engine fallback WARNINGs, and that rendering changes nothing else in the session.
//!
//! Owned by WP 1.24 in M1. This library is intentionally empty.
#![deny(unsafe_code)]
