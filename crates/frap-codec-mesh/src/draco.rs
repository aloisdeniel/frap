// SPDX-License-Identifier: GPL-3.0-or-later
//! `DracoCodec` (ARCHITECTURE §17.2): `KHR_draco_mesh_compression` encode / decode.
//!
//! CONTRACT (M7 W0, decision log M7W0-5): the trait, [`DracoQuantization`], [`DracoEncoded`],
//! [`DracoBackend`] and [`codec`]; WP 7.2 added [`DracoCodec::encode_preserving_order`],
//! [`DracoCodec::decode_counted`] / [`DracoDecoded`] and the [`DracoBackend`] helpers. WP 7.2
//! owns this file, `draco_core.rs` (draco-core, feature `draco`) and `draco_sys.rs`
//! (`frap-draco-sys`, feature `draco-sys`). The spike gate of §22.9 passed (draco-core is
//! byte-identical to Blender's encoder and decodes its files), so `DracoBackend::Auto` picks
//! draco-core. The add-on's encoder is `io/exp/draco.py` over Blender's
//! `bf_intern_draco_bridge` (Google Draco 1.5.7).

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

/// A decoded bitstream with the point count of its mesh (`decoderGetVertexCount`, known even
/// when no attribute is requested).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DracoDecoded {
    /// Indices and the requested attributes.
    pub mesh: MeshData,
    /// `mesh->num_points()`.
    pub point_count: u32,
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

    /// [`DracoCodec::encode`] keeping the points and triangles in input order
    /// (`MESH_SEQUENTIAL_ENCODING`: the bridge's `preserveTriangleOrder`, which the add-on sets
    /// for primitives with morph targets, whose accessors stay uncompressed). WP 7.2, additive.
    fn encode_preserving_order(
        &self,
        mesh: &MeshData,
        quantization: &DracoQuantization,
        level: u8,
    ) -> Result<DracoEncoded, CodecError>;

    /// Decodes a bitstream; `attribute_ids` are the extension's ids in the order the caller
    /// wants the attributes back. The decoded attributes keep the bitstream's data types; an
    /// id the bitstream lacks is a [`CodecError::Decode`].
    fn decode(&self, bytes: &[u8], attribute_ids: &[u32]) -> Result<MeshData, CodecError>;

    /// [`DracoCodec::decode`] with the point count (WP 7.2, additive).
    fn decode_counted(
        &self,
        bytes: &[u8],
        attribute_ids: &[u32],
    ) -> Result<DracoDecoded, CodecError> {
        let mesh = self.decode(bytes, attribute_ids)?;
        let point_count = mesh.vertex_count() as u32;
        Ok(DracoDecoded { mesh, point_count })
    }
}

/// The codec `backend` selects in this build; `None` when it is not built in.
///
/// Spike gate (§22.9, WP 7.2): draco-core writes Blender's bytes and decodes Blender's files,
/// so `Auto` is draco-core; `frap-draco-sys` (Google Draco, feature `draco-sys`) is the
/// fallback `Auto` uses only in a build without draco-core.
pub fn codec(backend: DracoBackend) -> Option<&'static dyn DracoCodec> {
    match backend {
        DracoBackend::Auto => crate::draco_core::codec().or_else(crate::draco_sys::codec),
        DracoBackend::Core => crate::draco_core::codec(),
        DracoBackend::Sys => crate::draco_sys::codec(),
    }
}

impl DracoBackend {
    /// Parses a `frap_draco_backend` identifier.
    pub fn from_identifier(s: &str) -> Option<Self> {
        match s {
            "AUTO" => Some(DracoBackend::Auto),
            "CORE" => Some(DracoBackend::Core),
            "SYS" => Some(DracoBackend::Sys),
            _ => None,
        }
    }

    /// The `frap_draco_backend` identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            DracoBackend::Auto => "AUTO",
            DracoBackend::Core => "CORE",
            DracoBackend::Sys => "SYS",
        }
    }

    /// Why [`codec`] answers `None` for this backend.
    pub fn unavailable(self) -> CodecError {
        match self {
            DracoBackend::Auto | DracoBackend::Core => CodecError::Unavailable {
                codec: "Draco",
                feature: "draco",
            },
            DracoBackend::Sys if crate::draco_sys::BUILT_WITH_FEATURE => {
                CodecError::NotImplemented(
                    "Draco: frap_draco_backend='SYS' is the Google Draco fallback, which this \
                     build does not carry (the draco-core spike gate passed, so frap-draco-sys \
                     is not filled); use 'AUTO' or 'CORE'"
                        .into(),
                )
            }
            DracoBackend::Sys => CodecError::Unavailable {
                codec: "Draco (frap_draco_backend='SYS')",
                feature: "draco-sys",
            },
        }
    }
}
