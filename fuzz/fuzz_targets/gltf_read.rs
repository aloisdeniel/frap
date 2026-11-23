// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: `frap_gltf::glb::read` (GLB or glTF JSON), the validator, the buffer loader,
//! every accessor and every primitive's topology on arbitrary bytes: never panic, never
//! allocate without bound, errors only (ARCHITECTURE §20.1, nightly from M8; WP 8.7).
//!
//! The input is used twice: as the whole file, and (when it starts with `{`) as the JSON chunk
//! of a GLB whose BIN chunk is the input's last quarter, so the fuzzer reaches the accessor
//! decoder with JSON it mutates as text. Invariants besides "no panic": `validate_file`
//! succeeds exactly when `read` does (both refuse only containers and JSON they cannot parse),
//! and a document that reads writes and reads back to an equal document.
#![no_main]

use frap_gltf::{WriteOpts, glb, validate, validate_file};
use libfuzzer_sys::fuzz_target;

/// External URIs are never fetched: the loader gets a stand-in error.
fn no_files(uri: &str) -> Result<Vec<u8>, String> {
    Err(format!("{uri}: external files are not read while fuzzing"))
}

fn exercise(bytes: &[u8]) {
    let read = glb::read(bytes);
    let validated = validate_file(bytes);
    assert_eq!(
        read.is_ok(),
        validated.is_ok(),
        "read and validate_file disagree: {:?} vs {:?}",
        read.as_ref().err(),
        validated.as_ref().err()
    );
    let Ok(g) = read else { return };
    let doc = &g.document;
    let _ = validate::validate_glb(&g);
    // Write → read: the model is lossless for what it read.
    let again = glb::read(&doc.to_vec(&WriteOpts::default()))
        .expect("a document frap read must read back after writing it");
    assert!(again.document == *doc, "write → read changed the document");
    let _ = glb::write_glb(&g, &WriteOpts::default());
    let Ok(buffers) = frap_gltf::load_buffers(doc, g.bin.as_deref(), no_files) else {
        return;
    };
    let _ = validate::validate_data(doc, &buffers);
    for i in 0..doc.accessors.len() {
        if let Ok(a) = frap_gltf::read_accessor(doc, &buffers, i as u32) {
            let _ = a.to_f32();
            let _ = a.to_u32();
            let _ = a.min_max();
            let _ = a.encode();
        }
    }
    for mesh in &doc.meshes {
        for prim in &mesh.primitives {
            let _ = frap_gltf::primitive::read_topology(doc, &buffers, prim);
        }
    }
}

/// A GLB holding `json` as its JSON chunk (padded with spaces) and `bin` as its BIN chunk
/// (padded with zeros; none when empty).
fn glb_of(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let pad = |n: usize| n.div_ceil(4) * 4;
    let (jl, bl) = (pad(json.len()), pad(bin.len()));
    let total = 20 + jl + if bin.is_empty() { 0 } else { 8 + bl };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&glb::MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(jl as u32).to_le_bytes());
    out.extend_from_slice(&glb::CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(json);
    out.resize(20 + jl, b' ');
    if !bin.is_empty() {
        out.extend_from_slice(&(bl as u32).to_le_bytes());
        out.extend_from_slice(&glb::CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(bin);
        out.resize(total, 0);
    }
    out
}

fuzz_target!(|data: &[u8]| {
    exercise(data);
    if data.first() == Some(&b'{') {
        let (json, bin) = data.split_at(data.len() - data.len() / 4);
        exercise(&glb_of(json, bin));
    }
});
