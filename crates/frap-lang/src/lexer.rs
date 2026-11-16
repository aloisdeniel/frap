// SPDX-License-Identifier: GPL-3.0-or-later
//! Hand-written lexer shared by both syntaxes (ARCHITECTURE §11.3/§11.4).

use frap_rna::{FrapError, PyExc};
use smol_str::SmolStr;

use crate::errors::SYNTAX;

/// Token kinds.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// Identifier.
    Ident(SmolStr),
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// String literal (escapes resolved).
    Str(String),
    /// Single punctuation character: `( ) [ ] { } , . = : ; + - * / @`.
    Punct(char),
    /// Two-character operator: `+= -= *= /= ** == !=`.
    Op2(&'static str),
}

/// A token with its byte range (absolute offsets into the parsed source).
#[derive(Clone, Debug)]
pub struct Token {
    /// Kind.
    pub tok: Tok,
    /// Start byte offset.
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
    /// Whether whitespace precedes the token (short form is whitespace-sensitive).
    pub space_before: bool,
}

/// Lexes `src[start..end]` (one statement); offsets in tokens are absolute.
pub fn lex(src: &str, start: usize, end: usize) -> Result<Vec<Token>, (FrapError, usize)> {
    let b = src.as_bytes();
    let mut i = start;
    let mut out = Vec::new();
    let mut space = false;
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
            }
            b'\'' | b'"' => {
                let (s, n) = lex_string(src, i, end, false)?;
                out.push(Token {
                    tok: Tok::Str(s),
                    start: i as u32,
                    end: n as u32,
                    space_before: space,
                });
                space = false;
                i = n;
            }
            b'r' | b'R' if i + 1 < end && (b[i + 1] == b'\'' || b[i + 1] == b'"') => {
                let (s, n) = lex_string(src, i + 1, end, true)?;
                out.push(Token {
                    tok: Tok::Str(s),
                    start: i as u32,
                    end: n as u32,
                    space_before: space,
                });
                space = false;
                i = n;
            }
            b'0'..=b'9' => {
                let (t, n) = lex_number(src, i, end)?;
                out.push(Token {
                    tok: t,
                    start: i as u32,
                    end: n as u32,
                    space_before: space,
                });
                space = false;
                i = n;
            }
            b'.' if i + 1 < end && b[i + 1].is_ascii_digit() => {
                let (t, n) = lex_number(src, i, end)?;
                out.push(Token {
                    tok: t,
                    start: i as u32,
                    end: n as u32,
                    space_before: space,
                });
                space = false;
                i = n;
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 => {
                let s = i;
                while i < end && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 0x80) {
                    i += 1;
                }
                out.push(Token {
                    tok: Tok::Ident(SmolStr::new(&src[s..i])),
                    start: s as u32,
                    end: i as u32,
                    space_before: space,
                });
                space = false;
            }
            _ => {
                let two = if i + 1 < end { &src[i..i + 2] } else { "" };
                let op2 = ["+=", "-=", "*=", "/=", "**", "==", "!="]
                    .into_iter()
                    .find(|o| *o == two);
                if let Some(o) = op2 {
                    out.push(Token {
                        tok: Tok::Op2(o),
                        start: i as u32,
                        end: (i + 2) as u32,
                        space_before: space,
                    });
                    i += 2;
                } else if "()[]{},.=:;+-*/@".as_bytes().contains(&c) {
                    out.push(Token {
                        tok: Tok::Punct(c as char),
                        start: i as u32,
                        end: (i + 1) as u32,
                        space_before: space,
                    });
                    i += 1;
                } else {
                    let ch = src[i..].chars().next().unwrap_or('?');
                    return Err((
                        FrapError::new(
                            PyExc::SyntaxError,
                            SYNTAX,
                            format!("invalid character '{ch}'"),
                        ),
                        i,
                    ));
                }
                space = false;
            }
        }
    }
    Ok(out)
}

fn lex_string(
    src: &str,
    q_at: usize,
    end: usize,
    raw: bool,
) -> Result<(String, usize), (FrapError, usize)> {
    let b = src.as_bytes();
    let q = b[q_at];
    let mut i = q_at + 1;
    let mut out = String::new();
    while i < end {
        let c = b[i];
        if c == q {
            return Ok((out, i + 1));
        }
        if c == b'\n' {
            break;
        }
        if c == b'\\' && i + 1 < end {
            let n = b[i + 1];
            if raw {
                out.push('\\');
                out.push(n as char);
                i += 2;
                continue;
            }
            let esc = match n {
                b'n' => Some('\n'),
                b't' => Some('\t'),
                b'r' => Some('\r'),
                b'0' => Some('\0'),
                b'\\' => Some('\\'),
                b'\'' => Some('\''),
                b'"' => Some('"'),
                _ => None,
            };
            match esc {
                Some(e) => {
                    out.push(e);
                    i += 2;
                }
                None => {
                    out.push('\\');
                    i += 1;
                }
            }
            continue;
        }
        let ch = src[i..]
            .chars()
            .next()
            .expect("invariant: i < end is a char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    Err((
        FrapError::new(PyExc::SyntaxError, SYNTAX, "unterminated string literal"),
        q_at,
    ))
}

fn lex_number(src: &str, start: usize, end: usize) -> Result<(Tok, usize), (FrapError, usize)> {
    let b = src.as_bytes();
    let mut i = start;
    let mut is_float = false;
    while i < end && (b[i].is_ascii_digit() || b[i] == b'_') {
        i += 1;
    }
    if i < end
        && b[i] == b'.'
        && !(i + 1 < end && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_'))
    {
        is_float = true;
        i += 1;
        while i < end && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
    }
    if i < end && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < end && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < end && b[j].is_ascii_digit() {
            is_float = true;
            i = j;
            while i < end && b[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    let text: String = src[start..i].chars().filter(|c| *c != '_').collect();
    let bad = || {
        (
            FrapError::new(PyExc::SyntaxError, SYNTAX, "invalid decimal literal"),
            start,
        )
    };
    let tok = if is_float {
        Tok::Float(text.parse().map_err(|_| bad())?)
    } else {
        Tok::Int(text.parse().map_err(|_| bad())?)
    };
    Ok((tok, i))
}
