// SPDX-License-Identifier: GPL-3.0-or-later
//! `DracoCodec` (ARCHITECTURE §17.2): `KHR_draco_mesh_compression` encode / decode.
//!
//! CONTRACT (M7 W0, decision log M7W0-5): the trait, [`DracoQuantization`], [`DracoEncoded`],
//! [`DracoBackend`] and [`codec`]. WP 7.2 owns this file, `draco_core.rs` (draco-core,
//! feature `draco`) and `draco_sys.rs` (`frap-draco-sys`, feature `draco-sys`): the spike gate
//! of §22.9 decides which backend `DracoBackend::Auto` picks. The add-on's encoder is
//! `io/exp/draco.py` over Blender's `bf_intern_draco_bridge` (Google Draco).

use crate::error::CodecError;
use crate::mesh::MeshData;

/// Quantization bits per attribute kind (the `export_draco_*_quantization` props; 0 = lossless
/// for that kind) and the compression level (`export_draco_mesh_compression_level`, 0–10).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DracoQuantization {
    /// `export_draco_position_quantization` (default 14)
    pub position: u8,
    /// `export_draco_normal_quantization` (default 10)
    pub normal: u8,
    /// `export_draco_texcoord_quantization` (default 12)
    pub texcoord: u8,
    /// `export_draco_color_quantization` (default 10)
    pub color: u8,
    /// `export_draco_generic_quantization` (default 12)
    pub generic: u8,
}

impl Default for DracoQuantization {
    /// The add-on's defaults.
    fn default() -> Self {
        DracoQuantization {
            position: 14,
            normal: 10,
            texcoord: 12,
            color: 10,
            generic: 12,
        }
    }
}

/// An encoded primitive.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DracoEncoded {
    /// The Draco bitstream (one buffer view).
    pub bytes: Vec<u8>,
    /// The Draco unique id of each input attribute, in input order (the extension's
    /// `attributes` map).
    pub attribute_ids: Vec<u32>,
    /// Points (vertices) the decoder produces (the accessors' `count`).
    pub point_count: u32,
    /// Indices the decoder produces (the index accessor's `count`).
    pub index_count: u32,
}

/// The backend to use (`frap_draco_backend`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum DracoBackend {
    /// `AUTO`: the spike gate's choice among the backends of this build.
    #[default]
    Auto,
    /// `CORE`: draco-core.
    Core,
    /// `SYS`: Google Draco (`frap-draco-sys`).
    Sys,
}

/// The Draco codec.
pub trait DracoCodec: Send + Sync {
    /// Encodes a primitive at `level` (0–10, the add-on's `compression_level`).
    fn encode(
        &self,
        mesh: &MeshData,
        quantization: &DracoQuantization,
        level: u8,
    ) -> Result<DracoEncoded, CodecError>;

    /// Decodes a bitstream; `attribute_ids` are the extension's ids in the order the caller
    /// wants the attributes back. The decoded attributes keep the bitstream's data types.
    fn decode(&self, bytes: &[u8], attribute_ids: &[u32]) -> Result<MeshData, CodecError>;
}

/// The codec `backend` selects in this build (WP 7.2); `None` when it is not built in.
pub fn codec(backend: DracoBackend) -> Option<&'static dyn DracoCodec> {
    match backend {
        DracoBackend::Auto | DracoBackend::Core => crate::draco_core::codec(),
        DracoBackend::Sys => crate::draco_sys::codec(),
    }
}
