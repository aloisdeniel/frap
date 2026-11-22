// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared harness of the §21.7 budget tests (WP 8.8).
//!
//! - **Gating.** A budget is enforced only in release builds (`cargo test -p frap-perf-tests
//!   --release`, which `cargo xtask perf` runs); a debug run (the workspace gate) measures the
//!   same scenes, smaller where [`release`] says so, and only checks that they run and give
//!   the expected result.
//! - **Measurements.** Every measurement is printed as one `FRAP_PERF {json}` line on stdout
//!   and, when `FRAP_PERF_OUT` names a file, appended to it as one JSON line; `cargo xtask
//!   perf` collects that file into its report and `--record` writes it to `xtask/perf.toml`.
//! - **Noise.** Timed sections take the median of several repetitions and never overlap: the
//!   tests of one binary serialise on [`serial`].
#![allow(dead_code)]

use std::io::Write as _;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use frap_session::{Response, Session, SessionOptions, Startup, Verbosity};
use serde_json::Value as J;

/// True in release builds: budgets gate, scenes have their full §21.7 sizes.
pub const fn release() -> bool {
    !cfg!(debug_assertions)
}

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialises the timed tests of one test binary (a poisoned lock is still a lock).
pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}

/// A session with the given startup, its working directory a scratch directory.
pub fn session(startup: Startup) -> Session {
    let mut o = SessionOptions::default();
    o.cwd = std::env::temp_dir();
    o.startup = startup;
    Session::new(o).expect("session starts")
}

/// A factory-startup session.
pub fn factory() -> Session {
    session(Startup::Factory)
}

/// A factory-startup session answering with `verbosity`.
pub fn factory_with(verbosity: Verbosity) -> Session {
    let mut o = SessionOptions::default();
    o.cwd = std::env::temp_dir();
    o.verbosity = verbosity;
    Session::new(o).expect("session starts")
}

/// Runs one statement and checks that it finished; its response.
pub fn ok(s: &mut Session, src: &str) -> J {
    let mut v: Vec<Response> = s.exec_source(src);
    assert_eq!(v.len(), 1, "{src}: one statement");
    let r = v.remove(0).to_json();
    assert_eq!(r["status"], "FINISHED", "{src}: {r}");
    r
}

/// Runs one statement; its `value` (an expression).
pub fn value(s: &mut Session, src: &str) -> J {
    ok(s, src)["value"].clone()
}

/// Runs one statement whose value is an integer.
pub fn int(s: &mut Session, src: &str) -> i64 {
    let v = value(s, src);
    v.as_i64().unwrap_or_else(|| panic!("{src}: not an int: {v}"))
}

/// Runs `src` as one statement, timed as a front-end sees it: execution plus the JSON
/// encoding of the response. Returns the elapsed time and the response.
pub fn timed(s: &mut Session, src: &str) -> (Duration, J) {
    let t = Instant::now();
    let mut v = s.exec_source(src);
    let r = v.remove(0).to_json();
    let text = serde_json::to_string(&r).expect("response encodes");
    let d = t.elapsed();
    assert!(!text.is_empty());
    assert_eq!(r["status"], "FINISHED", "{src}: {r}");
    (d, r)
}

/// The median of `xs` (sorted in place; an even count takes the upper middle).
pub fn median(xs: &mut [Duration]) -> Duration {
    assert!(!xs.is_empty());
    xs.sort();
    xs[xs.len() / 2]
}

/// Milliseconds as a float.
pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// One measurement: printed, appended to `FRAP_PERF_OUT`, and in release builds checked
/// against `budget_ms` (`None`: recorded only, gated elsewhere by a ratio or a counter).
pub fn report(name: &str, measured_ms: f64, budget_ms: Option<f64>, extra: J) {
    let line = serde_json::json!({
        "name": name,
        "ms": (measured_ms * 1000.0).round() / 1000.0,
        "budget_ms": budget_ms,
        "release": release(),
        "extra": extra,
    });
    println!("FRAP_PERF {line}");
    if let Some(path) = std::env::var_os("FRAP_PERF_OUT") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("FRAP_PERF_OUT opens");
        writeln!(f, "{line}").expect("FRAP_PERF_OUT writes");
    }
    if let Some(b) = budget_ms
        && release()
    {
        assert!(
            measured_ms < b,
            "§21.7 budget exceeded: {name} took {measured_ms:.2} ms (budget {b} ms)"
        );
    }
}

/// `(vertices, edges, faces)` of the active object's mesh as Python reads it (the edit-mode
/// view in Edit Mode, §6.3).
pub fn counts(s: &mut Session) -> (i64, i64, i64) {
    (
        len(s, "bpy.context.object.data.vertices"),
        len(s, "bpy.context.object.data.edges"),
        len(s, "bpy.context.object.data.polygons"),
    )
}

/// The `len` of a collection read (`{"$coll": …, "len": n}`, §13.3).
pub fn len(s: &mut Session, path: &str) -> i64 {
    let v = value(s, path);
    v["len"]
        .as_i64()
        .unwrap_or_else(|| panic!("{path}: not a collection: {v}"))
}
