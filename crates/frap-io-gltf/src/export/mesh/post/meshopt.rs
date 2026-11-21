// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_meshopt_compression` / `KHR_meshopt_compression` (WP 7.1; M7 W0 stub):
//! `export_meshopt_compression_enable`, `export_meshopt_extension`. The add-on compresses at
//! the buffer level (`blender/exp/exporter.py::finalize_buffer`, `io/exp/meshopt.py`): the
//! views go to a second buffer compressed, the first becomes a `fallback` buffer without data.
//! `compress.rs` calls [`compress_buffer_views`] once, before the buffer is finished; the
//! per-primitive post-processor can record what each view holds (mode, filter) in the export's
//! state (`Gather::state`), since only it sees the drafts.
//!
//! Until WP 7.1 nothing is compressed (the operator warns about the props as not supported).

use frap_gltf::BufferBuilder;

use crate::export::ExportError;
use crate::export::gather::Gather;
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

/// Compresses the buffer views of the export (WP 7.1). The stub leaves them unchanged.
pub fn compress_buffer_views(_g: &mut Gather) -> Result<(), ExportError> {
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static MESHOPT: PrimitivePostProcess = PrimitivePostProcess {
    name: "meshopt",
    order: 300,
    run,
};
