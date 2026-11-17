// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-project` (layer L9): the `.frap` container: deterministic zip, manifest,
//! content-addressed buffers/images, scene mapping, migrations (ARCHITECTURE §15).
//!
//! Ownership (M1): `container/`, `manifest/`, `buffers/`, `migrate/` WP 1.9; `scene_map/`
//! WP 1.23; this file the integrator.
#![deny(unsafe_code)]

pub mod buffers;
pub mod container;
pub mod manifest;
pub mod migrate;
pub mod scene_map;
