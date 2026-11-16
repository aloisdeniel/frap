// SPDX-License-Identifier: GPL-3.0-or-later
//! Live mode (ARCHITECTURE §20.5): with `FRAP_GOLDEN_LIVE=1` and a Blender 5.2 binary
//! (`$BLENDER_BIN`, then `blender` on `PATH`, then `~/Applications/Blender-5.2.app`), re-runs
//! `tools/blender/dump_primitives.py` and checks the committed fixtures have not drifted. The UV
//! sphere's edge/face order varies between Blender runs (see `primitives/uv_sphere.rs`), so only
//! its order-stable fields are compared. Skipped with a notice otherwise.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value as J;

fn blender() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BLENDER_BIN") {
        return Some(PathBuf::from(p));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let c = dir.join("blender");
            if c.is_file() {
                return Some(c);
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let mac = home.join("Applications/Blender-5.2.app/Contents/MacOS/Blender");
    mac.is_file().then_some(mac)
}

#[test]
fn dump_matches_live_blender() {
    if std::env::var("FRAP_GOLDEN_LIVE").as_deref() != Ok("1") {
        eprintln!("NOTICE: dump_matches_live_blender skipped (set FRAP_GOLDEN_LIVE=1 to run)");
        return;
    }
    let Some(bin) = blender() else {
        eprintln!("NOTICE: dump_matches_live_blender skipped (Blender 5.2 not found)");
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::env::temp_dir().join(format!("frap-primitives-live-{}", std::process::id()));
    let status = Command::new(&bin)
        .args(["-b", "--factory-startup", "--python"])
        .arg(root.join("tools/blender/dump_primitives.py"))
        .arg("--")
        .arg("--out")
        .arg(&out)
        .status()
        .expect("run Blender");
    assert!(status.success(), "Blender failed");
    let committed = root.join("spec/blender-5.2/primitives");
    let mut checked = 0;
    for entry in std::fs::read_dir(&committed).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == "_meta.json" || !name.ends_with(".json") {
            continue;
        }
        let a: J = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let b: J =
            serde_json::from_str(&std::fs::read_to_string(out.join(&name)).unwrap()).unwrap();
        if name.starts_with("uv_sphere__") || name == "mesh__smooth_uv_sphere.json" {
            for k in ["positions", "normals_domain", "name"] {
                assert_eq!(a["mesh"][k], b["mesh"][k], "{name}: {k} drifted");
            }
            // Vertex normals sum face normals in face order, which varies between runs.
            let flat = |v: &J| -> Vec<f64> {
                v.as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|x| x.as_array().unwrap().iter().map(|c| c.as_f64().unwrap()))
                    .collect()
            };
            let (na, nb) = (
                flat(&a["mesh"]["vertex_normals"]),
                flat(&b["mesh"]["vertex_normals"]),
            );
            assert_eq!(na.len(), nb.len(), "{name}: vertex normal count");
            for (x, y) in na.iter().zip(&nb) {
                assert!(
                    (x - y).abs() < 1e-6,
                    "{name}: vertex normals drifted ({x} vs {y})"
                );
            }
        } else {
            assert_eq!(a, b, "{name} drifted from live Blender");
        }
        checked += 1;
    }
    let _ = std::fs::remove_dir_all(&out);
    assert!(checked > 50, "{checked} fixtures checked");
}
