// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask budgets [--record]`: clean build time of each `frap-spec-*` crate against
//! `xtask/budgets.toml` (ARCHITECTURE §4.2). CI fails when a crate exceeds `factor` × baseline.
//! `--record` also writes `recorded_on = "<runner class>"` (`crate::perf::runner_class`): the
//! baselines gate in `cargo xtask perf build-budgets` once recorded on ubuntu-24.04.

use std::process::Command;
use std::time::Instant;

fn cargo(args: &[&str]) -> Result<(), String> {
    let st = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(args)
        .current_dir(crate::workspace_root())
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    let record = args.iter().any(|a| a == "--record");
    let path = crate::workspace_root().join("xtask/budgets.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut doc: toml::Table = text.parse().map_err(|e| format!("budgets.toml: {e}"))?;
    let factor = doc
        .get("factor")
        .and_then(|f| f.as_float())
        .ok_or("budgets.toml: missing `factor`")?;
    let crates: Vec<(String, f64)> = doc
        .get("baseline_secs")
        .and_then(|t| t.as_table())
        .ok_or("budgets.toml: missing [baseline_secs]")?
        .iter()
        .map(|(k, v)| (k.clone(), v.as_float().unwrap_or(0.0)))
        .collect();
    // Dependencies first, so each measurement covers the crate alone.
    let names: Vec<&str> = crates.iter().map(|(k, _)| k.as_str()).collect();
    let mut build_all = vec!["build"];
    for n in &names {
        build_all.extend(["-p", n]);
    }
    cargo(&build_all)?;
    let mut over = Vec::new();
    let mut measured = Vec::new();
    for (name, baseline) in &crates {
        cargo(&["clean", "-p", name])?;
        let t = Instant::now();
        cargo(&["build", "-p", name])?;
        let secs = t.elapsed().as_secs_f64();
        measured.push((name.clone(), secs));
        let status = if *baseline <= 0.0 {
            crate::ci_warning(
                "budgets",
                &format!(
                    "{name}: no baseline recorded (WP 1.1 runs `cargo xtask budgets --record`); not enforced"
                ),
            );
            "no baseline recorded".to_owned()
        } else if secs > factor * baseline {
            over.push(format!("{name}: {secs:.2}s > {factor} × {baseline:.2}s"));
            "OVER BUDGET".to_owned()
        } else {
            format!("ok (budget {:.2}s)", factor * baseline)
        };
        println!("{name:<26} {secs:>7.2}s  {status}");
    }
    if record {
        let table = doc
            .get_mut("baseline_secs")
            .and_then(|t| t.as_table_mut())
            .expect("invariant: checked above");
        for (name, secs) in measured {
            table.insert(name, toml::Value::Float((secs * 100.0).round() / 100.0));
        }
        // Provenance of the baselines (M8 WP 8.8): `cargo xtask perf build-budgets` gates only
        // on baselines recorded on the CI runner class.
        let class = crate::perf::runner_class(&|k| std::env::var(k).ok());
        doc.insert(
            crate::perf::RECORDED_ON_KEY.into(),
            toml::Value::String(class),
        );
        let header: String = text
            .lines()
            .take_while(|l| l.starts_with('#') || l.is_empty())
            .map(|l| format!("{l}\n"))
            .collect();
        let body = toml::to_string(&doc).map_err(|e| e.to_string())?;
        std::fs::write(&path, header + &body).map_err(|e| e.to_string())?;
        println!("recorded baselines in {}", path.display());
        return Ok(());
    }
    if over.is_empty() {
        Ok(())
    } else {
        Err(over.join("; "))
    }
}
