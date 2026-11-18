// SPDX-License-Identifier: GPL-3.0-or-later
//! Blender 5.2.4 goldens of the BMesh core (WP 2.1): `tests/fixtures/core.blender.json`, written
//! by `tests/blender/dump_core.py`. Each case rebuilds its setup mesh, converts it with
//! `BMesh::from_mesh`, replays the steps Blender ran on its edit-mode BMesh with the frap
//! kernel, compares BMesh state (selection history, active face, element counts, normals, step
//! results) and the mesh after `to_mesh` at T1: element order, every attribute in order with
//! its values, positions within 1e-5.
//!
//! Covered: edit-mode round trips (primitives, a mesh with every attribute kind, flags, pins
//! and vertex groups), `BLI_mempool` order (LIFO reuse, the reset when the last element of a
//! multi-chunk pool is freed), `bmesh.ops.delete` (every context) and `duplicate`, the kernel
//! through `bmesh.utils` (SEMV/edge split with interpolation, SFME/face split, vertex
//! separate, face flip, face join, vertex splice, element creation/removal), custom-data
//! interpolation, `BM_mesh_normals_update`, selection set/flush, the select-mode conversion
//! table, and the edge loop / edge ring / island / vertex shell walkers (through Blender's
//! selection operators).

mod common;

use common::*;
use frap_bmesh::ops::delete::{DeleteIn, delete};
use frap_bmesh::ops::duplicate::{DuplicateIn, duplicate};
use frap_bmesh::walkers::{LoopDelimit, WalkDelimit, WalkOpts};
use frap_bmesh::{
    BMElem, BMesh, CreateFlag, EdgeId, ElemTypes, FaceId, FromMeshParams, SelectMode, ToMeshParams,
    VertId,
};
use serde_json::{Value as J, json};

fn mode_of(v: &J) -> SelectMode {
    let mut m = SelectMode::empty();
    for s in v.as_array().unwrap() {
        m |= match s.as_str().unwrap() {
            "VERT" => SelectMode::VERT,
            "EDGE" => SelectMode::EDGE,
            "FACE" => SelectMode::FACE,
            x => panic!("mode {x}"),
        };
    }
    m
}

fn idx(v: &J) -> usize {
    v.as_u64().unwrap() as usize
}
fn f3(v: &J) -> [f32; 3] {
    let a = v.as_array().unwrap();
    [0, 1, 2].map(|i| a[i].as_f64().unwrap() as f32)
}

fn elem(bm: &BMesh, r: &J) -> BMElem {
    let t = r[0].as_str().unwrap();
    let i = idx(&r[1]);
    match t {
        "V" => BMElem::Vert(bm.vert_ids()[i]),
        "E" => BMElem::Edge(bm.edge_ids()[i]),
        "F" => BMElem::Face(bm.face_ids()[i]),
        _ => panic!("elem {t}"),
    }
}

fn refs(bm: &mut BMesh, elems: &[BMElem]) -> J {
    bm.index.dirty = ElemTypes::all();
    bm.elem_index_ensure(ElemTypes::all());
    J::Array(
        elems
            .iter()
            .map(|&e| {
                let t = match e {
                    BMElem::Vert(_) => "V",
                    BMElem::Edge(_) => "E",
                    BMElem::Face(_) => "F",
                };
                json!([t, bm.index_of(e)])
            })
            .collect(),
    )
}

fn selected(bm: &mut BMesh) -> J {
    let v: Vec<usize> = bm
        .vert_ids()
        .into_iter()
        .enumerate()
        .filter(|(_, x)| bm.is_selected(BMElem::Vert(*x)))
        .map(|(i, _)| i)
        .collect();
    let e: Vec<usize> = bm
        .edge_ids()
        .into_iter()
        .enumerate()
        .filter(|(_, x)| bm.is_selected(BMElem::Edge(*x)))
        .map(|(i, _)| i)
        .collect();
    let f: Vec<usize> = bm
        .face_ids()
        .into_iter()
        .enumerate()
        .filter(|(_, x)| bm.is_selected(BMElem::Face(*x)))
        .map(|(i, _)| i)
        .collect();
    json!({"verts": v, "edges": e, "faces": f})
}

/// `bpy.ops.mesh.<name>(**kw)` emulated with the frap BMesh API (selection operators only).
fn bpy_op(bm: &mut BMesh, name: &str, kw: &J) {
    let set = |key: &str| -> Vec<String> {
        kw.get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    };
    match name {
        "select_mode" => {
            let t = match kw["type"].as_str().unwrap() {
                "VERT" => SelectMode::VERT,
                "EDGE" => SelectMode::EDGE,
                _ => SelectMode::FACE,
            };
            bm.select_mode_convert(t, kw["use_expand"].as_bool().unwrap_or(false));
        }
        "select_edge_loop_multi" | "select_edge_ring_multi" => {
            let mut d = LoopDelimit::empty();
            let key = if name == "select_edge_loop_multi" {
                "delimit_edge_loop"
            } else {
                "delimit_edge_ring"
            };
            for s in set(key) {
                d |= match (name, s.as_str()) {
                    (_, "SEAM") => LoopDelimit::SEAM,
                    (_, "SHARP") => LoopDelimit::SHARP,
                    (_, "MATERIAL") => LoopDelimit::MATERIAL,
                    ("select_edge_loop_multi", "NGONS") => LoopDelimit::LOOP_NGONS,
                    (_, "NGONS") => LoopDelimit::RING_NGONS,
                    (_, "INNER_CORNERS") => LoopDelimit::INNER_CORNERS,
                    (_, "OUTER_CORNERS") => LoopDelimit::OUTER_CORNERS,
                    (_, x) => panic!("delimit {x}"),
                };
            }
            let opts = WalkOpts {
                delimit: d,
                ..WalkOpts::default()
            };
            let start: Vec<EdgeId> = bm
                .edge_ids()
                .into_iter()
                .filter(|&e| bm.is_selected(BMElem::Edge(e)))
                .collect();
            for e in start {
                let walked = if name == "select_edge_loop_multi" {
                    bm.walk_edge_loop_ex(e, &opts)
                } else {
                    bm.walk_edge_ring_ex(e, &opts)
                };
                for x in walked {
                    bm.select_set(BMElem::Edge(x), true);
                }
            }
            bm.select_flush_mode();
        }
        "select_linked" => {
            let mut d = WalkDelimit::default();
            for s in set("delimit") {
                match s.as_str() {
                    "SEAM" => d.seam = true,
                    "SHARP" => d.sharp = true,
                    "MATERIAL" => d.material = true,
                    "NORMAL" => d.normal = true,
                    "UV" => d.uv = true,
                    x => panic!("delimit {x}"),
                }
            }
            if bm.select_mode.contains(SelectMode::VERT) {
                assert_eq!(
                    d,
                    WalkDelimit::default(),
                    "vertex-mode delimit not emulated"
                );
                let start: Vec<VertId> = bm
                    .vert_ids()
                    .into_iter()
                    .filter(|&v| bm.is_selected(BMElem::Vert(v)))
                    .collect();
                for v in start {
                    for e in bm.walk_vert_shell_edges(v, &WalkOpts::default()) {
                        bm.select_set(BMElem::Edge(e), true);
                    }
                }
                bm.select_flush_mode();
            } else {
                let start: Vec<FaceId> = bm
                    .face_ids()
                    .into_iter()
                    .filter(|&f| bm.is_selected(BMElem::Face(f)))
                    .collect();
                for f in start {
                    for x in bm.walk_face_island(f, d) {
                        bm.select_set(BMElem::Face(x), true);
                    }
                }
            }
        }
        _ => panic!("bpy op {name} not emulated"),
    }
}

fn run_step(bm: &mut BMesh, s: &J) -> J {
    let op = s["op"].as_str().unwrap();
    let v = |bm: &BMesh, k: &str| bm.vert_ids()[idx(&s[k])];
    let f = |bm: &BMesh, k: &str| bm.face_ids()[idx(&s[k])];
    let e = |bm: &BMesh, k: &str| bm.edge_ids()[idx(&s[k])];
    match op {
        "vert_new" => {
            bm.vert_create(f3(&s["co"]), None, CreateFlag::empty());
        }
        "vert_remove" => {
            let x = v(bm, "v");
            bm.vert_kill(x);
        }
        "edge_new" => {
            let ids = bm.vert_ids();
            let vs: Vec<VertId> = s["verts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ids[idx(i)])
                .collect();
            bm.edge_create(vs[0], vs[1], None, CreateFlag::empty());
        }
        "edge_remove" => {
            let x = e(bm, "e");
            bm.edge_kill(x);
        }
        "face_new" => {
            let ids = bm.vert_ids();
            let vs: Vec<VertId> = s["verts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ids[idx(i)])
                .collect();
            bm.face_create_verts(&vs, None, CreateFlag::empty(), true)
                .unwrap();
        }
        "face_remove" => {
            let x = f(bm, "f");
            bm.face_kill(x);
        }
        "co" => {
            let x = v(bm, "v");
            bm.vert_mut(x).co = f3(&s["co"]);
        }
        "delete" => {
            let geom: Vec<BMElem> = s["geom"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| elem(bm, r))
                .collect();
            let ctx: &'static str =
                Box::leak(s["context"].as_str().unwrap().to_string().into_boxed_str());
            delete(bm, &DeleteIn { geom, context: ctx }).unwrap();
        }
        "duplicate" => {
            let geom: Vec<BMElem> = s["geom"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| elem(bm, r))
                .collect();
            let out = duplicate(
                bm,
                &DuplicateIn {
                    geom,
                    use_select_history: s["use_select_history"].as_bool().unwrap_or(false),
                    use_edge_flip_from_face: s["use_edge_flip_from_face"]
                        .as_bool()
                        .unwrap_or(false),
                    ..Default::default()
                },
            )
            .unwrap();
            let geom = refs(bm, &out.geom);
            let pairs = |bm: &BMesh, m: Vec<(BMElem, BMElem)>| {
                let mut p: Vec<[usize; 2]> = m
                    .into_iter()
                    .map(|(a, b)| [bm.index_of(a), bm.index_of(b)])
                    .collect();
                p.sort();
                p
            };
            let bmap = pairs(
                bm,
                out.boundary_map
                    .iter()
                    .map(|&(a, b)| (BMElem::Edge(a), BMElem::Edge(b)))
                    .collect(),
            );
            let imap = pairs(
                bm,
                out.isovert_map
                    .iter()
                    .map(|&(a, b)| (BMElem::Vert(a), BMElem::Vert(b)))
                    .collect(),
            );
            return json!({"geom": geom, "boundary_map": bmap, "isovert_map": imap});
        }
        "edge_split" => {
            let x = e(bm, "e");
            let end = idx(&s["end"]);
            let vv = bm.edge(x).v[end];
            let fac = (s["fac"].as_f64().unwrap() as f32).clamp(0.0, 1.0);
            bm.edge_split(x, vv, fac);
        }
        "face_split" => {
            let x = f(bm, "f");
            let vs = bm.face_verts(x);
            let la = bm.face_vert_share_loop(x, vs[idx(&s["a"])]).unwrap();
            let lb = bm.face_vert_share_loop(x, vs[idx(&s["b"])]).unwrap();
            bm.face_split(x, la, lb, None, true).expect("face split");
        }
        "face_join" => {
            let ids = bm.face_ids();
            let fs: Vec<FaceId> = s["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ids[idx(i)])
                .collect();
            let remove = s["remove"].as_bool().unwrap_or(true);
            let (fnew, dbl) = bm.faces_join(&fs, remove, true);
            assert!(fnew.is_some() && dbl.is_none());
        }
        "vert_separate" => {
            let x = v(bm, "v");
            let ve = bm.vert_edges(x);
            let es: Vec<EdgeId> = s["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ve[idx(i)])
                .collect();
            let out = bm.vert_separate(x, &es, false);
            let el: Vec<BMElem> = out.into_iter().map(BMElem::Vert).collect();
            return refs(bm, &el);
        }
        "face_flip" => {
            let x = f(bm, "f");
            bm.face_normal_flip(x);
        }
        "vert_splice" => {
            let (src, dst) = (v(bm, "v"), v(bm, "target"));
            assert!(bm.vert_splice(dst, src));
        }
        "select_set" => {
            let x = elem(bm, &s["elem"]);
            bm.select_set(x, s["value"].as_bool().unwrap());
        }
        "select_flush_mode" => bm.select_flush_mode(),
        "select_flush" => {
            if s["value"].as_bool().unwrap() {
                bm.select_flush();
            } else {
                bm.deselect_flush();
            }
        }
        "select_mode" => bm.select_mode = mode_of(&s["mode"]),
        "history_add" => {
            let x = elem(bm, &s["elem"]);
            bm.select_history_store_if_absent(x);
        }
        "normal_update" => bm.normals_update(),
        "loop_interp" => {
            let x = f(bm, "f");
            let l = bm.face_loops(x)[idx(&s["corner"])];
            let src = f(bm, "src");
            bm.loop_interp_from_face(l, src, s["vert"].as_bool().unwrap());
        }
        "vert_interp" => {
            let (x, a, b) = (v(bm, "v"), v(bm, "a"), v(bm, "b"));
            let fac = (s["fac"].as_f64().unwrap() as f32).clamp(0.0, 1.0);
            bm.data_interp_from_verts(a, b, x, fac);
        }
        "deselect_all" => {
            // `x.select = False` on every element (BM_elem_select_set), then the history.
            for x in bm.vert_ids() {
                bm.select_set(BMElem::Vert(x), false);
            }
            for x in bm.edge_ids() {
                bm.select_set(BMElem::Edge(x), false);
            }
            for x in bm.face_ids() {
                bm.select_set(BMElem::Face(x), false);
            }
            bm.select_history_clear();
        }
        "bpy" => bpy_op(bm, s["name"].as_str().unwrap(), &s["kw"]),
        "selected" => return selected(bm),
        _ => panic!("step {op}"),
    }
    J::Null
}

fn cmp_vec3s(what: &str, a: &[[f32; 3]], b: &J, errs: &mut Vec<String>) {
    let b: Vec<[f32; 3]> = b.as_array().unwrap().iter().map(f3).collect();
    if a.len() != b.len() {
        errs.push(format!("{what}: {} vs Blender {}", a.len(), b.len()));
        return;
    }
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        if (0..3).any(|k| (x[k] - y[k]).abs() > 1e-5) {
            errs.push(format!("{what}[{i}]: {x:?} vs Blender {y:?}"));
            return;
        }
    }
}

fn run_case(name: &str, c: &J, setups: &J) -> Vec<String> {
    let before = &setups[c["setup"].as_str().unwrap()];
    let me = mesh_from_dump(before);
    // Entering Edit Mode: `EDBM_mesh_make` (from_mesh + select mode + flush).
    let mut bm = BMesh::edit_mesh_make(&me, mode_of(&c["mode"]));
    // `Mesh.act_face` is not part of frap's Mesh yet: restore it as `BM_mesh_bm_from_me` would.
    if let Some(i) = before["act_face"]
        .as_i64()
        .and_then(|i| usize::try_from(i).ok())
    {
        bm.act_face = bm.face_ids().get(i).copied();
    }
    let mut errs = Vec::new();
    let mut results = Vec::new();
    for s in c["steps"].as_array().unwrap() {
        results.push(run_step(&mut bm, s));
        let v = bm.validate();
        if !v.is_empty() {
            errs.push(format!("invalid after step {s}: {v:?}"));
            return errs;
        }
    }
    let b = &c["bm"];
    // Results: walk cases compare the selection as sets; others exactly.
    let bres = b["results"].as_array().unwrap();
    for (i, (r, br)) in results.iter().zip(bres).enumerate() {
        if r != br {
            errs.push(format!("step {i} result: frap {r} vs Blender {br}"));
        }
    }
    bm.index.dirty = ElemTypes::all();
    bm.elem_index_ensure(ElemTypes::all());
    let hist: Vec<J> = refs(&mut bm.clone(), &bm.select_history.clone())
        .as_array()
        .unwrap()
        .clone();
    if J::Array(hist.clone()) != b["history"] {
        errs.push(format!(
            "select history: frap {hist:?} vs Blender {}",
            b["history"]
        ));
    }
    let act = bm.act_face.map(|f| bm.index_of(BMElem::Face(f)));
    if json!(act) != b["act_face"] {
        errs.push(format!(
            "act_face: frap {act:?} vs Blender {}",
            b["act_face"]
        ));
    }
    let counts = json!([bm.verts_num(), bm.edges_num(), bm.faces_num()]);
    if counts != b["counts"] {
        errs.push(format!("counts: frap {counts} vs Blender {}", b["counts"]));
    }
    if let Some(vn) = b.get("vert_normals") {
        let fv: Vec<[f32; 3]> = bm.vert_ids().iter().map(|&v| bm.vert(v).no).collect();
        cmp_vec3s("vert normals", &fv, vn, &mut errs);
        let ff: Vec<[f32; 3]> = bm.face_ids().iter().map(|&f| bm.face(f).no).collect();
        cmp_vec3s("face normals", &ff, &b["face_normals"], &mut errs);
    }
    if let Some(after) = c.get("after") {
        let out = bm.to_mesh(&me, &ToMeshParams::default());
        // `mesh.select_mode` also re-derives the UV sync selection when flushing down from a
        // mixed selection (`ED_uvedit_sync_uvselect_ensure_if_needed`, UV editing: not part of
        // the BMesh core), so the UV selection attributes are not compared for those cases.
        let skip: &[&str] = if name.starts_with("selmode/") {
            &[".uv_select_vert", ".uv_select_edge", ".uv_select_face"]
        } else {
            &[]
        };
        errs.extend(compare_mesh(&out, after, skip));
    }
    errs.into_iter().map(|e| format!("{name}: {e}")).collect()
}

#[test]
fn bmesh_core_matches_blender() {
    let fx = fixture();
    let setups = &fx["setups"];
    let cases = fx["cases"].as_object().unwrap();
    assert!(cases.len() >= 100, "fixture has {} cases", cases.len());
    let filter = std::env::var("FRAP_CASE").ok();
    let mut failures = Vec::new();
    let mut ran = 0;
    for (name, c) in cases {
        if filter.as_ref().is_some_and(|f| !name.contains(f.as_str())) {
            continue;
        }
        ran += 1;
        let errs = std::panic::catch_unwind(|| run_case(name, c, setups))
            .unwrap_or_else(|_| vec![format!("{name}: panicked")]);
        failures.extend(errs);
    }
    assert!(ran > 0);
    if !failures.is_empty() {
        let n = failures.len();
        let text: Vec<String> = failures
            .into_iter()
            .map(|s| s.chars().take(1500).collect())
            .collect();
        panic!("{n} mismatches:\n{}", text.join("\n"));
    }
}

/// The setup meshes survive `from_mesh` → `to_mesh` unchanged when Blender's round trip leaves
/// them unchanged (it reorders the attributes of `from_pydata` meshes, see `roundtrip/attrs`).
#[test]
fn identity_round_trip_of_blender_primitives() {
    let fx = fixture();
    let setups = fx["setups"].as_object().unwrap();
    // The torus is a Python operator (`from_pydata`): its attributes are reordered by the
    // round trip (covered by the `roundtrip/torus` case).
    for name in [
        "cube",
        "cube_nouv",
        "plane",
        "circle_ngon",
        "circle_trifan",
        "cone",
        "cylinder",
        "grid",
        "ico",
        "monkey",
    ] {
        let me = mesh_from_dump(&setups[name]);
        let out = BMesh::from_mesh(&me, &FromMeshParams::default())
            .to_mesh(&me, &ToMeshParams::default());
        let errs = compare_mesh(&out, &setups[name], &[]);
        assert!(errs.is_empty(), "{name}: {errs:?}");
    }
}
