// SPDX-License-Identifier: GPL-3.0-or-later
//! §21.7: "cold start < 30 ms without GPU init": from spawning `frap --empty` to reading its
//! ready event on stdout (process start, operator and RNA registries, the empty scene). The
//! GPU is never touched before the first render (the render service is created lazily).
//!
//! The binary is the `frap` of this build's profile (`target/<profile>/frap`, built by
//! `cargo xtask perf`; `FRAP_BIN` overrides it). A release run without the binary fails; a
//! debug run without it only measures the in-process start.

mod support;

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use frap_session::Startup;
use serde_json::json;
use support::*;

/// `target/<profile>/frap`: this test runs from `target/<profile>/deps/`.
fn frap_bin() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("FRAP_BIN") {
        return Some(PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.parent()?;
    let bin = dir.join(format!("frap{}", std::env::consts::EXE_SUFFIX));
    bin.is_file().then_some(bin)
}

/// Spawns `frap --empty`; the time to its first stdout line, and that line.
fn start(bin: &PathBuf) -> (Duration, String) {
    let t = Instant::now();
    let mut child = Command::new(bin)
        .arg("--empty")
        .env_remove("FRAP_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("frap starts");
    let mut out = BufReader::new(child.stdout.take().expect("stdout"));
    let mut line = String::new();
    out.read_line(&mut line).expect("ready event");
    let d = t.elapsed();
    // EOF on stdin: frap prints its end event and exits 0.
    drop(child.stdin.take());
    let mut rest = String::new();
    out.read_line(&mut rest).expect("end event");
    assert!(rest.contains("\"event\":\"end\""), "{rest}");
    assert!(child.wait().expect("frap exits").success());
    (d, line)
}

#[test]
fn cold_start_empty_to_ready_event() {
    let _g = serial();
    let Some(bin) = frap_bin() else {
        assert!(
            !release(),
            "no frap binary next to the test (run `cargo build --release -p frap-cli` first; `cargo xtask perf` does)"
        );
        eprintln!("cold_start: no debug frap binary built; skipped");
        return;
    };
    // The first start pays for the page cache and code signing checks, not frap's start-up.
    let _ = start(&bin);
    let runs = if release() { 15 } else { 3 };
    let mut times = Vec::new();
    for _ in 0..runs {
        let (d, line) = start(&bin);
        let ev: serde_json::Value = serde_json::from_str(line.trim()).expect("ready JSON");
        assert_eq!(ev["event"], "ready", "{line}");
        assert_eq!(ev["startup"], "empty", "{line}");
        times.push(d);
    }
    let med = median(&mut times);
    report(
        "cold_start",
        ms(med),
        Some(30.0),
        json!({ "min_ms": ms(times[0]), "max_ms": ms(*times.last().expect("runs")), "binary": bin }),
    );
}

#[test]
fn in_process_start_needs_no_gpu() {
    let _g = serial();
    let mut times = Vec::new();
    for _ in 0..if release() { 15 } else { 3 } {
        let t = Instant::now();
        let s = session(Startup::Empty);
        times.push(t.elapsed());
        assert!(!s.render_service_started(), "start-up initialised the GPU");
        assert_eq!(s.main().objects.len(), 0);
    }
    let mut s = session(Startup::Empty);
    ok(&mut s, "bpy.ops.mesh.primitive_cube_add()");
    assert!(!s.render_service_started(), "an operator initialised the GPU");
    // The first start in a process pays for the lazily built registries, as `frap` does.
    let first = ms(times[0]);
    report(
        "session_new_empty",
        ms(median(&mut times)),
        None,
        json!({ "first_ms": first }),
    );
}
