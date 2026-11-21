// SPDX-License-Identifier: GPL-3.0-or-later
//! meshopt buffer-view decoding (WP 7.1; M7 W0 stub): every view with
//! `EXT_meshopt_compression` / `KHR_meshopt_compression` is decoded through
//! `frap_codec_mesh::meshopt::codec` and re-pointed at the decoded bytes, as
//! `io/imp/gltf2_io_binary_meshopt.py` does. The stub leaves the views compressed.

use frap_gltf::Document;
use frap_rna::Report;

use crate::import::ImportError;

/// Decodes the compressed views of `doc` (see the module docs).
pub(super) fn decode_views(
    _doc: &mut Document,
    _buffers: &mut Vec<Vec<u8>>,
    _reports: &mut Vec<Report>,
) -> Result<(), ImportError> {
    Ok(())
}
