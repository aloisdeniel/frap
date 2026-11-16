// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: the statement splitter, `is_complete` and the raw-mode line accumulator never
//! panic and never wait: feeding every line and then EOF always drains the accumulator, and
//! every chunk parses to a result (WP 1.3, ARCHITECTURE §11.1).
#![no_main]

use frap_lang::split::{RawSplitter, split};
use frap_lang::{is_complete, parse_complete_at};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    for p in split(src) {
        assert!(p.start <= p.end && p.end <= src.len());
        assert!(src.is_char_boundary(p.start) && src.is_char_boundary(p.end));
    }
    let _ = is_complete(src);
    let mut raw = RawSplitter::new();
    let mut chunks = Vec::new();
    for line in src.split_inclusive('\n') {
        chunks.extend(raw.push_line(line));
    }
    chunks.extend(raw.finish());
    assert!(!raw.is_pending(), "EOF must end every continuation");
    for c in chunks {
        let _ = parse_complete_at(&c.src, c.first_line);
    }
});
