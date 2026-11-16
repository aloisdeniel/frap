// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-golden` (layer L12, dev only): the `GoldenState` schema, the frap-side runner (snapshot
//! through frap's own RNA), the Blender runner, comparators (one file each), fixtures and bless
//! (ARCHITECTURE §20.2–§20.5).
//!
//! Ownership (M1): WP 1.8 owns everything except this file. CONTRACT (frozen at M1 W0):
//! `state.rs` (`GoldenState`).
#![deny(unsafe_code)]

pub mod blender;
pub mod case;
pub mod runner;
mod state;

/// Comparators, one file each (`lines`, `rna_values`, `mesh_exact`, `mesh_canonical`,
/// `invariants`, `gltf_canonical`, `image_ssim`, `gltf_roundtrip`).
pub mod compare {
    automod::dir!(pub "src/compare");
}

pub use state::*;
