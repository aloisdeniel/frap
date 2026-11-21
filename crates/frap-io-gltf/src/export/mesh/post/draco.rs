// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_draco_mesh_compression` (WP 7.2; M7 W0 stub): `export_draco_mesh_compression_enable`,
//! `export_draco_mesh_compression_level`, `export_draco_*_quantization`, `frap_draco_backend`.
//! The post-processor encodes the draft through `frap_codec_mesh::draco::codec`, pushes the
//! bitstream as a buffer view, and sets [`PrimitiveEncoding`] `without_buffer_views`,
//! `extensions` (`KHR_draco_mesh_compression`) and `extensions_used` (required), as the
//! add-on's `io/exp/draco.py` (`encode_scene_primitives`) and `exporter.py`
//! (`add_draco_extension`) do.
//!
//! Until WP 7.2 the draft is written uncompressed (the operator warns about the props).
//!
//! [`PrimitiveEncoding`]: crate::export::mesh::ext::PrimitiveEncoding

use frap_gltf::BufferBuilder;

use crate::export::ExportError;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_POST_PROCESS, PrimitiveDraft, PrimitivePostProcess,
};

fn run(
    _cx: &MeshExportCx,
    _draft: &mut PrimitiveDraft,
    _buffer: &mut BufferBuilder,
) -> Result<(), ExportError> {
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static DRACO: PrimitivePostProcess = PrimitivePostProcess {
    name: "draco",
    order: 400,
    run,
};
