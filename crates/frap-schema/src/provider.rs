// SPDX-License-Identifier: GPL-3.0-or-later
//! The `frap_op::DESCRIBE_PROVIDERS` entry that serves `bpy.ops.frap.describe(target=…)` from
//! the manifest of this crate (M2 W0 contract delta, ARCHITECTURE §14).
//!
//! **Stub** created by M2 W0 (WP 2.0): nothing is registered yet, so `frap.describe` keeps
//! answering `OP_NOT_IMPLEMENTED`. WP 2.13 registers the provider here (with
//! `#[linkme(crate = frap_op::linkme)]`) and implements `crates/frap-ops-frap/src/ops/describe.rs`.
