// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-material` (layer L7): `lower(NodeTree) -> LoweredMaterial` with one pattern
//! recogniser per file (mirroring io_scene_gltf2 `exp/material/*`), and `raise` for the
//! importer (ARCHITECTURE §16.2). Both the glTF exporter and the renderer consume
//! `LoweredMaterial`: whatever exports correctly renders correctly.
//!
//! Ownership (M1): WP 1.11 owns everything except this file. CONTRACT: `lowered.rs`.
#![deny(unsafe_code)]

mod lowered;

/// Pattern recognisers, one file per glTF material feature (`patterns/<feature>.rs`).
pub mod patterns {
    automod::dir!(pub "src/patterns");
}

pub use lowered::*;
