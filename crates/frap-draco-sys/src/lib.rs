// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-draco-sys`: the cc-built Google Draco fallback of `frap_codec_mesh::draco`
//! (ARCHITECTURE §3.6: feature `draco-sys`, off by default; no cmake, no network in `build.rs`).
//!
//! M7 W0 stub (decision log M7W0-5): WP 7.2 fills this crate (vendored Draco sources, a `cc`
//! build script and a safe wrapper) only if the draco-core spike gate fails; until then it
//! links nothing and [`AVAILABLE`] is `false`.
#![deny(unsafe_code)]

/// Whether this build carries the Google Draco encoder/decoder.
pub const AVAILABLE: bool = false;
