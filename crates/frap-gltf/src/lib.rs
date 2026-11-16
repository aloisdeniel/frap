// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-gltf` (layer L3): glTF 2.0 serde model (lossless for unknown extensions/extras), GLB
//! container, accessor codec, `BufferBuilder`, validator, one file per typed extension.
//! Blender-agnostic (ARCHITECTURE §17.1).
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.5.
//!
//! - [`json`]: the document model ([`Document`], [`Extensions`], [`Extension`]). Every object
//!   keeps unknown extensions (as JSON values) and `extras` (as raw JSON text), and every
//!   optional property keeps whether it was present, so `read → write` reproduces the JSON.
//! - [`glb`]: the binary container ([`read_glb`], [`write_glb`]) and [`read`], which accepts
//!   either a GLB or a `.gltf` JSON file.
//! - [`accessor`]: decode/encode of accessor data (strides, sparse, normalized and quantized
//!   integers, matrix column padding, min/max) and buffer/data-URI loading.
//! - [`primitive`]: primitive topology (POINTS … TRIANGLE_FAN) to points, edges and triangles.
//! - [`builder`]: [`BufferBuilder`], which lays out buffer views and accessors.
//! - [`mod@validate`]: structural and data validation; typed extensions add their own checks
//!   through [`json::EXTENSIONS`]. [`validate_file`] validates file bytes, JSON-text checks
//!   included.
#![deny(unsafe_code)]

pub mod accessor;
pub mod builder;
pub mod error;
pub mod glb;
pub mod json;
pub mod primitive;
pub mod validate;

/// Typed extensions, one file each (`ext/<extension_name_lowercase>.rs`); each registers an
/// [`json::ExtensionDef`] in [`json::EXTENSIONS`].
pub mod ext {
    automod::dir!(pub "src/ext");
}

pub use accessor::{AccessorData, Components, load_buffers, read_accessor};
pub use builder::{AccessorOpts, BufferBuilder, repack};
pub use error::GltfError;
pub use glb::{Glb, WriteOpts, read, read_glb, write_glb};
pub use json::{Document, Extension, Extensions, Extras};
pub use validate::{Issue, Report, Severity, validate, validate_data, validate_file, validate_glb};
