// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression for the validator's size/offset arithmetic: every numeric member of every
//! vendored sample (extensions included, `extras` aside) is replaced in turn by extreme values
//! (`u64::MAX`, `2^63`, `2^32`, …; `±1e308` for floats). Reading, validating, decoding and
//! topology must never panic (debug builds trap on overflow) and must reject what no longer
//! fits, instead of wrapping into a pass.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use frap_gltf::accessor::uri_to_path;
use frap_gltf::primitive::read_topology;
use frap_gltf::{
    Document, load_buffers, read, read_accessor, validate, validate_data, validate_file,
};
use serde_json::Value;

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/gltf-samples")
}

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
    assert!(v.len() >= 20);
    v
}

/// JSON pointers of every number outside `extras`.
fn number_pointers(v: &Value, ptr: &mut String, out: &mut Vec<(String, bool)>) {
    match v {
        Value::Number(n) => out.push((ptr.clone(), n.is_u64() || n.is_i64())),
        Value::Array(items) => {
            for (i, x) in items.iter().enumerate() {
                let len = ptr.len();
                ptr.push_str(&format!("/{i}"));
                number_pointers(x, ptr, out);
                ptr.truncate(len);
            }
        }
        Value::Object(map) => {
            for (k, x) in map {
                if k == "extras" {
                    continue;
                }
                let len = ptr.len();
                ptr.push('/');
                ptr.push_str(&k.replace('~', "~0").replace('/', "~1"));
                number_pointers(x, ptr, out);
                ptr.truncate(len);
            }
        }
        _ => {}
    }
}

const INTS: [u64; 7] = [
    u64::MAX,
    u64::MAX - 3,
    1 << 63,
    1 << 32,
    u32::MAX as u64,
    (1 << 31) + 4,
    (1 << 28) + 4,
];
const FLOATS: [f64; 2] = [1e308, -1e308];

/// Everything a reader of an untrusted file does with it.
fn exercise(json: &[u8], buffers: &[Vec<u8>]) {
    let _ = validate_file(json);
    let Ok(doc) = Document::from_slice(json) else {
        return;
    };
    let structural = validate(&doc);
    let r = validate_data(&doc, buffers);
    for i in 0..doc.accessors.len() {
        let _ = read_accessor(&doc, buffers, i as u32);
    }
    for mesh in &doc.meshes {
        for prim in &mesh.primitives {
            let _ = read_topology(&doc, buffers, prim);
        }
    }
    let _ = (structural, r, doc.to_vec(&Default::default()));
}

#[test]
fn extreme_numbers_never_panic() {
    let mut failures = Vec::new();
    let mut runs = 0usize;
    for p in samples() {
        let bytes = std::fs::read(&p).unwrap();
        let glb = read(&bytes).unwrap();
        let dir = p.parent().unwrap();
        let buffers = load_buffers(&glb.document, glb.bin.as_deref(), |uri| {
            std::fs::read(dir.join(uri_to_path(uri))).map_err(|e| e.to_string())
        })
        .unwrap();
        let json: Value = glb.document.to_value();
        let mut ptrs = Vec::new();
        number_pointers(&json, &mut String::new(), &mut ptrs);
        for (ptr, is_int) in ptrs {
            let values: Vec<Value> = if is_int {
                INTS.iter().map(|&v| Value::from(v)).collect()
            } else {
                FLOATS.iter().map(|&v| Value::from(v)).collect()
            };
            for v in values {
                let mut m = json.clone();
                *m.pointer_mut(&ptr).unwrap() = v.clone();
                let text = serde_json::to_vec(&m).unwrap();
                runs += 1;
                if catch_unwind(AssertUnwindSafe(|| exercise(&text, &buffers))).is_err() {
                    failures.push(format!(
                        "{} {ptr} = {v}",
                        p.strip_prefix(samples_dir()).unwrap().display()
                    ));
                }
            }
        }
    }
    assert!(runs > 5000, "only {runs} mutations");
    assert!(failures.is_empty(), "panics:\n{}", failures.join("\n"));
}

/// The overflow sites the review found, each now a clean error rather than a panic or a
/// wrapped pass.
#[test]
fn overflowing_offsets_and_counts_are_errors() {
    let p = samples_dir().join("AnimatedMorphCube/glTF-Binary/AnimatedMorphCube.glb");
    let doc = read(&std::fs::read(p).unwrap()).unwrap().document;

    let mut d = doc.clone();
    d.accessors[0].byte_offset = Some(u64::MAX - 3);
    let r = validate(&d);
    assert!(r.has("ACCESSOR_TOO_LONG"), "{r}");

    let mut d = doc.clone();
    d.buffer_views[0].byte_offset = Some(u64::MAX - 3);
    let r = validate(&d);
    assert!(r.has("BUFFER_VIEW_TOO_LONG"), "{r}");

    // input.count × targets wraps to output.count in release builds without checks.
    let mut d = doc.clone();
    let smp = d.animations[0].samplers[0].clone();
    d.accessors[smp.input as usize].count = (1 << 63) + 1;
    let r = validate(&d);
    assert!(
        r.has("ANIMATION_SAMPLER_OUTPUT_ACCESSOR_INVALID_COUNT"),
        "{r}"
    );
}

#[test]
fn sparse_extent_overflow_is_an_error() {
    let p = samples_dir().join("SimpleSparseAccessor/glTF-Embedded/SimpleSparseAccessor.gltf");
    let doc = read(&std::fs::read(p).unwrap()).unwrap().document;
    let (i, _) = doc
        .accessors
        .iter()
        .enumerate()
        .find(|(_, a)| a.sparse.is_some())
        .unwrap();
    let mut d = doc.clone();
    let sp = d.accessors[i].sparse.as_mut().unwrap();
    sp.values.byte_offset = Some(u64::MAX - 8);
    let r = validate(&d);
    assert!(r.has("ACCESSOR_SPARSE_TOO_LONG"), "{r}");
}

/// A bufferView-less accessor cannot make a reader allocate far more than the file holds.
#[test]
fn zero_fill_is_bounded_by_the_buffers() {
    let p = samples_dir().join("SimpleSparseAccessor/glTF-Embedded/SimpleSparseAccessor.gltf");
    let bytes = std::fs::read(&p).unwrap();
    let glb = read(&bytes).unwrap();
    let buffers = load_buffers(&glb.document, None, |_| Err("external".into())).unwrap();
    let mut d = glb.document.clone();
    let i = d.accessors.iter().position(|a| a.sparse.is_some()).unwrap();
    d.accessors[i].buffer_view = None;
    d.accessors[i].byte_offset = None;
    // 2^26 VEC3 floats = 768 MiB of zeros from a few hundred bytes of buffer.
    d.accessors[i].count = 1 << 26;
    let e = read_accessor(&d, &buffers, i as u32).unwrap_err();
    assert!(e.to_string().contains("zero-fill"), "{e}");
    // A modest count still zero-fills (the sparse values then land on it).
    let base = glb.document.accessors[i].count;
    d.accessors[i].count = base;
    let data = read_accessor(&d, &buffers, i as u32).unwrap();
    assert_eq!(data.count() as u64, base);
}
