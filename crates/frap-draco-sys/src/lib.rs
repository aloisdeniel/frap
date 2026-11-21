// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-draco-sys`: the cc-built Google Draco fallback of `frap_codec_mesh::draco`
//! (ARCHITECTURE §3.6: feature `draco-sys`, off by default; no cmake, no network in `build.rs`).
//!
//! WP 7.2 spike gate (§22.9): the pure-Rust draco-core backend passed (its bitstreams are byte
//! for byte Blender 5.2.4's — Google Draco 1.5.7 — for every `export_draco_*` setting and
//! attribute kind the add-on writes, and it decodes Blender's files within 1e-6 of Blender's
//! decoder; frap-io-gltf `export/mesh/post/draco.rs` and `import/decompress/draco.rs` tests,
//! `tests/golden/cases/gltf/draco`). So this crate stays empty: it vendors no Draco sources,
//! links nothing, and [`AVAILABLE`] is `false`; `frap_draco_backend='SYS'` reports that the
//! fallback is not built. Filling it (vendored Draco 1.5.7, a `cc` build script, a safe
//! wrapper) is only needed if draco-core regresses.
#![deny(unsafe_code)]

/// Whether this build carries the Google Draco encoder/decoder.
pub const AVAILABLE: bool = false;
