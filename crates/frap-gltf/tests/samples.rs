// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.5 acceptance on the vendored Khronos subset (`tests/data/gltf-samples`):
//! - every sample reads, decodes and validates;
//! - JSON round-trips losslessly (value for value, unknown extensions and extras included);
//! - written back as GLB it is lossless modulo buffer layout (same JSON apart from buffers and
//!   buffer-view placement, identical decoded accessor and image data);
//! - every primitive mode decodes;
//! - the independent `gltf =1.4.1` reader accepts frap's output exactly when it accepts the
//!   source, and reads the same data from it.

use std::path::{Path, PathBuf};

use frap_gltf::accessor::{buffer_view_bytes, read_accessor, uri_to_path};
use frap_gltf::json::{ComponentType, Mode, json_eq};
use frap_gltf::primitive::read_topology;
use frap_gltf::{
    Document, Glb, WriteOpts, load_buffers, read, read_glb, repack, validate_data, validate_glb,
    write_glb,
};
use serde_json::Value;

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/gltf-samples")
}

/// Every `.gltf`/`.glb` under the samples directory, sorted.
fn samples() -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if matches!(p.extension().and_then(|e| e.to_str()), Some("gltf" | "glb")) {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(&samples_dir(), &mut v);
    v.sort();
    assert!(v.len() >= 20, "only {} samples found", v.len());
    v
}

fn name(p: &Path) -> String {
    p.strip_prefix(samples_dir()).unwrap().display().to_string()
}

struct Loaded {
    glb: Glb,
    buffers: Vec<Vec<u8>>,
    /// The JSON text as stored in the file.
    json: Value,
}

fn load(p: &Path) -> Loaded {
    let bytes = std::fs::read(p).unwrap();
    let glb = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", name(p)));
    let dir = p.parent().unwrap();
    let buffers = load_buffers(&glb.document, glb.bin.as_deref(), |uri| {
        std::fs::read(dir.join(uri_to_path(uri))).map_err(|e| e.to_string())
    })
    .unwrap_or_else(|e| panic!("{}: {e}", name(p)));
    let json_bytes = if frap_gltf::glb::is_glb(&bytes) {
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        bytes[20..20 + len].to_vec()
    } else {
        bytes
    };
    let text = String::from_utf8(json_bytes).unwrap();
    let json = serde_json::from_str(
        text.trim_end_matches([' ', '\0'])
            .trim_start_matches('\u{feff}'),
    )
    .unwrap();
    Loaded { glb, buffers, json }
}

#[test]
fn every_sample_reads_decodes_and_validates() {
    for p in samples() {
        let l = load(&p);
        let doc = &l.glb.document;
        let r = validate_data(doc, &l.buffers);
        assert!(r.is_ok(), "{}:\n{r}", name(&p));
        for i in 0..doc.accessors.len() {
            read_accessor(doc, &l.buffers, i as u32)
                .unwrap_or_else(|e| panic!("{} accessor {i}: {e}", name(&p)));
        }
        if p.extension().unwrap() == "glb" {
            let r = validate_glb(&l.glb);
            assert!(r.is_ok(), "{}:\n{r}", name(&p));
        }
    }
}

#[test]
fn json_round_trip_is_lossless() {
    for p in samples() {
        let l = load(&p);
        let doc = &l.glb.document;
        // Value for value, every member kept (including spelled-out defaults, unknown
        // extensions and extras).
        let written: Value = serde_json::from_slice(&doc.to_vec(&WriteOpts::default())).unwrap();
        assert!(
            json_eq(&written, &l.json),
            "{}: JSON changed\nsource: {}\nfrap:   {}",
            name(&p),
            l.json,
            written
        );
        // Text → model → text → model is stable, compact and pretty.
        for opts in [
            WriteOpts::default(),
            WriteOpts {
                indent: Some("\t".into()),
            },
        ] {
            let again = Document::from_slice(&doc.to_vec(&opts)).unwrap();
            assert_eq!(&again, doc, "{}", name(&p));
        }
    }
}

/// Removes the members that only describe buffer layout.
fn strip_layout(mut v: Value) -> Value {
    let o = v.as_object_mut().unwrap();
    o.remove("buffers");
    if let Some(Value::Array(views)) = o.get_mut("bufferViews") {
        for bv in views {
            let bv = bv.as_object_mut().unwrap();
            bv.remove("buffer");
            bv.remove("byteOffset");
        }
    }
    v
}

#[test]
fn glb_write_back_is_lossless_modulo_buffer_layout() {
    for p in samples() {
        let l = load(&p);
        let doc = &l.glb.document;
        let (packed, bin) = repack(doc, &l.buffers).unwrap();
        let bytes = write_glb(
            &Glb {
                document: packed,
                bin: Some(bin),
            },
            &WriteOpts::default(),
        );
        let back = read_glb(&bytes).unwrap();
        let r = validate_glb(&back);
        assert!(r.is_ok(), "{}:\n{r}", name(&p));
        assert!(
            json_eq(
                &strip_layout(back.document.to_value()),
                &strip_layout(l.json.clone())
            ),
            "{}: JSON differs beyond buffer layout",
            name(&p)
        );
        let bufs = load_buffers(&back.document, back.bin.as_deref(), |u| {
            // Only images keep external URIs; buffers all moved into the BIN chunk.
            Err(format!("unexpected external buffer {u}"))
        })
        .unwrap();
        for i in 0..doc.accessors.len() as u32 {
            assert_eq!(
                read_accessor(&back.document, &bufs, i).unwrap(),
                read_accessor(doc, &l.buffers, i).unwrap(),
                "{} accessor {i}",
                name(&p)
            );
        }
        for (i, img) in doc.images.iter().enumerate() {
            if let Some(v) = img.buffer_view {
                assert_eq!(
                    buffer_view_bytes(&back.document, &bufs, v).unwrap(),
                    buffer_view_bytes(doc, &l.buffers, v).unwrap(),
                    "{} image {i}",
                    name(&p)
                );
            }
        }
        // Writing the round-tripped file again is byte-stable.
        let again = write_glb(&back, &WriteOpts::default());
        assert_eq!(again, bytes, "{}", name(&p));
    }
}

#[test]
fn every_primitive_mode_decodes() {
    let p = samples_dir().join("MeshPrimitiveModes/glTF-Embedded/MeshPrimitiveModes.gltf");
    let l = load(&p);
    let doc = &l.glb.document;
    let mut seen = Vec::new();
    for mesh in &doc.meshes {
        for prim in &mesh.primitives {
            let mode = prim.mode_or_default();
            let t = read_topology(doc, &l.buffers, prim).unwrap();
            let vcount = doc.accessors[prim.attributes["POSITION"] as usize].count as u32;
            let all: Vec<u32> = t
                .points
                .iter()
                .copied()
                .chain(t.edges.iter().flatten().copied())
                .chain(t.triangles.iter().flatten().copied())
                .collect();
            assert!(all.iter().all(|&i| i < vcount), "{mode:?}");
            let counts = (t.points.len(), t.edges.len(), t.triangles.len());
            let expected = match mode {
                Mode::Points => (7, 0, 0),
                Mode::Lines => (0, 6, 0),
                Mode::LineLoop => (0, 7, 0),
                Mode::LineStrip => (0, 6, 0),
                Mode::Triangles => (0, 0, 6),
                Mode::TriangleStrip => (0, 0, 4),
                Mode::TriangleFan => (0, 0, 6),
            };
            assert_eq!(counts, expected, "{mode:?}");
            seen.push(mode);
        }
    }
    for m in Mode::ALL {
        assert!(seen.contains(&m), "{m:?} not covered");
    }
    // Primitives without indices draw their vertices in order.
    let p = samples_dir().join("TriangleWithoutIndices/glTF-Embedded/TriangleWithoutIndices.gltf");
    let l = load(&p);
    let prim = &l.glb.document.meshes[0].primitives[0];
    assert!(prim.indices.is_none());
    assert_eq!(
        read_topology(&l.glb.document, &l.buffers, prim)
            .unwrap()
            .triangles,
        [[0, 1, 2]]
    );
}

#[test]
fn gltf_crate_reads_frap_output() {
    let mut accepted = 0;
    let mut stripped = 0;
    for p in samples() {
        let l = load(&p);
        let (packed, bin) = repack(&l.glb.document, &l.buffers).unwrap();
        let packed_doc = packed.clone();
        let out = write_glb(
            &Glb {
                document: packed,
                bin: Some(bin.clone()),
            },
            &WriteOpts::default(),
        );
        let source = gltf::Gltf::from_slice(&std::fs::read(&p).unwrap());
        let ours = gltf::Gltf::from_slice(&out);
        // gltf 1.4.1 (no extension features) rejects any extensionsRequired: it must judge
        // frap's output exactly as it judges the source.
        assert_eq!(
            source.is_ok(),
            ours.is_ok(),
            "{}: source {:?}, frap {:?}",
            name(&p),
            source.as_ref().err().map(ToString::to_string),
            ours.as_ref().err().map(ToString::to_string)
        );
        let g = match ours {
            Ok(g) => g,
            // Rejected only for extensionsRequired: drop the list (the data stays readable by
            // gltf, which ignores extension members) and compare the data all the same.
            Err(_) if !packed_doc.extensions_required.is_empty() => {
                let mut d = packed_doc.clone();
                d.extensions_required.clear();
                let out = write_glb(
                    &Glb {
                        document: d,
                        bin: Some(bin.clone()),
                    },
                    &WriteOpts::default(),
                );
                // Files gltf rejects for other reasons too (LightVisibility's
                // KHR_animation_pointer channels have no `node`) stay skipped.
                let Ok(g) = gltf::Gltf::from_slice(&out) else {
                    continue;
                };
                stripped += 1;
                g
            }
            Err(_) => continue,
        };
        accepted += 1;
        let doc = &l.glb.document;
        assert_eq!(g.nodes().count(), doc.nodes.len());
        assert_eq!(g.meshes().count(), doc.meshes.len());
        assert_eq!(g.accessors().count(), doc.accessors.len());
        assert_eq!(g.animations().count(), doc.animations.len());
        assert_eq!(g.skins().count(), doc.skins.len());
        for (m, mesh) in g.meshes().enumerate() {
            for (k, prim) in mesh.primitives().enumerate() {
                let fp = &doc.meshes[m].primitives[k];
                assert_eq!(prim.mode().as_gl_enum(), fp.mode_or_default() as u32);
                let reader = prim.reader(|b| (b.index() == 0).then_some(&bin[..]));
                let float_positions = fp.attributes.get("POSITION").is_some_and(|&a| {
                    doc.accessors[a as usize].component_type == ComponentType::Float
                });
                // gltf reads POSITION as f32 only (it asserts on quantized positions).
                if float_positions && let Some(pos) = reader.read_positions() {
                    let theirs: Vec<[f32; 3]> = pos.collect();
                    let mine = read_accessor(doc, &l.buffers, fp.attributes["POSITION"])
                        .unwrap()
                        .to_f32_elements::<3>()
                        .unwrap();
                    assert_eq!(theirs, mine, "{} mesh {m} prim {k}", name(&p));
                }
                if let Some(ix) = reader.read_indices() {
                    let theirs: Vec<u32> = ix.into_u32().collect();
                    let mine = read_accessor(doc, &l.buffers, fp.indices.unwrap())
                        .unwrap()
                        .to_u32()
                        .unwrap();
                    assert_eq!(theirs, mine, "{} mesh {m} prim {k}", name(&p));
                }
            }
        }
    }
    eprintln!(
        "gltf 1.4.1 read {accepted} of {} outputs ({stripped} after dropping extensionsRequired)",
        samples().len()
    );
    assert!(
        stripped >= 2,
        "only {stripped} outputs read after dropping extensionsRequired"
    );
    assert!(
        accepted >= 23,
        "gltf 1.4.1 accepted only {accepted} outputs"
    );
}
