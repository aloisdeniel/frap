// SPDX-License-Identifier: GPL-3.0-or-later
//! Compression wiring (ARCHITECTURE §17.2 `compress.rs`; M7 W0, decision log M7W0-6): the
//! integrator-owned call sites of the M7 codecs outside the primitive post-processors, so the
//! W1 work packages never edit `write.rs` or `nodes.rs`:
//!
//! - [`finish_buffers`]: buffer-level meshopt compression (WP 7.1,
//!   `mesh/post/meshopt.rs::compress_buffer_views`), before the buffer is finished;
//! - [`mesh_node`]: the dequantization transform of a quantized mesh on its node (WP 7.1,
//!   `mesh/post/quantize.rs::node_transform`).
//!
//! Draco runs per primitive (`mesh/post/draco.rs`); `export_use_gltfpack` post-processes the
//! written file (`gltfpack.rs`, called by `export()`).

use frap_gltf::json::{Index, Node};

use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::mesh::post::{meshopt, quantize};

/// Buffer-level compression, once per export, before `BufferBuilder::finish`.
pub fn finish_buffers(g: &mut Gather) -> Result<(), ExportError> {
    meshopt::compress_buffer_views(g)
}

/// Called for every node that gets glTF mesh `mesh`, after its TRS.
pub fn mesh_node(g: &mut Gather, mesh: Index, node: &mut Node) -> Result<(), ExportError> {
    quantize::node_transform(g, mesh, node)
}
