// SPDX-License-Identifier: GPL-3.0-or-later
//! frap's Catmull-Clark against Blender 5.2.4's Subdivision Surface modifier, index by index
//! (T1): every `tests/data/*.json` holds a base mesh, the modifier settings and the evaluated
//! mesh Blender produced (written by `tests/data/dump_cases.py`, see its header).
//!
//! Positions and UVs within `1e-5`; vertex, edge, face and corner order exact; `crease_edge`
//! copied like Blender's generic edge attributes.

use std::path::{Path, PathBuf};

use frap_subdiv::catmull_clark::CatmullClark;
use frap_subdiv::{FVarLinear, SubdivSettings, SubdivTopology, Subdivider, VtxBoundary};
use serde_json::Value;

struct MeshData {
    positions: Vec<[f32; 3]>,
    edges: Vec<[u32; 2]>,
    face_offsets: Vec<u32>,
    corner_verts: Vec<u32>,
    corner_edges: Vec<u32>,
    uvs: Vec<Vec<[f32; 2]>>,
    crease_edge: Option<Vec<f32>>,
    crease_vert: Option<Vec<f32>>,
}

fn f(v: &Value) -> f32 {
    v.as_f64().expect("number") as f32
}

fn u(v: &Value) -> u32 {
    v.as_u64().expect("index") as u32
}

fn floats(v: &Value) -> Option<Vec<f32>> {
    v.as_array().map(|a| a.iter().map(f).collect())
}

fn mesh(v: &Value) -> MeshData {
    let positions = v["positions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [f(&p[0]), f(&p[1]), f(&p[2])])
        .collect();
    let edges = v["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| [u(&e[0]), u(&e[1])])
        .collect();
    let mut face_offsets = vec![0u32];
    let mut corner_verts = Vec::new();
    for face in v["faces"].as_array().unwrap() {
        for c in face.as_array().unwrap() {
            corner_verts.push(u(c));
        }
        face_offsets.push(corner_verts.len() as u32);
    }
    let corner_edges = v["corner_edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(u)
        .collect();
    let uvs = v["uvs"]
        .as_array()
        .map(|layers| {
            layers
                .iter()
                .map(|l| {
                    l.as_array()
                        .unwrap()
                        .iter()
                        .map(|p| [f(&p[0]), f(&p[1])])
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default();
    MeshData {
        positions,
        edges,
        face_offsets,
        corner_verts,
        corner_edges,
        uvs,
        crease_edge: floats(&v["crease_edge"]),
        crease_vert: floats(&v["crease_vert"]),
    }
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

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Cases that differ from Blender beyond the tolerance by design, with the largest position
/// and UV errors accepted. Past the isolation level (`levels > quality`, `SIMPLE` past level 1)
/// Blender evaluates Gregory end caps: with the `osd` feature (default) positions come from
/// `opensubdiv-rs`'s patches and match, but its face-varying patches are built over a separate
/// value mesh, so UVs inside face-varying Gregory caps differ (deviations.md D-121); without it
/// frap evaluates the exact limit there (`frap_subdiv::catmull_clark`).
#[cfg(feature = "osd")]
const APPROXIMATE: &[(&str, f32, f32)] = &[
    ("cone_l2_simple", 1e-5, 3e-3),
    ("cone_l3_simple", 1e-5, 3e-3),
    ("cylinder_l2_simple", 1e-5, 5e-3),
    ("cylinder_l3_simple", 1e-5, 5e-3),
    ("cylinder_l4", 1e-5, 1e-4),
    ("ngon_l4", 1e-5, 2e-4),
    ("sphere_l2_simple", 1e-5, 1e-3),
    ("sphere_l3_simple", 1e-5, 1e-3),
];
#[cfg(not(feature = "osd"))]
const APPROXIMATE: &[(&str, f32, f32)] = &[
    ("cube_l2_q1", 2e-2, 3e-3),
    ("cube_l3_q2", 7e-3, 2e-3),
    ("cube_l4", 3e-3, 1e-3),
    ("cone_l2_simple", 3e-2, 7e-3),
    ("cone_l3_simple", 4e-2, 9e-3),
    ("cylinder_l2_simple", 5e-2, 1.2e-2),
    ("cylinder_l3_simple", 7.1e-2, 1.7e-2),
    ("cylinder_l4", 1.3e-2, 4.1e-3),
    ("ngon_l4", 2.5e-2, 1.3e-2),
    ("sphere_l2_simple", 6.1e-3, 2.5e-3),
    ("sphere_l3_simple", 8.3e-3, 3.3e-3),
    ("tris_l3_simple", 1.7e-2, 1e-5),
];

/// Largest position and UV errors of `backend` on a dump; `Err` on a topology difference.
fn errors(name: &str, backend: &dyn Subdivider) -> Result<(f32, usize, f32), String> {
    let text = std::fs::read_to_string(data_dir().join(format!("{name}.json"))).unwrap();
    let case: Value = serde_json::from_str(&text).unwrap();
    let inp = mesh(&case["input"]);
    let want = mesh(&case["output"]);
    let s = settings(&case["settings"]);
    let mut topo = SubdivTopology::new(
        &inp.positions,
        &inp.edges,
        &inp.face_offsets,
        &inp.corner_verts,
        &inp.corner_edges,
    );
    topo.edge_creases = inp.crease_edge.as_deref();
    topo.vert_creases = inp.crease_vert.as_deref();
    topo.uv_layers = inp.uvs.iter().map(|l| l.as_slice()).collect();
    let got = backend.subdivide(&topo, &s).map_err(|e| e.to_string())?;
    if got.positions.len() != want.positions.len() {
        return Err(format!(
            "vertex count {} != {}",
            got.positions.len(),
            want.positions.len()
        ));
    }
    if got.edges != want.edges {
        let i = got.edges.iter().zip(&want.edges).position(|(a, b)| a != b);
        return Err(format!(
            "edges differ (first at {i:?}, counts {} / {})",
            got.edges.len(),
            want.edges.len()
        ));
    }
    if got.face_offsets != want.face_offsets || got.corner_verts != want.corner_verts {
        let i = got
            .corner_verts
            .iter()
            .zip(&want.corner_verts)
            .position(|(a, b)| a != b);
        return Err(format!("faces differ (first corner {i:?})"));
    }
    if got.corner_edges != want.corner_edges {
        let i = got
            .corner_edges
            .iter()
            .zip(&want.corner_edges)
            .position(|(a, b)| a != b);
        return Err(format!("corner edges differ (first {i:?})"));
    }
    let mut max_err = 0.0f32;
    let mut worst = 0;
    for (i, (a, b)) in got.positions.iter().zip(&want.positions).enumerate() {
        let e = (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max);
        if e > max_err {
            max_err = e;
            worst = i;
        }
    }
    if got.uv_layers.len() != want.uvs.len() {
        return Err("UV layer count".into());
    }
    let mut uv_err = 0.0f32;
    for (a, b) in got.uv_layers.iter().zip(&want.uvs) {
        for (p, q) in a.iter().zip(b) {
            uv_err = uv_err.max((p[0] - q[0]).abs().max((p[1] - q[1]).abs()));
        }
    }
    if let Some(want_c) = &want.crease_edge {
        let got_c = got.edge_creases.as_ref().ok_or("no creases")?;
        if got_c != want_c {
            return Err("crease_edge differs".into());
        }
    }
    Ok((max_err, worst, uv_err))
}

fn check(name: &str) -> Result<f32, String> {
    let (pos, worst, uv) = errors(name, &CatmullClark)?;
    let (tol, uv_tol) = APPROXIMATE
        .iter()
        .find(|(n, _, _)| *n == name)
        .map_or((1e-5, 1e-5), |&(_, p, u)| (p, u));
    if pos > tol {
        return Err(format!("position error {pos:e} at vertex {worst}"));
    }
    if uv > uv_tol {
        return Err(format!("UV error {uv:e}"));
    }
    Ok(pos)
}

fn case_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension()? == "json").then(|| p.file_stem().unwrap().to_string_lossy().into())
        })
        .collect();
    names.sort();
    names
}

/// The OpenSubdiv backend: same layout, positions and UVs within its documented accuracy.
#[cfg(feature = "osd")]
#[test]
fn osd_backend_matches_blender() {
    let mut failures = Vec::new();
    for n in case_names() {
        match errors(&n, &frap_subdiv::osd::OpenSubdiv) {
            Ok((pos, worst, uv)) => {
                eprintln!("{n}: position {pos:e} (vertex {worst}), uv {uv:e}");
                // Smooth boundary corners: opensubdiv-rs has no legacy sharp corner patches;
                // UVs inside face-varying Gregory caps (D-121).
                if pos > 6e-3 || uv > 5e-3 {
                    failures.push(format!("{n}: position {pos:e}, uv {uv:e}"));
                }
            }
            Err(e) => failures.push(format!("{n}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_dump_matches_blender() {
    let names = case_names();
    assert!(names.len() >= 30, "{names:?}");
    let mut failures = Vec::new();
    for n in &names {
        match check(n) {
            Ok(e) => eprintln!("{n}: ok (max position error {e:e})"),
            Err(e) => {
                eprintln!("{n}: FAIL {e}");
                failures.push(format!("{n}: {e}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
