// SPDX-License-Identifier: GPL-3.0-or-later
//! §21.7: "1M-corner GLB export < 2 s": `bpy.ops.export_scene.gltf(export_format='GLB')` of
//! a 500 × 500 grid (250,000 quads, 1,000,000 face corners, 251,001 vertices) with its
//! default options (UVs, normals, the default material), through a real `Session`, file
//! written included.
//!
//! **Golden.** Blender 5.2.4's file for the same scene (`xtask/perf/blender_reference.py`,
//! which also times it: 0.8–1.5 s on an M1 Max): frap's JSON chunk equals it but for
//! `asset.generator`, and its 14,032,032-byte BIN chunk is byte-identical (compared by FNV-1a
//! hash). The debug run (a 60 × 60 grid) checks the accessor counts: one glTF vertex per grid
//! vertex (flat, one UV per vertex), two triangles per quad.

mod support;

use std::time::Duration;

use serde_json::json;
use support::*;

/// Faces per side (1M corners in release).
fn side() -> u32 {
    if release() { 500 } else { 60 }
}

/// The JSON chunk Blender 5.2.4 writes for the 500 × 500 grid
/// (`xtask/perf/blender_reference.py`).
const BLENDER_JSON: &str = r#"{"asset":{"generator":"Khronos glTF Blender I/O v5.2.41","version":"2.0"},"scene":0,"scenes":[{"name":"Scene","nodes":[0]}],"nodes":[{"mesh":0,"name":"Grid"}],"meshes":[{"name":"Grid","primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"indices":3}]}],"accessors":[{"bufferView":0,"componentType":5126,"count":251001,"max":[1,0,1],"min":[-1,0,-1],"type":"VEC3"},{"bufferView":1,"componentType":5126,"count":251001,"type":"VEC3"},{"bufferView":2,"componentType":5126,"count":251001,"type":"VEC2"},{"bufferView":3,"componentType":5125,"count":1500000,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteLength":3012012,"byteOffset":0,"target":34962},{"buffer":0,"byteLength":3012012,"byteOffset":3012012,"target":34962},{"buffer":0,"byteLength":2008008,"byteOffset":6024024,"target":34962},{"buffer":0,"byteLength":6000000,"byteOffset":8032032,"target":34963}],"buffers":[{"byteLength":14032032}]}"#;

/// Length and FNV-1a 64 hash of the BIN chunk.
fn bin_chunk(bytes: &[u8]) -> (usize, u64) {
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")) as usize;
    let off = 20 + json_len;
    let len = u32::from_le_bytes(bytes[off..off + 4].try_into().expect("4 bytes")) as usize;
    assert_eq!(&bytes[off + 4..off + 8], b"BIN\0");
    let hash = bytes[off + 8..off + 8 + len]
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    (len, hash)
}

fn glb_json(bytes: &[u8]) -> serde_json::Value {
    assert_eq!(&bytes[..4], b"glTF", "GLB magic");
    let len = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")) as usize;
    assert_eq!(&bytes[16..20], b"JSON");
    serde_json::from_slice(&bytes[20..20 + len]).expect("GLB JSON chunk")
}

#[test]
fn glb_export_of_1m_corners() {
    let _g = serial();
    let n = side();
    let dir = std::env::temp_dir().join(format!("frap-perf-glb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("grid.glb");
    let mut s = factory();
    ok(&mut s, "bpy.ops.wm.read_factory_settings(use_empty=True)");
    ok(
        &mut s,
        &format!(
            "bpy.ops.mesh.primitive_grid_add(x_subdivisions={n}, y_subdivisions={n}, size=2.0)"
        ),
    );
    let corners = len(&mut s, "bpy.context.object.data.loops");
    assert_eq!(corners, i64::from(4 * n * n));
    let stmt = format!(
        "bpy.ops.export_scene.gltf(filepath={:?}, export_format='GLB')",
        path.to_string_lossy()
    );
    let reps = if release() { 3 } else { 1 };
    let mut times: Vec<Duration> = Vec::new();
    for rep in 0..=reps {
        let (d, _) = timed(&mut s, &stmt);
        if rep > 0 {
            times.push(d);
        }
    }
    let bytes = std::fs::read(&path).expect("GLB written");
    let doc = glb_json(&bytes);
    let verts = i64::from((n + 1) * (n + 1));
    let prim = &doc["meshes"][0]["primitives"][0];
    let acc = |k: &str| {
        doc["accessors"][prim["attributes"][k].as_u64().expect("attr") as usize]["count"].as_i64()
    };
    assert_eq!(acc("POSITION"), Some(verts));
    assert_eq!(acc("NORMAL"), Some(verts));
    assert_eq!(acc("TEXCOORD_0"), Some(verts));
    let idx = prim["indices"].as_u64().expect("indices") as usize;
    assert_eq!(
        doc["accessors"][idx]["count"].as_i64(),
        Some(i64::from(6 * n * n))
    );
    if n == 500 {
        // Blender 5.2.4's file, but for the generator string (deviations: `asset.generator`).
        let mut blender: serde_json::Value =
            serde_json::from_str(BLENDER_JSON).expect("golden JSON");
        let mut ours = doc.clone();
        blender["asset"]["generator"] = serde_json::Value::Null;
        ours["asset"]["generator"] = serde_json::Value::Null;
        assert_eq!(ours, blender, "JSON chunk differs from Blender 5.2.4");
        let (bin_len, hash) = bin_chunk(&bytes);
        assert_eq!(bin_len, 14_032_032, "BIN chunk length");
        assert_eq!(
            hash, 0x3834_8242_abe5_5c42,
            "BIN chunk differs from Blender 5.2.4's (FNV-1a 64)"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
    let med = median(&mut times);
    report(
        "glb_export_1m_corners",
        ms(med),
        Some(2000.0),
        json!({
            "corners": corners,
            "bytes": bytes.len(),
            "min_ms": ms(times[0]),
            "max_ms": ms(*times.last().expect("reps")),
        }),
    );
}
