// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask` (ARCHITECTURE §3.1, §4.2, §20.5). Owned by the integrator (WP 1.0: this file,
//! `Cargo.toml`, `check_deps.rs`, `budgets.rs`), except `src/codegen/**` (WP 1.1) and
//! `src/golden.rs` (WP 1.8). In M1 W1, WP 1.1 and WP 1.8 may append `foo.workspace = true`
//! lines to `Cargo.toml` for dependencies already in `[workspace.dependencies]` (decision W0-28).
//!
//! ```text
//! cargo xtask check-deps              layer rules (§3.2) and no dev-dependency cycles (rule 2.2.11)
//! cargo xtask budgets [--record]      clean build time of each frap-spec-* crate vs xtask/budgets.toml
//! cargo xtask codegen [--check]       Blender dumps -> generated Rust (WP 1.1)
//! cargo xtask fmt [--check]           rustfmt on every tracked hand-written .rs (automod modules too)
//! cargo xtask golden <bless|live|report> [--filter <glob>]   (WP 1.8)
//! cargo xtask spec-refresh [--check] [--blender <path>]       Blender 5.2.x spec refresh (M8 WP 8.9)
//! cargo xtask perf [--record]         §21.7 performance budgets vs xtask/perf.toml (M8 WP 8.8)
//! ```
//!
//! M8 W0 (decision log M8W0-9): `spec_refresh.rs` (WP 8.9) and `perf.rs` (WP 8.8) are wired
//! here as stubs; each WP owns its file (and its directory below it).
#![deny(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

mod budgets;
mod check_deps;
mod codegen;
mod fmt;
mod golden;
mod perf;
mod spec_refresh;

/// Prints a warning; on GitHub Actions also as a `::warning` annotation, so a check that passes
/// vacuously (codegen before WP 1.1, budgets without baselines) is visible in the run summary.
pub fn ci_warning(title: &str, msg: &str) {
    eprintln!("warning: {title}: {msg}");
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        println!("::warning title={title}::{msg}");
    }
}

/// The workspace root (parent of `xtask/`).
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("invariant: xtask lives in the workspace root")
        .to_path_buf()
}

const USAGE: &str = "usage: cargo xtask <check-deps | budgets [--record] | codegen [--check] | fmt [--check] | golden <bless|live|report> [--filter <glob>] | spec-refresh [--check] | perf [--record]>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let r = match cmd.as_str() {
        "check-deps" => check_deps::run(rest),
        "budgets" => budgets::run(rest),
        "codegen" => codegen::run(rest),
        "fmt" => fmt::run(rest),
        "golden" => golden::run(rest),
        "spec-refresh" => spec_refresh::run(rest),
        "perf" => perf::run(rest),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n{USAGE}")),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask {cmd}: {e}");
            ExitCode::FAILURE
        }
    }
}
