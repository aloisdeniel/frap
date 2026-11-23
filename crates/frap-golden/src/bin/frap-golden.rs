// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-golden`: the golden harness from the command line (ARCHITECTURE §20.5). WP 1.8.
//! `cargo xtask golden …` forwards here.
//!
//! ```text
//! frap-golden bless  [--filter <glob>]   run Blender on the cases, write fixtures
//! frap-golden check  [--filter <glob>]   run frap on the cases, compare with the fixtures
//! frap-golden live   [--filter <glob>]   run Blender live, compare with the fixtures (drift)
//! frap-golden report [--filter <glob>]   operator × tier coverage matrix
//! frap-golden diff   <case.py>           run both tools live and print every mismatch
//! frap-golden fuzz   [options]           the differential fuzzer (WP 8.7, `frap_golden::fuzz`)
//! ```
//! Exit status: 0 ok, 1 mismatches or failures, 2 usage or Blender missing.

use std::path::Path;
use std::process::ExitCode;

use frap_golden::blender::{self, Repo, ScratchDir};
use frap_golden::case::Case;
use frap_golden::runner::{self, Props, RunOptions};

const USAGE: &str = "usage: frap-golden bless|check|live|report [--filter <glob>] | diff <case.py> | fuzz [options]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("frap-golden: {e}");
            ExitCode::from(2)
        }
    }
}

fn filter(args: &[String]) -> Result<Option<String>, String> {
    match args {
        [] => Ok(None),
        [f, g] if f == "--filter" => Ok(Some(g.clone())),
        _ => Err(USAGE.into()),
    }
}

fn need_blender() -> Result<std::path::PathBuf, String> {
    frap_golden::locate_blender().ok_or_else(|| {
        format!(
            "Blender 5.2 not found: set {}, put `blender` on PATH, or install ~/{}",
            blender::BLENDER_ENV,
            blender::MACOS_DEFAULT
        )
    })
}

fn run(args: &[String]) -> Result<bool, String> {
    let repo = Repo::locate();
    let (cmd, rest) = args.split_first().ok_or(USAGE)?;
    match cmd.as_str() {
        "bless" => {
            let f = filter(rest)?;
            let b = need_blender()?;
            let cases = repo.cases(f.as_deref()).map_err(|e| e.to_string())?;
            let mut ok = true;
            for c in cases.iter().filter(|c| !c.skip_blender) {
                match blender::bless(&b, &repo, c) {
                    Ok(p) => println!(
                        "blessed {} -> {}",
                        c.id(),
                        p.strip_prefix(&repo.root).unwrap_or(&p).display()
                    ),
                    Err(e) => {
                        ok = false;
                        eprintln!("FAILED {}: {e}", c.id());
                    }
                }
            }
            println!("{} case(s) with Blender {}", cases.len(), b.display());
            Ok(ok)
        }
        "check" => {
            let f = filter(rest)?;
            let props = Props::load(&repo.props_path()).map_err(|e| e.to_string())?;
            let cases = repo.cases(f.as_deref()).map_err(|e| e.to_string())?;
            let strict = std::env::var_os(runner::STRICT_RNA_ENV).is_some_and(|v| v == "1");
            let reports: Vec<_> = cases
                .iter()
                .map(|c| runner::check_against_fixture(&repo, &props, c, strict))
                .collect();
            for r in &reports {
                println!(
                    "{} {}{}",
                    if r.passed() { "ok  " } else { "FAIL" },
                    r.case,
                    if r.rna_fallbacks.is_empty() {
                        String::new()
                    } else {
                        format!(" ({} RNA fallbacks)", r.rna_fallbacks.len())
                    }
                );
            }
            print!("{}", runner::format_reports(&reports, 30));
            Ok(reports.iter().all(|r| r.passed()))
        }
        "live" => {
            let f = filter(rest)?;
            let b = need_blender()?;
            let cases = repo.cases(f.as_deref()).map_err(|e| e.to_string())?;
            let mut ok = true;
            for c in cases.iter().filter(|c| !c.skip_blender) {
                match drift(&b, &repo, c) {
                    Ok(m) if m.is_empty() => println!("ok   {}", c.id()),
                    Ok(m) => {
                        ok = false;
                        println!("DRIFT {}", c.id());
                        for x in m.iter().take(30) {
                            println!("  {x}");
                        }
                    }
                    Err(e) => {
                        ok = false;
                        println!("FAIL {}: {e}", c.id());
                    }
                }
            }
            Ok(ok)
        }
        "report" => {
            let f = filter(rest)?;
            let cases = repo.cases(f.as_deref()).map_err(|e| e.to_string())?;
            print!(
                "{}",
                frap_golden::case::coverage_report(&cases, &repo.fixtures_dir())
            );
            Ok(true)
        }
        "diff" => {
            let [path] = rest else {
                return Err(USAGE.into());
            };
            let b = need_blender()?;
            let case = Case::load(Path::new(path), &repo.cases_dir()).map_err(|e| e.to_string())?;
            let props = Props::load(&repo.props_path()).map_err(|e| e.to_string())?;
            let (wf, wb) = (
                ScratchDir::new("frap").map_err(|e| e.to_string())?,
                ScratchDir::new("blender").map_err(|e| e.to_string())?,
            );
            let fr = runner::run_frap(
                &case,
                &props,
                &RunOptions {
                    workdir: wf.path().to_owned(),
                    strict_rna: false,
                },
            )
            .map_err(|e| e.to_string())?;
            let bl = blender::run_live(&b, &repo, &case, wb.path()).map_err(|e| e.to_string())?;
            for fb in &fr.rna_fallbacks {
                println!("rna fallback: {fb}");
            }
            let m = runner::compare(&case, &fr.state, &bl);
            for x in &m {
                println!("{x}");
            }
            println!("{} mismatches", m.len());
            Ok(m.is_empty())
        }
        // WP 8.7: the differential fuzzer owns its arguments.
        "fuzz" => frap_golden::fuzz::main(rest),
        _ => Err(USAGE.into()),
    }
}

/// Blender now vs the committed fixture (both sides are Blender: the case's comparators with
/// its tolerance).
fn drift(b: &Path, repo: &Repo, case: &Case) -> Result<Vec<frap_golden::case::Mismatch>, String> {
    let fixture = blender::load_fixture(case, &repo.fixtures_dir())
        .map_err(|e| e.to_string())?
        .ok_or("not blessed")?;
    let w = ScratchDir::new(&case.name).map_err(|e| e.to_string())?;
    let live = blender::run_live(b, repo, case, w.path()).map_err(|e| e.to_string())?;
    Ok(runner::compare(case, &live, &fixture))
}
