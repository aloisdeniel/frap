// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: the raw-mode splitter and `is_complete` must never panic (WP 1.3).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        let _ = frap_lang::split::split(src);
        let _ = frap_lang::is_complete(src);
    }
});
