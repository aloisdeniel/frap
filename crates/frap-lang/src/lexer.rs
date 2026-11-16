// SPDX-License-Identifier: GPL-3.0-or-later
//! Hand-written lexer shared by both syntaxes (ARCHITECTURE §11.3/§11.4).
//!
//! The lexer never fails: malformed input becomes a [`Tok::Error`] token that the parser
//! reports when (and only when) it reaches it. This matters for the short form, whose `BARE`
//! words (`Cube.001a`, `my-object`) are scanned from the source text and may cover bytes that
//! do not form valid Python tokens.
//!
//! Number, string and identifier rules follow CPython 3.13 (the interpreter bundled with
//! Blender 5.2.4); error wording is CPython's, verified with `compile()` inside Blender.

use smol_str::SmolStr;

/// Token kinds.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// Identifier (Unicode letters allowed, as in Python).
    Ident(SmolStr),
    /// Integer literal (decimal, `0x`, `0o`, `0b`, underscores).
    Int(i64),
    /// Float literal.
    Float(f64),
    /// String literal (escapes resolved; adjacent literals are joined by the parser).
    Str(String),
    /// Single punctuation character: `( ) [ ] { } , . = : ; + - * / @ % < > & | ^ ~ ! $ ? \``.
    Punct(char),
    /// Multi-character operator: `+= -= *= /= //= **= @= %= ** // == != <= >= -> :=`.
    Op(&'static str),
    /// A malformed token, reported by the parser if it is reached.
    Error(LexError),
}

/// Lexical errors (CPython wording through [`LexError::message`]).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LexError {
    /// `'abc` (single-quoted string ended by a newline or the end of input).
    UnterminatedString,
    /// `'''abc` reaching the end of input.
    UnterminatedTripleString,
    /// `€`
    InvalidCharacter(char),
    /// U+00A0 and other non-printable characters.
    NonPrintable(char),
    /// `1a`, `1__0`, `1_`, `1e+`
    InvalidDecimal,
    /// `0x`, `0x1G`
    InvalidHex,
    /// `0o`
    InvalidOctal,
    /// `0b`
    InvalidBinary,
    /// `0o9`, `0b2`: (digit, "octal" | "binary")
    InvalidDigit(char, &'static str),
    /// `01`
    LeadingZeros,
    /// `x = \ 1`
    LineContinuation,
    /// A `\` line continuation at the end of input.
    UnexpectedEof,
    /// `'\x4'`: (first byte, last byte, which escape) inside the literal body.
    UnicodeEscape(usize, usize, &'static str),
    /// Valid Python that frap does not accept (f-strings, bytes, complex numbers, `\N{…}`).
    Unsupported(&'static str),
    /// An integer outside the signed 64-bit range.
    IntTooLarge,
}

impl LexError {
    /// CPython's message (`detected_line` is the line used by "unterminated" messages).
    pub fn message(&self, detected_line: u32) -> String {
        match self {
            LexError::UnterminatedString => {
                format!("unterminated string literal (detected at line {detected_line})")
            }
            LexError::UnterminatedTripleString => format!(
                "unterminated triple-quoted string literal (detected at line {detected_line})"
            ),
            LexError::InvalidCharacter(c) => {
                format!("invalid character '{c}' (U+{:04X})", *c as u32)
            }
            LexError::NonPrintable(c) => {
                format!("invalid non-printable character U+{:04X}", *c as u32)
            }
            LexError::InvalidDecimal => "invalid decimal literal".into(),
            LexError::InvalidHex => "invalid hexadecimal literal".into(),
            LexError::InvalidOctal => "invalid octal literal".into(),
            LexError::InvalidBinary => "invalid binary literal".into(),
            LexError::InvalidDigit(c, base) => format!("invalid digit '{c}' in {base} literal"),
            LexError::LeadingZeros => {
                "leading zeros in decimal integer literals are not permitted; \
                                       use an 0o prefix for octal integers"
                    .into()
            }
            LexError::LineContinuation => {
                "unexpected character after line continuation character".into()
            }
            LexError::UnexpectedEof => "unexpected EOF while parsing".into(),
            LexError::UnicodeEscape(a, b, what) => format!(
                "(unicode error) 'unicodeescape' codec can't decode bytes in position {a}-{b}: {what}"
            ),
            LexError::Unsupported(what) => format!("frap: {what} are not supported"),
            LexError::IntTooLarge => {
                "frap: integer literals must fit in a signed 64-bit integer".into()
            }
        }
    }

    /// True for valid Python that frap rejects (`UNSUPPORTED_SYNTAX` rather than `SYNTAX`).
    pub fn is_unsupported(&self) -> bool {
        matches!(self, LexError::Unsupported(_) | LexError::IntTooLarge)
    }
}

/// A token with its byte range (absolute offsets into the parsed source).
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// Kind.
    pub tok: Tok,
    /// Start byte offset.
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
    /// Whether whitespace (or a comment) precedes the token (the short form is
    /// whitespace-sensitive).
    pub space_before: bool,
}

/// Python identifier start (ASCII letters, `_`, Unicode letters).
pub fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

/// Python identifier continuation.
pub fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

const OPS3: [&str; 2] = ["**=", "//="];
const OPS2: [&str; 14] = [
    "+=", "-=", "*=", "/=", "@=", "%=", "**", "//", "==", "!=", "<=", ">=", "->", ":=",
];
const PUNCT: &str = "()[]{},.=:;+-*/@%<>&|^~!$?`";

/// Lexes `src[start..end]` (one statement); offsets in tokens are absolute. Never fails.
pub fn lex(src: &str, start: usize, end: usize) -> Vec<Token> {
    let b = src.as_bytes();
    let end = end.min(src.len());
    let mut i = start;
    let mut out = Vec::new();
    let mut space = false;
    let push = |out: &mut Vec<Token>, tok: Tok, s: usize, e: usize, space: &mut bool| {
        out.push(Token {
            tok,
            start: s as u32,
            end: e as u32,
            space_before: *space,
        });
        *space = false;
    };
    while i < end {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' => {
                space = true;
                i += 1;
            }
            b'#' => {
                while i < end && b[i] != b'\n' {
                    i += 1;
                }
                space = true;
            }
            b'\\' => {
                // Explicit line joining: `\` immediately followed by a newline.
                let mut j = i + 1;
                if j < end && b[j] == b'\r' {
                    j += 1;
                }
                if j < end && b[j] == b'\n' {
                    space = true;
                    i = j + 1;
                } else if j >= end {
                    push(
                        &mut out,
                        Tok::Error(LexError::UnexpectedEof),
                        i,
                        end,
                        &mut space,
                    );
                    i = end;
                } else {
                    push(
                        &mut out,
                        Tok::Error(LexError::LineContinuation),
                        i,
                        i + 1,
                        &mut space,
                    );
                    i += 1;
                }
            }
            b'\'' | b'"' => {
                let (tok, n) = lex_string(src, i, end, StrKind::Normal);
                push(&mut out, tok, i, n, &mut space);
                i = n;
            }
            b'0'..=b'9' => {
                let (tok, n) = lex_number(src, i, end);
                push(&mut out, tok, i, n, &mut space);
                i = n;
            }
            b'.' if i + 1 < end && b[i + 1].is_ascii_digit() => {
                let (tok, n) = lex_number(src, i, end);
                push(&mut out, tok, i, n, &mut space);
                i = n;
            }
            _ => {
                let ch = src[i..end].chars().next().unwrap_or('\u{fffd}');
                if is_ident_start(ch) {
                    let s = i;
                    i += ch.len_utf8();
                    while i < end {
                        let c2 = src[i..end].chars().next().unwrap_or(' ');
                        if !is_ident_continue(c2) {
                            break;
                        }
                        i += c2.len_utf8();
                    }
                    let word = &src[s..i];
                    // String prefixes: r, u, b, f and their combinations.
                    if i < end && (b[i] == b'\'' || b[i] == b'"') && word.len() <= 2 {
                        let lower = word.to_ascii_lowercase();
                        let kind = match lower.as_str() {
                            "r" => Some(StrKind::Raw),
                            "u" => Some(StrKind::Normal),
                            "b" | "br" | "rb" => Some(StrKind::Bytes),
                            "f" | "fr" | "rf" => Some(StrKind::Format),
                            _ => None,
                        };
                        if let Some(kind) = kind {
                            let (tok, n) = lex_string(src, i, end, kind);
                            push(&mut out, tok, s, n, &mut space);
                            i = n;
                            continue;
                        }
                    }
                    push(&mut out, Tok::Ident(SmolStr::new(word)), s, i, &mut space);
                    continue;
                }
                let rest = &src[i..end];
                if let Some(op) = OPS3
                    .iter()
                    .chain(OPS2.iter())
                    .find(|o| rest.starts_with(**o))
                {
                    push(&mut out, Tok::Op(op), i, i + op.len(), &mut space);
                    i += op.len();
                } else if c.is_ascii() && PUNCT.as_bytes().contains(&c) {
                    push(&mut out, Tok::Punct(c as char), i, i + 1, &mut space);
                    i += 1;
                } else {
                    let err = if ch.is_whitespace() || ch.is_control() || c < 0x20 {
                        LexError::NonPrintable(ch)
                    } else {
                        LexError::InvalidCharacter(ch)
                    };
                    let n = i + ch.len_utf8();
                    push(&mut out, Tok::Error(err), i, n, &mut space);
                    i = n;
                }
            }
        }
    }
    out
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum StrKind {
    Normal,
    Raw,
    Bytes,
    Format,
}

/// Lexes a string literal whose opening quote is at `q_at` (any prefix precedes it).
fn lex_string(src: &str, q_at: usize, end: usize, kind: StrKind) -> (Tok, usize) {
    let b = src.as_bytes();
    let q = b[q_at];
    let triple = q_at + 2 < end && b[q_at + 1] == q && b[q_at + 2] == q;
    let body = if triple { q_at + 3 } else { q_at + 1 };
    let mut i = body;
    let mut out = String::new();
    let mut err: Option<LexError> = None;
    let raw = kind == StrKind::Raw;
    loop {
        if i >= end {
            let e = if triple {
                LexError::UnterminatedTripleString
            } else {
                LexError::UnterminatedString
            };
            return (Tok::Error(e), end);
        }
        let c = b[i];
        if c == q {
            if !triple {
                i += 1;
                break;
            }
            if i + 2 < end && b[i + 1] == q && b[i + 2] == q {
                i += 3;
                break;
            }
        }
        if c == b'\n' && !triple {
            // CPython reports the unterminated literal; the newline is not part of it.
            return (Tok::Error(LexError::UnterminatedString), i);
        }
        if c == b'\\' {
            if i + 1 >= end {
                let e = if triple {
                    LexError::UnterminatedTripleString
                } else {
                    LexError::UnterminatedString
                };
                return (Tok::Error(e), end);
            }
            let n = b[i + 1];
            if raw {
                // Raw strings keep the backslash; an escaped quote does not end the literal.
                out.push('\\');
                let ch = src[i + 1..end].chars().next().unwrap_or('\\');
                out.push(ch);
                i += 1 + ch.len_utf8();
                continue;
            }
            let at = i - body;
            match n {
                b'\n' => i += 2, // line continuation inside the literal
                b'\r' if i + 2 < end && b[i + 2] == b'\n' => i += 3,
                b'n' | b't' | b'r' | b'\\' | b'\'' | b'"' | b'a' | b'b' | b'f' | b'v' => {
                    out.push(match n {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'a' => '\x07',
                        b'b' => '\x08',
                        b'f' => '\x0c',
                        b'v' => '\x0b',
                        other => other as char,
                    });
                    i += 2;
                }
                b'0'..=b'7' => {
                    let mut j = i + 1;
                    let mut v = 0u32;
                    while j < end && j < i + 4 && (b'0'..=b'7').contains(&b[j]) {
                        v = v * 8 + u32::from(b[j] - b'0');
                        j += 1;
                    }
                    out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                    i = j;
                }
                b'x' | b'u' | b'U' => {
                    let (want, what) = match n {
                        b'x' => (2, "truncated \\xXX escape"),
                        b'u' => (4, "truncated \\uXXXX escape"),
                        _ => (8, "truncated \\UXXXXXXXX escape"),
                    };
                    let mut j = i + 2;
                    while j < end && j < i + 2 + want && b[j].is_ascii_hexdigit() {
                        j += 1;
                    }
                    if j - (i + 2) < want {
                        err.get_or_insert(LexError::UnicodeEscape(at, j - 1 - body, what));
                        i = j;
                        continue;
                    }
                    let v = u32::from_str_radix(&src[i + 2..j], 16).unwrap_or(0xfffd);
                    match char::from_u32(v) {
                        Some(ch) => out.push(ch),
                        None => {
                            err.get_or_insert(LexError::UnicodeEscape(
                                at,
                                j - 1 - body,
                                "illegal Unicode character",
                            ));
                        }
                    }
                    i = j;
                }
                b'N' => {
                    err.get_or_insert(LexError::Unsupported("\\N{...} escapes"));
                    i += 2;
                }
                _ => {
                    // Unknown escape: CPython keeps the backslash (with a SyntaxWarning).
                    out.push('\\');
                    i += 1;
                }
            }
            continue;
        }
        let ch = src[i..end].chars().next().unwrap_or('\u{fffd}');
        out.push(ch);
        i += ch.len_utf8();
    }
    if let Some(e) = err {
        return (Tok::Error(e), i);
    }
    match kind {
        StrKind::Bytes => (Tok::Error(LexError::Unsupported("bytes literals")), i),
        StrKind::Format => (Tok::Error(LexError::Unsupported("f-strings")), i),
        _ => (Tok::Str(out), i),
    }
}

/// Lexes a number starting at `start` (a digit, or `.` followed by a digit).
fn lex_number(src: &str, start: usize, end: usize) -> (Tok, usize) {
    let b = src.as_bytes();
    let at = |k: usize| if k < end { b[k] } else { 0 };
    let ident_follows =
        |k: usize| k < end && src[k..end].chars().next().is_some_and(is_ident_continue);
    // Skips to the end of an identifier-ish run so one bad literal is one error token.
    let bad_end = |mut k: usize| {
        while k < end {
            let c = src[k..end].chars().next().unwrap_or(' ');
            if !(is_ident_continue(c) || c == '.') {
                break;
            }
            k += c.len_utf8();
        }
        k
    };

    // Prefixed integers.
    if at(start) == b'0' && matches!(at(start + 1), b'x' | b'X' | b'o' | b'O' | b'b' | b'B') {
        let (radix, base_name, err) = match at(start + 1) {
            b'x' | b'X' => (16, "hexadecimal", LexError::InvalidHex),
            b'o' | b'O' => (8, "octal", LexError::InvalidOctal),
            _ => (2, "binary", LexError::InvalidBinary),
        };
        let mut i = start + 2;
        let mut digits = String::new();
        loop {
            if at(i) == b'_' {
                i += 1;
            }
            let c = at(i) as char;
            if c.is_digit(radix) {
                digits.push(c);
                i += 1;
            } else {
                break;
            }
        }
        if radix != 16 && at(i).is_ascii_digit() {
            return (
                Tok::Error(LexError::InvalidDigit(at(i) as char, base_name)),
                bad_end(i),
            );
        }
        if digits.is_empty() || at(i - 1) == b'_' {
            return (Tok::Error(err), bad_end(i));
        }
        if ident_follows(i) {
            return (Tok::Error(err), bad_end(i));
        }
        return match i64::from_str_radix(&digits, radix) {
            Ok(v) => (Tok::Int(v), i),
            Err(_) => (Tok::Error(LexError::IntTooLarge), i),
        };
    }

    let mut text = String::new();
    let mut i = start;
    // Digit run with single underscores between digits; returns false on a bad underscore.
    let digit_run = |i: &mut usize, text: &mut String| -> bool {
        let mut prev_digit = false;
        while *i < end {
            let c = b[*i];
            if c.is_ascii_digit() {
                text.push(c as char);
                prev_digit = true;
            } else if c == b'_' {
                if !prev_digit || !(*i + 1 < end && b[*i + 1].is_ascii_digit()) {
                    return false;
                }
                prev_digit = false;
            } else {
                break;
            }
            *i += 1;
        }
        true
    };
    if !digit_run(&mut i, &mut text) {
        return (Tok::Error(LexError::InvalidDecimal), bad_end(i));
    }
    let int_part = text.clone();
    let mut is_float = false;
    if at(i) == b'.' {
        is_float = true;
        text.push('.');
        i += 1;
        if at(i).is_ascii_digit() && !digit_run(&mut i, &mut text) {
            return (Tok::Error(LexError::InvalidDecimal), bad_end(i));
        }
    }
    if matches!(at(i), b'e' | b'E') {
        let mut j = i + 1;
        if matches!(at(j), b'+' | b'-') {
            j += 1;
        }
        if !at(j).is_ascii_digit() {
            return (Tok::Error(LexError::InvalidDecimal), bad_end(j));
        }
        is_float = true;
        text.push('e');
        text.push_str(&src[i + 1..j]);
        i = j;
        if !digit_run(&mut i, &mut text) {
            return (Tok::Error(LexError::InvalidDecimal), bad_end(i));
        }
    }
    if matches!(at(i), b'j' | b'J') {
        return (
            Tok::Error(LexError::Unsupported("complex numbers")),
            bad_end(i + 1),
        );
    }
    if ident_follows(i) {
        return (Tok::Error(LexError::InvalidDecimal), bad_end(i));
    }
    if is_float {
        // Rust does not accept "1." / ".5" / "1.e5" in every form: normalise.
        let mut t = text.replace(".e", ".0e");
        if t.ends_with('.') {
            t.push('0');
        }
        if t.starts_with('.') {
            t.insert(0, '0');
        }
        return match t.parse::<f64>() {
            Ok(v) => (Tok::Float(v), i),
            Err(_) => (Tok::Error(LexError::InvalidDecimal), i),
        };
    }
    if int_part.len() > 1 && int_part.starts_with('0') && int_part.bytes().any(|c| c != b'0') {
        return (Tok::Error(LexError::LeadingZeros), i);
    }
    match int_part.parse::<i64>() {
        Ok(v) => (Tok::Int(v), i),
        Err(_) => (Tok::Error(LexError::IntTooLarge), i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s, 0, s.len()).into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(
            toks("0x1F 0o17 0b101"),
            vec![Tok::Int(31), Tok::Int(15), Tok::Int(5)]
        );
        assert_eq!(toks("1_000.5e-3"), vec![Tok::Float(1.0005)]);
        assert_eq!(
            toks("1. .5 1.e5 00 0_0"),
            vec![
                Tok::Float(1.0),
                Tok::Float(0.5),
                Tok::Float(1e5),
                Tok::Int(0),
                Tok::Int(0)
            ]
        );
        assert_eq!(toks("1e309"), vec![Tok::Float(f64::INFINITY)]);
        assert_eq!(toks("01"), vec![Tok::Error(LexError::LeadingZeros)]);
        assert_eq!(toks("01.5"), vec![Tok::Float(1.5)]);
        assert_eq!(toks("1__0"), vec![Tok::Error(LexError::InvalidDecimal)]);
        assert_eq!(toks("1_"), vec![Tok::Error(LexError::InvalidDecimal)]);
        assert_eq!(toks("1a"), vec![Tok::Error(LexError::InvalidDecimal)]);
        assert_eq!(toks("1e+"), vec![Tok::Error(LexError::InvalidDecimal)]);
        assert_eq!(toks("0x"), vec![Tok::Error(LexError::InvalidHex)]);
        assert_eq!(toks("0x1G"), vec![Tok::Error(LexError::InvalidHex)]);
        assert_eq!(
            toks("0o9"),
            vec![Tok::Error(LexError::InvalidDigit('9', "octal"))]
        );
        assert_eq!(
            toks("0b12"),
            vec![Tok::Error(LexError::InvalidDigit('2', "binary"))]
        );
        assert_eq!(
            toks("1j"),
            vec![Tok::Error(LexError::Unsupported("complex numbers"))]
        );
        assert_eq!(
            toks("99999999999999999999"),
            vec![Tok::Error(LexError::IntTooLarge)]
        );
        assert_eq!(toks("9223372036854775807"), vec![Tok::Int(i64::MAX)]);
    }

    #[test]
    fn strings() {
        assert_eq!(toks(r"'é\x41\101\a'"), vec![Tok::Str("éAA\x07".into())]);
        assert_eq!(toks(r"r'a\nb'"), vec![Tok::Str("a\\nb".into())]);
        assert_eq!(toks(r"'a\qb'"), vec![Tok::Str("a\\qb".into())]);
        assert_eq!(
            toks("u'x' \"y\""),
            vec![Tok::Str("x".into()), Tok::Str("y".into())]
        );
        assert_eq!(toks("'''a\nb'''"), vec![Tok::Str("a\nb".into())]);
        assert_eq!(
            toks("'it''s'"),
            vec![Tok::Str("it".into()), Tok::Str("s".into())]
        );
        assert_eq!(toks("'abc"), vec![Tok::Error(LexError::UnterminatedString)]);
        assert_eq!(
            toks("'''abc"),
            vec![Tok::Error(LexError::UnterminatedTripleString)]
        );
        assert_eq!(
            toks("f'{a}'"),
            vec![Tok::Error(LexError::Unsupported("f-strings"))]
        );
        assert_eq!(
            toks("b'a'"),
            vec![Tok::Error(LexError::Unsupported("bytes literals"))]
        );
        assert_eq!(
            toks(r"'\x4'"),
            vec![Tok::Error(LexError::UnicodeEscape(
                0,
                2,
                "truncated \\xXX escape"
            ))]
        );
        assert_eq!(
            LexError::UnicodeEscape(0, 2, "truncated \\xXX escape").message(1),
            "(unicode error) 'unicodeescape' codec can't decode bytes in position 0-2: truncated \\xXX escape"
        );
    }

    #[test]
    fn operators_and_characters() {
        assert_eq!(
            toks("a **= b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Op("**="),
                Tok::Ident("b".into())
            ]
        );
        assert_eq!(
            toks("x\\\n+1"),
            vec![Tok::Ident("x".into()), Tok::Punct('+'), Tok::Int(1)]
        );
        assert_eq!(toks("€"), vec![Tok::Error(LexError::InvalidCharacter('€'))]);
        assert_eq!(
            LexError::InvalidCharacter('€').message(1),
            "invalid character '€' (U+20AC)"
        );
        assert_eq!(
            toks("\u{a0}"),
            vec![Tok::Error(LexError::NonPrintable('\u{a0}'))]
        );
        assert_eq!(toks("Würfel"), vec![Tok::Ident("Würfel".into())]);
        assert_eq!(
            toks("x \\"),
            vec![Tok::Ident("x".into()), Tok::Error(LexError::UnexpectedEof)]
        );
    }
}
