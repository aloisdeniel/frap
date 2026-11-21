// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshoptCodec` (ARCHITECTURE §17.2): the buffer-view codec of `EXT_meshopt_compression` and
//! `KHR_meshopt_compression` (vertex codec version 0 / 1), with the attribute filters.
//!
//! CONTRACT (M7 W0, decision log M7W0-5): the trait, [`MeshoptMode`], [`MeshoptFilter`],
//! [`MeshoptVersion`] and [`codec`]. WP 7.1 owns this file: the meshoptimizer backend (feature
//! `meshopt`), filter encoding helpers, and any additive items. The add-on's encoder is
//! `io/exp/meshopt.py` over Blender's `bf_intern_meshopt_bridge` (meshoptimizer), so a
//! meshoptimizer backend gives the add-on's bytes.

use crate::error::CodecError;

/// `mode` of a compressed buffer view.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MeshoptMode {
    /// `ATTRIBUTES`: vertex data (`byteStride` bytes per element).
    Attributes,
    /// `TRIANGLES`: triangle-list indices.
    Triangles,
    /// `INDICES`: any other index sequence.
    Indices,
}

impl MeshoptMode {
    /// The glTF identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            MeshoptMode::Attributes => "ATTRIBUTES",
            MeshoptMode::Triangles => "TRIANGLES",
            MeshoptMode::Indices => "INDICES",
        }
    }
}

/// `filter` of a compressed buffer view (applied after decoding).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MeshoptFilter {
    /// `NONE`
    #[default]
    None,
    /// `OCTAHEDRAL` (normals, tangents)
    Octahedral,
    /// `QUATERNION` (rotations)
    Quaternion,
    /// `EXPONENTIAL` (floats with a shared exponent)
    Exponential,
    /// `COLOR` (`KHR_meshopt_compression` only)
    Color,
}

impl MeshoptFilter {
    /// The glTF identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            MeshoptFilter::None => "NONE",
            MeshoptFilter::Octahedral => "OCTAHEDRAL",
            MeshoptFilter::Quaternion => "QUATERNION",
            MeshoptFilter::Exponential => "EXPONENTIAL",
            MeshoptFilter::Color => "COLOR",
        }
    }
}

/// Vertex-codec bitstream version: 0 for `EXT_meshopt_compression`, 1 for
/// `KHR_meshopt_compression` (the add-on's `encodeVertexVersion`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum MeshoptVersion {
    /// `EXT_meshopt_compression`
    #[default]
    Ext,
    /// `KHR_meshopt_compression`
    Khr,
}

/// The meshopt buffer-view codec.
pub trait MeshoptCodec: Send + Sync {
    /// Encodes `count` vertices of `stride` bytes each (`data.len() == count * stride`).
    fn encode_vertex(
        &self,
        data: &[u8],
        count: usize,
        stride: usize,
        version: MeshoptVersion,
    ) -> Result<Vec<u8>, CodecError>;

    /// Encodes indices of `index_size` (2 or 4) bytes; `mode` is `Triangles` or `Indices`.
    fn encode_index(
        &self,
        indices: &[u32],
        vertex_count: usize,
        index_size: usize,
        mode: MeshoptMode,
    ) -> Result<Vec<u8>, CodecError>;

    /// Decodes a compressed view into `count * stride` bytes and applies `filter`.
    fn decode(
        &self,
        data: &[u8],
        count: usize,
        stride: usize,
        mode: MeshoptMode,
        filter: MeshoptFilter,
    ) -> Result<Vec<u8>, CodecError>;
}

/// The codec of this build, `None` without a meshopt backend (WP 7.1).
pub fn codec() -> Option<&'static dyn MeshoptCodec> {
    None
}
