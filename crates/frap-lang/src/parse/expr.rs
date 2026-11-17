// SPDX-License-Identifier: GPL-3.0-or-later
//! bpy-form expressions, paths and call arguments (ARCHITECTURE §11.3).
//!
//! Grammar (Python precedence; `**` is right-associative and binds tighter than a unary sign
//! on its left, so `-2 ** 2` is `-(2 ** 2)`):
//!
//! ```text
//! expr_list := expr (',' expr)* ','?           # `x = 1, 2, 3` is a tuple, as in Python
//! expr      := sum
//! sum       := term (('+' | '-') term)*
//! term      := factor (('*' | '/' | '@') factor)*
//! factor    := ('+' | '-') factor | power
//! power     := primary ('**' factor)?
//! primary   := atom ('.' NAME '(' args ')')*   # method calls on values: .normalized()
//! atom      := literal | STRING+ | path | builtin | '(' … ')' | '[' … ']' | '{' … '}'
//! ```

use frap_rna::{FrapError, PyExc, codes};
use smol_str::SmolStr;

use super::{P, PY_BUILTINS, PathOut, R, Ws, syntax, unsupported};
use crate::errors::NAME;
use crate::ir::*;
use crate::lexer::Tok;

/// `math.<name>` functions frap evaluates.
const MATH_FUNCS: &[(&str, Builtin)] = &[
    ("radians", Builtin::Radians),
    ("degrees", Builtin::Degrees),
    ("sqrt", Builtin::Sqrt),
    ("sin", Builtin::Sin),
    ("cos", Builtin::Cos),
    ("tan", Builtin::Tan),
];

/// `mathutils` constructors.
const MATHUTILS_TYPES: &[(&str, Builtin)] = &[
    ("Vector", Builtin::Vector),
    ("Euler", Builtin::Euler),
    ("Quaternion", Builtin::Quaternion),
    ("Matrix", Builtin::Matrix),
    ("Color", Builtin::Color),
];

/// `Matrix.<name>` class methods.
const MATRIX_CTORS: &[(&str, Builtin)] = &[
    ("Translation", Builtin::MatrixTranslation),
    ("Rotation", Builtin::MatrixRotation),
    ("Scale", Builtin::MatrixScale),
    ("Identity", Builtin::MatrixIdentity),
];

fn lookup(table: &[(&str, Builtin)], name: &str) -> Option<Builtin> {
    table.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

impl<'a> P<'a> {
    // ------------------------------------------------------------------ paths

    /// Parses a path starting at `bpy.data` / `bpy.context` / `D` / `C` / `data` / `context`.
    pub(in crate::parse) fn path(&mut self, ws: Ws) -> R<PathOut> {
        let start = self.pos();
        let at = self.i;
        let first = self.expect_ident()?;
        let root = match first.as_str() {
            "bpy" => {
                if ws == Ws::Tight && !self.tight() {
                    return Err(self.bad());
                }
                self.expect_punct('.')?;
                if ws == Ws::Tight && !self.tight() {
                    return Err(self.bad());
                }
                let name_at = self.i;
                let name = self.expect_ident()?;
                match name.as_str() {
                    "data" => PathRoot::BpyData,
                    "context" => PathRoot::BpyContext,
                    "ops" => {
                        return Err(unsupported(
                            "frap: operator calls cannot be used inside expressions or as paths",
                        )
                        .with_hint("call the operator as its own statement")
                        .with_span(self.span_from(start)));
                    }
                    "app" | "types" | "utils" | "props" | "path" | "msgbus" => {
                        return Err(unsupported(format!("frap: bpy.{name} is not supported"))
                            .with_hint("frap exposes bpy.ops, bpy.data and bpy.context")
                            .with_span(self.tok_span(name_at)));
                    }
                    other => {
                        return Err(FrapError::new(
                            PyExc::AttributeError,
                            codes::RNA_NO_ATTRIBUTE,
                            format!("module 'bpy' has no attribute '{other}'"),
                        )
                        .with_span(self.tok_span(name_at)));
                    }
                }
            }
            "D" | "data" => PathRoot::BpyData,
            "C" | "context" => PathRoot::BpyContext,
            _ => return Err(syntax("invalid syntax").with_span(self.tok_span(at))),
        };
        let root_end = self.prev_end();
        let mut out = PathOut {
            expr: PathExpr {
                root,
                segs: Vec::new(),
                span: self.span_from(start),
            },
            root_end,
            seg_ends: Vec::new(),
        };
        self.path_continue(&mut out, ws)?;
        Ok(out)
    }

    /// Parses further `.attr`, `[key]` and `(args)` segments onto `out`.
    pub(in crate::parse) fn path_continue(&mut self, out: &mut PathOut, ws: Ws) -> R<()> {
        let start = out.expr.span.start as usize;
        loop {
            if ws == Ws::Tight && !self.tight() {
                break;
            }
            if self.is_punct('.') {
                self.bump();
                if ws == Ws::Tight && !self.tight() {
                    return Err(self.bad());
                }
                let name = self.expect_ident()?;
                out.expr.segs.push(PathSeg::Attr(name));
            } else if self.is_punct('[') {
                self.bump();
                let seg = self.subscript()?;
                out.expr.segs.push(seg);
            } else if self.is_punct('(') {
                self.bump();
                let args = self.call_args()?;
                out.expr.segs.push(PathSeg::Call(args));
            } else {
                break;
            }
            out.seg_ends.push(self.prev_end());
        }
        out.expr.span = self.span_from(start);
        Ok(())
    }

    /// After `[`: a string key, an integer index, an integer slice or a `(name, library)` key.
    fn subscript(&mut self) -> R<PathSeg> {
        let unsupported_key = |p: &Self| {
            p.err_here(unsupported(
                "frap: a subscript must be a string, an integer, an integer slice or a \
                 ('name', library) key literal",
            ))
        };
        let seg = if self.is_punct(':') || self.int_then_colon() {
            let a = self.opt_signed_int()?;
            self.expect_punct(':')?;
            let b = self.opt_signed_int()?;
            if self.is_punct(':') {
                return Err(self.err_here(unsupported("frap: slice steps are not supported")));
            }
            PathSeg::Slice(a, b)
        } else if matches!(self.peek(), Some(Tok::Str(_))) {
            let key = self.strings()?;
            if self.is_punct(',') {
                self.bump();
                let lib = match self.peek() {
                    Some(Tok::Str(_)) => Some(self.strings()?),
                    Some(Tok::Ident(n)) if n == "None" => {
                        self.bump();
                        None
                    }
                    _ => return Err(unsupported_key(self)),
                };
                if self.is_punct(',') {
                    return Err(unsupported_key(self));
                }
                PathSeg::KeyLib(key, lib)
            } else {
                PathSeg::Key(key)
            }
        } else if let Some(v) = self.opt_signed_int()? {
            PathSeg::Index(v)
        } else if matches!(self.peek(), Some(Tok::Error(_))) || self.is_punct(']') {
            return Err(self.bad());
        } else {
            return Err(unsupported_key(self));
        };
        if !self.is_punct(']') {
            if self.at_end() {
                return Err(self.bad());
            }
            return Err(if matches!(self.peek(), Some(Tok::Error(_))) {
                self.bad()
            } else {
                unsupported_key(self)
            });
        }
        self.bump();
        Ok(seg)
    }

    fn int_then_colon(&self) -> bool {
        let k = usize::from(matches!(self.peek(), Some(Tok::Punct('-' | '+'))));
        matches!(self.peek_at(k).map(|t| &t.tok), Some(Tok::Int(_)))
            && matches!(self.peek_at(k + 1).map(|t| &t.tok), Some(Tok::Punct(':')))
    }

    /// An optionally signed integer literal, if one is next.
    fn opt_signed_int(&mut self) -> R<Option<i64>> {
        let neg = match self.peek() {
            Some(Tok::Punct('-')) => true,
            Some(Tok::Punct('+')) => false,
            Some(Tok::Int(v)) => {
                let v = *v;
                self.bump();
                return Ok(Some(v));
            }
            _ => return Ok(None),
        };
        match self.peek_at(1).map(|t| &t.tok) {
            Some(Tok::Int(v)) => {
                let v = *v;
                self.bump();
                self.bump();
                Ok(Some(if neg { v.wrapping_neg() } else { v }))
            }
            _ => Ok(None),
        }
    }

    /// One or more adjacent string literals, joined (Python's implicit concatenation).
    fn strings(&mut self) -> R<String> {
        let mut s = String::new();
        while let Some(Tok::Str(part)) = self.peek() {
            s.push_str(part);
            self.bump();
        }
        Ok(s)
    }

    // ------------------------------------------------------------------ call arguments

    /// After `(`: positional and keyword arguments up to and including `)`. Positional
    /// arguments come first (`name: None`), as Python requires.
    pub(in crate::parse) fn call_args(&mut self) -> R<Vec<Arg>> {
        let mut args: Vec<Arg> = Vec::new();
        loop {
            if self.is_punct(')') {
                self.bump();
                return Ok(args);
            }
            if self.is_punct('*') || self.is_op("**") {
                return Err(self.err_here(unsupported(
                    "frap: argument unpacking (*args, **kwargs) is not supported",
                )));
            }
            let start = self.pos();
            let is_kw = self.ident_at(0).is_some()
                && matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Punct('=')));
            if is_kw {
                let name_at = self.i;
                let name = self.expect_ident()?;
                self.bump(); // '='
                if self.is_punct(',') || self.is_punct(')') || self.at_end() {
                    return Err(syntax("expected argument value expression")
                        .with_span(self.span_from(start)));
                }
                if args
                    .iter()
                    .any(|a| a.name.as_deref() == Some(name.as_str()))
                {
                    return Err(syntax(format!("keyword argument repeated: {name}"))
                        .with_span(self.tok_span(name_at)));
                }
                let value = self.expr()?;
                args.push(Arg {
                    name: Some(name),
                    value,
                    span: self.span_from(start),
                });
            } else {
                if self.is_punct('=') || self.is_punct(',') {
                    return Err(self.bad());
                }
                let value = self.expr()?;
                if args.iter().any(|a| a.name.is_some()) {
                    return Err(syntax("positional argument follows keyword argument")
                        .with_span(self.span_from(start)));
                }
                args.push(Arg {
                    name: None,
                    value,
                    span: self.span_from(start),
                });
            }
            if self.is_punct(',') {
                self.bump();
            } else if !self.is_punct(')') {
                return Err(self.after_item_error());
            }
        }
    }

    /// The error when an item in a bracketed list is followed by something unexpected.
    fn after_item_error(&self) -> FrapError {
        if let Some(e) = self.unsupported_continuation() {
            return self.err_here(e);
        }
        if self.starts_expr() {
            return self.err_here(syntax("invalid syntax. Perhaps you forgot a comma?"));
        }
        self.bad()
    }

    fn starts_expr(&self) -> bool {
        match self.peek() {
            Some(Tok::Int(_) | Tok::Float(_) | Tok::Str(_)) => true,
            Some(Tok::Punct('(' | '[' | '{' | '-' | '+')) => true,
            Some(Tok::Ident(s)) => !matches!(
                s.as_str(),
                "if" | "else" | "and" | "or" | "not" | "in" | "is" | "for"
            ),
            _ => false,
        }
    }

    /// When the next token continues an expression with Python syntax frap does not support,
    /// the matching `UNSUPPORTED_SYNTAX` error.
    pub(in crate::parse) fn unsupported_continuation(&self) -> Option<FrapError> {
        let msg = match self.peek()? {
            Tok::Op("==" | "!=" | "<=" | ">=") | Tok::Punct('<' | '>') => {
                "frap: comparisons are not supported"
            }
            Tok::Op("//") | Tok::Punct('%') => {
                "frap: the '//' and '%' operators are not supported (use /)"
            }
            Tok::Op("<<" | ">>") => "frap: shift operators are not supported",
            Tok::Punct('&' | '|' | '^' | '~') => "frap: bitwise operators are not supported",
            Tok::Op(":=") => {
                return Some(super::variables_error(":="));
            }
            Tok::Ident(s) => match s.as_str() {
                "and" | "or" | "not" | "in" | "is" => {
                    "frap: boolean operators and membership tests are not supported"
                }
                "if" | "else" => "frap: conditional expressions are not supported",
                "for" | "async" => {
                    "frap: comprehensions and generator expressions are not supported"
                }
                _ => return None,
            },
            _ => return None,
        };
        Some(unsupported(msg))
    }

    // ------------------------------------------------------------------ expressions

    /// `expr (',' expr)* ','?`: a bare comma list is a tuple (`x = 1, 2, 3`).
    pub(in crate::parse) fn expr_list(&mut self) -> R<Expr> {
        let start = self.pos();
        let first = self.expr()?;
        if !self.is_punct(',') {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.is_punct(',') {
            self.bump();
            if self.at_end() || self.is_punct('=') {
                break;
            }
            items.push(self.expr()?);
        }
        Ok(Expr::Tuple(items, self.span_from(start)))
    }

    /// One expression (no bare comma tuple).
    pub(in crate::parse) fn expr(&mut self) -> R<Expr> {
        let e = self.sum()?;
        if let Some(err) = self.unsupported_continuation() {
            return Err(self.err_here(err));
        }
        Ok(e)
    }

    fn sum(&mut self) -> R<Expr> {
        let start = self.pos();
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Punct('+')) => BinOp::Add,
                Some(Tok::Punct('-')) => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.term()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs), self.span_from(start));
        }
    }

    fn term(&mut self) -> R<Expr> {
        let start = self.pos();
        let mut lhs = self.factor()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Punct('*')) => BinOp::Mul,
                Some(Tok::Punct('/')) => BinOp::Div,
                Some(Tok::Punct('@')) => BinOp::MatMul,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.factor()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs), self.span_from(start));
        }
    }

    /// Every recursive cycle of the expression grammar (brackets, calls, unary signs, `**`)
    /// goes through here, so this is where nesting is bounded.
    fn factor(&mut self) -> R<Expr> {
        self.nest()?;
        let r = self.factor_inner();
        self.leave();
        r
    }

    fn factor_inner(&mut self) -> R<Expr> {
        let start = self.pos();
        let op = match self.peek() {
            Some(Tok::Punct('-')) => UnOp::Neg,
            Some(Tok::Punct('+')) => UnOp::Pos,
            Some(Tok::Punct('~')) => {
                return Err(self.err_here(unsupported("frap: bitwise operators are not supported")));
            }
            Some(Tok::Ident(n)) if n == "not" => {
                return Err(self.err_here(unsupported(
                    "frap: boolean operators and membership tests are not supported",
                )));
            }
            _ => return self.power(),
        };
        self.bump();
        let inner = self.factor()?;
        Ok(Expr::Unary(op, Box::new(inner), self.span_from(start)))
    }

    fn power(&mut self) -> R<Expr> {
        let start = self.pos();
        let base = self.primary()?;
        if self.is_op("**") {
            self.bump();
            let exp = self.factor()?;
            return Ok(Expr::Binary(
                BinOp::Pow,
                Box::new(base),
                Box::new(exp),
                self.span_from(start),
            ));
        }
        Ok(base)
    }

    /// An atom followed by method calls (`Vector((1, 0, 0)).normalized()`). Paths take their
    /// own trailers (`.attr`, `[key]`, `(args)`) in [`P::path_continue`].
    pub(in crate::parse) fn primary(&mut self) -> R<Expr> {
        let start = self.pos();
        let mut e = self.atom()?;
        if matches!(e, Expr::Path(_)) {
            return Ok(e);
        }
        loop {
            if self.is_punct('.') {
                self.bump();
                let name_at = self.i;
                let name = self.expect_ident()?;
                if !self.is_punct('(') {
                    return Err(unsupported(format!(
                        "frap: reading the attribute '{name}' of a computed value is not \
                         supported (only method calls such as .normalized())"
                    ))
                    .with_span(self.tok_span(name_at)));
                }
                self.bump();
                let args = self.call_args()?;
                e = Expr::Method {
                    recv: Box::new(e),
                    name,
                    args,
                    span: self.span_from(start),
                };
            } else if self.is_punct('[') {
                return Err(self.err_here(unsupported(
                    "frap: subscripting a computed value is not supported (only paths)",
                )));
            } else if self.is_punct('(') {
                return Err(self.err_here(unsupported(
                    "frap: calling a computed value is not supported",
                )));
            } else {
                return Ok(e);
            }
        }
    }

    fn atom(&mut self) -> R<Expr> {
        let start = self.pos();
        let Some(tok) = self.peek().cloned() else {
            return Err(self.bad());
        };
        match tok {
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Lit(Lit::Int(v), self.span_from(start)))
            }
            Tok::Float(v) => {
                self.bump();
                Ok(Expr::Lit(Lit::Float(v), self.span_from(start)))
            }
            Tok::Str(_) => {
                let s = self.strings()?;
                Ok(Expr::Lit(Lit::Str(s), self.span_from(start)))
            }
            Tok::Ident(id) => self.name_atom(id),
            Tok::Punct('(') => self.paren(),
            Tok::Punct('[') => {
                self.bump();
                let items = self.items(']')?;
                Ok(Expr::List(items, self.span_from(start)))
            }
            Tok::Punct('{') => self.brace(),
            Tok::Punct('*') => {
                Err(self.err_here(unsupported("frap: unpacking (*value) is not supported")))
            }
            Tok::Op("...") => Err(self.err_here(super::ellipsis_error())),
            _ => Err(self.bad()),
        }
    }

    /// Comma-separated expressions up to and including `close` (trailing comma allowed).
    fn items(&mut self, close: char) -> R<Vec<Expr>> {
        let mut items = Vec::new();
        loop {
            if self.is_punct(close) {
                self.bump();
                return Ok(items);
            }
            if self.is_punct('*') || self.is_op("**") {
                return Err(self.err_here(unsupported("frap: unpacking (*value) is not supported")));
            }
            items.push(self.expr()?);
            if self.is_punct(',') {
                self.bump();
            } else if !self.is_punct(close) {
                return Err(self.after_item_error());
            }
        }
    }

    /// `()`, `(expr)`, `(a,)`, `(a, b)`.
    fn paren(&mut self) -> R<Expr> {
        let start = self.pos();
        self.bump(); // (
        if self.is_punct(')') {
            self.bump();
            return Ok(Expr::Tuple(vec![], self.span_from(start)));
        }
        let first = self.expr()?;
        if self.is_punct(')') {
            self.bump();
            return Ok(first);
        }
        if !self.is_punct(',') {
            return Err(self.after_item_error());
        }
        self.bump();
        let mut items = vec![first];
        items.extend(self.items(')')?);
        Ok(Expr::Tuple(items, self.span_from(start)))
    }

    /// `{}` (empty dict), `{k: v, ...}` or `{a, b}`.
    fn brace(&mut self) -> R<Expr> {
        let start = self.pos();
        self.bump(); // {
        if self.is_punct('}') {
            self.bump();
            return Ok(Expr::Dict(vec![], self.span_from(start)));
        }
        if self.is_op("**") {
            return Err(self.err_here(unsupported("frap: unpacking (**value) is not supported")));
        }
        let first = self.expr()?;
        if !self.is_punct(':') {
            let mut items = vec![first];
            if self.is_punct(',') {
                self.bump();
                items.extend(self.items('}')?);
            } else {
                if !self.is_punct('}') {
                    return Err(self.after_item_error());
                }
                self.bump();
            }
            return Ok(Expr::Set(items, self.span_from(start)));
        }
        let mut entries = Vec::new();
        let mut key = first;
        loop {
            self.expect_punct(':')?;
            let value = self.expr()?;
            entries.push((key, value));
            if self.is_punct(',') {
                self.bump();
            } else if !self.is_punct('}') {
                return Err(self.after_item_error());
            }
            if self.is_punct('}') {
                self.bump();
                return Ok(Expr::Dict(entries, self.span_from(start)));
            }
            if self.is_op("**") {
                return Err(
                    self.err_here(unsupported("frap: unpacking (**value) is not supported"))
                );
            }
            key = self.expr()?;
        }
    }

    /// Atoms that start with a name: literals, paths, built-ins.
    fn name_atom(&mut self, id: SmolStr) -> R<Expr> {
        let start = self.pos();
        let at = self.i;
        let next_is = |p: &Self, c: char| matches!(p.peek_at(1).map(|t| &t.tok), Some(Tok::Punct(x)) if *x == c);
        match id.as_str() {
            "True" | "False" => {
                self.bump();
                Ok(Expr::Lit(Lit::Bool(id == "True"), self.span_from(start)))
            }
            "None" => {
                self.bump();
                Ok(Expr::Lit(Lit::None, self.span_from(start)))
            }
            "bpy" | "D" | "C" => Ok(Expr::Path(self.path(Ws::Free)?.expr)),
            "data" | "context" if next_is(self, '.') || next_is(self, '[') => {
                Ok(Expr::Path(self.path(Ws::Free)?.expr))
            }
            "math" => {
                self.bump();
                self.expect_punct('.')?;
                let name_at = self.i;
                let name = self.expect_ident()?;
                if name == "pi" {
                    return Ok(Expr::Builtin {
                        name: Builtin::Pi,
                        args: vec![],
                        span: self.span_from(start),
                    });
                }
                match lookup(MATH_FUNCS, &name) {
                    Some(b) => self.builtin_call(b, start, &format!("math.{name}")),
                    None => Err(unsupported(format!(
                        "frap: math.{name} is not supported (supported: pi, radians, degrees, \
                         sqrt, sin, cos, tan)"
                    ))
                    .with_span(self.tok_span(name_at))),
                }
            }
            "mathutils" => {
                self.bump();
                self.expect_punct('.')?;
                let name_at = self.i;
                let name = self.expect_ident()?;
                match lookup(MATHUTILS_TYPES, &name) {
                    Some(b) => self.mathutils_ctor(b, &name, start),
                    None => Err(unsupported(format!(
                        "frap: mathutils.{name} is not supported (supported: Vector, Euler, \
                         Quaternion, Matrix, Color)"
                    ))
                    .with_span(self.tok_span(name_at))),
                }
            }
            "pi" => {
                self.bump();
                Ok(Expr::Builtin {
                    name: Builtin::Pi,
                    args: vec![],
                    span: self.span_from(start),
                })
            }
            "float" | "int" | "set" => {
                self.bump();
                let b = match id.as_str() {
                    "float" => Builtin::Float,
                    "int" => Builtin::Int,
                    _ => Builtin::Set,
                };
                self.builtin_call(b, start, &id)
            }
            name if lookup(MATH_FUNCS, name).is_some() => {
                self.bump();
                let b = lookup(MATH_FUNCS, name).unwrap_or(Builtin::Radians);
                self.builtin_call(b, start, name)
            }
            name if lookup(MATHUTILS_TYPES, name).is_some() => {
                self.bump();
                let b = lookup(MATHUTILS_TYPES, name).unwrap_or(Builtin::Vector);
                self.mathutils_ctor(b, name, start)
            }
            "lambda" | "await" | "yield" => {
                Err(self.err_here(unsupported(format!("frap: '{id}' is not supported"))))
            }
            "bmesh" => Err(self.err_here(unsupported(
                "frap: the bmesh module is not supported; use bpy.ops.mesh and frap.* operators",
            ))),
            "print" => Err(self.err_here(unsupported("frap: print() must be its own statement"))),
            name if PY_BUILTINS.contains(&name) => Err(self.err_here(unsupported(format!(
                "frap: the built-in '{name}' is not supported"
            )))),
            // `(y := 1)`: an assignment expression binds a variable.
            name if matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Op(":="))) => {
                Err(super::variables_error(name).with_span(self.tok_span(at)))
            }
            name => {
                // `[o.name for o in ...]`: the comprehension, not the name, is the problem.
                if let Some(k) = self.t[self.i..]
                    .iter()
                    .position(|t| matches!(&t.tok, Tok::Ident(f) if f == "for"))
                {
                    return Err(unsupported(
                        "frap: comprehensions and generator expressions are not supported",
                    )
                    .with_span(self.tok_span(self.i + k)));
                }
                let mut e = FrapError::new(
                    PyExc::NameError,
                    NAME,
                    format!("name '{name}' is not defined"),
                )
                .with_hint(match name {
                    "true" | "false" | "none" => {
                        "Python spells these True, False and None (the short form accepts both)"
                    }
                    _ => {
                        "frap has no variables: quote strings ('Cube') and use full paths such \
                         as bpy.data.objects['Cube']"
                    }
                })
                .with_span(self.tok_span(at));
                e.detail.insert("name".into(), name.into());
                Err(e)
            }
        }
    }

    /// After `Vector` / `Matrix` / ...: a constructor call, or `Matrix.Translation(...)`.
    fn mathutils_ctor(&mut self, b: Builtin, name: &str, start: usize) -> R<Expr> {
        if b == Builtin::Matrix && self.is_punct('.') {
            self.bump();
            let m_at = self.i;
            let m = self.expect_ident()?;
            return match lookup(MATRIX_CTORS, &m) {
                Some(mb) => self.builtin_call(mb, start, &format!("Matrix.{m}")),
                None => Err(unsupported(format!(
                    "frap: Matrix.{m} is not supported (supported: Translation, Rotation, \
                     Scale, Identity)"
                ))
                .with_span(self.tok_span(m_at))),
            };
        }
        self.builtin_call(b, start, name)
    }

    /// `(args)` after a built-in name.
    fn builtin_call(&mut self, name: Builtin, start: usize, shown: &str) -> R<Expr> {
        if !self.is_punct('(') {
            return Err(self.err_here(unsupported(format!(
                "frap: {shown} must be called, e.g. {shown}(...)"
            ))));
        }
        self.bump();
        let args = self.call_args()?;
        Ok(Expr::Builtin {
            name,
            args,
            span: self.span_from(start),
        })
    }
}
