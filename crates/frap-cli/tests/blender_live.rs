// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end checks of the M0 exit transcript's GLB:
//! - validated by the independent `gltf =1.4.1` reader (always runs);
//! - imported by Blender 5.2's `import_scene.gltf`, compared with Blender's own export of the
//!   same transcript (runs when Blender is found, otherwise skipped with a visible notice);
//! - the committed spec/m0 dumps equal a fresh dump from the live Blender (same condition).
//!
//! Blender is located like `frap-golden::locate_blender` (§20.3): `$BLENDER_BIN`, then `blender`
//! on PATH, then the usual install locations. `FRAP_REQUIRE_BLENDER=1` turns a skip into a failure.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as J;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("frap-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs `frap run skeleton.py` in a fresh directory; returns (dir, stdout lines).
fn run_transcript(name: &str) -> (PathBuf, Vec<J>) {
    let d = tmpdir(name);
    std::fs::copy(
        repo().join("tests/cli/m0/skeleton.in/skeleton.py"),
        d.join("skeleton.py"),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_frap"))
        .arg("run")
        .arg("skeleton.py")
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (d, lines)
}

fn locate_blender() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BLENDER_BIN") {
        return Some(PathBuf::from(p));
    }
    let exe = if cfg!(windows) {
        "blender.exe"
    } else {
        "blender"
    };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let c = dir.join(exe);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    [
        home.join("Applications/Blender-5.2.app/Contents/MacOS/Blender"),
        home.join("Applications/Blender.app/Contents/MacOS/Blender"),
        PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

fn blender_or_skip(test: &str) -> Option<PathBuf> {
    let b = locate_blender();
    if b.is_none() {
        if std::env::var_os("FRAP_REQUIRE_BLENDER").is_some() {
            panic!("{test}: FRAP_REQUIRE_BLENDER is set but no Blender was found");
        }
        eprintln!("SKIPPED {test}: Blender 5.2 not found (set BLENDER_BIN)");
    }
    b
}

#[test]
fn transcript_glb_passes_gltf_crate_validation() {
    let (d, lines) = run_transcript("validate");
    // ready + 5 responses + end, all ok.
    assert_eq!(lines.len(), 7);
    let kinds: Vec<&str> = lines[1..6]
        .iter()
        .map(|l| l["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["op", "op", "set", "get", "op"]);
    assert!(lines[1..6].iter().all(|l| l["ok"] == true));
    let bytes = std::fs::read(d.join("out/m0.glb")).unwrap();
    let g = gltf::Gltf::from_slice(&bytes).expect("gltf =1.4.1 validation");
    let names: Vec<_> = g
        .document
        .nodes()
        .map(|n| n.name().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Cube", "Cube.001", "Cube.002"]);
    let t: Vec<[f32; 3]> = g
        .document
        .nodes()
        .map(|n| n.transform().decomposed().0)
        .collect();
    assert_eq!(
        t,
        [[1.0, 3.0, -2.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        "Y-up translations"
    );
    for m in g.document.meshes() {
        let p = m.primitives().next().unwrap();
        assert_eq!(p.get(&gltf::Semantic::Positions).unwrap().count(), 24);
        assert!(p.get(&gltf::Semantic::Normals).is_some());
        assert!(p.get(&gltf::Semantic::TexCoords(0)).is_some());
    }
}

#[test]
fn transcript_glb_imports_in_blender_like_blenders_own_export() {
    let Some(blender) = blender_or_skip("transcript_glb_imports_in_blender") else {
        return;
    };
    let (d, _) = run_transcript("blender-import");
    let out_json = d.join("import.json");
    let status = Command::new(&blender)
        .args(["-b", "--factory-startup", "--python"])
        .arg(repo().join("tests/cli/m0/blender_import_check.py"))
        .arg("--")
        .arg(d.join("out/m0.glb"))
        .arg(&out_json)
        .arg(&d)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "Blender failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let j: J = serde_json::from_str(&std::fs::read_to_string(&out_json).unwrap()).unwrap();
    assert!(
        j["blender"].as_str().unwrap().starts_with("5.2"),
        "expected Blender 5.2, got {}",
        j["blender"]
    );
    assert_eq!(
        j["frap"]["result"],
        serde_json::json!(["FINISHED"]),
        "{}",
        j["frap"]
    );
    let objs = j["frap"]["objects"].as_object().unwrap();
    assert_eq!(
        objs.keys().collect::<Vec<_>>(),
        ["Cube", "Cube.001", "Cube.002"]
    );
    assert_eq!(objs["Cube"]["location"], serde_json::json!([1.0, 2.0, 3.0]));
    assert_eq!(
        objs["Cube.001"]["location"],
        serde_json::json!([0.0, 0.0, 1.0])
    );
    assert_eq!(
        objs["Cube"]["mesh"]["materials"],
        serde_json::json!(["Material"])
    );
    // Differential: importing frap's GLB gives exactly what importing Blender's GLB gives.
    assert_eq!(
        j["frap"], j["blender_export"],
        "frap import != Blender export import"
    );
}

#[test]
fn m0_spec_matches_live_blender() {
    let Some(blender) = blender_or_skip("m0_spec_matches_live_blender") else {
        return;
    };
    let d = tmpdir("spec-dump");
    let out = Command::new(&blender)
        .args(["-b", "--factory-startup", "--python"])
        .arg(repo().join("spec/m0/dump_m0.py"))
        .arg("--")
        .arg(&d)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let committed = repo().join("spec/m0");
    let mut n = 0;
    for sub in ["ops", "rna", "primitives", "."] {
        for e in std::fs::read_dir(d.join(sub)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let rel = p.strip_prefix(&d).unwrap();
            let mut fresh: J = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let mut old: J = serde_json::from_str(
                &std::fs::read_to_string(committed.join(rel))
                    .unwrap_or_else(|_| panic!("missing committed {rel:?}")),
            )
            .unwrap();
            fresh.as_object_mut().unwrap().remove("_stamp");
            old.as_object_mut().unwrap().remove("_stamp");
            assert_eq!(
                fresh,
                old,
                "spec/m0/{} drifted from the live Blender",
                rel.display()
            );
            n += 1;
        }
    }
    assert!(n >= 13, "only {n} dump files compared");
}
