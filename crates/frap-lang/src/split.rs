// SPDX-License-Identifier: GPL-3.0-or-later
//! Statement splitting (ARCHITECTURE §11.1, §11.3): newline or `;` outside brackets and strings.

/// Byte range of one statement plus the bracket left open at its end, if any.
#[derive(Clone, Debug)]
pub struct Piece {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// `(char, offset)` of the innermost unclosed bracket when the source ended inside it.
    pub unclosed: Option<(char, usize)>,
}

/// Scanner state, reusable by the raw-stdin line accumulator.
#[derive(Clone, Debug, Default)]
struct Scan {
    stack: Vec<(char, usize)>,
}

/// Splits a complete source into statement pieces (blank and comment-only pieces dropped).
pub fn split(src: &str) -> Vec<Piece> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut sc = Scan::default();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        match c {
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'\'' | b'"' => {
                i = skip_string(b, i);
                continue;
            }
            b'(' | b'[' | b'{' => sc.stack.push((c as char, i)),
            b')' | b']' | b'}' => {
                sc.stack.pop();
            }
            b'\n' | b';' if sc.stack.is_empty() => {
                push_piece(src, &mut out, start, i, None);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let unclosed = sc.stack.first().copied();
    push_piece(src, &mut out, start, b.len(), unclosed);
    out
}

fn skip_string(b: &[u8], q_at: usize) -> usize {
    let q = b[q_at];
    let mut i = q_at + 1;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == q || b[i] == b'\n' {
            return i + 1;
        }
        i += 1;
    }
    i
}

fn push_piece(
    src: &str,
    out: &mut Vec<Piece>,
    start: usize,
    end: usize,
    unclosed: Option<(char, usize)>,
) {
    let text = &src[start..end];
    let meaningful = text.lines().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#')
    });
    if meaningful {
        out.push(Piece {
            start,
            end,
            unclosed,
        });
    }
}

/// True when `src` has no unclosed bracket (outside strings and comments): the raw-stdin
/// reader keeps appending lines until this holds, a blank line arrives, or EOF (§11.1).
pub fn is_complete(src: &str) -> bool {
    split(src).last().is_none_or(|p| p.unclosed.is_none())
}
