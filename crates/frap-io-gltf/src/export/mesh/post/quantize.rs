// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_mesh_quantization` (WP 7.1; M7 W0 stub): `frap_mesh_quantization` and the quantization
//! of the `export_gltfpack_*` mapping (`gltfpack.rs`) write POSITION / NORMAL / TANGENT /
//! TEXCOORD_n (and morph targets) as integer columns through
//! [`PrimitiveEncoding::attributes`](crate::export::mesh::ext::PrimitiveEncoding) and list the
//! extension (required) through `PrimitiveEncoding::extensions_used`. A dequantization
//! transform on the mesh nodes goes through [`node_transform`], which `compress.rs` asks.
//!
//! Until WP 7.1 the post-processor leaves every draft unchanged (the operator warns about
//! `frap_mesh_quantization` as not supported: no `SUPPORTED_EXPORT_PROPS` entry yet).

use frap_gltf::BufferBuilder;
use frap_gltf::json::{Index, Node};

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

/// The dequantization transform of glTF mesh `mesh` on its node `node` (WP 7.1; called by
/// `compress::mesh_node` for every node with a mesh). The stub leaves the node unchanged.
pub fn node_transform(_g: &mut Gather, _mesh: Index, _node: &mut Node) -> Result<(), ExportError> {
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static QUANTIZE: PrimitivePostProcess = PrimitivePostProcess {
    name: "quantize",
    order: 200,
    run,
};
