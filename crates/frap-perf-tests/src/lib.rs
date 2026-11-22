// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-only crate (rule 2.2.11): the performance budgets of ARCHITECTURE §21.7 (cold start
//! < 30 ms without GPU init; a 100k-face edit-mode operator < 200 ms; Edit Mode property reads
//! never re-evaluate modifiers; a 1M-corner GLB export < 2 s; snapshot clone O(#IDs)), one
//! test file each under `tests/` (M8 W0 stub, decision log M8W0-9; WP 8.8 owns everything
//! except `Cargo.toml` and this file).
#![deny(unsafe_code)]
