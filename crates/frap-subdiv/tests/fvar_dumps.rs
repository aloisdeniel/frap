// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 8.11 acceptance: the subdivided UV maps are bit-identical to Blender 5.2.4's
//! (`tests/fvar/data/*.json`, written by `tests/fvar/dump_fvar.py`): Suzanne at levels 1 and 2,
//! the cube, the UV sphere, cylinders, `SIMPLE` at levels 2 and 3 with n-gons, every
//! `uv_smooth`, levels above `quality`, no limit surface, boundary corners, creases, two UV maps
//! and the smart-projected mirrored Suzanne. Every corner's bits are compared (an FNV-1a hash
//! on large meshes), and so is the number of distinct (vertex, u, v) corner tuples the glTF
//! exporter splits on (deviations D-121, D-124 closed).

use std::path::{Path, PathBuf};

use frap_subdiv::catmull_clark::CatmullClark;
use frap_subdiv::{FVarLinear, SubdivSettings, SubdivTopology, Subdivider, VtxBoundary};
use serde_json::Value;

fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| u32::from_str_radix(&s[8 * i..8 * i + 8], 16).expect("hex"))
        .collect()
}

fn uv_layer(s: &str) -> Vec<[f32; 2]> {
    hex_words(s)
        .chunks(2)
        .map(|w| [f32::from_bits(w[0]), f32::from_bits(w[1])])
        .collect()
}

fn fnv(words: impl Iterator<Item = u32>) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn settings(v: &Value) -> SubdivSettings {
    let mut s = SubdivSettings::default();
    s.level = v["levels"].as_u64().unwrap() as u32;
    s.quality = v["quality"].as_u64().unwrap() as u32;
    s.use_limit_surface = v["use_limit_surface"].as_bool().unwrap();
    s.is_simple = v["subdivision_type"] == "SIMPLE";
    s.use_creases = v["use_creases"].as_bool().unwrap();
    s.vtx_boundary = VtxBoundary::from_boundary_smooth(v["boundary_smooth"].as_str().unwrap());
    s.fvar_linear = FVarLinear::from_uv_smooth(v["uv_smooth"].as_str().unwrap());
    s
}

fn floats(v: &Value) -> Option<Vec<f32>> {
    v.as_array().map(|a| {
        a.iter()
            .map(|x| f32::from_bits(u32::from_str_radix(x.as_str().unwrap(), 16).unwrap()))
            .collect()
    })
}

/// `Err` with a description of the first difference.
fn check(path: &Path) -> Result<(), String> {
    let case: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let inp = &case["input"];
    let nv = inp["num_vertices"].as_u64().unwrap() as usize;
    let positions = vec![[0.0f32; 3]; nv];
    let u = |x: &Value| x.as_u64().unwrap() as u32;
    let edges: Vec<[u32; 2]> = inp["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| [u(&e[0]), u(&e[1])])
        .collect();
    let mut face_offsets = vec![0u32];
    let mut corner_verts = Vec::new();
    for f in inp["faces"].as_array().unwrap() {
        corner_verts.extend(f.as_array().unwrap().iter().map(u));
        face_offsets.push(corner_verts.len() as u32);
    }
    let corner_edges: Vec<u32> = inp["corner_edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(u)
        .collect();
    let uvs: Vec<Vec<[f32; 2]>> = inp["uvs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| uv_layer(l.as_str().unwrap()))
        .collect();
    let crease_edge = floats(&inp["crease_edge"]);
    let crease_vert = floats(&inp["crease_vert"]);
    let mut topo = SubdivTopology::new(
        &positions,
        &edges,
        &face_offsets,
        &corner_verts,
        &corner_edges,
    );
    topo.edge_creases = crease_edge.as_deref();
    topo.vert_creases = crease_vert.as_deref();
    topo.uv_layers = uvs.iter().map(|l| l.as_slice()).collect();
    let s = settings(&case["settings"]);
    let got = CatmullClark
        .subdivide(&topo, &s)
        .map_err(|e| e.to_string())?;
    let out = &case["output"];
    let corners = out["corners"].as_u64().unwrap() as usize;
    if got.corner_verts.len() != corners {
        return Err(format!(
            "corner count {} != {corners}",
            got.corner_verts.len()
        ));
    }
    let want_layers = out["uvs"].as_array().unwrap();
    if got.uv_layers.len() != want_layers.len() {
        return Err("UV layer count".into());
    }
    for (li, (g, w)) in got.uv_layers.iter().zip(want_layers).enumerate() {
        let words = || g.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]);
        if let Some(bits) = w["bits"].as_str() {
            let want = uv_layer(bits);
            if let Some(i) = (0..want.len()).find(|&i| {
                g[i][0].to_bits() != want[i][0].to_bits()
                    || g[i][1].to_bits() != want[i][1].to_bits()
            }) {
                let n = (0..want.len())
                    .filter(|&i| {
                        g[i][0].to_bits() != want[i][0].to_bits()
                            || g[i][1].to_bits() != want[i][1].to_bits()
                    })
                    .count();
                return Err(format!(
                    "layer {li}: {n} corners differ, first {i}: {:?} != Blender {:?}",
                    g[i], want[i]
                ));
            }
        }
        let h = fnv(words());
        if h != w["hash"].as_str().unwrap() {
            return Err(format!("layer {li}: hash {h} != Blender {}", w["hash"]));
        }
        let mut tuples: Vec<(u32, u32, u32)> = got
            .corner_verts
            .iter()
            .zip(g)
            .map(|(&v, p)| (v, p[0].to_bits(), p[1].to_bits()))
            .collect();
        tuples.sort_unstable();
        tuples.dedup();
        let want_distinct = w["distinct"].as_u64().unwrap() as usize;
        if tuples.len() != want_distinct {
            return Err(format!(
                "layer {li}: {} distinct (vertex, uv) corners != Blender {want_distinct}",
                tuples.len()
            ));
        }
    }
    Ok(())
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fvar/data")
}

fn dumps_in(dir: &Path) -> Vec<PathBuf> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    names
}

/// The committed dumps, plus those of the directories in `FRAP_FVAR_DUMPS` (separated like
/// `PATH`; for dumps written by `tests/fvar/dump_fvar.py` that are too large to commit).
#[test]
fn uvs_are_bit_identical_to_blender() {
    let mut names = dumps_in(&data_dir());
    assert!(names.len() >= 50, "{} dumps", names.len());
    if let Some(extra) = std::env::var_os("FRAP_FVAR_DUMPS") {
        for dir in std::env::split_paths(&extra) {
            names.extend(dumps_in(&dir));
        }
    }
    let mut failures = Vec::new();
    for p in &names {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        match check(p) {
            Ok(()) => eprintln!("{name}: bit-identical"),
            Err(e) => {
                eprintln!("{name}: FAIL {e}");
                failures.push(format!("{name}: {e}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}

/// D-124's numbers: Suzanne + SUBSURF 1 has 3033 distinct (vertex, UV) corners in Blender
/// (the exporter writes 2893 glTF vertices after flipping V), the face-varying-only frap backend
/// before WP 8.11 had 2109 (shared corners welded).
#[test]
fn suzanne_level1_has_blenders_corner_split() {
    check(&data_dir().join("monkey_l1.json")).unwrap();
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(data_dir().join("monkey_l1.json")).unwrap())
            .unwrap();
    assert_eq!(case["output"]["uvs"][0]["distinct"], 3033);
}
