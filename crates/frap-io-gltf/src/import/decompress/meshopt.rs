// SPDX-License-Identifier: GPL-3.0-or-later
//! meshopt buffer-view decoding (WP 7.1): every view with `EXT_meshopt_compression` /
//! `KHR_meshopt_compression` is decoded through `frap_codec_mesh::meshopt::codec` and
//! re-pointed at the decoded bytes, as `io/imp/gltf2_io_binary_meshopt.py`
//! (`MeshoptDecoder.get_buffer_view`) does before any accessor is read.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/io/imp/gltf2_io_binary_meshopt.py
//! (Apache-2.0, The glTF-Blender-IO authors): the `EXT_` extension is looked up first, then the
//! `KHR_` one; the stream is `buffers[ext.buffer][byteOffset .. byteOffset + byteLength]`;
//! `ATTRIBUTES` / `TRIANGLES` / `INDICES` select the vertex, index-buffer or index-sequence
//! decoder; a filter applies to `ATTRIBUTES` only. Differences, all toward the specification:
//! `COLOR` is decoded with meshoptimizer's colour filter (the add-on runs the exponential
//! filter on it, `decodeFilterExp`, which garbles the colours); a decoded `ATTRIBUTES` view
//! gets the extension's `byteStride` when it had none (the add-on keeps the view's, which only
//! differs for padded elements); a `QUATERNION` view of byteStride 16 (the add-on's own
//! sampled object rotations, `blender/exp/animation/sampled/object/sampler.py`, against the
//! specification's 8) decodes as Blender's bridge decodes it: the vertex stream at stride 16,
//! the filter on its first `count * 8` bytes (release `meshopt_decodeFilterQuat` ignores the
//! stride), the view those bytes; and a build without the codec decodes nothing but still
//! imports views whose fallback data is present (the extension then only saves space), where
//! the add-on fails without its library.
//!
//! The decoded views are appended to one new buffer (no `uri`, never written back), and the
//! extension is removed from them, so the mesh importer reads ordinary views. The decoder
//! itself is `export/mesh/post/meshopt.rs::decode_document`, next to the encoder, since
//! `export_use_gltfpack` decodes the written file too.

use frap_gltf::Document;
use frap_rna::{Report, ReportType};

use crate::export::mesh::post::meshopt::{MeshoptDecodeError, decode_document};
use crate::import::ImportError;

/// Decodes the compressed views of `doc` (see the module docs).
pub(super) fn decode_views(
    doc: &mut Document,
    buffers: &mut Vec<Vec<u8>>,
    reports: &mut Vec<Report>,
) -> Result<(), ImportError> {
    let decoded = decode_document(doc, buffers).map_err(|e| match e {
        MeshoptDecodeError::Unavailable(m) => ImportError::NotImplemented(m),
        MeshoptDecodeError::Data(m) => ImportError::Data(m),
    })?;
    if let Some(name) = decoded.fallback_used {
        reports.push(Report::new(
            ReportType::Warning,
            format!(
                "{name}: {}; the uncompressed fallback data was imported instead",
                frap_codec_mesh::meshopt::unavailable()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    //! Blender-written meshopt files import as Blender imports them
    //! (`tests/golden/cases/gltf/meshopt/import_blender.inputs`, written by
    //! `tests/cli/m7/meshopt/blender_meshopt.py make` with Blender 5.2.4, its own import of each
    //! file dumped in `blender_import.json`).
    use crate::export::mesh::post::meshopt::tests::{diff, dump, fixtures, import};
    use serde_json::Value as J;

    #[test]
    fn blender_meshopt_files_import_as_in_blender() {
        let dir = fixtures();
        let dumps: J = serde_json::from_str(
            &std::fs::read_to_string(dir.join("blender_import.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(dumps["blender"], "5.2.4 LTS");
        for name in ["ext.glb", "khr.glb", "ext_sep.gltf", "khr_sep.gltf"] {
            let (main, reports) = import(&dir.join(name));
            assert!(
                !reports.iter().any(|m| m.message.contains("meshopt")),
                "{name}: {reports:?}"
            );
            let got = dump(&main);
            if let Some(d) = diff(name, &got, &dumps[name]["objects"], 2e-6) {
                panic!("frap's import of Blender's {name} differs from Blender's: {d}");
            }
        }
    }

    #[test]
    fn blender_instance_transforms_decode() {
        // Blender's EXT_mesh_gpu_instancing accessors with KHR meshopt: TRANSLATION and SCALE
        // through the exponential filter, ROTATION through the quaternion filter (normalized
        // SHORT, 8 bits).
        let bytes = std::fs::read(fixtures().join("instances.glb")).unwrap();
        let glb = frap_gltf::read(&bytes).unwrap();
        let mut doc = glb.document;
        let mut buffers =
            frap_gltf::load_buffers(&doc, glb.bin.as_deref(), |_| Err("external".into())).unwrap();
        let n =
            crate::export::mesh::post::meshopt::decode_document(&mut doc, &mut buffers).unwrap();
        assert!(n.views >= 4, "{n:?}");
        let holder = doc
            .nodes
            .iter()
            .find(|n| n.extensions.contains("EXT_mesh_gpu_instancing"))
            .expect("instanced node");
        let attrs = holder
            .extensions
            .get_raw("EXT_mesh_gpu_instancing")
            .unwrap()["attributes"]
            .clone();
        let read = |k: &str| {
            frap_gltf::read_accessor(&doc, &buffers, attrs[k].as_u64().unwrap() as u32)
                .unwrap()
                .to_f32()
        };
        let t = read("TRANSLATION");
        let s = read("SCALE");
        let q = read("ROTATION");
        // Blender locations (1, 0, 0), (0, 2, 0), (-1.5, 0, 1) in glTF Y-up: (x, z, -y).
        let want_t = [1.0, 0.0, 0.0, 0.0, 0.0, -2.0, -1.5, 1.0, 0.0];
        for (a, b) in t.iter().zip(want_t) {
            assert!((a - b).abs() < 2e-3, "{t:?}");
        }
        for (a, b) in s.iter().zip([1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 2.0, 2.0, 2.0]) {
            assert!((a - b).abs() < 2e-3, "{s:?}");
        }
        // Unit quaternions within the 8-bit filter's precision.
        for e in q.chunks_exact(4) {
            let len: f32 = e.iter().map(|c| c * c).sum::<f32>().sqrt();
            assert!((len - 1.0).abs() < 2e-2, "{q:?}");
        }
        // The first instance: 0.5 rad about Blender Z = glTF Y.
        let half = 0.25f32;
        let want = [0.0, half.sin(), 0.0, half.cos()];
        let got = &q[..4];
        let sign = if got[3] < 0.0 { -1.0 } else { 1.0 };
        for (a, b) in got.iter().zip(want) {
            assert!((a * sign - b).abs() < 2e-2, "{got:?} vs {want:?}");
        }
    }
}
