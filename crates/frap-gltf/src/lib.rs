// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-gltf` (layer L3): glTF 2.0 serde model (lossless for unknown extensions/extras), GLB
//! container, accessor codec, `BufferBuilder`, validator, one file per typed extension.
//! Blender-agnostic (ARCHITECTURE §17.1).
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.5.
#![deny(unsafe_code)]

pub mod accessor;
pub mod builder;
pub mod glb;
pub mod json;
pub mod validate;

/// Typed extensions, one file each (`ext/<extension_name_lowercase>.rs`).
pub mod ext {
    automod::dir!(pub "src/ext");
}

pub use builder::BufferBuilder;
