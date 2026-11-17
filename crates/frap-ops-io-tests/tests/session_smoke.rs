// SPDX-License-Identifier: GPL-3.0-or-later
//! Proves the test-only crate links a full `Session` with every operator registered (the
//! WP 1.23 save/open proptest builds on this).

use frap_session::{Session, SessionOptions};

#[test]
fn session_runs_an_io_operator() {
    let dir = std::env::temp_dir().join(format!("frap-ops-io-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut opts = SessionOptions::default();
    opts.cwd = dir.clone();
    let mut s = Session::new(opts).unwrap();
    let r = s.exec_source("bpy.ops.export_scene.gltf(filepath='smoke.glb')");
    assert!(r[0].ok(), "{}", r[0].to_json());
    assert!(dir.join("smoke.glb").exists());
}
