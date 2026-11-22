// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: `frap_imbuf` format detection and decoding (PNG, JPEG, WebP, HDR, EXR, KTX2) of arbitrary bytes: never panics, bounded memory (ARCHITECTURE §20.1, nightly from M8).
//!
//! M8 W0 stub (decision log M8W0-9): WP 8.7 owns this file and writes the target.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = data;
});
