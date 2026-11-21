// SPDX-License-Identifier: GPL-3.0-or-later
//! Decompression before the scene is built (M7 W0 dispatcher, decision log M7W0-6; the plan's
//! `decompress.rs` of WP 7.2 split by codec so WPs 7.1 and 7.2 own one file each):
//!
//! - [`meshopt`] (WP 7.1): `EXT_meshopt_compression` / `KHR_meshopt_compression` buffer views
//!   decoded into plain views of a new buffer (`io/imp/gltf2_io_binary_meshopt.py`);
//! - [`draco`] (WP 7.2): `KHR_draco_mesh_compression` primitives decoded into new accessors,
//!   the extension removed from the primitive
//!   (`blender/imp/draco_compression_extension.py::decode_primitive`).
//!
//! The mesh importer then reads ordinary accessors. Until the WPs land, compressed data is
//! left as is and the importer answers its M1 "imported from M7" errors.

use frap_gltf::Document;
use frap_rna::Report;

use super::ImportError;

/// `EXT_/KHR_meshopt_compression` (WP 7.1).
mod meshopt;

/// `KHR_draco_mesh_compression` (WP 7.2).
mod draco;

/// Decodes every compressed view and primitive of `doc` in place (`buffers` gains the decoded
/// data). Returns the reports to add (e.g. a WARNING for a codec this build lacks).
pub(super) fn run(
    doc: &mut Document,
    buffers: &mut Vec<Vec<u8>>,
) -> Result<Vec<Report>, ImportError> {
    let mut reports = Vec::new();
    meshopt::decode_views(doc, buffers, &mut reports)?;
    draco::decode_primitives(doc, buffers, &mut reports)?;
    Ok(reports)
}
