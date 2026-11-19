// SPDX-License-Identifier: GPL-3.0-or-later
//! Live mode (ARCHITECTURE §20.5): with `FRAP_GOLDEN_LIVE=1` and a Blender 5.2 binary
//! (`$BLENDER_BIN`, then `blender` on `PATH`, then `~/Applications/Blender-5.2.app`), re-runs
//! `tests/blender/dump_uv.py` and checks that the committed fixture
//! (`tests/fixtures/uv.blender.json`) has not drifted. Skipped with a notice otherwise.

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
fn uv_fixture_matches_live_blender() {
    if std::env::var("FRAP_GOLDEN_LIVE").as_deref() != Ok("1") {
        eprintln!("NOTICE: uv_fixture_matches_live_blender skipped (set FRAP_GOLDEN_LIVE=1)");
        return;
    }
    let Some(bin) = blender() else {
        eprintln!("NOTICE: uv_fixture_matches_live_blender skipped (Blender 5.2 not found)");
        return;
    };
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = std::env::temp_dir().join(format!("frap-uv-{}.json", std::process::id()));
    let status = Command::new(&bin)
        .args(["-b", "--factory-startup", "--python"])
        .arg(dir.join("tests/blender/dump_uv.py"))
        .arg("--")
        .arg("--out")
        .arg(&out)
        .status()
        .expect("run Blender");
    assert!(status.success(), "Blender failed");
    let read =
        |p: &PathBuf| -> J { serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap() };
    let live = read(&out);
    let committed = read(&dir.join("tests/fixtures/uv.blender.json"));
    let _ = std::fs::remove_file(&out);
    assert_eq!(live["setups"], committed["setups"], "setup meshes drifted");
    let (lc, cc) = (
        live["cases"].as_object().unwrap(),
        committed["cases"].as_object().unwrap(),
    );
    assert_eq!(lc.len(), cc.len(), "case count drifted");
    for (name, c) in cc {
        assert_eq!(&lc[name], c, "case {name} drifted");
    }
}
