// SPDX-License-Identifier: GPL-3.0-or-later
//! Writing (ARCHITECTURE §17.2): the asset, the scene, the buffer, then the file in the
//! requested format.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/io/exp/export.py (`save_gltf`:
//! top-level key order, compact JSON for GLB, tab-indented JSON with `,`/`:` separators and a
//! final newline for `.gltf`, the GLB container with no BIN chunk when there is no binary
//! data) and blender/exp/exporter.py (`finalize_buffer`: no buffer at all when it is empty;
//! `GLTF_SEPARATE` writes `<file stem>.bin`, `GLTF_EMBEDDED` a base64 data URI). Empty arrays
//! and absent members are dropped by the `frap-gltf` model; its formatter writes integral
//! numbers as integers (`__fix_json`).

use frap_gltf::accessor::encode_data_uri;
use frap_gltf::json::{Asset, Index, Scene};
use frap_gltf::{Glb, WriteOpts, write_glb};

use crate::export::gather::Gather;
use crate::export::images::path_to_uri;
use crate::export::{ExportError, ExportFormat, ExportOutput};

/// Finishes the document whose scene roots are `roots` and serializes it.
pub fn finish(mut g: Gather, roots: Vec<Index>) -> Result<ExportOutput, ExportError> {
    let s = g.settings;
    g.doc.asset = Asset {
        generator: Some(s.generator.clone()),
        copyright: (!s.export_copyright.is_empty()).then(|| s.export_copyright.clone()),
        ..Asset::default()
    };
    let scene_name = if s.collection.is_empty() {
        g.ctx.scene().header.name.to_string()
    } else {
        s.collection.clone()
    };
    g.doc.scenes.push(Scene {
        name: Some(scene_name),
        nodes: roots,
        ..Scene::default()
    });
    g.doc.scene = Some(0);

    let buf = std::mem::take(&mut g.buf);
    let mut files = std::mem::take(&mut g.files);
    let bin: Option<Vec<u8>> = if buf.bytes.is_empty() {
        // Nothing in the buffer: no buffer object (views and accessors, if any, have none).
        let mut buf = buf;
        g.doc.buffer_views.append(&mut buf.views);
        g.doc.accessors.append(&mut buf.accessors);
        None
    } else {
        let uri = match s.export_format {
            ExportFormat::Glb => None,
            ExportFormat::GltfSeparate => Some(path_to_uri(&format!("{}.bin", s.file_stem))),
            // The data URI needs the final bytes: patched below.
            ExportFormat::GltfEmbedded => Some(String::new()),
        };
        let bytes = buf.finish(&mut g.doc, uri);
        match s.export_format {
            ExportFormat::Glb => Some(bytes),
            ExportFormat::GltfSeparate => {
                files.insert(0, (format!("{}.bin", s.file_stem), bytes));
                None
            }
            ExportFormat::GltfEmbedded => {
                g.doc.buffers[0].uri = Some(encode_data_uri("application/octet-stream", &bytes));
                None
            }
        }
    };

    let nodes = g.doc.nodes.len();
    let meshes = g.doc.meshes.len();
    let bytes = match s.export_format {
        ExportFormat::Glb => write_glb(
            &Glb {
                document: g.doc.clone(),
                bin,
            },
            &WriteOpts::default(),
        ),
        ExportFormat::GltfSeparate | ExportFormat::GltfEmbedded => {
            let mut v = g.doc.to_vec(&WriteOpts {
                indent: Some("\t".to_owned()),
            });
            v.push(b'\n');
            v
        }
    };
    Ok(ExportOutput {
        bytes,
        files,
        extensions_used: g.doc.extensions_used.clone(),
        nodes,
        meshes,
        reports: g.reports,
        document: g.doc,
    })
}
