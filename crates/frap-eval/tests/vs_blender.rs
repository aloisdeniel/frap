// SPDX-License-Identifier: GPL-3.0-or-later
//! World matrices (OBJECT, VERTEX, VERTEX_3 parents, `matrix_parent_inverse`, every rotation
//! mode, delta channels), `dimensions` and `bound_box` against Blender 5.2.4
//! (`tests/data/eval_cases.json`, written by `tests/blender/dump_eval_cases.py`).

mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use common::TestHost;
use frap_eval::{EvalCache, bound_box, dimensions};
use frap_rna::{IdRef, ObjectId};
use frap_scene::{EvalMode, Evaluator};
use glam::Mat4;
use serde_json::Value as J;

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/eval_cases.json")
}

fn f32s<const N: usize>(j: &J) -> [f32; N] {
    let a = j.as_array().expect("array");
    std::array::from_fn(|i| a[i].as_f64().expect("number") as f32)
}

/// Row-major JSON rows → glam (column-major).
fn mat(j: &J) -> Mat4 {
    let r: Vec<[f32; 4]> = j.as_array().expect("rows").iter().map(f32s::<4>).collect();
    Mat4::from_cols_array_2d(&std::array::from_fn(|c| {
        std::array::from_fn(|row| r[row][c])
    }))
}

/// Builds the dumped scene; returns the host and the objects in dump order.
fn build(j: &J) -> (TestHost, Vec<(IdRef<ObjectId>, J)>) {
    let mut h = TestHost::factory();
    let mut meshes = HashMap::new();
    for m in j["meshes"].as_array().expect("meshes") {
        let verts: Vec<[f32; 3]> = m["positions"]
            .as_array()
            .expect("positions")
            .iter()
            .map(f32s::<3>)
            .collect();
        let faces: Vec<Vec<i32>> = m["faces"]
            .as_array()
            .expect("faces")
            .iter()
            .map(|f| {
                f.as_array()
                    .expect("face")
                    .iter()
                    .map(|v| v.as_i64().expect("index") as i32)
                    .collect()
            })
            .collect();
        let name = m["name"].as_str().expect("name");
        meshes.insert(name.to_owned(), h.add_mesh(name, &verts, &faces));
    }
    let mut out = Vec::new();
    for o in j["objects"].as_array().expect("objects") {
        let name = o["name"].as_str().expect("name");
        let data = o["data"].as_str().map(|d| meshes[d].any());
        let ob = h.add_object(name, data);
        assert_eq!(h.main.objects.name_of(ob), Some(name), "no name collision");
        let parent = o["parent"].as_str().map(|p| h.obj(p));
        h.edit_obj(ob, |x| {
            x.location = f32s(&o["location"]);
            x.rotation_mode = o["rotation_mode"].as_str().expect("mode").into();
            x.rotation_euler = f32s(&o["rotation_euler"]);
            x.rotation_quaternion = f32s(&o["rotation_quaternion"]);
            x.rotation_axis_angle = f32s(&o["rotation_axis_angle"]);
            x.scale = f32s(&o["scale"]);
            x.delta_location = f32s(&o["delta_location"]);
            x.delta_rotation_euler = f32s(&o["delta_rotation_euler"]);
            x.delta_rotation_quaternion = f32s(&o["delta_rotation_quaternion"]);
            x.delta_scale = f32s(&o["delta_scale"]);
            x.parent = parent;
            x.parent_type = o["parent_type"].as_str().expect("type").into();
            let pv: [f32; 3] = f32s(&o["parent_vertices"]);
            x.parent_vertices = pv.map(|v| v as i32);
            x.matrix_parent_inverse = mat(&o["matrix_parent_inverse"]);
        });
        out.push((ob, o.clone()));
    }
    (h, out)
}

fn ulps(a: f32, b: f32) -> u32 {
    if a == b {
        return 0;
    }
    let (ia, ib) = (a.to_bits() as i32, b.to_bits() as i32);
    if (ia < 0) != (ib < 0) {
        return u32::MAX;
    }
    ia.abs_diff(ib)
}

#[test]
fn world_matrices_dimensions_and_bound_box_equal_blender() {
    let j: J = serde_json::from_str(&std::fs::read_to_string(data_path()).expect("fixture"))
        .expect("json");
    let (h, obs) = build(&j);
    let mut eval = EvalCache::new();
    let mut failures = Vec::new();
    for (ob, o) in &obs {
        let name = o["name"].as_str().expect("name");
        let eo = eval.object(&h, *ob, EvalMode::Viewport).expect("evaluates");
        let want = mat(&o["matrix_world"]);
        let got = eo.matrix_world;
        // Roots and OBJECT/VERTEX parents are bit-exact. VERTEX_3 goes through acosf, atan2f,
        // sinf and cosf: frap uses the portable `libm` crate (rule 2.2.12) where Blender uses
        // the platform's, which differs in the last bit (1 ulp on `Vertex3Degenerate` against
        // macOS), so a few ulps are allowed there (and an absolute 1e-7 near zero).
        let max_ulps = if o["parent_type"] == "VERTEX_3" { 4 } else { 0 };
        let worst = want
            .to_cols_array()
            .iter()
            .zip(got.to_cols_array())
            .map(|(a, b)| {
                let u = ulps(*a, b);
                if max_ulps > 0 && (a - b).abs() < 1e-7 {
                    u.min(max_ulps)
                } else {
                    u
                }
            })
            .max()
            .unwrap_or(0);
        if worst > max_ulps {
            failures.push(format!(
                "{name}: matrix_world differs by {worst} ulps\n  want {want:?}\n  got  {got:?}"
            ));
        }
        let dims = dimensions(&eo);
        if dims != f32s::<3>(&o["dimensions"]) {
            failures.push(format!(
                "{name}: dimensions {dims:?} != {}",
                o["dimensions"]
            ));
        }
        let bb = bound_box(&eo);
        let want_bb: Vec<[f32; 3]> = o["bound_box"]
            .as_array()
            .expect("bound_box")
            .iter()
            .map(f32s::<3>)
            .collect();
        if bb.to_vec() != want_bb {
            failures.push(format!("{name}: bound_box {bb:?} != {want_bb:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn bit_exact_for_object_parents() {
    // Separate from the test above so a regression in the exact path is named on its own.
    let j: J = serde_json::from_str(&std::fs::read_to_string(data_path()).expect("fixture"))
        .expect("json");
    let (h, obs) = build(&j);
    let mut eval = EvalCache::new();
    for (ob, o) in &obs {
        if o["parent_type"] == "VERTEX_3" {
            continue;
        }
        let eo = eval.object(&h, *ob, EvalMode::Viewport).expect("evaluates");
        let want = mat(&o["matrix_world"]).to_cols_array().map(f32::to_bits);
        assert_eq!(
            eo.matrix_world.to_cols_array().map(f32::to_bits),
            want,
            "{}",
            o["name"]
        );
    }
}

#[test]
fn dump_matches_live_blender() {
    let bin = std::env::var_os("BLENDER_BIN")
        .map(PathBuf::from)
        .or_else(|| {
            let p = PathBuf::from(std::env::var_os("HOME")?)
                .join("Applications/Blender-5.2.app/Contents/MacOS/Blender");
            p.is_file().then_some(p)
        });
    let Some(bin) = bin else {
        eprintln!(
            "NOTICE: dump_matches_live_blender skipped (Blender 5.2 not found; set BLENDER_BIN)"
        );
        return;
    };
    let out = std::env::temp_dir().join(format!("frap-eval-cases-{}.json", std::process::id()));
    let status = std::process::Command::new(bin)
        .args(["-b", "--factory-startup", "--python"])
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/blender/dump_eval_cases.py"))
        .args(["--", "--out"])
        .arg(&out)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("run Blender");
    assert!(status.success(), "Blender failed");
    let live: J =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("live dump")).expect("json");
    let _ = std::fs::remove_file(&out);
    let committed: J =
        serde_json::from_str(&std::fs::read_to_string(data_path()).expect("fixture"))
            .expect("json");
    assert_eq!(
        live["objects"], committed["objects"],
        "re-run the dump script"
    );
    assert_eq!(
        live["meshes"], committed["meshes"],
        "re-run the dump script"
    );
}
