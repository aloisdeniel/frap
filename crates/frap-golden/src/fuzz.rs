// SPDX-License-Identifier: GPL-3.0-or-later
//! The differential fuzzer (ARCHITECTURE §20.1): random sequences of implemented operators
//! and RNA writes (from a seed, `frap_math::BliRng`-free so both tools replay them) run in
//! frap and in Blender through the golden runners, every mismatch minimised (delta debugging
//! over the statements) into a case under `tests/golden/cases/fuzz/`.
//!
//! M8 W0 stub (decision log M8W0-8): WP 8.7 owns this file (and `fuzz/**`), with
//! `xtask/src/golden.rs` reassigned (`cargo xtask golden fuzz`).
