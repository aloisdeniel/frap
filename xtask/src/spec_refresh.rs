// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask spec-refresh [--check] [--blender <path>]`: the Blender 5.2.x spec-refresh
//! routine (ARCHITECTURE §22.10 WP 8.9, §23 "Blender 5.3 API churn"): re-runs the dump
//! scripts (`tools/blender/dump_{rna,ops,nodes,modifiers,startup}.py`) against a Blender 5.2.x
//! into a scratch directory, compares them with `spec/blender-5.2/` (operators, properties,
//! enum items, defaults, poll matrices, structs added or removed), writes the drift report,
//! and with `--check` fails on any drift. A refresh then regenerates through
//! `cargo xtask codegen`. The routine is documented in `docs/spec-refresh.md`.
//!
//! ```text
//! cargo xtask spec-refresh --check [--blender <path>]   re-dump, report, fail on drift
//! cargo xtask spec-refresh [--blender <path>]           re-dump, report, write spec/blender-5.2/
//!     --out <dir>        scratch directory (default <target>/spec-refresh): dumps/, logs/,
//!                        report.md, report.json
//!     --seed <script>    a Python script run before every dump script (seeded drift fixture:
//!                        tools/blender/spec_refresh_seed.py)
//!     --spec <dir>       the spec directory compared and refreshed (default spec/blender-5.2)
//! ```
//!
//! Only the recorded Blender version / build hash differing is not drift: `--check` passes and
//! a refresh records the new version (any 5.2.x is accepted; others are refused).

use std::path::{Path, PathBuf};

mod blender;
mod compare;
mod diff;
mod followups;
mod report;
#[cfg(test)]
mod tests;

use compare::{Comparison, Family, Files};
use report::{BlenderInfo, Report};

const USAGE: &str = "usage: cargo xtask spec-refresh [--check] [--blender <path>] [--out <dir>] \
                     [--seed <script.py>] [--spec <dir>]";

/// Parsed command line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub check: bool,
    pub blender: Option<PathBuf>,
    pub out: Option<PathBuf>,
    pub seed: Option<PathBuf>,
    pub spec: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| {
            it.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a value\n{USAGE}"))
        };
        match a.as_str() {
            "--check" => o.check = true,
            "--blender" => o.blender = Some(val("--blender")?),
            "--out" => o.out = Some(val("--out")?),
            "--seed" => o.seed = Some(val("--seed")?),
            "--spec" => o.spec = Some(val("--spec")?),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    Ok(o)
}

/// What a refresh did.
#[derive(Debug)]
pub struct Outcome {
    pub report: Report,
    /// The scratch directory.
    pub out: PathBuf,
    /// Without `--check`: files written to / removed from the spec directory.
    pub applied: Option<(Vec<String>, Vec<String>)>,
}

/// `<target>/spec-refresh`.
fn default_out(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
        .join("spec-refresh")
}

/// Reads the dump files of every family under `dir` (missing ones are absent).
pub fn load(dir: &Path) -> Result<Files, String> {
    let mut files = Files::new();
    for f in Family::ALL {
        let p = dir.join(f.output());
        if f == Family::Startup {
            if p.is_file() {
                let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                files.insert(f.output().to_owned(), b);
            }
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&p) else {
            continue;
        };
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if path.is_file() && name.ends_with(".json") {
                let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                files.insert(format!("{}/{name}", f.output()), b);
            }
        }
    }
    Ok(files)
}

/// Writes every new or changed file of `new` into `spec`, removes the files `new` lacks.
fn apply(spec: &Path, old: &Files, new: &Files) -> Result<(Vec<String>, Vec<String>), String> {
    let mut written = Vec::new();
    for (rel, b) in new {
        if old.get(rel) != Some(b) {
            let p = spec.join(rel);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            std::fs::write(&p, b).map_err(|e| format!("{}: {e}", p.display()))?;
            written.push(rel.clone());
        }
    }
    let mut removed = Vec::new();
    for rel in old.keys().filter(|k| !new.contains_key(*k)) {
        let p = spec.join(rel);
        std::fs::remove_file(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        removed.push(rel.clone());
    }
    Ok((written, removed))
}

fn current_milestone(root: &Path) -> u32 {
    std::fs::read_to_string(root.join("spec/current_milestone"))
        .ok()
        .and_then(|s| s.trim().trim_start_matches('M').parse().ok())
        .unwrap_or(0)
}

/// The report of a comparison: runs the codegen dry run on `dumps` when there is drift.
pub fn build_report(
    root: &Path,
    blender: BlenderInfo,
    seed: Option<String>,
    cmp: Comparison,
    dumps: &Path,
    shadow: &Path,
) -> Report {
    let (codegen, codegen_dropped_rows) = if cmp.has_drift() {
        let removed: Vec<String> = cmp
            .items
            .iter()
            .filter(|i| i.category == "operator removed")
            .map(|i| i.subject.label())
            .collect();
        followups::codegen(root, dumps, shadow, &removed)
    } else {
        (followups::Codegen::Skipped, vec![])
    };
    let md = std::fs::read_to_string(root.join("docs/OPERATORS.md")).unwrap_or_default();
    let operators_md = followups::operators_md(&md, &cmp, current_milestone(root));
    let migrations = followups::migrations(&cmp, followups::schema_version(root));
    Report {
        blender,
        seed,
        cmp,
        codegen,
        codegen_dropped_rows,
        operators_md,
        migrations,
    }
}

fn resolve(root: &Path, p: &Path) -> PathBuf {
    if p.is_absolute() || p.exists() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// The whole routine, without printing.
pub fn refresh(root: &Path, o: &Options) -> Result<Outcome, String> {
    if o.seed.is_some() && !o.check && o.spec.is_none() {
        return Err(
            "--seed dumps a seeded drift: use it with --check (or refresh another --spec <dir>), \
             never into spec/blender-5.2/"
                .into(),
        );
    }
    let blender = blender::locate(o.blender.clone())?;
    let ver = blender::probe(&blender)?;
    let seed = o.seed.as_deref().map(|s| resolve(root, s));
    if let Some(s) = &seed
        && !s.is_file()
    {
        return Err(format!("--seed {}: no such file", s.display()));
    }
    let out = o.out.clone().unwrap_or_else(|| default_out(root));
    let dumps = out.join("dumps");
    let logs = out.join("logs");
    let _ = std::fs::remove_dir_all(&dumps);
    for d in [&dumps, &logs] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    for f in Family::ALL {
        eprintln!("spec-refresh: dump_{}.py", f.script());
        blender::run_dump(
            &blender,
            root,
            f,
            seed.as_deref(),
            &dumps,
            &logs.join(format!("{}.log", f.script())),
        )?;
    }
    let spec = o
        .spec
        .clone()
        .unwrap_or_else(|| root.join("spec/blender-5.2"));
    let old = load(&spec)?;
    let new = load(&dumps)?;
    let cmp = compare::compare(&old, &new);
    if let Some(r) = &cmp.new_version
        && (r.version != ver.version || r.build_hash != ver.build_hash)
    {
        return Err(format!(
            "the dumps record Blender {} ({}) but `--version` says {} ({})",
            r.version, r.build_hash, ver.version, ver.build_hash
        ));
    }
    let differs = cmp.differs();
    let report = build_report(
        root,
        BlenderInfo {
            path: blender.display().to_string(),
            version: ver.version,
            build_hash: ver.build_hash,
        },
        o.seed.as_ref().map(|s| s.display().to_string()),
        cmp,
        &dumps,
        &out.join("shadow"),
    );
    let md = report.markdown();
    let json = serde_json::to_string_pretty(&report.json()).expect("invariant: plain JSON");
    for (name, body) in [("report.md", md), ("report.json", json + "\n")] {
        let p = out.join(name);
        std::fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let applied = if !o.check && differs {
        Some(apply(&spec, &old, &new)?)
    } else {
        None
    };
    Ok(Outcome {
        report,
        out,
        applied,
    })
}

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let o = parse_args(args)?;
    let root = crate::workspace_root();
    let r = refresh(&root, &o)?;
    print!("{}", r.report.markdown());
    let report = r.out.join("report.md");
    println!("\nreport: {}", report.display());
    let cmp = &r.report.cmp;
    if let Some((written, removed)) = &r.applied {
        println!(
            "spec-refresh: wrote {} and removed {} file(s) of {}; next: review `git diff spec/`, \
             then `cargo xtask codegen` (docs/spec-refresh.md)",
            written.len(),
            removed.len(),
            o.spec
                .as_deref()
                .map_or("spec/blender-5.2/".into(), |p| p.display().to_string())
        );
    }
    if !cmp.has_drift() && !cmp.version_only.is_empty() {
        crate::ci_warning(
            "spec-refresh",
            &format!(
                "no API drift, but spec/blender-5.2/ records Blender {} and this is {}{}",
                cmp.old_version.as_ref().map_or("?", |v| v.version.as_str()),
                r.report.blender.version,
                if o.check {
                    " (run without --check to record it)"
                } else {
                    ""
                }
            ),
        );
    }
    if o.check && cmp.has_drift() {
        return Err(format!(
            "spec drift: {} item(s) (see {})",
            cmp.items.len(),
            report.display()
        ));
    }
    Ok(())
}
