// SPDX-License-Identifier: GPL-3.0-or-later
//! bpy-line and short-form parsers (ARCHITECTURE §11.3, §11.4).
//!
//! `parse_complete` splits a complete source unit into statements (`split.rs`), lexes each
//! one (`lexer.rs`), detects its form lexically (§11.4 "Form detection") and parses it into a
//! [`Stmt`]. It never waits for more input: anything left open is an immediate `SyntaxError`
//! with CPython's wording. Every error carries a span (UTF-8 byte `col` and code-point
//! `col_chars`, §11.2).
//!
//! Submodules: `expr` (bpy expressions, paths, call arguments), `short` (short-form values,
//! operator calls and path statements).

use frap_rna::{FrapError, PyExc, Span};
use smol_str::SmolStr;

use crate::errors::{NAME, SYNTAX, UNSUPPORTED_SYNTAX};
use crate::ir::*;
use crate::lexer::{Tok, Token, lex};
use crate::split::{BadClose, Open, Piece, split};

mod expr;
mod short;

/// Maps byte offsets to line/column in O(log n) per span, whatever the line length (agents
/// send one-line `foreach_set` tuples of megabytes).
pub(crate) struct LineIndex<'a> {
    src: &'a str,
    starts: Vec<usize>,
    first_line: u32,
    /// Empty for ASCII sources (code points = bytes). Otherwise `chunk_chars[k]` is the
    /// number of code points in `src[..k * CHUNK]`.
    chunk_chars: Vec<u32>,
}

/// Granularity of [`LineIndex::chunk_chars`].
const CHUNK: usize = 64;

/// True for a byte that starts a UTF-8 code point.
fn starts_char(b: u8) -> bool {
    (b & 0xC0) != 0x80
}

impl<'a> LineIndex<'a> {
    fn new(src: &'a str, first_line: u32) -> Self {
        let mut starts = vec![0];
        starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        let chunk_chars = if src.is_ascii() {
            Vec::new()
        } else {
            let mut acc = 0u32;
            let mut v = vec![0];
            for chunk in src.as_bytes().chunks(CHUNK) {
                acc += chunk.iter().filter(|b| starts_char(**b)).count() as u32;
                v.push(acc);
            }
            v
        };
        LineIndex {
            src,
            starts,
            first_line,
            chunk_chars,
        }
    }
    fn line_idx(&self, pos: usize) -> usize {
        self.starts.partition_point(|s| *s <= pos) - 1
    }
    /// 1-based line of a byte offset (offset by `first_line`).
    fn line_of(&self, pos: usize) -> u32 {
        self.first_line + self.line_idx(pos) as u32
    }
    /// Code points in `src[..pos]`.
    fn chars_before(&self, pos: usize) -> usize {
        if self.chunk_chars.is_empty() {
            return pos;
        }
        let k = pos / CHUNK;
        let rest = &self.src.as_bytes()[k * CHUNK..pos];
        self.chunk_chars[k] as usize + rest.iter().filter(|b| starts_char(**b)).count()
    }
    /// Span of `start..end` (byte offsets; `start` must be a char boundary).
    fn span(&self, start: usize, end: usize) -> Span {
        let start = start.min(self.src.len());
        let li = self.line_idx(start);
        let ls = self.starts[li];
        Span {
            start: start as u32,
            len: end.saturating_sub(start) as u32,
            line: self.first_line + li as u32,
            col: (start - ls + 1) as u32,
            col_chars: (self.chars_before(start) - self.chars_before(ls) + 1) as u32,
        }
    }
}

/// Parses a complete source unit; never waits for more input (§11.1). One result per statement.
pub fn parse_complete(src: &str) -> Vec<Result<Stmt, FrapError>> {
    parse_complete_at(src, 1)
}

/// Like [`parse_complete`], with line numbers starting at `first_line` (raw-stdin chunks).
pub fn parse_complete_at(src: &str, first_line: u32) -> Vec<Result<Stmt, FrapError>> {
    let li = LineIndex::new(src, first_line);
    split(src)
        .iter()
        .map(|piece| parse_piece(src, &li, piece))
        .collect()
}

type R<T> = Result<T, FrapError>;

pub(super) fn syntax(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::SyntaxError, SYNTAX, msg)
}
pub(super) fn unsupported(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::SyntaxError, UNSUPPORTED_SYNTAX, msg)
}

/// `frap: variables are not supported; …` (§11.3, §13.4).
pub(super) fn variables_error(name: &str) -> FrapError {
    let mut e = unsupported(
        "frap: variables are not supported; use a full path such as bpy.data.objects['Cube']",
    )
    .with_hint("use a full path such as bpy.data.objects['Cube']");
    e.detail.insert("name".into(), name.into());
    e
}

/// `...` (valid Python: the `Ellipsis` constant).
pub(super) fn ellipsis_error() -> FrapError {
    unsupported("frap: Ellipsis (...) is not supported")
}

/// Rule 5 of §11.4: neither form.
pub(super) fn form_error() -> FrapError {
    syntax("invalid syntax").with_hint(
        "write either the bpy form, e.g. bpy.ops.mesh.primitive_cube_add(size=2) or \
         bpy.data.objects['Cube'].location = (1, 2, 3), or the short form, e.g. \
         mesh.primitive_cube_add size=2 or data.objects['Cube'].location = 1,2,3",
    )
}

/// Python statement keywords frap rejects (`for`, `if`, `def`, ...).
const STATEMENT_KEYWORDS: &[&str] = &[
    "for", "while", "if", "elif", "else", "def", "class", "with", "try", "except", "finally",
    "del", "return", "raise", "assert", "global", "nonlocal", "lambda", "async", "await", "yield",
    "break", "continue",
];

/// Python built-in functions an agent may reach for; frap names them in the error.
pub(super) const PY_BUILTINS: &[&str] = &[
    "abs",
    "all",
    "any",
    "bool",
    "callable",
    "dict",
    "dir",
    "enumerate",
    "eval",
    "exec",
    "filter",
    "format",
    "getattr",
    "hasattr",
    "help",
    "id",
    "input",
    "isinstance",
    "iter",
    "len",
    "list",
    "map",
    "max",
    "min",
    "next",
    "open",
    "ord",
    "chr",
    "pow",
    "range",
    "repr",
    "reversed",
    "round",
    "setattr",
    "sorted",
    "str",
    "sum",
    "tuple",
    "type",
    "vars",
    "zip",
    "hex",
    "oct",
    "bin",
    "divmod",
    "frozenset",
    "complex",
    "object",
    "super",
];

fn parse_piece(src: &str, li: &LineIndex<'_>, piece: &Piece) -> R<Stmt> {
    if piece.empty {
        return Err(syntax("invalid syntax").with_span(li.span(piece.start, piece.end)));
    }
    let toks = lex(src, piece.start, piece.end);
    let eof = toks.last().map(|t| t.end as usize).unwrap_or(piece.start);
    let p = P {
        src,
        t: toks,
        i: 0,
        li,
        start: piece.start,
        eof,
        depth: 0,
        short: false,
    };
    // Bracket problems are tokenizer errors: the first problem in source order wins, and an
    // unclosed bracket (found at the end of input) comes after any earlier tokenizer error.
    if piece.bad_close.is_some() || piece.open.is_some() || piece.too_deep.is_some() {
        let first_err =
            p.t.iter()
                .position(|t| matches!(t.tok, Tok::Error(_)))
                .map(|k| (p.t[k].start as usize, k));
        let bad_close_at = piece.bad_close.map(|bc| bc.at);
        let first = [first_err.map(|e| e.0), bad_close_at, piece.too_deep]
            .into_iter()
            .flatten()
            .min();
        if let Some(at) = first {
            if let Some((_, k)) = first_err.filter(|e| e.0 == at) {
                return Err(p.lex_error(k));
            }
            if piece.too_deep == Some(at) {
                return Err(syntax("too many nested parentheses").with_span(li.span(at, at + 1)));
            }
            if let Some(bc) = piece.bad_close {
                return Err(bad_close_error(li, bc));
            }
        }
        match piece.open {
            Some(Open::Bracket(c, at)) => {
                return Err(
                    syntax(format!("'{c}' was never closed")).with_span(li.span(at, at + 1))
                );
            }
            Some(Open::Backslash(at)) => {
                return Err(syntax("unexpected EOF while parsing").with_span(li.span(at, at + 1)));
            }
            // The lexer reported it above (an error token always exists for it).
            Some(Open::TripleString(at)) => {
                return Err(syntax("unterminated triple-quoted string literal")
                    .with_span(li.span(at, piece.end)));
            }
            None => {}
        }
    }
    let end = piece.end;
    let run = move |mut p: P<'_>| {
        let first = p.pos();
        p.statement()
            .map_err(|e| p.prefer_tokenizer_error(e))
            .map_err(|e| e.with_span(p.li.span(first, end.max(first))))
    };
    if nesting_estimate(&p.t) <= INLINE_NESTING {
        return run(p);
    }
    // Deeply nested input: recurse on a dedicated stack so that the caller's stack size (and
    // the build profile's frame sizes) never matter. MAX_NESTING bounds what is used.
    let fallback = p.li.span(p.pos(), end.max(p.pos()));
    std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .name("frap-lang-parse".into())
            .stack_size(DEEP_PARSE_STACK)
            .spawn_scoped(s, move || run(p));
        match handle {
            Ok(h) => h
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload)),
            Err(_) => Err(unsupported("frap: expression too complex to parse")
                .with_hint("split the value into simpler statements")
                .with_span(fallback)),
        }
    })
}

/// Parsing runs on the caller's stack while [`nesting_estimate`] stays at or below this.
/// The worst debug-build cost of one estimate unit is about 20 KiB (a `float(` level is
/// two units), so the caller needs well under 1 MiB.
const INLINE_NESTING: usize = 24;

/// Stack of the dedicated parse thread for deeper input: [`MAX_NESTING`] levels at the
/// worst debug-build frame cost (about 40 KiB per level) need about 10 MiB.
const DEEP_PARSE_STACK: usize = 64 * 1024 * 1024;

/// An upper bound on the parsers' nesting (calls to [`P::nest`] on one recursion path) for
/// a token list with balanced brackets: each bracket level costs at most two (`(` in a
/// short-form value enters `sitem` and `factor`), each unary sign and each `**` at most one,
/// plus the statement's top-level value.
fn nesting_estimate(t: &[Token]) -> usize {
    let (mut depth, mut max_depth, mut chain) = (0usize, 0usize, 0usize);
    let mut after_operand = false;
    for tok in t {
        after_operand = match &tok.tok {
            Tok::Punct('(' | '[' | '{') => {
                depth += 1;
                max_depth = max_depth.max(depth);
                false
            }
            Tok::Punct(')' | ']' | '}') => {
                depth = depth.saturating_sub(1);
                true
            }
            Tok::Op("**") => {
                chain += 1;
                false
            }
            Tok::Punct('+' | '-' | '~') => {
                if !after_operand {
                    chain += 1;
                }
                false
            }
            Tok::Ident(_) | Tok::Int(_) | Tok::Float(_) | Tok::Str(_) => true,
            _ => false,
        };
    }
    2 * max_depth + chain + 2
}

fn bad_close_error(li: &LineIndex<'_>, bc: BadClose) -> FrapError {
    let msg = match bc.open {
        None => format!("unmatched '{}'", bc.close),
        Some((o, oat)) => {
            let (ol, cl) = (li.line_of(oat), li.line_of(bc.at));
            if ol == cl {
                format!(
                    "closing parenthesis '{}' does not match opening parenthesis '{o}'",
                    bc.close
                )
            } else {
                format!(
                    "closing parenthesis '{}' does not match opening parenthesis '{o}' on line {ol}",
                    bc.close
                )
            }
        }
    };
    syntax(msg).with_span(li.span(bc.at, bc.at + 1))
}

/// Whitespace policy for paths: the bpy form ignores whitespace between tokens, the short form
/// requires `.`, `[` and `(` to touch the previous token (§11.4).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Ws {
    Free,
    Tight,
}

/// A parsed path with the byte offset where each segment ends (for `Call` target spans).
pub(super) struct PathOut {
    pub(super) expr: PathExpr,
    pub(super) root_end: usize,
    pub(super) seg_ends: Vec<usize>,
}

impl PathOut {
    pub(super) fn ends_in_call(&self) -> bool {
        matches!(self.expr.segs.last(), Some(PathSeg::Call(_)))
    }
}

pub(super) struct P<'a> {
    pub(super) src: &'a str,
    pub(super) t: Vec<Token>,
    pub(super) i: usize,
    pub(super) li: &'a LineIndex<'a>,
    /// Piece start (span of an empty statement).
    pub(super) start: usize,
    /// End of the last token.
    pub(super) eof: usize,
    /// Current expression nesting (see [`P::nest`]).
    pub(super) depth: u32,
    /// Set once the statement is known to be in the short form, whose `BARE` words may cover
    /// bytes that are not valid Python tokens (`Cube.001a`).
    pub(super) short: bool,
}

/// Maximum expression nesting the parsers recurse into (unary and `**` chains, brackets,
/// calls, displays). Brackets alone stop at [`crate::split::MAX_BRACKET_DEPTH`] with
/// CPython's error; this bounds the remaining recursion so that no input can overflow the
/// stack (a 2 MiB thread in a debug build included).
pub(super) const MAX_NESTING: u32 = 250;

impl<'a> P<'a> {
    // ------------------------------------------------------------------ token access

    pub(super) fn tok(&self) -> Option<&Token> {
        self.t.get(self.i)
    }
    pub(super) fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i).map(|t| &t.tok)
    }
    pub(super) fn peek_at(&self, k: usize) -> Option<&Token> {
        self.t.get(self.i + k)
    }
    pub(super) fn at_end(&self) -> bool {
        self.i >= self.t.len()
    }
    pub(super) fn bump(&mut self) -> Option<Token> {
        let t = self.t.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }
    pub(super) fn is_punct(&self, c: char) -> bool {
        matches!(self.peek(), Some(Tok::Punct(p)) if *p == c)
    }
    pub(super) fn is_op(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if *o == s)
    }
    pub(super) fn is_ident(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(x)) if x == s)
    }
    pub(super) fn ident_at(&self, k: usize) -> Option<&SmolStr> {
        match self.peek_at(k).map(|t| &t.tok) {
            Some(Tok::Ident(s)) => Some(s),
            _ => None,
        }
    }
    /// True when the current token exists and touches the previous one.
    pub(super) fn tight(&self) -> bool {
        self.tok().is_some_and(|t| !t.space_before)
    }
    /// True when the token `k` ahead exists and touches the one before it.
    pub(super) fn tight_at(&self, k: usize) -> bool {
        self.peek_at(k).is_some_and(|t| !t.space_before)
    }
    pub(super) fn pos(&self) -> usize {
        self.tok().map(|t| t.start as usize).unwrap_or(self.eof)
    }
    pub(super) fn prev_end(&self) -> usize {
        if self.i == 0 {
            return self.start;
        }
        self.t
            .get(self.i - 1)
            .map(|t| t.end as usize)
            .unwrap_or(self.eof)
    }
    pub(super) fn span_from(&self, start: usize) -> Span {
        self.li.span(start, self.prev_end().max(start))
    }
    pub(super) fn tok_span(&self, k: usize) -> Span {
        match self.t.get(k) {
            Some(t) => self.li.span(t.start as usize, t.end as usize),
            None => self.li.span(self.eof, self.eof),
        }
    }
    pub(super) fn here_span(&self) -> Span {
        self.tok_span(self.i)
    }

    // ------------------------------------------------------------------ errors

    /// The error for token `k`, which must be a [`Tok::Error`].
    pub(super) fn lex_error(&self, k: usize) -> FrapError {
        let Some(t) = self.t.get(k) else {
            return syntax("invalid syntax").with_span(self.tok_span(k));
        };
        let Tok::Error(le) = &t.tok else {
            return syntax("invalid syntax").with_span(self.tok_span(k));
        };
        // "detected at line N": the line where the literal stops (its last byte).
        let last = (t.end as usize).saturating_sub(1).max(t.start as usize);
        let detected = self.li.line_of(last);
        let msg = le.message(detected);
        let e = if le.is_unsupported() {
            unsupported(msg)
        } else {
            syntax(msg)
        };
        let at = (t.err_at as usize).clamp(t.start as usize, t.end as usize);
        e.with_span(self.li.span(at, t.end as usize))
    }

    /// `invalid syntax` at the current token (or its lexer error, or end of input).
    pub(super) fn bad(&self) -> FrapError {
        if matches!(self.peek(), Some(Tok::Error(_))) {
            return self.lex_error(self.i);
        }
        syntax("invalid syntax").with_span(self.here_span())
    }

    pub(super) fn err_here(&self, e: FrapError) -> FrapError {
        if matches!(self.peek(), Some(Tok::Error(_))) {
            return self.lex_error(self.i);
        }
        e.with_span(self.here_span())
    }

    /// Enters one level of expression nesting; fails past [`MAX_NESTING`] instead of
    /// recursing further. Pair with [`P::leave`].
    pub(super) fn nest(&mut self) -> R<()> {
        if self.depth >= MAX_NESTING {
            return Err(self.too_complex());
        }
        self.depth += 1;
        Ok(())
    }

    pub(super) fn leave(&mut self) {
        self.depth -= 1;
    }

    #[cold]
    #[inline(never)]
    fn too_complex(&self) -> FrapError {
        unsupported(format!(
            "frap: expression too complex to parse (more than {MAX_NESTING} nested levels)"
        ))
        .with_hint("split the value into simpler statements")
        .with_span(self.here_span())
    }

    /// CPython reports tokenizer errors before parser errors: when a bpy-form statement fails
    /// with frap's own error (`variables are not supported`, `invalid syntax`, ...), a real
    /// tokenizer error anywhere in it wins (`x.a = €`). Short-form statements are exempt
    /// because their `BARE` words may cover bytes that are not Python tokens.
    fn prefer_tokenizer_error(&self, e: FrapError) -> FrapError {
        if self.short {
            return e;
        }
        let tok_err = self
            .t
            .iter()
            .position(|t| matches!(&t.tok, Tok::Error(le) if !le.is_unsupported()));
        match tok_err {
            Some(k) => self.lex_error(k),
            None => e,
        }
    }

    pub(super) fn expect_punct(&mut self, c: char) -> R<()> {
        if self.is_punct(c) {
            self.bump();
            Ok(())
        } else {
            Err(self.bad())
        }
    }

    pub(super) fn expect_ident(&mut self) -> R<SmolStr> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.bump();
                Ok(s)
            }
            _ => Err(self.bad()),
        }
    }

    /// Requires the end of the statement; explains frap's limits when an operator follows.
    pub(super) fn expect_end(&mut self) -> R<()> {
        if self.at_end() {
            return Ok(());
        }
        if let Some(e) = self.unsupported_continuation() {
            return Err(self.err_here(e));
        }
        Err(self.bad())
    }

    /// True when the next token is an arithmetic binary operator (`path * 2`).
    fn continues_arithmetic(&self) -> bool {
        matches!(
            self.peek(),
            Some(Tok::Punct('+' | '-' | '*' | '/' | '@')) | Some(Tok::Op("**"))
        )
    }

    /// A bare expression statement (`bpy.data.objects['Cube'].location * 2`, `1 + 2`): valid
    /// Python, but frap only evaluates values inside a statement.
    #[cold]
    #[inline(never)]
    fn bare_expression_error(&self) -> FrapError {
        let text = self.src[self.start..self.eof].trim();
        unsupported(
            "frap: an expression is not a statement; read or assign a path, call an \
             operator or function, or print(...) the value",
        )
        .with_hint(format!("print({text})"))
    }

    // ------------------------------------------------------------------ statements

    fn statement(&mut self) -> R<Stmt> {
        let start = self.pos();
        let (cmd, form) = self.command()?;
        let span = self.span_from(start);
        Ok(Stmt {
            cmd,
            span,
            line: span.line,
            form,
        })
    }

    fn command(&mut self) -> R<(Command, Form)> {
        let first = match self.peek() {
            Some(Tok::Ident(s)) => s.clone(),
            Some(Tok::Error(_)) => return Err(self.bad()),
            Some(Tok::Op("...")) => return Err(self.err_here(ellipsis_error())),
            Some(
                Tok::Int(_)
                | Tok::Float(_)
                | Tok::Str(_)
                | Tok::Punct('(' | '[' | '{' | '-' | '+' | '~'),
            ) => return Err(self.err_here(self.bare_expression_error())),
            _ => return Err(self.err_here(form_error())),
        };
        let next_tight_dot_or_bracket = self.tight_at(1)
            && matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Punct('.' | '[')));
        match first.as_str() {
            "import" | "from" => self.import_stmt().map(|c| (c, Form::Bpy)),
            "pass" => {
                self.bump();
                self.expect_end()?;
                Ok((
                    Command::Noop {
                        reason: NoopReason::Pass,
                    },
                    Form::Bpy,
                ))
            }
            "print" => self.print_stmt().map(|c| (c, Form::Bpy)),
            "bpy" => self.bpy_stmt().map(|c| (c, Form::Bpy)),
            "D" | "C" => self.dc_stmt(),
            "data" | "context" if next_tight_dot_or_bracket => {
                self.short_path_stmt().map(|c| (c, Form::Short))
            }
            kw if STATEMENT_KEYWORDS.contains(&kw) => Err(self.err_here(
                unsupported(format!("frap: '{kw}' statements are not supported")).with_hint(
                    "frap runs single statements: no loops, conditionals, definitions or \
                         variables; repeat the statement instead of looping",
                ),
            )),
            _ if self.tight_at(1)
                && matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Punct('.')))
                && self.tight_at(2)
                && self.ident_at(2).is_some() =>
            {
                self.short_op().map(|c| (c, Form::Short))
            }
            "True" | "False" | "None" => Err(self.err_here(self.bare_expression_error())),
            "and" | "or" | "not" | "in" | "is" | "as" => Err(self.err_here(form_error())),
            name => {
                let next = self.peek_at(1).map(|t| t.tok.clone());
                match next {
                    Some(Tok::Punct('=' | ',' | ':' | '[' | '.'))
                    | Some(Tok::Op(
                        "+=" | "-=" | "*=" | "/=" | "**=" | "//=" | "@=" | "%=" | ":=" | "<<="
                        | ">>=" | "&=" | "|=" | "^=",
                    )) => Err(self.err_here(variables_error(name))),
                    // Valid Python that fails at run time in CPython: `x`, `cube_add()`.
                    None | Some(Tok::Punct('(')) if !PY_BUILTINS.contains(&name) => {
                        let mut e = FrapError::new(
                            PyExc::NameError,
                            NAME,
                            format!("name '{name}' is not defined"),
                        );
                        e.hint = form_error().hint;
                        e.detail.insert("name".into(), name.into());
                        Err(self.err_here(e))
                    }
                    Some(Tok::Punct('(')) if PY_BUILTINS.contains(&name) => Err(self.err_here(
                        unsupported(format!("frap: the built-in '{name}' is not supported"))
                            .with_hint(
                                "read a path to get its value (collections report len and \
                                 item names), or use print(...)",
                            ),
                    )),
                    _ => Err(self.err_here(form_error())),
                }
            }
        }
    }

    /// `import bpy`, `import math, mathutils`, `from mathutils import Vector, Matrix`.
    fn import_stmt(&mut self) -> R<Command> {
        const ALLOWED: &[&str] = &["bpy", "math", "mathutils", "bmesh"];
        let check_module = |p: &mut Self| -> R<()> {
            let at = p.i;
            let top = p.expect_ident()?;
            if !ALLOWED.contains(&top.as_str()) {
                return Err(unsupported(format!(
                    "frap: only bpy, bmesh, math and mathutils can be imported (got '{top}')"
                ))
                .with_span(p.tok_span(at)));
            }
            while p.is_punct('.') {
                p.bump();
                p.expect_ident()?;
            }
            if p.is_ident("as") {
                return Err(p.err_here(unsupported(
                    "frap: import aliases are not supported (they bind a variable)",
                )));
            }
            Ok(())
        };
        if self.is_ident("import") {
            self.bump();
            loop {
                check_module(self)?;
                if self.is_punct(',') {
                    self.bump();
                } else {
                    break;
                }
            }
        } else {
            self.bump(); // from
            if self.is_punct('.') {
                return Err(self.err_here(unsupported("frap: relative imports are not supported")));
            }
            check_module(self)?;
            if !self.is_ident("import") {
                return Err(self.bad());
            }
            self.bump();
            if self.is_punct('*') {
                self.bump();
            } else {
                let paren = self.is_punct('(');
                if paren {
                    self.bump();
                }
                loop {
                    self.expect_ident()?;
                    if self.is_ident("as") {
                        return Err(self.err_here(unsupported(
                            "frap: import aliases are not supported (they bind a variable)",
                        )));
                    }
                    if self.is_punct(',') {
                        self.bump();
                        if paren && self.is_punct(')') {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if paren {
                    self.expect_punct(')')?;
                }
            }
        }
        self.expect_end()?;
        Ok(Command::Noop {
            reason: NoopReason::Import,
        })
    }

    /// `print(a, b)`.
    fn print_stmt(&mut self) -> R<Command> {
        self.bump(); // print
        if !self.is_punct('(') {
            if self.at_end() {
                return Err(self.err_here(unsupported(
                    "frap: 'print' must be called: print(value, ...)",
                )));
            }
            return Err(self.err_here(syntax(
                "Missing parentheses in call to 'print'. Did you mean print(...)?",
            )));
        }
        self.bump();
        let args = self.call_args()?;
        if let Some(a) = args.iter().find(|a| a.name.is_some()) {
            return Err(
                unsupported("frap: print() keyword arguments are not supported").with_span(a.span),
            );
        }
        self.expect_end()?;
        Ok(Command::Print {
            values: args.into_iter().map(|a| a.value).collect(),
        })
    }

    /// Statements starting with `bpy`.
    fn bpy_stmt(&mut self) -> R<Command> {
        let is_ops = matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Punct('.')))
            && self.ident_at(2).is_some_and(|s| s == "ops");
        if is_ops {
            return self.ops_stmt();
        }
        let path = self.path(Ws::Free)?;
        self.bpy_tail(path)
    }

    /// `bpy.ops.ns.op(...)`, `bpy.ops.ns.op.poll()`, ...
    fn ops_stmt(&mut self) -> R<Command> {
        let start = self.pos();
        self.bump(); // bpy
        self.bump(); // .
        self.bump(); // ops
        let need_call = |p: &Self, what: &str| {
            unsupported(format!(
                "frap: {what} must be called, e.g. bpy.ops.mesh.primitive_cube_add(size=2)"
            ))
            .with_span(p.span_from(start))
        };
        if !self.is_punct('.') {
            return Err(if self.at_end() {
                need_call(self, "bpy.ops")
            } else {
                self.bad()
            });
        }
        self.bump();
        let ns = self.expect_ident()?;
        if !self.is_punct('.') {
            return Err(if self.at_end() {
                need_call(self, &format!("bpy.ops.{ns}"))
            } else {
                self.bad()
            });
        }
        self.bump();
        let op = self.expect_ident()?;
        let idname: SmolStr = format!("{ns}.{op}").into();
        if self.is_punct('.') {
            self.bump();
            let at = self.i;
            let name = self.expect_ident()?;
            let what = match name.as_str() {
                "poll" => OpMetaKind::Poll,
                "get_rna_type" => OpMetaKind::GetRnaType,
                "idname" => OpMetaKind::Idname,
                "idname_py" => OpMetaKind::IdnamePy,
                other => {
                    return Err(unsupported(format!(
                        "frap: bpy.ops.{idname}.{other} is not supported; use .poll(), \
                         .get_rna_type(), .idname() or .idname_py()"
                    ))
                    .with_span(self.tok_span(at)));
                }
            };
            if !self.is_punct('(') {
                return Err(self.err_here(unsupported(format!(
                    "frap: bpy.ops.{idname}.{name} must be called: bpy.ops.{idname}.{name}()"
                ))));
            }
            self.bump();
            let args = self.call_args()?;
            if let Some(a) = args.first() {
                return Err(unsupported(format!(
                    "frap: bpy.ops.{idname}.{name}() takes no arguments"
                ))
                .with_span(a.span));
            }
            self.expect_end()?;
            return Ok(Command::OpMeta { idname, what });
        }
        if !self.is_punct('(') {
            return Err(if self.at_end() {
                need_call(self, &format!("bpy.ops.{idname}"))
            } else {
                self.bad()
            });
        }
        self.bump();
        let all = self.call_args()?;
        self.expect_end()?;
        let (pos, kw): (Vec<Arg>, Vec<Arg>) = all.into_iter().partition(|a| a.name.is_none());
        Ok(Command::Invoke {
            idname,
            positional: pos.into_iter().map(|a| a.value).collect(),
            args: kw,
        })
    }

    /// After a path in the bpy form: read, assignment, augmented assignment or call.
    pub(super) fn bpy_tail(&mut self, path: PathOut) -> R<Command> {
        if self.at_end() {
            return Ok(self.get_or_call(path));
        }
        let op = match self.peek() {
            Some(Tok::Punct('=')) => Some(AssignOp::Assign),
            Some(Tok::Op("+=")) => Some(AssignOp::Add),
            Some(Tok::Op("-=")) => Some(AssignOp::Sub),
            Some(Tok::Op("*=")) => Some(AssignOp::Mul),
            Some(Tok::Op("/=")) => Some(AssignOp::Div),
            Some(Tok::Op(
                o @ ("**=" | "//=" | "@=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="),
            )) => {
                return Err(self.err_here(unsupported(format!(
                    "frap: augmented assignment '{o}' is not supported (use = += -= *= /=)"
                ))));
            }
            Some(Tok::Punct(',')) => {
                return Err(self.err_here(unsupported(
                    "frap: assigning to several targets is not supported; use one statement per target",
                )));
            }
            _ => None,
        };
        let Some(op) = op else {
            if self.continues_arithmetic() {
                return Err(self.err_here(self.bare_expression_error()));
            }
            return Err(self.expect_end().err().unwrap_or_else(|| self.bad()));
        };
        self.check_target(&path, op)?;
        self.bump();
        let value = self.expr_list()?;
        if self.is_punct('=') {
            return Err(self.err_here(unsupported(
                "frap: chained assignment is not supported; use one statement per target",
            )));
        }
        self.expect_end()?;
        Ok(Command::Set {
            target: path.expr,
            op,
            value,
        })
    }

    /// Validates an assignment target (CPython's messages for calls).
    pub(super) fn check_target(&self, path: &PathOut, op: AssignOp) -> R<()> {
        let span = path.expr.span;
        if path.ends_in_call() {
            return Err(if op == AssignOp::Assign {
                syntax("cannot assign to function call here. Maybe you meant '==' instead of '='?")
            } else {
                syntax("'function call' is an illegal expression for augmented assignment")
            }
            .with_span(span));
        }
        if path.expr.segs.is_empty() {
            let name = &self.src[span.start as usize..(span.start + span.len) as usize];
            return Err(variables_error(name).with_span(span));
        }
        Ok(())
    }

    /// A path statement with no assignment: a read, or an RNA function call when the path ends
    /// with a call (§11.2).
    pub(super) fn get_or_call(&self, mut path: PathOut) -> Command {
        if !path.ends_in_call() {
            return Command::Get { target: path.expr };
        }
        let Some(PathSeg::Call(args)) = path.expr.segs.pop() else {
            unreachable!("ends_in_call checked the last segment");
        };
        path.seg_ends.pop();
        let end = path.seg_ends.last().copied().unwrap_or(path.root_end);
        path.expr.span = self.li.span(path.expr.span.start as usize, end);
        Command::Call {
            target: path.expr,
            args,
        }
    }

    /// Statements starting with `D` or `C` (§11.4 rule 2): what follows the full path decides
    /// the form.
    fn dc_stmt(&mut self) -> R<(Command, Form)> {
        let mut path = self.path(Ws::Tight)?;
        if self.at_end() {
            return Ok((self.get_or_call(path), Form::Bpy));
        }
        if self.starts_short_value() {
            return self.short_call_tail(path).map(|c| (c, Form::Short));
        }
        // `D.objects ['Cube']` is a short call with a list value; anything else continues in
        // the bpy form (`D.objects['Cube'].select_set (True)`, `= ...`, `+= ...`).
        self.path_continue(&mut path, Ws::Free)?;
        self.bpy_tail(path).map(|c| (c, Form::Bpy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chunked code-point index agrees with counting from the line start, across chunk
    /// boundaries and multi-byte characters of every length.
    #[test]
    fn line_index_col_chars_matches_naive_count() {
        let mut src = String::new();
        for k in 0..400 {
            src.push_str(["a", "é", "€", "𝔘", "\n", "bc", "Würfel "][k % 7]);
        }
        let li = LineIndex::new(&src, 3);
        for (pos, _) in src.char_indices() {
            let ls = src[..pos].rfind('\n').map_or(0, |i| i + 1);
            let span = li.span(pos, pos);
            assert_eq!(span.col as usize, pos - ls + 1);
            assert_eq!(span.col_chars as usize, src[ls..pos].chars().count() + 1);
            assert_eq!(span.line as usize, 3 + src[..pos].matches('\n').count());
        }
        let ascii = "x = 1\ny = 2";
        let li = LineIndex::new(ascii, 1);
        assert!(li.chunk_chars.is_empty());
        assert_eq!(li.span(8, 9).col_chars, 3);
    }

    #[test]
    fn nesting_estimate_counts_brackets_signs_and_powers() {
        let est = |s: &str| nesting_estimate(&lex(s, 0, s.len()));
        assert_eq!(est("x = 1 - 2 - 3"), 2);
        assert_eq!(est("x = -1"), 3);
        assert_eq!(est("x = ((1))"), 6);
        assert_eq!(est("x = 2 ** -2 ** 2"), 5);
        assert_eq!(est("f(a)[0] - (b)"), 4);
    }
}
