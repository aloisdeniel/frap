// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask spec-refresh [--check] [--blender <path>]`: the Blender 5.2.x spec-refresh
//! routine (ARCHITECTURE §22.10 WP 8.9, §23 "Blender 5.3 API churn"): re-runs the dump
//! scripts (`tools/blender/dump_{rna,ops,nodes,modifiers,startup}.py`) against a Blender 5.2.x
//! into a scratch directory, compares them with `spec/blender-5.2/` (operators, properties,
//! enum items, defaults, poll matrices, structs added or removed), writes the drift report,
//! and with `--check` fails on any drift. A refresh then regenerates through
//! `cargo xtask codegen`.
//!
//! M8 W0 stub (decision log M8W0-9): WP 8.9 owns this file (and `spec_refresh/**`).

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    let _ = args;
    Err("not implemented yet (M8, WP 8.9)".into())
}
