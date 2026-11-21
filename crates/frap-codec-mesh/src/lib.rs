// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-codec-mesh` (layer L3): mesh compression codecs behind frap traits (ARCHITECTURE §3.2,
//! §17.2): [`draco::DracoCodec`] (draco-core, fallback `frap-draco-sys`) and
//! [`meshopt::MeshoptCodec`] (meshoptimizer). glTF- and Blender-agnostic: `frap-io-gltf`
//! maps accessors and buffer views onto [`MeshData`] and byte slices.
//!
//! M7 W0 (decision log M7W0-5) froze the contract: [`mesh`] (the data a codec sees),
//! [`error`], the two traits and their backend selection. Ownership: `meshopt.rs` WP 7.1;
//! `draco.rs`, `draco_core.rs`, `draco_sys.rs` WP 7.2; this file, `mesh.rs` and `error.rs` the
//! integrator. Backends are cargo features (`meshopt`, `draco`, `draco-sys`, §3.6): without
//! one, the selection functions answer `None` and the caller reports the feature as missing.
#![deny(unsafe_code)]

pub mod draco;
/// The draco-core backend of [`draco::DracoCodec`] (feature `draco`, WP 7.2).
pub mod draco_core;
/// The `frap-draco-sys` backend of [`draco::DracoCodec`] (feature `draco-sys`, WP 7.2).
pub mod draco_sys;
pub mod error;
pub mod mesh;
pub mod meshopt;

pub use draco::{DracoBackend, DracoCodec, DracoEncoded, DracoQuantization};
pub use error::CodecError;
pub use mesh::{AttrData, Attribute, AttributeKind, MeshData};
pub use meshopt::{MeshoptCodec, MeshoptFilter, MeshoptMode, MeshoptVersion};
