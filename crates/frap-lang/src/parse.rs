// SPDX-License-Identifier: GPL-3.0-or-later
//! bpy-line and short-form parsers (ARCHITECTURE §11.3, §11.4). M0 subset: operator calls,
//! path reads and plain assignments, imports. Everything else is a clean `SyntaxError`.

use frap_rna::{FrapError, PyExc, Span};
use smol_str::SmolStr;

use crate::errors::{SYNTAX, UNSUPPORTED_SYNTAX};
use crate::ir::*;
use crate::lexer::{Tok, Token, lex};
use crate::split::split;

/// Maps byte offsets to line/column.
struct LineIndex<'a> {
    src: &'a str,
    starts: Vec<usize>,
    first_line: u32,
}

impl<'a> LineIndex<'a> {
    fn new(src: &'a str, first_line: u32) -> Self {
        let mut starts = vec![0];
        starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex {
            src,
            starts,
            first_line,
        }
    }
    fn span(&self, start: usize, end: usize) -> Span {
        let li = self.starts.partition_point(|s| *s <= start) - 1;
        let ls = self.starts[li];
        Span {
            start: start as u32,
            len: (end.saturating_sub(start)) as u32,
            line: self.first_line + li as u32,
            col: (start - ls + 1) as u32,
            col_chars: (self.src[ls..start].chars().count() + 1) as u32,
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
        .into_iter()
        .map(|piece| {
            let first_tok = first_non_space(src, piece.start, piece.end);
            if let Some((ch, at)) = piece.unclosed {
                return Err(FrapError::new(
                    PyExc::SyntaxError,
                    SYNTAX,
                    format!("'{ch}' was never closed"),
                )
                .with_span(li.span(at, at + 1)));
            }
            let toks = lex(src, piece.start, piece.end)
                .map_err(|(e, at)| e.with_span(li.span(at, at + 1)))?;
            let mut p = P {
                src,
                t: toks,
                i: 0,
                li: &li,
                end: piece.end,
            };
            p.statement().map_err(|e| {
                let at = first_tok;
                let sp = li.span(at, piece.end);
                e.with_span(sp)
            })
        })
        .collect()
}

fn first_non_space(src: &str, start: usize, end: usize) -> usize {
    let b = src.as_bytes();
    let mut i = start;
    while i < end && (b[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    i
}

struct P<'a> {
    src: &'a str,
    t: Vec<Token>,
    i: usize,
    li: &'a LineIndex<'a>,
    end: usize,
}

type R<T> = Result<T, FrapError>;

fn syntax(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::SyntaxError, SYNTAX, msg)
}
fn unsupported(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::SyntaxError, UNSUPPORTED_SYNTAX, msg)
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i).map(|t| &t.tok)
    }
    fn peek_at(&self, k: usize) -> Option<&Token> {
        self.t.get(self.i + k)
    }
    fn at_end(&self) -> bool {
        self.i >= self.t.len()
    }
    fn bump(&mut self) -> Option<Token> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn is_punct(&self, c: char) -> bool {
        matches!(self.peek(), Some(Tok::Punct(p)) if *p == c)
    }
    fn contiguous(&self) -> bool {
        self.t.get(self.i).is_some_and(|t| !t.space_before)
    }
    fn pos(&self) -> usize {
        self.t
            .get(self.i)
            .map(|t| t.start as usize)
            .unwrap_or(self.end)
    }
    fn prev_end(&self) -> usize {
        self.t
            .get(self.i.wrapping_sub(1))
            .map(|t| t.end as usize)
            .unwrap_or(self.end)
    }
    fn span(&self, start: usize) -> Span {
        self.li.span(start, self.prev_end())
    }
    fn err_here(&self, e: FrapError) -> FrapError {
        let at = self.pos();
        e.with_span(self.li.span(at, (at + 1).min(self.end.max(at + 1))))
    }
    fn expect_punct(&mut self, c: char) -> R<()> {
        if self.is_punct(c) {
            self.bump();
            Ok(())
        } else {
            Err(self.err_here(syntax("invalid syntax")))
        }
    }
    fn expect_ident(&mut self) -> R<SmolStr> {
        match self.bump() {
            Some(Token {
                tok: Tok::Ident(s), ..
            }) => Ok(s),
            _ => {
                self.i -= 1;
                Err(self.err_here(syntax("invalid syntax")))
            }
        }
    }
    fn expect_end(&mut self) -> R<()> {
        if self.at_end() {
            return Ok(());
        }
        let e = match self.peek() {
            Some(Tok::Punct('+' | '-' | '*' | '/' | '@')) | Some(Tok::Op2("**")) => {
                unsupported("frap M0: arithmetic expressions are not supported yet")
            }
            _ => syntax("invalid syntax"),
        };
        Err(self.err_here(e))
    }

    fn statement(&mut self) -> R<Stmt> {
        let start = self.pos();
        let (cmd, form) = self.command()?;
        let span = self.span(start);
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
            _ => return Err(self.err_here(form_error())),
        };
        match first.as_str() {
            "import" | "from" => {
                self.i = self.t.len();
                Ok((Command::Noop { reason: NoopReason::Import }, Form::Bpy))
            }
            "pass" if self.t.len() == 1 => {
                self.bump();
                Ok((Command::Noop { reason: NoopReason::Pass }, Form::Bpy))
            }
            "bpy" => self.bpy_statement().map(|c| (c, Form::Bpy)),
            "print" => Err(self.err_here(unsupported("frap M0: print() is not supported yet"))),
            "D" | "C" => {
                let path = self.path()?;
                self.bpy_path_tail(path).map(|c| (c, Form::Bpy))
            }
            "data" | "context"
                if matches!(self.peek_at(1), Some(t) if !t.space_before
                    && matches!(t.tok, Tok::Punct('.' | '['))) =>
            {
                let path = self.path()?;
                self.short_path_tail(path).map(|c| (c, Form::Short))
            }
            _ if matches!(self.peek_at(1), Some(t) if !t.space_before && t.tok == Tok::Punct('.'))
                && matches!(self.peek_at(2), Some(t) if !t.space_before && matches!(t.tok, Tok::Ident(_))) =>
            {
                self.short_op().map(|c| (c, Form::Short))
            }
            _ if matches!(self.peek_at(1), Some(t) if t.tok == Tok::Punct('=')) => Err(self.err_here(
                unsupported(format!(
                    "frap: variables are not supported; use a full path such as bpy.data.objects['Cube'] (got '{first}')"
                ))
                .with_hint("use a full path such as bpy.data.objects['Cube']"),
            )),
            _ => Err(self.err_here(form_error())),
        }
    }

    // ---------------------------------------------------------------- bpy-line form

    fn bpy_statement(&mut self) -> R<Command> {
        // bpy . (ops|data|context)
        if matches!(self.peek_at(2), Some(Token { tok: Tok::Ident(s), .. }) if s == "ops") {
            self.bump(); // bpy
            self.expect_punct('.')?;
            self.bump(); // ops
            self.expect_punct('.')?;
            let ns = self.expect_ident()?;
            self.expect_punct('.')?;
            let op = self.expect_ident()?;
            if self.is_punct('.') {
                return Err(self.err_here(unsupported(
                    "frap M0: operator metadata calls (.poll(), .idname(), ...) are not supported yet",
                )));
            }
            self.expect_punct('(')?;
            let (positional, args) = self.call_args()?;
            self.expect_end()?;
            return Ok(Command::Invoke {
                idname: format!("{ns}.{op}").into(),
                positional,
                args,
            });
        }
        let path = self.path()?;
        self.bpy_path_tail(path)
    }

    fn bpy_path_tail(&mut self, target: PathExpr) -> R<Command> {
        if self.at_end() {
            return Ok(Command::Get { target });
        }
        match self.peek() {
            Some(Tok::Punct('=')) => {
                self.bump();
                let value = self.expr()?;
                self.expect_end()?;
                Ok(Command::Set {
                    target,
                    op: AssignOp::Assign,
                    value,
                })
            }
            Some(Tok::Op2("+=" | "-=" | "*=" | "/=")) => Err(self.err_here(unsupported(
                "frap M0: augmented assignment is not supported yet",
            ))),
            Some(Tok::Punct('(')) => Err(self.err_here(unsupported(
                "frap M0: RNA function calls are not supported yet",
            ))),
            _ => {
                self.expect_end()?;
                Ok(Command::Get { target })
            }
        }
    }

    fn call_args(&mut self) -> R<(Vec<Expr>, Vec<Arg>)> {
        let mut positional = Vec::new();
        let mut args = Vec::new();
        loop {
            if self.is_punct(')') {
                self.bump();
                return Ok((positional, args));
            }
            let start = self.pos();
            let kw = matches!(self.peek(), Some(Tok::Ident(_)))
                && matches!(self.peek_at(1), Some(t) if t.tok == Tok::Punct('='));
            if kw {
                let name = self.expect_ident()?;
                self.bump(); // '='
                let value = self.expr()?;
                args.push(Arg {
                    name: Some(name),
                    value,
                    span: self.span(start),
                });
            } else {
                if !args.is_empty() {
                    return Err(
                        self.err_here(syntax("positional argument follows keyword argument"))
                    );
                }
                positional.push(self.expr()?);
            }
            if self.is_punct(',') {
                self.bump();
            } else if !self.is_punct(')') {
                return Err(self.err_here(syntax("invalid syntax. Perhaps you forgot a comma?")));
            }
        }
    }

    fn expr(&mut self) -> R<Expr> {
        let start = self.pos();
        let e = match self.peek().cloned() {
            Some(Tok::Punct(c @ ('-' | '+'))) => {
                self.bump();
                let inner = self.expr_atom()?;
                let op = if c == '-' { UnOp::Neg } else { UnOp::Pos };
                Expr::Unary(op, Box::new(inner), self.span(start))
            }
            _ => self.expr_atom()?,
        };
        Ok(e)
    }

    fn expr_atom(&mut self) -> R<Expr> {
        let start = self.pos();
        let Some(tok) = self.peek().cloned() else {
            return Err(self.err_here(syntax("invalid syntax")));
        };
        match tok {
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Lit(Lit::Int(v), self.span(start)))
            }
            Tok::Float(v) => {
                self.bump();
                Ok(Expr::Lit(Lit::Float(v), self.span(start)))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Lit(Lit::Str(s), self.span(start)))
            }
            Tok::Ident(id) => match id.as_str() {
                "True" | "False" => {
                    self.bump();
                    Ok(Expr::Lit(Lit::Bool(id == "True"), self.span(start)))
                }
                "None" => {
                    self.bump();
                    Ok(Expr::Lit(Lit::None, self.span(start)))
                }
                "bpy" | "D" | "C" => Ok(Expr::Path(self.path()?)),
                _ => Err(self.err_here(
                    unsupported(format!("frap M0: '{id}' is not supported in expressions"))
                        .with_hint("literals, tuples, lists and bpy.data / bpy.context paths are supported in M0"),
                )),
            },
            Tok::Punct('(') => {
                self.bump();
                if self.is_punct(')') {
                    self.bump();
                    return Ok(Expr::Tuple(vec![], self.span(start)));
                }
                let first = self.expr()?;
                if self.is_punct(')') {
                    self.bump();
                    return Ok(first); // parenthesized
                }
                let mut items = vec![first];
                while self.is_punct(',') {
                    self.bump();
                    if self.is_punct(')') {
                        break;
                    }
                    items.push(self.expr()?);
                }
                self.expect_punct(')')?;
                Ok(Expr::Tuple(items, self.span(start)))
            }
            Tok::Punct('[') => {
                self.bump();
                let mut items = Vec::new();
                while !self.is_punct(']') {
                    items.push(self.expr()?);
                    if self.is_punct(',') {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.expect_punct(']')?;
                Ok(Expr::List(items, self.span(start)))
            }
            Tok::Punct('{') => Err(self.err_here(unsupported("frap M0: sets and dicts are not supported yet"))),
            _ => Err(self.err_here(syntax("invalid syntax"))),
        }
    }

    /// Parses a path starting at `bpy.data` / `bpy.context` / `D` / `C` / `data` / `context`.
    fn path(&mut self) -> R<PathExpr> {
        let start = self.pos();
        let first = self.expect_ident()?;
        let root = match first.as_str() {
            "bpy" => {
                self.expect_punct('.')?;
                match self.expect_ident()?.as_str() {
                    "data" => PathRoot::BpyData,
                    "context" => PathRoot::BpyContext,
                    other => {
                        return Err(FrapError::new(
                            PyExc::AttributeError,
                            UNSUPPORTED_SYNTAX,
                            format!("module 'bpy' has no attribute '{other}'"),
                        ));
                    }
                }
            }
            "D" | "data" => PathRoot::BpyData,
            "C" | "context" => PathRoot::BpyContext,
            _ => return Err(syntax("invalid syntax")),
        };
        let mut segs = Vec::new();
        loop {
            if self.is_punct('.') && self.contiguous() {
                self.bump();
                segs.push(PathSeg::Attr(self.expect_ident()?));
            } else if self.is_punct('[') && self.contiguous() {
                self.bump();
                let neg = if self.is_punct('-') {
                    self.bump();
                    true
                } else {
                    false
                };
                match self.bump().map(|t| t.tok) {
                    Some(Tok::Str(s)) if !neg => segs.push(PathSeg::Key(s)),
                    Some(Tok::Int(i)) => segs.push(PathSeg::Index(if neg { -i } else { i })),
                    _ => {
                        self.i -= 1;
                        return Err(self.err_here(unsupported(
                            "frap M0: collection keys must be a string or integer literal",
                        )));
                    }
                }
                self.expect_punct(']')?;
            } else {
                break;
            }
        }
        Ok(PathExpr {
            root,
            segs,
            span: self.span(start),
        })
    }

    // ---------------------------------------------------------------- short form

    fn short_op(&mut self) -> R<Command> {
        let ns = self.expect_ident()?;
        self.bump(); // '.'
        let op = self.expect_ident()?;
        let mut args = Vec::new();
        while !self.at_end() {
            if self.contiguous() {
                return Err(self.err_here(syntax("invalid syntax")));
            }
            let start = self.pos();
            let mut key = String::from(self.expect_ident()?.as_str());
            while self.is_punct('.') && self.contiguous() {
                self.bump();
                key.push('.');
                key.push_str(&self.expect_ident()?);
            }
            if !(self.is_punct('=') && self.contiguous()) {
                return Err(self.err_here(
                    syntax("invalid syntax")
                        .with_hint("short-form operator arguments are key=value, e.g. size=2"),
                ));
            }
            self.bump();
            if !self.contiguous() {
                return Err(self.err_here(
                    syntax("invalid syntax").with_hint("no space after '=' in key=value"),
                ));
            }
            let value = self.svalue()?;
            args.push(Arg {
                name: Some(key.into()),
                value,
                span: self.span(start),
            });
        }
        Ok(Command::Invoke {
            idname: format!("{ns}.{op}").into(),
            positional: vec![],
            args,
        })
    }

    fn short_path_tail(&mut self, target: PathExpr) -> R<Command> {
        if self.at_end() {
            return Ok(Command::Get { target });
        }
        if self.is_punct('=') {
            self.bump();
            let value = self.svalue()?;
            self.expect_end()?;
            return Ok(Command::Set {
                target,
                op: AssignOp::Assign,
                value,
            });
        }
        Err(self.err_here(unsupported(
            "frap M0: RNA function calls are not supported yet",
        )))
    }

    /// `item (',' item)*` with no whitespace inside.
    fn svalue(&mut self) -> R<Expr> {
        let start = self.pos();
        let first = self.sitem()?;
        if !(self.is_punct(',') && self.contiguous()) {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.is_punct(',') && self.contiguous() {
            self.bump();
            if !self.contiguous() {
                return Err(self.err_here(
                    syntax("invalid syntax")
                        .with_hint("no spaces inside a short-form tuple: 0,0,1"),
                ));
            }
            items.push(self.sitem()?);
        }
        Ok(Expr::Tuple(items, self.span(start)))
    }

    fn sitem(&mut self) -> R<Expr> {
        let start = self.pos();
        let Some(tok) = self.peek().cloned() else {
            return Err(self.err_here(syntax("invalid syntax")));
        };
        match tok {
            Tok::Punct('(') | Tok::Punct('[') => self.expr_atom(),
            Tok::Punct('-' | '+') => self.expr(),
            Tok::Int(_) | Tok::Float(_) | Tok::Str(_) => self.expr_atom(),
            Tok::Punct('{') => {
                Err(self.err_here(unsupported("frap M0: sets are not supported yet")))
            }
            Tok::Ident(id) => {
                match id.as_str() {
                    "true" | "True" | "false" | "False" => {
                        self.bump();
                        return Ok(Expr::Lit(
                            Lit::Bool(id.eq_ignore_ascii_case("true")),
                            self.span(start),
                        ));
                    }
                    "none" | "None" => {
                        self.bump();
                        return Ok(Expr::Lit(Lit::None, self.span(start)));
                    }
                    "data" | "context" | "D" | "C"
                        if matches!(self.peek_at(1), Some(t) if !t.space_before
                            && matches!(t.tok, Tok::Punct('.' | '['))) =>
                    {
                        return Ok(Expr::Path(self.path()?));
                    }
                    _ => {}
                }
                // BARE := [A-Za-z_][A-Za-z0-9_.\-]*  (contiguous tokens, stops at ',')
                self.bump();
                while let Some(t) = self.t.get(self.i) {
                    let ok = !t.space_before
                        && matches!(
                            t.tok,
                            Tok::Ident(_) | Tok::Int(_) | Tok::Float(_) | Tok::Punct('.' | '-')
                        );
                    if !ok {
                        break;
                    }
                    self.i += 1;
                }
                let text = &self.src[start..self.prev_end()];
                Ok(Expr::Bare(text.into(), self.span(start)))
            }
            _ => Err(self.err_here(syntax("invalid syntax"))),
        }
    }
}

fn form_error() -> FrapError {
    syntax("invalid syntax").with_hint(
        "write either bpy.ops.mesh.primitive_cube_add(size=2) / bpy.data.objects['Cube'].location = (1, 2, 3), \
         or the short form mesh.primitive_cube_add size=2 / data.objects['Cube'].location = 1,2,3",
    )
}
