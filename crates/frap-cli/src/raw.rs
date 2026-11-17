// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw mode (§11.1, §13.1): `frap` on stdin, `frap run <file>`, `frap -c "<statements>"`.
//!
//! On stdin a statement continues across lines only while a bracket, a triple-quoted string or
//! a `\` continuation is open, and a blank line or EOF ends a pending continuation
//! (`frap_lang::split::RawSplitter`): the chunk is parsed as is and an unclosed bracket becomes
//! `SyntaxError: '(' was never closed`, so a human session never hangs on a typo. Invalid
//! UTF-8 on a line is a `SyntaxError` response for that line (kind `invalid`). A script file or
//! `-c` text is one complete unit (`parse_complete`).

use std::io::{self, BufRead, Write};

use frap_lang::errors::SYNTAX;
use frap_lang::split::RawSplitter;
use frap_rna::{FrapError, PyExc, Span};
use frap_session::Session;

use crate::out::Out;

/// What a run ended with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Every statement ran (or failed in-band).
    Done,
    /// `--fail-fast` stopped after a failed statement.
    Stopped,
}

/// Runs a complete source unit starting at `first_line`; with `fail_fast`, stops after the
/// first failed response.
pub fn run_unit<W: Write>(
    session: &mut Session,
    out: &mut Out<W>,
    src: &str,
    first_line: u32,
    fail_fast: bool,
) -> io::Result<Flow> {
    let _span = tracing::debug_span!("chunk", line = first_line).entered();
    for r in session.exec_source_at(src, first_line) {
        out.response(&r)?;
        if fail_fast && !r.ok() {
            return Ok(Flow::Stopped);
        }
    }
    Ok(Flow::Done)
}

/// CPython's wording for a line that is not UTF-8:
/// `(unicode error) 'utf-8' codec can't decode byte 0xff in position 3: invalid start byte`.
pub fn utf8_error(bytes: &[u8], e: &std::str::Utf8Error, line: u32) -> FrapError {
    let pos = e.valid_up_to();
    let b = bytes[pos];
    let reason = match e.error_len() {
        None => "unexpected end of data",
        Some(_) if matches!(b, 0x80..=0xC1 | 0xF5..=0xFF) => "invalid start byte",
        Some(_) => "invalid continuation byte",
    };
    let prefix = std::str::from_utf8(&bytes[..pos]).unwrap_or_default();
    let col_chars = u32::try_from(prefix.chars().count()).unwrap_or(u32::MAX);
    let col = u32::try_from(pos).unwrap_or(u32::MAX);
    FrapError::new(
        PyExc::SyntaxError,
        SYNTAX,
        format!(
            "(unicode error) 'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: {reason}"
        ),
    )
    .with_span(Span {
        start: col,
        len: 1,
        line,
        col: col.saturating_add(1),
        col_chars: col_chars.saturating_add(1),
    })
}

/// Reads statements line by line from `input` until EOF (§11.1), answering each statement as
/// soon as it is complete.
pub fn run_lines<R: BufRead, W: Write>(
    session: &mut Session,
    out: &mut Out<W>,
    mut input: R,
    fail_fast: bool,
) -> io::Result<Flow> {
    let mut splitter = RawSplitter::new();
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if input.read_until(b'\n', &mut raw)? == 0 {
            break;
        }
        match std::str::from_utf8(&raw) {
            Ok(line) => {
                if let Some(chunk) = splitter.push_line(line)
                    && run_unit(session, out, &chunk.src, chunk.first_line, fail_fast)?
                        == Flow::Stopped
                {
                    return Ok(Flow::Stopped);
                }
            }
            Err(e) => {
                // The bad line ends a pending continuation like a blank line would, then is
                // answered on its own (kind `invalid`).
                if let Some(chunk) = splitter.push_line("\n")
                    && run_unit(session, out, &chunk.src, chunk.first_line, fail_fast)?
                        == Flow::Stopped
                {
                    return Ok(Flow::Stopped);
                }
                let line_no = splitter.lines_seen();
                let r = session.reject(line_no, utf8_error(&raw, &e, line_no));
                out.response(&r)?;
                if fail_fast {
                    return Ok(Flow::Stopped);
                }
            }
        }
    }
    if let Some(chunk) = splitter.finish() {
        return run_unit(session, out, &chunk.src, chunk.first_line, fail_fast);
    }
    Ok(Flow::Done)
}

/// `frap run <file>`: a UTF-8 script is one complete unit; a script with invalid UTF-8 is read
/// line by line like stdin, so only the bad lines fail.
pub fn run_bytes<W: Write>(
    session: &mut Session,
    out: &mut Out<W>,
    bytes: &[u8],
    fail_fast: bool,
) -> io::Result<Flow> {
    match std::str::from_utf8(bytes) {
        Ok(src) => run_unit(
            session,
            out,
            src.strip_prefix('\u{feff}').unwrap_or(src),
            1,
            fail_fast,
        ),
        Err(_) => run_lines(session, out, bytes, fail_fast),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_session::SessionOptions;
    use serde_json::Value as J;

    fn lines(out: Out<Vec<u8>>) -> Vec<J> {
        String::from_utf8(out.into_inner())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn session() -> Session {
        Session::new(SessionOptions::default()).unwrap()
    }

    #[test]
    fn blank_line_ends_an_unclosed_bracket() {
        let mut s = session();
        let mut out = Out::new(Vec::new());
        let input: &[u8] = b"print(1,\n\nprint(2,\n 3)\nprint(4\n";
        assert_eq!(
            run_lines(&mut s, &mut out, input, false).unwrap(),
            Flow::Done
        );
        let v = lines(out);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0]["kind"], "invalid");
        assert_eq!(v[0]["error"]["exception"], "SyntaxError");
        assert_eq!(v[0]["error"]["message"], "'(' was never closed");
        assert_eq!(v[0]["line"], 1);
        assert_eq!(v[1]["value"], serde_json::json!([2, 3]));
        assert_eq!(v[1]["line"], 3);
        // EOF ends the last continuation.
        assert_eq!(v[2]["error"]["message"], "'(' was never closed");
        assert_eq!(v[2]["line"], 5);
    }

    #[test]
    fn invalid_utf8_line_is_a_syntax_error_for_that_line_only() {
        let mut s = session();
        let mut out = Out::new(Vec::new());
        let input: &[u8] = b"print(1)\nprint('\xff')\nprint(3)\n";
        run_lines(&mut s, &mut out, input, false).unwrap();
        let v = lines(out);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1]["kind"], "invalid");
        assert_eq!(v[1]["line"], 2);
        assert_eq!(v[1]["error"]["code"], "SYNTAX");
        assert_eq!(
            v[1]["error"]["message"],
            "(unicode error) 'utf-8' codec can't decode byte 0xff in position 7: invalid start byte"
        );
        assert_eq!(v[1]["error"]["span"]["col"], 8);
        assert_eq!(v[2]["line"], 3);
        assert_eq!(v[2]["value"], serde_json::json!([3]));
    }

    #[test]
    fn invalid_utf8_ends_a_pending_continuation() {
        let mut s = session();
        let mut out = Out::new(Vec::new());
        let input: &[u8] = b"print(1,\n\xc3\x28\nprint(5)";
        run_lines(&mut s, &mut out, input, false).unwrap();
        let v = lines(out);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0]["error"]["message"], "'(' was never closed");
        assert!(
            v[1]["error"]["message"]
                .as_str()
                .unwrap()
                .ends_with("position 0: invalid continuation byte")
        );
        assert_eq!(v[2]["line"], 3);
    }

    #[test]
    fn fail_fast_stops_after_the_failing_statement() {
        let mut s = session();
        let mut out = Out::new(Vec::new());
        let input: &[u8] = b"print(1)\nbpy.ops.mesh.nope()\nprint(3)\n";
        assert_eq!(
            run_lines(&mut s, &mut out, input, true).unwrap(),
            Flow::Stopped
        );
        let v = lines(out);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1]["error"]["code"], "OP_UNKNOWN");
    }

    #[test]
    fn a_script_is_one_complete_unit() {
        let mut s = session();
        let mut out = Out::new(Vec::new());
        // A blank line inside brackets is fine in a file (Python semantics).
        let src = "\u{feff}print(1,\n\n 2)\nprint(3)\n";
        run_bytes(&mut s, &mut out, src.as_bytes(), false).unwrap();
        let v = lines(out);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0]["value"], serde_json::json!([1, 2]));
        assert_eq!(v[1]["line"], 4);
    }
}
