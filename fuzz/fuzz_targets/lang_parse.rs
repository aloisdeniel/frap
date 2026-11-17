// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: `frap_lang::parse_complete` on arbitrary UTF-8 never panics, and every
//! statement and error carries an in-bounds span whose line/col/col_chars match its byte
//! offset (WP 1.3, ARCHITECTURE §11.2).
#![no_main]

use frap_lang::{Command, parse_complete, parse_complete_at};
use libfuzzer_sys::fuzz_target;

/// `(start, len, line, col, col_chars)` of a span (the fuzz crate does not depend on
/// frap-rna, which defines `Span`).
fn check(src: &str, (start, len, line, col, col_chars): (u32, u32, u32, u32, u32)) {
    let start = start as usize;
    assert!(start <= src.len() && start + len as usize <= src.len());
    assert!(src.is_char_boundary(start));
    let line_start = src[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    assert_eq!(line as usize, src[..start].matches('\n').count() + 1);
    assert_eq!(col as usize, start - line_start + 1);
    assert_eq!(
        col_chars as usize,
        src[line_start..start].chars().count() + 1
    );
}

macro_rules! fields {
    ($s:expr) => {
        ($s.start, $s.len, $s.line, $s.col, $s.col_chars)
    };
}

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    for r in parse_complete(src) {
        match r {
            Ok(stmt) => {
                check(src, fields!(stmt.span));
                if let Command::Invoke { args, .. } | Command::Call { args, .. } = &stmt.cmd {
                    args.iter().for_each(|a| check(src, fields!(a.span)));
                }
            }
            Err(e) => {
                let s = e.span.expect("every error has a span");
                check(src, fields!(s));
            }
        }
    }
    // Line offsets shift lines only.
    let _ = parse_complete_at(src, 1000);
});
