// SPDX-License-Identifier: GPL-3.0-or-later
//! The differential fuzzer (ARCHITECTURE §20.1, WP 8.7): random sequences of implemented
//! operators and RNA writes (drawn from a seed with the fuzzer's own SplitMix64, so both tools
//! replay a case from its text alone) run in frap and in Blender through the golden runners;
//! every mismatch is minimised (delta debugging over the statements) into a golden case.
//!
//! ```text
//! cargo xtask golden fuzz [--seed N] [--count N | --minutes M] [--len N] [--jobs N]
//!                         [--budget N] [--no-minimize] [--out DIR]
//! cargo xtask golden fuzz replay <trial seed> [--len N]   print one sequence as a case
//! cargo xtask golden fuzz catalog                          list what the fuzzer draws from
//! cargo xtask golden fuzz check <case.py>                  run a case's statements in both tools
//! cargo xtask golden fuzz minimize <case.py> [--budget N]  minimise a case on its first
//!                                                         unexplained mismatch
//! ```
//!
//! - [`catalog`]: the operators (properties varied within their Blender 5.2.4 definitions)
//!   and the RNA statement templates.
//! - [`sequence`]: the sequence generator and its scene model.
//! - [`driver`]: running both tools, signatures, minimisation, cases, the report.
//! - [`minimize`]: `ddmin`.
//! - [`known`]: mismatches `docs/deviations.md` explains, by their cause (the documented
//!   statement ran in both tools and acted on the mismatched mesh or object), never by
//!   co-occurrence.
//!
//! Triage: a minimised case written to `--out` (default
//! `target/fuzz-cases/`) is either a bug, fixed by the owner of the code and then moved to
//! `tests/golden/cases/fuzz/` and blessed as a regression case, or a documented deviation,
//! added to [`known::KNOWN`] with its `docs/deviations.md` row. The nightly workflow
//! (`.github/workflows/nightly.yml`) runs an hour of it and fails on any unexplained
//! mismatch, and on a run that did not compare the tools (no clean trial, or Blender failing
//! on more than 5% of the trials: [`driver::Report::health_problem`]).
//! `tests/golden/cases/fuzz/seed_<hex>.py` are clean sequences replayed by the golden
//! suite (`replay <hex>`, blessed): they pin the generator (a test checks that each still
//! comes out of `replay`) and keep both tools agreeing on them.

pub mod catalog;
pub mod driver;
pub mod known;
pub mod minimize;
pub mod rng;
pub mod sequence;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::Duration;

use crate::blender::Repo;

/// The command-line usage of `frap-golden fuzz`.
pub const USAGE: &str = "usage: frap-golden fuzz [--seed N] [--count N | --minutes M] [--len N] \
     [--jobs N] [--budget N] [--no-minimize] [--out DIR] | fuzz replay <seed> [--len N] | fuzz catalog \
     | fuzz check <case.py> | fuzz minimize <case.py> [--budget N]";

/// Default statements per sequence.
pub const DEFAULT_LEN: usize = 14;
/// Default tool runs per minimisation.
pub const DEFAULT_BUDGET: usize = 150;

fn parse_u64(s: &str) -> Result<u64, String> {
    let r = match s.strip_prefix("0x") {
        Some(h) => u64::from_str_radix(h, 16),
        None => s.parse(),
    };
    r.map_err(|_| format!("not a number: `{s}`"))
}

/// The case text of trial seed `seed` (statements only; `compare=` from a frap run).
pub fn replay(seed: u64, len: usize) -> String {
    let st = sequence::generate(seed, len);
    let specs = driver::compare_specs(&st);
    driver::case_text(&st, &specs, &format!("seed={seed:#x} (replay)"))
}

fn need_blender() -> Result<PathBuf, String> {
    crate::locate_blender().ok_or_else(|| {
        format!(
            "Blender 5.2 not found: set {}, put `blender` on PATH, or install ~/{}",
            crate::blender::BLENDER_ENV,
            crate::blender::MACOS_DEFAULT
        )
    })
}

/// The statements of a fuzz case: every line but comments, blank lines and `import bpy`.
pub fn case_statements(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "import bpy")
        .map(str::to_owned)
        .collect()
}

/// `frap-golden fuzz …`; `Ok(false)` when unexplained mismatches were found or the run was
/// unhealthy ([`driver::Report::health_problem`]).
pub fn main(args: &[String]) -> Result<bool, String> {
    match args.first().map(String::as_str) {
        Some("replay") => {
            let seed = parse_u64(args.get(1).ok_or(USAGE)?)?;
            let len = match args.get(2..) {
                Some([f, n]) if f == "--len" => parse_u64(n)? as usize,
                Some([]) => DEFAULT_LEN,
                _ => return Err(USAGE.into()),
            };
            print!("{}", replay(seed, len));
            return Ok(true);
        }
        Some("catalog") => {
            let c = catalog::Catalog::get();
            for id in c.operator_ids() {
                println!("{id}");
            }
            for r in &c.rna {
                println!("rna: {}", r.template);
            }
            println!(
                "{} operators, {} RNA statements",
                c.operator_ids().len(),
                c.rna.len()
            );
            return Ok(true);
        }
        Some(cmd @ ("check" | "minimize")) => {
            let path = args.get(1).ok_or(USAGE)?;
            let budget = match args.get(2..) {
                Some([f, n]) if f == "--budget" => parse_u64(n)? as usize,
                Some([]) => DEFAULT_BUDGET,
                _ => return Err(USAGE.into()),
            };
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let statements = case_statements(&text);
            let repo = Repo::locate();
            let props =
                crate::runner::Props::load(&repo.props_path()).map_err(|e| e.to_string())?;
            let blender = need_blender()?;
            let r = driver::run_both(&repo, &props, &blender, &statements);
            if let Some(e) = r.blender_failure {
                return Err(format!("Blender failed: {e}"));
            }
            if r.mismatches.is_empty() {
                println!("no mismatch");
                return Ok(true);
            }
            if cmd == "check" {
                // What each statement did, as the classification sees it.
                for (i, st) in statements.iter().enumerate() {
                    let t = &r.trace;
                    println!(
                        "# {i}: {:?} meshes {:?} objects {:?}: {st}",
                        t.outcomes.get(i),
                        t.meshes.get(i),
                        t.objects.get(i)
                    );
                }
                let why = known::explain_all(&statements, &r.trace, &r.mismatches);
                for (m, k) in r.mismatches.iter().zip(&why) {
                    match k {
                        Some(k) => println!("{m}\n    explained: {}", k.deviation),
                        None => println!("{m}\n    UNEXPLAINED"),
                    }
                }
            }
            let first = match driver::classify(&statements, &r.trace, &r.mismatches) {
                driver::Verdict::Explained(d) => {
                    println!("explained: {d}");
                    return Ok(true);
                }
                driver::Verdict::Unexplained(m) => m,
            };
            if cmd == "check" {
                println!("UNEXPLAINED: {first}");
                return Ok(false);
            }
            // Minimised on its first unexplained mismatch.
            let sig = driver::signature(&statements, first);
            let (min, mr) = driver::minimise(&repo, &props, &blender, &statements, &sig, budget);
            let mism = mr.mismatches;
            let specs = driver::compare_specs(&min);
            print!(
                "{}",
                driver::case_text(&min, &specs, &format!("minimised signature={sig}"))
            );
            for m in mism.iter().take(10) {
                println!("# {m}");
            }
            return Ok(false);
        }
        _ => {}
    }
    let repo = Repo::locate();
    let mut opts = driver::Options {
        seed: 1,
        count: None,
        duration: None,
        len: DEFAULT_LEN,
        jobs: std::thread::available_parallelism().map_or(4, |n| n.get().saturating_sub(2).max(1)),
        minimize: true,
        budget: DEFAULT_BUDGET,
        out: repo.root.join("target/fuzz-cases"),
        blender: PathBuf::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .ok_or_else(|| format!("{a} needs a value\n{USAGE}"))
        };
        match a.as_str() {
            "--seed" => opts.seed = parse_u64(val()?)?,
            "--count" => opts.count = Some(parse_u64(val()?)?),
            "--minutes" => {
                let m: f64 = val()?.parse().map_err(|_| USAGE.to_owned())?;
                opts.duration = Some(Duration::from_secs_f64(m * 60.0));
            }
            "--len" => opts.len = parse_u64(val()?)? as usize,
            "--jobs" => opts.jobs = parse_u64(val()?)? as usize,
            "--budget" => opts.budget = parse_u64(val()?)? as usize,
            "--out" => opts.out = PathBuf::from(val()?),
            "--no-minimize" => opts.minimize = false,
            _ => return Err(USAGE.into()),
        }
    }
    if opts.count.is_none() && opts.duration.is_none() {
        opts.count = Some(100);
    }
    opts.blender = need_blender()?;
    let report = driver::run(&repo, &opts)?;
    print!("{}", report.summary());
    println!("report: {}", opts.out.join("report.json").display());
    if let Some(p) = report.health_problem() {
        eprintln!("fuzz: the run did not compare the tools: {p}");
        return Ok(false);
    }
    Ok(report.findings.is_empty())
}
