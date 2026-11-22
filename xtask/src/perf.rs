// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask perf [--record]`: the performance budgets of ARCHITECTURE §21.7, measured by
//! the release-mode tests of `crates/frap-perf-tests` and compared with `xtask/perf.toml`
//! (`--record` rewrites the measured values on the CI runner class).
//!
//! M8 W0 stub (decision log M8W0-9): WP 8.8 owns this file (and `perf/**`, `xtask/perf.toml`).

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    let _ = args;
    Err("not implemented yet (M8, WP 8.8)".into())
}
