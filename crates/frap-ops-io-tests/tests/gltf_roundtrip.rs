// SPDX-License-Identifier: GPL-3.0-or-later
//! The differential glTF round-trip law (ARCHITECTURE §17.2, WP 1.23): for every file of
//! `tests/data/gltf-roundtrip/allow.toml`,
//! `gltf_canonical(frap import → frap export) == gltf_canonical(Blender import → Blender export)`
//! with default options, both starting from the empty scene. frap's side runs the operators in
//! a session.
//!
//! Blender's re-exports are committed as `tests/golden/fixtures/gltf/m1/roundtrip/<path with
//! '/' → '__'>.blender.glb`:
//! - `FRAP_ROUNDTRIP_BLESS=1` rewrites them with the located Blender 5.2
//!   (`frap_golden::blender::roundtrip`);
//! - `FRAP_GOLDEN_LIVE=1` re-runs Blender and fails when its output no longer matches the
//!   committed one (drift alarm), as the golden live mode does.

mod support;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Live, Repo, live, roundtrip};
use frap_golden::compare::gltf_roundtrip::AllowList;
use frap_golden::compare::{gltf_canonical, gltf_roundtrip};
use support::*;

const TOL: f32 = 1e-4;

fn reference_path(root: &Path, entry: &str) -> PathBuf {
    root.join("tests/golden/fixtures/gltf/m1/roundtrip")
        .join(format!("{}.blender.glb", entry.replace('/', "__")))
}

/// frap's side: import into the empty scene, export with default options.
fn frap_roundtrip(src: &Path) -> Result<Vec<u8>, String> {
    let d = Scratch::new("roundtrip");
    let mut s = session_in(d.path(), true);
    let src = src
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    for stmt in [
        format!("bpy.ops.import_scene.gltf(filepath='{src}')"),
        "bpy.ops.export_scene.gltf(filepath='roundtrip.glb')".to_owned(),
    ] {
        let r = run(&mut s, &stmt);
        if !r.ok() {
            return Err(format!("{stmt}: {}", r.to_json()));
        }
    }
    std::fs::read(d.path().join("roundtrip.glb")).map_err(|e| e.to_string())
}

#[test]
fn allow_list_parses_and_names_existing_files() {
    let root = repo_root();
    let allow = AllowList::load(&root.join("tests/data/gltf-roundtrip/allow.toml")).unwrap();
    assert!(
        allow.files.len() >= 10,
        "the M1 allow list has at least 10 files"
    );
    for f in &allow.files {
        assert!(
            root.join("tests/data").join(&f.path).is_file(),
            "{}",
            f.path
        );
    }
}

#[test]
fn roundtrip_law_on_the_allow_list() {
    let root = repo_root();
    let repo = Repo { root: root.clone() };
    let allow = AllowList::load(&root.join("tests/data/gltf-roundtrip/allow.toml")).unwrap();
    let bless = std::env::var_os("FRAP_ROUNDTRIP_BLESS").is_some_and(|v| v != "0" && !v.is_empty());
    let blender = if bless {
        Some(frap_golden::locate_blender().expect("FRAP_ROUNDTRIP_BLESS needs Blender 5.2"))
    } else {
        match live() {
            Live::Run(b) => Some(b),
            Live::Skip(why) => panic!("{why}"),
            Live::Off => None,
        }
    };
    let mut failures = Vec::new();
    for f in &allow.files {
        let src = root.join("tests/data").join(&f.path);
        let reference = reference_path(&root, &f.path);
        if let Some(b) = &blender {
            let fresh = roundtrip(b, &repo, &src).unwrap_or_else(|e| panic!("{}: {e}", f.path));
            if bless {
                std::fs::create_dir_all(reference.parent().unwrap()).unwrap();
                std::fs::write(&reference, &fresh).unwrap();
            } else {
                let committed = std::fs::read(&reference).unwrap();
                let canon = |b: &[u8]| {
                    gltf_canonical::load(b, None).and_then(|d| gltf_canonical::canonicalize(&d))
                };
                let drift = gltf_canonical::compare(
                    &f.path,
                    &canon(&fresh).unwrap(),
                    &canon(&committed).unwrap(),
                    TOL,
                );
                if !drift.is_empty() {
                    failures.push(format!(
                        "{}: Blender drifted from the committed reference",
                        f.path
                    ));
                }
            }
        }
        let blender_out = std::fs::read(&reference).unwrap_or_else(|e| {
            panic!(
                "{}: {e} (bless with FRAP_ROUNDTRIP_BLESS=1)",
                reference.display()
            )
        });
        let frap_out = match frap_roundtrip(&src) {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("{}: frap failed: {e}", f.path));
                continue;
            }
        };
        let m = gltf_roundtrip::check(&f.path, &allow, &frap_out, None, &blender_out, None, TOL);
        if !m.is_empty() {
            let lines: Vec<String> = m.iter().take(8).map(|x| format!("  {x}")).collect();
            failures.push(format!("{}:\n{}", f.path, lines.join("\n")));
        }
    }
    assert!(
        failures.is_empty(),
        "round-trip law failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn a_file_off_the_list_is_reported() {
    let allow = AllowList::default();
    let m = gltf_roundtrip::check("gltf-samples/X.glb", &allow, b"", None, b"", None, TOL);
    assert_eq!(m.len(), 1);
    assert!(m[0].detail.contains("allow list"));
}
