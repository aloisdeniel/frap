// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask perf`: the performance budgets of ARCHITECTURE §21.7 (M8 WP 8.8).
//!
//! ```text
//! cargo xtask perf [--record]                 build `frap` (release), run the release-mode
//!                                             budget tests of crates/frap-perf-tests, report
//! cargo xtask perf build-budgets [--record]   the frap-spec-* build-time budgets (`budgets`)
//! cargo xtask perf blender [--blender <path>] Blender's side of the same scenes (reference)
//! ```
//!
//! - **Runtime budgets.** The tests gate by themselves (a release build fails a test over its
//!   budget); this command builds the `frap` binary the cold-start test spawns, runs the tests
//!   one at a time with `FRAP_PERF_OUT` set, and prints every measurement next to its budget
//!   and the value recorded for this runner class in `xtask/perf.toml`. A budget in a test that
//!   differs from `[budgets_ms]` fails the run (one source of truth, checked both ways); a
//!   measurement over `regression_factor` × its recorded value is a warning. `--record` writes
//!   the measurements under `[recorded."<runner class>"]`.
//! - **Build-time budgets.** `build-budgets` runs `cargo xtask budgets`. Its baselines gate once
//!   they were recorded on the CI runner class: `cargo xtask budgets --record` writes
//!   `recorded_on = "<runner class>"` beside `[baseline_secs]` in `xtask/budgets.toml`, so
//!   recording on ubuntu-24.04 (`budgets --record`, or `build-budgets --record`, which refuses
//!   other classes) makes an over-budget crate fail `build-budgets` there. Before (no
//!   `recorded_on`: the provisional M1 Max baselines) and on other runner classes it is a
//!   warning.
//! - **Runner class.** `$FRAP_RUNNER_CLASS`, else `ubuntu-24.04` / `macos-15` / … on GitHub
//!   Actions (`$ImageOS`), else `local-<os>-<arch>`.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The CI runner class the budgets gate on (§22.10 8.8).
pub const CI_CLASS: &str = "ubuntu-24.04";

const USAGE: &str = "usage: cargo xtask perf [--record] | perf build-budgets [--record] | perf blender [--blender <path>]";

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("build-budgets") => build_budgets(&args[1..]),
        Some("blender") => blender(&args[1..]),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => runtime(args),
    }
}

fn perf_toml() -> PathBuf {
    crate::workspace_root().join("xtask/perf.toml")
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::workspace_root().join("target"))
}

/// The runner class (module docs).
pub fn runner_class(env: &dyn Fn(&str) -> Option<String>) -> String {
    if let Some(c) = env("FRAP_RUNNER_CLASS").filter(|c| !c.is_empty()) {
        return c;
    }
    if env("GITHUB_ACTIONS").is_some()
        && let Some(image) = env("ImageOS")
    {
        // `ImageOS` is `ubuntu24`, `macos15`, `win22`, …
        let (os, ver) = image.split_at(
            image
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or(image.len()),
        );
        return match os {
            "ubuntu" if !ver.is_empty() => format!("ubuntu-{ver}.04"),
            "macos" if !ver.is_empty() => format!("macos-{ver}"),
            "win" if !ver.is_empty() => format!("windows-20{ver}"),
            _ => image,
        };
    }
    format!("local-{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn env_var(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

fn cargo(args: &[&str], envs: &[(&str, &Path)]) -> Result<(), String> {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.args(args).current_dir(crate::workspace_root());
    for (k, v) in envs {
        c.env(k, v);
    }
    let st = c.status().map_err(|e| format!("cannot run cargo: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

/// One `FRAP_PERF` line of a test.
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    pub name: String,
    pub ms: f64,
    pub budget_ms: Option<f64>,
}

/// Parses the `FRAP_PERF_OUT` file (one JSON object per line); a name measured twice keeps
/// its last value.
pub fn parse_measurements(text: &str) -> Result<Vec<Measurement>, String> {
    let mut out: Vec<Measurement> = Vec::new();
    for (i, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("measurement line {}: {e}", i + 1))?;
        let name = v["name"]
            .as_str()
            .ok_or_else(|| format!("measurement line {}: no name", i + 1))?
            .to_owned();
        let ms = v["ms"]
            .as_f64()
            .ok_or_else(|| format!("measurement line {}: no ms", i + 1))?;
        let m = Measurement {
            name,
            ms,
            budget_ms: v["budget_ms"].as_f64(),
        };
        match out.iter_mut().find(|o| o.name == m.name) {
            Some(o) => *o = m,
            None => out.push(m),
        }
    }
    Ok(out)
}

/// What `perf.toml` says about one runner class.
#[derive(Debug)]
pub struct PerfConfig {
    pub budgets_ms: BTreeMap<String, f64>,
    pub regression_factor: f64,
    pub recorded: BTreeMap<String, f64>,
}

impl PerfConfig {
    pub fn parse(doc: &toml::Table, class: &str) -> Result<PerfConfig, String> {
        let floats = |t: Option<&toml::Value>| -> BTreeMap<String, f64> {
            t.and_then(|t| t.as_table())
                .map(|t| {
                    t.iter()
                        .filter_map(|(k, v)| {
                            v.as_float()
                                .or_else(|| v.as_integer().map(|i| i as f64))
                                .map(|f| (k.clone(), f))
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let budgets_ms = floats(doc.get("budgets_ms"));
        if budgets_ms.is_empty() {
            return Err("perf.toml: missing [budgets_ms]".into());
        }
        let regression_factor = doc
            .get("regression_factor")
            .and_then(|f| f.as_float())
            .ok_or("perf.toml: missing `regression_factor`")?;
        let recorded = floats(
            doc.get("recorded")
                .and_then(|r| r.as_table())
                .and_then(|r| r.get(class)),
        );
        Ok(PerfConfig {
            budgets_ms,
            regression_factor,
            recorded,
        })
    }
}

/// One row of the report, and the problems it raises.
#[derive(Debug, Default)]
pub struct Verdict {
    pub rows: Vec<String>,
    /// Budget mismatches and budgets exceeded (fail the run).
    pub errors: Vec<String>,
    /// Regressions against the recorded values (warnings).
    pub warnings: Vec<String>,
}

/// Compares the measurements of a release run with `perf.toml` (module docs).
pub fn judge(ms: &[Measurement], cfg: &PerfConfig) -> Verdict {
    let mut v = Verdict::default();
    for m in ms {
        let budget = cfg.budgets_ms.get(&m.name).copied();
        if m.budget_ms != budget {
            v.errors.push(format!(
                "{}: the test's budget {:?} ms differs from perf.toml [budgets_ms] {:?}",
                m.name, m.budget_ms, budget
            ));
        }
        let mut status = String::new();
        if let Some(b) = budget {
            if m.ms >= b {
                v.errors
                    .push(format!("{}: {:.2} ms over its {b} ms budget", m.name, m.ms));
                status.push_str("OVER BUDGET");
            } else {
                status.push_str(&format!("ok (budget {b} ms)"));
            }
        } else {
            status.push_str("tracked");
        }
        if let Some(r) = cfg.recorded.get(&m.name) {
            status.push_str(&format!(", recorded {r:.3} ms"));
            // Sub-millisecond values are timer noise more than signal.
            if m.ms > cfg.regression_factor * r && m.ms - r > 1.0 {
                v.warnings.push(format!(
                    "{}: {:.2} ms > {} × the recorded {:.2} ms",
                    m.name, m.ms, cfg.regression_factor, r
                ));
                status.push_str(" REGRESSION");
            }
        }
        v.rows
            .push(format!("{:<48} {:>10.3} ms  {status}", m.name, m.ms));
    }
    for name in cfg.budgets_ms.keys() {
        if !ms.iter().any(|m| &m.name == name) {
            v.errors
                .push(format!("{name}: a perf.toml budget no test measured"));
        }
    }
    v
}

/// The leading comment block of a TOML file (kept when it is rewritten).
fn header(text: &str) -> String {
    text.lines()
        .take_while(|l| l.starts_with('#') || l.is_empty())
        .map(|l| format!("{l}\n"))
        .collect()
}

/// Rewrites `perf.toml` with `f` applied (the leading comment block is kept).
fn edit_perf_toml(f: impl FnOnce(&mut toml::Table)) -> Result<(), String> {
    let path = perf_toml();
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut doc: toml::Table = text.parse().map_err(|e| format!("perf.toml: {e}"))?;
    f(&mut doc);
    let body = toml::to_string(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, header(&text) + &body).map_err(|e| e.to_string())
}

/// Writes `ms` under `[recorded."<class>"]` (rounded to µs).
pub fn record_into(doc: &mut toml::Table, class: &str, ms: &[Measurement]) {
    let recorded = doc
        .entry("recorded")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .expect("invariant: [recorded] is a table");
    let mut t = toml::Table::new();
    for m in ms {
        t.insert(
            m.name.clone(),
            toml::Value::Float((m.ms * 1000.0).round() / 1000.0),
        );
    }
    recorded.insert(class.to_owned(), toml::Value::Table(t));
}

fn runtime(args: &[String]) -> Result<(), String> {
    let mut record = false;
    for a in args {
        match a.as_str() {
            "--record" => record = true,
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let class = runner_class(&env_var);
    let text = std::fs::read_to_string(perf_toml()).map_err(|e| format!("perf.toml: {e}"))?;
    let doc: toml::Table = text.parse().map_err(|e| format!("perf.toml: {e}"))?;
    let cfg = PerfConfig::parse(&doc, &class)?;

    // The binary the cold-start test spawns (`target/release/frap`), with its shipped features.
    cargo(&["build", "--release", "-p", "frap-cli"], &[])?;
    let out = target_dir().join("perf/measurements.jsonl");
    std::fs::create_dir_all(out.parent().expect("invariant: has a parent"))
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&out);
    let tests = cargo(
        &[
            "test",
            "--release",
            "-p",
            "frap-perf-tests",
            "--",
            "--test-threads=1",
        ],
        &[("FRAP_PERF_OUT", &out)],
    );
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let ms = parse_measurements(&text)?;
    let v = judge(&ms, &cfg);
    println!("§21.7 performance budgets, runner class {class}:");
    for r in &v.rows {
        println!("  {r}");
    }
    for w in &v.warnings {
        crate::ci_warning("perf", w);
    }
    tests.map_err(|e| format!("{e} (a test failed or exceeded its budget)"))?;
    if !v.errors.is_empty() {
        return Err(v.errors.join("; "));
    }
    if record {
        edit_perf_toml(|doc| record_into(doc, &class, &ms))?;
        println!(
            "recorded {} measurements for {class} in {}",
            ms.len(),
            perf_toml().display()
        );
    }
    Ok(())
}

/// The key of `xtask/budgets.toml` that `cargo xtask budgets --record` sets to the runner
/// class the baselines were recorded on.
pub const RECORDED_ON_KEY: &str = "recorded_on";

fn budgets_toml() -> PathBuf {
    crate::workspace_root().join("xtask/budgets.toml")
}

/// `recorded_on` of `budgets.toml` (`None`: provisional baselines, recorded by hand).
pub fn build_baselines_recorded_on(doc: &toml::Table) -> Option<String> {
    doc.get(RECORDED_ON_KEY)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

/// Whether an over-budget build fails `build-budgets` (module docs).
pub fn build_budgets_gate(recorded_on: Option<&str>, class: &str) -> bool {
    recorded_on == Some(CI_CLASS) && class == CI_CLASS
}

fn build_budgets(args: &[String]) -> Result<(), String> {
    let mut record = false;
    for a in args {
        match a.as_str() {
            "--record" => record = true,
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let class = runner_class(&env_var);
    if record {
        if class != CI_CLASS {
            return Err(format!(
                "build-time baselines are recorded on {CI_CLASS} (the CI runner class), not on {class}; \
                 run the `perf` workflow with `record` (or set FRAP_RUNNER_CLASS on such a machine)"
            ));
        }
        // `budgets --record` writes `recorded_on = "ubuntu-24.04"` beside the baselines.
        crate::budgets::run(&["--record".to_owned()])?;
        println!(
            "build-time baselines recorded on {CI_CLASS}: `perf build-budgets` gates from now on"
        );
        return Ok(());
    }
    let text = std::fs::read_to_string(budgets_toml()).map_err(|e| format!("budgets.toml: {e}"))?;
    let doc: toml::Table = text.parse().map_err(|e| format!("budgets.toml: {e}"))?;
    let recorded_on = build_baselines_recorded_on(&doc);
    let gate = build_budgets_gate(recorded_on.as_deref(), &class);
    match crate::budgets::run(&[]) {
        Ok(()) => Ok(()),
        Err(e) if gate => Err(e),
        Err(e) => {
            crate::ci_warning(
                "build-budgets",
                &format!(
                    "{e} (not gating: baselines recorded on {}, runner class {class}; \
                     `cargo xtask budgets --record` on {CI_CLASS} makes it gating)",
                    recorded_on
                        .as_deref()
                        .unwrap_or("no CI runner (provisional)")
                ),
            );
            Ok(())
        }
    }
}

/// `--blender <path>`, else `$BLENDER_BIN`, else `blender` on `PATH`.
fn locate_blender(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => {
            Ok(std::env::var_os("BLENDER_BIN")
                .map_or_else(|| PathBuf::from("blender"), PathBuf::from))
        }
        [flag, path] if flag == "--blender" => Ok(PathBuf::from(path)),
        [flag] if flag == "--blender" => Err("--blender needs a path".to_owned()),
        _ => Err(format!("unknown arguments {args:?}\n{USAGE}")),
    }
}

fn blender(args: &[String]) -> Result<(), String> {
    let bin = locate_blender(args)?;
    let script = crate::workspace_root().join("xtask/perf/blender_reference.py");
    let out = target_dir().join("perf/blender_reference.jsonl");
    std::fs::create_dir_all(out.parent().expect("invariant: has a parent"))
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&out);
    let st = Command::new(&bin)
        .args(["-b", "--factory-startup", "--python"])
        .arg(&script)
        .arg("--")
        .arg(&out)
        .status()
        .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
    if !st.success() {
        return Err(format!("{} failed", bin.display()));
    }
    let text = std::fs::read_to_string(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    println!(
        "Blender reference ({}), compare with [blender_reference_ms] of xtask/perf.toml:",
        bin.display()
    );
    for m in parse_measurements(&text)? {
        println!("  {:<48} {:>10.3} ms", m.name, m.ms);
    }
    Ok(())
}
