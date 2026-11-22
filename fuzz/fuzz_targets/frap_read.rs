// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: `frap_project::container::read` of arbitrary bytes (a `.frap` zip): never panics, refuses with `ProjectError` (ARCHITECTURE §20.1, nightly from M8).
//!
//! M8 W0 stub (decision log M8W0-9): WP 8.7 owns this file and writes the target.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = data;
});
