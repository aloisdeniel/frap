// SPDX-License-Identifier: GPL-3.0-or-later
//! Draco primitive decoding (WP 7.2; M7 W0 stub): every primitive with
//! `KHR_draco_mesh_compression` is decoded through `frap_codec_mesh::draco::codec`, its
//! attributes and indices re-pointed at new accessors over the decoded data and the extension
//! removed, as the add-on's `blender/imp/draco_compression_extension.py::decode_primitive`
//! does. The stub leaves the primitives compressed (the mesh importer then answers its M1
//! "imported from M7" error).

use frap_gltf::Document;
use frap_rna::Report;

use crate::import::ImportError;

/// Decodes the Draco primitives of `doc` (see the module docs).
pub(super) fn decode_primitives(
    _doc: &mut Document,
    _buffers: &mut Vec<Vec<u8>>,
    _reports: &mut Vec<Report>,
) -> Result<(), ImportError> {
    Ok(())
}
