// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-geom` (layer L1): `BLI_polyfill_2d` + beautify, Newell normals, corner/face/vertex
//! normals (`mesh_normals.cc` port), kd-tree, spatial hash for welding (ARCHITECTURE §3.2).
//! MikkTSpace (`bevy_mikktspace`) and CDT (`spade`) arrive with their milestone's W0.
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.4. A WP that needs another module
//! declares it as a submodule of one of these files.
#![deny(unsafe_code)]

pub mod kdtree;
pub mod normals;
pub mod polyfill;
pub mod spatial_hash;
