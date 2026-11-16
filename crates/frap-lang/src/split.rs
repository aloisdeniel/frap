// SPDX-License-Identifier: GPL-3.0-or-later
//! Statement splitting (ARCHITECTURE §11.1, §11.3): newline or `;` outside brackets, strings
//! and `\` line continuations; `#` comments run to the end of the line.
//!
//! [`split`] works on a *complete* unit and never waits: whatever is still open at the end of
//! the source is recorded in [`Piece::open`] and reported by the parser as CPython does
//! (`'(' was never closed`). [`RawSplitter`] is the only place that continues a statement
//! across input lines (raw stdin); it ends a pending continuation on a blank line or at EOF, so
//! a human session cannot hang on a typo either.

/// Something still open at the end of a piece.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Open {
    /// The innermost unclosed bracket and its byte offset (CPython reports the innermost one).
    Bracket(char, usize),
    /// An unterminated triple-quoted string starting at this byte offset.
    TripleString(usize),
    /// A `\` line continuation at this byte offset with nothing after it.
    Backslash(usize),
}

/// A closing bracket that does not match.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BadClose {
    /// Byte offset of the closing bracket.
    pub at: usize,
    /// The closing bracket.
    pub close: char,
    /// The bracket it should have closed, if any (`None`: `unmatched ')'`).
    pub open: Option<(char, usize)>,
}

/// Byte range of one statement plus what was left open at its end, if anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// What is still open when the source ends inside this piece.
    pub open: Option<Open>,
    /// The first mismatched or unmatched closing bracket in this piece.
    pub bad_close: Option<BadClose>,
}

fn closer_of(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

/// Splits a complete source into statement pieces (blank and comment-only pieces dropped).
pub fn split(src: &str) -> Vec<Piece> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut bad_close = None;
    let mut start = 0usize;
    let mut i = 0usize;
    let mut open_at_end = None;
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
                match skip_string(b, i) {
                    Ok(n) => i = n,
                    Err(at) => {
                        open_at_end = Some(Open::TripleString(at));
                        i = b.len();
                    }
                }
                continue;
            }
            b'\\' => {
                let mut j = i + 1;
                if j < b.len() && b[j] == b'\r' {
                    j += 1;
                }
                if j < b.len() && b[j] == b'\n' {
                    if j + 1 >= b.len() {
                        open_at_end = Some(Open::Backslash(i));
                    }
                    i = j + 1;
                    continue;
                }
            }
            b'(' | b'[' | b'{' => stack.push((c as char, i)),
            b')' | b']' | b'}' => match stack.last() {
                Some(&(o, _)) if closer_of(o) == c as char => {
                    stack.pop();
                }
                top => {
                    bad_close.get_or_insert(BadClose {
                        at: i,
                        close: c as char,
                        open: top.copied(),
                    });
                    stack.pop();
                }
            },
            b'\n' | b';' if stack.is_empty() => {
                push_piece(src, &mut out, start, i, None, bad_close.take());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let open = open_at_end.or_else(|| stack.last().map(|&(c, at)| Open::Bracket(c, at)));
    push_piece(src, &mut out, start, b.len(), open, bad_close.take());
    out
}

/// Returns the offset after the string starting at `q_at`, or `Err(q_at)` for an unterminated
/// triple-quoted string (a single-quoted one ends at the newline, which the lexer reports).
fn skip_string(b: &[u8], q_at: usize) -> Result<usize, usize> {
    let q = b[q_at];
    let triple = q_at + 2 < b.len() && b[q_at + 1] == q && b[q_at + 2] == q;
    let mut i = if triple { q_at + 3 } else { q_at + 1 };
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == q {
            if !triple {
                return Ok(i + 1);
            }
            if i + 2 < b.len() && b[i + 1] == q && b[i + 2] == q {
                return Ok(i + 3);
            }
        }
        if b[i] == b'\n' && !triple {
            return Ok(i);
        }
        i += 1;
    }
    if triple { Err(q_at) } else { Ok(b.len()) }
}

fn push_piece(
    src: &str,
    out: &mut Vec<Piece>,
    start: usize,
    end: usize,
    open: Option<Open>,
    bad_close: Option<BadClose>,
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
            open,
            bad_close,
        });
    }
}

/// True when `src` has nothing open at its end (bracket, triple-quoted string, `\` line
/// continuation): the raw-stdin reader keeps appending lines until this holds, a blank line
/// arrives, or EOF (§11.1).
pub fn is_complete(src: &str) -> bool {
    split(src).last().is_none_or(|p| p.open.is_none())
}

/// A complete source chunk produced by [`RawSplitter`], ready for
/// [`crate::parse_complete_at`]`(&chunk.src, chunk.first_line)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// The accumulated lines (with their newlines).
    pub src: String,
    /// 1-based input line of the chunk's first line.
    pub first_line: u32,
}

/// Raw-stdin line accumulator (§11.1). Feed it one input line at a time; it returns a chunk
/// as soon as the accumulated text is complete. A statement continues across lines only while
/// a bracket, a triple-quoted string or a `\` continuation is open, and a **blank line or EOF
/// ends a pending continuation**: the chunk is returned as is and parsing it yields
/// `SyntaxError: '(' was never closed` instead of waiting for more input.
#[derive(Clone, Debug, Default)]
pub struct RawSplitter {
    buf: String,
    first_line: u32,
    lines_seen: u32,
}

impl RawSplitter {
    /// New, empty accumulator (the first line pushed is line 1).
    pub fn new() -> Self {
        Self::default()
    }

    /// True when a continuation is pending (some lines are buffered).
    pub fn is_pending(&self) -> bool {
        !self.buf.is_empty()
    }

    /// Number of input lines pushed so far.
    pub fn lines_seen(&self) -> u32 {
        self.lines_seen
    }

    /// Feeds one input line (with or without its trailing newline). Returns the chunk that
    /// this line completes, if any. Blank lines outside a continuation are skipped.
    pub fn push_line(&mut self, line: &str) -> Option<Chunk> {
        self.lines_seen += 1;
        let blank = line.trim().is_empty();
        if self.buf.is_empty() {
            if blank {
                return None;
            }
            self.first_line = self.lines_seen;
        }
        if blank {
            // A blank line ends a pending continuation (§11.1).
            return self.take();
        }
        self.buf.push_str(line);
        if !line.ends_with('\n') {
            self.buf.push('\n');
        }
        if is_complete(&self.buf) {
            return self.take();
        }
        None
    }

    /// Ends the input (EOF): returns whatever is buffered, complete or not.
    pub fn finish(&mut self) -> Option<Chunk> {
        self.take()
    }

    fn take(&mut self) -> Option<Chunk> {
        if self.buf.trim().is_empty() {
            self.buf.clear();
            return None;
        }
        Some(Chunk {
            src: std::mem::take(&mut self.buf),
            first_line: self.first_line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        split(src).iter().map(|p| &src[p.start..p.end]).collect()
    }

    #[test]
    fn splits_on_newline_and_semicolon_outside_brackets_and_strings() {
        assert_eq!(texts("a; b\nc"), vec!["a", " b", "c"]);
        assert_eq!(
            texts("f(1;\n2)\n'x;y'\n# c; d\n"),
            vec!["f(1;\n2)", "'x;y'"]
        );
        assert_eq!(texts("s = '''a\n;b'''; t"), vec!["s = '''a\n;b'''", " t"]);
        assert_eq!(texts("x = 1 + \\\n 2\ny"), vec!["x = 1 + \\\n 2", "y"]);
        assert_eq!(texts("\n\n  \n# only\n"), Vec::<&str>::new());
    }

    #[test]
    fn records_what_is_open() {
        let p = split("x = (1, [2");
        assert_eq!(p[0].open, Some(Open::Bracket('[', 8)));
        assert_eq!(split("x = '''a\nb")[0].open, Some(Open::TripleString(4)));
        assert_eq!(split("x = 1 \\\n")[0].open, Some(Open::Backslash(6)));
        assert!(!is_complete("bpy.ops.mesh.primitive_cube_add(size=2,"));
        assert!(!is_complete("x = '''abc"));
        assert!(!is_complete("x = 1 + \\\n"));
        assert!(is_complete("bpy.ops.mesh.primitive_cube_add(size=2)"));
        assert!(is_complete("x = '(' # ("));
        assert!(is_complete(""));
    }

    #[test]
    fn records_bad_closers() {
        let p = split("x = [1, 2)");
        assert_eq!(
            p[0].bad_close,
            Some(BadClose {
                at: 9,
                close: ')',
                open: Some(('[', 4))
            })
        );
        let p = split(")\nok");
        assert_eq!(
            p[0].bad_close,
            Some(BadClose {
                at: 0,
                close: ')',
                open: None
            })
        );
        assert_eq!(p[1].bad_close, None);
    }

    #[test]
    fn raw_splitter_continues_while_open_and_stops_on_blank_or_eof() {
        let mut r = RawSplitter::new();
        assert_eq!(r.push_line("\n"), None);
        assert_eq!(r.push_line("bpy.ops.mesh.primitive_cube_add(\n"), None);
        assert!(r.is_pending());
        assert_eq!(
            r.push_line("  size=2)\n"),
            Some(Chunk {
                src: "bpy.ops.mesh.primitive_cube_add(\n  size=2)\n".into(),
                first_line: 2
            })
        );
        // Blank line ends a pending continuation.
        assert_eq!(
            r.push_line("bpy.ops.mesh.primitive_cube_add(size=2\n"),
            None
        );
        assert_eq!(
            r.push_line("\n"),
            Some(Chunk {
                src: "bpy.ops.mesh.primitive_cube_add(size=2\n".into(),
                first_line: 4
            })
        );
        assert!(!r.is_pending());
        // EOF ends it too.
        assert_eq!(r.push_line("x = (1"), None);
        assert_eq!(
            r.finish(),
            Some(Chunk {
                src: "x = (1\n".into(),
                first_line: 6
            })
        );
        assert_eq!(r.finish(), None);
        assert_eq!(r.lines_seen(), 6);
    }
}
