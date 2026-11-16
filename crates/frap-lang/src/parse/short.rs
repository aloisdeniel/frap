// SPDX-License-Identifier: GPL-3.0-or-later
//! Short form (ARCHITECTURE §11.4).
//!
//! ```text
//! op_call  := NS '.' OPNAME (WS kwarg)*
//! kwarg    := key '=' svalue                 # no spaces around '='
//! key      := IDENT ('.' IDENT)?             # TRANSFORM_OT_translate.value=0,0,1 (macro sub-op)
//! path_stmt:= spath | spath WS? ASSIGN WS? svalue | spath (WS (kwarg | svalue))+
//!           | spath WS? '(' args ')'             # a call, also with a space before '('
//! svalue   := item (',' item)*               # no spaces: 0,0,1
//! item     := NUMBER | true|false|True|False|none|None | QUOTED | '{' item (',' item)* '}'
//!           | spath | BARE | '(' bpy-expr ')' | '[' bpy-expr ']'
//!           | BUILTIN '(' args ')'          # radians(90), Vector((0,0,1)) (frap convenience)
//! BARE     := [A-Za-z_][A-Za-z0-9_.\-]*      # scanned from the source text, not from tokens
//! ```
//!
//! Typed values lower to exactly the IR of the bpy form (`2` → `Lit::Int`, `'x'` → `Lit::Str`,
//! `-1.5` → `Unary(Neg, Lit::Float)`, `0,0,1` → `Tuple`); dotted macro keys are merged into one
//! `Dict` argument, as `TRANSFORM_OT_translate={"value": (0, 0, 1)}` is in the bpy form.

use smol_str::SmolStr;

use super::{P, PathOut, R, Ws, syntax, variables_error};
use crate::ir::*;
use crate::lexer::{Tok, is_ident_continue};

/// Words that, immediately followed by `(`, start a bpy expression instead of a `BARE`.
const SHORT_BUILTIN_CALLS: &[&str] = &[
    "radians",
    "degrees",
    "sqrt",
    "sin",
    "cos",
    "tan",
    "float",
    "int",
    "set",
    "Vector",
    "Euler",
    "Quaternion",
    "Matrix",
    "Color",
    "Matrix.Translation",
    "Matrix.Rotation",
    "Matrix.Scale",
    "Matrix.Identity",
    "math.radians",
    "math.degrees",
    "math.sqrt",
    "math.sin",
    "math.cos",
    "math.tan",
    "mathutils.Vector",
    "mathutils.Euler",
    "mathutils.Quaternion",
    "mathutils.Matrix",
    "mathutils.Color",
    "mathutils.Matrix.Translation",
    "mathutils.Matrix.Rotation",
    "mathutils.Matrix.Scale",
    "mathutils.Matrix.Identity",
];

impl<'a> P<'a> {
    /// `ns.op key=value ...`
    pub(in crate::parse) fn short_op(&mut self) -> R<Command> {
        let ns_at = self.i;
        let ns = self.expect_ident()?;
        self.bump(); // '.'
        let op = self.expect_ident()?;
        let idname: SmolStr = format!("{ns}.{op}").into();
        // Things that look like Python on a name: explain instead of "invalid syntax".
        match self.peek() {
            Some(Tok::Punct('(')) if self.tight() => {
                return Err(self.err_here(syntax("invalid syntax").with_hint(format!(
                    "call an operator as bpy.ops.{idname}(key=value, ...) or, in the short \
                     form, {idname} key=value ...; an RNA function needs a full path such as \
                     bpy.data.objects['Cube'].select_set(True)"
                ))));
            }
            Some(Tok::Punct('.')) if self.tight() => {
                let meta = self.ident_at(1).is_some_and(|s| {
                    matches!(s.as_str(), "poll" | "get_rna_type" | "idname" | "idname_py")
                });
                if meta {
                    return Err(self.err_here(syntax("invalid syntax").with_hint(format!(
                        "operator metadata uses the bpy form: bpy.ops.{idname}.poll()"
                    ))));
                }
                return Err(variables_error(&ns).with_span(self.tok_span(ns_at)));
            }
            Some(Tok::Punct('[')) if self.tight() => {
                return Err(variables_error(&ns).with_span(self.tok_span(ns_at)));
            }
            Some(
                Tok::Punct('=')
                | Tok::Op(
                    "+=" | "-=" | "*=" | "/=" | "**=" | "//=" | "@=" | "%=" | "<<=" | ">>=" | "&="
                    | "|=" | "^=",
                ),
            ) => {
                return Err(variables_error(&ns).with_span(self.tok_span(ns_at)));
            }
            _ => {}
        }
        // From here on `BARE` values may cover bytes that are not Python tokens.
        self.short = true;
        let mut args: Vec<Arg> = Vec::new();
        // Indices of arguments built from dotted keys (merged into one Dict).
        let mut dotted: Vec<usize> = Vec::new();
        while !self.at_end() {
            if self.tight() {
                return Err(self.bad());
            }
            let start = self.pos();
            if self.ident_at(0).is_none() {
                return Err(self.err_here(
                    syntax("invalid syntax")
                        .with_hint("short-form operator arguments are key=value, e.g. size=2"),
                ));
            }
            let key = self.expect_ident()?;
            let mut sub: Option<(SmolStr, usize)> = None;
            if self.is_punct('.') && self.tight() {
                self.bump();
                if !self.tight() {
                    return Err(self.bad());
                }
                let sub_at = self.pos();
                sub = Some((self.expect_ident()?, sub_at));
                if self.is_punct('.') && self.tight() {
                    return Err(self.err_here(syntax("invalid syntax").with_hint(
                        "a short-form key has at most one dot: SUBOP_OT_name.prop=value",
                    )));
                }
            }
            let key_end = self.prev_end();
            if !(self.is_punct('=') && self.tight()) {
                let hint = if self.is_punct('=') {
                    "no space before '=' in key=value"
                } else {
                    "short-form operator arguments are key=value, e.g. size=2"
                };
                return Err(self.err_here(syntax("invalid syntax").with_hint(hint)));
            }
            self.bump();
            if !self.tight() {
                return Err(self.err_here(
                    syntax("expected argument value expression")
                        .with_hint("no space after '=' in key=value"),
                ));
            }
            let value = self.svalue()?;
            self.value_end()?;
            let span = self.span_from(start);
            let repeated = |name: &str| {
                syntax(format!("keyword argument repeated: {name}"))
                    .with_span(self.li.span(start, key_end))
            };
            match sub {
                None => {
                    if args.iter().any(|a| a.name.as_deref() == Some(key.as_str())) {
                        return Err(repeated(&key));
                    }
                    args.push(Arg {
                        name: Some(key),
                        value,
                        span,
                    });
                }
                Some((prop, prop_at)) => {
                    let prop_key = Expr::Lit(
                        Lit::Str(prop.to_string()),
                        self.li.span(prop_at, prop_at + prop.len()),
                    );
                    let existing = args
                        .iter()
                        .position(|a| a.name.as_deref() == Some(key.as_str()));
                    match existing {
                        Some(k) if dotted.contains(&k) => {
                            let arg = &mut args[k];
                            let Expr::Dict(entries, dspan) = &mut arg.value else {
                                unreachable!("dotted arguments are dicts");
                            };
                            let dup = entries
                                .iter()
                                .any(|(k, _)| matches!(k, Expr::Lit(Lit::Str(s), _) if *s == prop));
                            if dup {
                                return Err(repeated(&format!("{key}.{prop}")));
                            }
                            entries.push((prop_key, value));
                            let first = arg.span.start as usize;
                            arg.span = self.li.span(first, span.start as usize + span.len as usize);
                            *dspan = arg.span;
                        }
                        Some(_) => return Err(repeated(&key)),
                        None => {
                            dotted.push(args.len());
                            args.push(Arg {
                                name: Some(key),
                                value: Expr::Dict(vec![(prop_key, value)], span),
                                span,
                            });
                        }
                    }
                }
            }
        }
        Ok(Command::Invoke {
            idname,
            positional: vec![],
            args,
        })
    }

    /// A value must be followed by whitespace or the end of the statement.
    fn value_end(&self) -> R<()> {
        if self.at_end() || !self.tight() {
            return Ok(());
        }
        Err(self.err_here(syntax("invalid syntax").with_hint(
            "quote values that contain other characters, e.g. filepath='out/part.glb'; \
             tuples have no spaces: 0,0,1",
        )))
    }

    /// `data.…` / `context.…` statements.
    pub(in crate::parse) fn short_path_stmt(&mut self) -> R<Command> {
        self.short = true;
        let mut path = self.path(Ws::Tight)?;
        if self.at_end() {
            return Ok(self.get_or_call(path));
        }
        let op = match self.peek() {
            Some(Tok::Punct('=')) => Some(AssignOp::Assign),
            Some(Tok::Op("+=")) => Some(AssignOp::Add),
            Some(Tok::Op("-=")) => Some(AssignOp::Sub),
            Some(Tok::Op("*=")) => Some(AssignOp::Mul),
            Some(Tok::Op("/=")) => Some(AssignOp::Div),
            _ => None,
        };
        if let Some(op) = op {
            self.check_target(&path, op)?;
            self.bump();
            if self.at_end() {
                return Err(self.bad());
            }
            let value = self.svalue()?;
            if !self.at_end() {
                self.value_end()?;
                return Err(self.err_here(syntax("invalid syntax").with_hint(
                    "a short-form value has no spaces (0,0,1); use (…) for a full expression",
                )));
            }
            return Ok(Command::Set {
                target: path.expr,
                op,
                value,
            });
        }
        if self.starts_short_value() {
            return self.short_call_tail(path);
        }
        if self.is_punct('(') {
            // `data.objects['Cube'].select_set (True)`: a call with whitespace before its
            // parentheses, as `D.…` / `C.…` and Python accept.
            self.path_continue(&mut path, Ws::Free)?;
            return self.bpy_tail(path);
        }
        Err(self.bad())
    }

    /// True when the current token is preceded by whitespace and starts a short-form value or
    /// `key=value` (an RNA function call in the short form, §11.4).
    pub(in crate::parse) fn starts_short_value(&self) -> bool {
        let Some(t) = self.tok() else {
            return false;
        };
        if !t.space_before {
            return false;
        }
        match &t.tok {
            Tok::Int(_) | Tok::Float(_) | Tok::Str(_) | Tok::Ident(_) | Tok::Error(_) => true,
            Tok::Punct('[' | '{') => true,
            Tok::Punct('-' | '+') => {
                self.tight_at(1)
                    && matches!(
                        self.peek_at(1).map(|t| &t.tok),
                        Some(Tok::Int(_) | Tok::Float(_) | Tok::Punct('('))
                    )
            }
            _ => false,
        }
    }

    /// `spath (WS (kwarg | svalue))+`: an RNA function call with whitespace-separated
    /// arguments (`data.objects['Cube'].select_set True`).
    pub(in crate::parse) fn short_call_tail(&mut self, path: PathOut) -> R<Command> {
        self.short = true;
        if path.ends_in_call() {
            return Err(self.bad());
        }
        let mut args: Vec<Arg> = Vec::new();
        while !self.at_end() {
            if self.tight() {
                return Err(self.bad());
            }
            let start = self.pos();
            let is_kw = self.ident_at(0).is_some()
                && self.tight_at(1)
                && matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Punct('=')));
            if is_kw {
                let name_at = self.i;
                let name = self.expect_ident()?;
                self.bump(); // '='
                if !self.tight() {
                    return Err(self.err_here(
                        syntax("expected argument value expression")
                            .with_hint("no space after '=' in key=value"),
                    ));
                }
                if args
                    .iter()
                    .any(|a| a.name.as_deref() == Some(name.as_str()))
                {
                    return Err(syntax(format!("keyword argument repeated: {name}"))
                        .with_span(self.tok_span(name_at)));
                }
                let value = self.svalue()?;
                self.value_end()?;
                args.push(Arg {
                    name: Some(name),
                    value,
                    span: self.span_from(start),
                });
            } else {
                let value = self.svalue()?;
                self.value_end()?;
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
        }
        Ok(Command::Call {
            target: path.expr,
            args,
        })
    }

    /// `item (',' item)*` with no whitespace inside.
    pub(in crate::parse) fn svalue(&mut self) -> R<Expr> {
        let start = self.pos();
        let first = self.sitem()?;
        if !(self.is_punct(',') && self.tight()) {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.is_punct(',') && self.tight() {
            self.bump();
            if !self.tight() {
                return Err(self.err_here(
                    syntax("invalid syntax")
                        .with_hint("no spaces inside a short-form tuple: 0,0,1"),
                ));
            }
            items.push(self.sitem()?);
        }
        Ok(Expr::Tuple(items, self.span_from(start)))
    }

    /// Short-form item; its recursion (signs, `{…}`, `(…)`) is bounded like the bpy form's.
    fn sitem(&mut self) -> R<Expr> {
        self.nest()?;
        let r = self.sitem_inner();
        self.leave();
        r
    }

    fn sitem_inner(&mut self) -> R<Expr> {
        let start = self.pos();
        let Some(tok) = self.peek().cloned() else {
            return Err(self.bad());
        };
        match tok {
            Tok::Punct('(' | '[') => self.primary(),
            Tok::Punct(c @ ('-' | '+')) => {
                self.bump();
                if !self.tight() {
                    return Err(self.bad());
                }
                let inner = match self.peek() {
                    Some(Tok::Int(_) | Tok::Float(_) | Tok::Punct('(')) => self.sitem()?,
                    Some(Tok::Punct('-' | '+')) => self.sitem()?,
                    _ => return Err(self.bad()),
                };
                let op = if c == '-' { UnOp::Neg } else { UnOp::Pos };
                Ok(Expr::Unary(op, Box::new(inner), self.span_from(start)))
            }
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Lit(Lit::Int(v), self.span_from(start)))
            }
            Tok::Float(v) => {
                self.bump();
                Ok(Expr::Lit(Lit::Float(v), self.span_from(start)))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Lit(Lit::Str(s), self.span_from(start)))
            }
            Tok::Punct('{') => self.sset(),
            Tok::Ident(id) => {
                match id.as_str() {
                    "true" | "True" | "false" | "False" => {
                        self.bump();
                        return Ok(Expr::Lit(
                            Lit::Bool(id.eq_ignore_ascii_case("true")),
                            self.span_from(start),
                        ));
                    }
                    "none" | "None" => {
                        self.bump();
                        return Ok(Expr::Lit(Lit::None, self.span_from(start)));
                    }
                    "data" | "context" | "D" | "C"
                        if self.tight_at(1)
                            && matches!(
                                self.peek_at(1).map(|t| &t.tok),
                                Some(Tok::Punct('.' | '['))
                            ) =>
                    {
                        return Ok(Expr::Path(self.path(Ws::Tight)?.expr));
                    }
                    _ => {}
                }
                // `radians(90)`, `Vector((0, 0, 1))`: a built-in call is a bpy expression (the
                // ANGLE_LIKELY_DEGREES hint suggests `rotation=0,0,radians(90)`, §13.2).
                let end = self.bare_end(start);
                if self.src[end..self.eof].starts_with('(')
                    && SHORT_BUILTIN_CALLS.contains(&&self.src[start..end])
                {
                    return self.primary();
                }
                self.bare()
            }
            _ => Err(self.bad()),
        }
    }

    /// `{item, item}` (whitespace allowed inside the braces); `{}` is the empty set.
    fn sset(&mut self) -> R<Expr> {
        let start = self.pos();
        self.bump(); // {
        let mut items = Vec::new();
        loop {
            if self.is_punct('}') {
                self.bump();
                return Ok(Expr::Set(items, self.span_from(start)));
            }
            items.push(self.sitem()?);
            if self.is_punct(',') {
                self.bump();
            } else if !self.is_punct('}') {
                return Err(self
                    .err_here(syntax("invalid syntax").with_hint("a short-form set is {A,B,C}")));
            }
        }
    }

    /// End byte offset of the `BARE` word starting at `start`.
    fn bare_end(&self, start: usize) -> usize {
        let mut end = start;
        for c in self.src[start..self.eof].chars() {
            if is_ident_continue(c) || c == '.' || c == '-' {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        end
    }

    /// `BARE := [A-Za-z_][A-Za-z0-9_.\-]*` scanned from the source text (Unicode letters and
    /// digits allowed, as in Blender names); skips the tokens it covers.
    fn bare(&mut self) -> R<Expr> {
        let start = self.pos();
        let end = self.bare_end(start);
        while let Some(t) = self.tok() {
            if (t.end as usize) <= end {
                self.i += 1;
            } else if (t.start as usize) < end {
                // A token straddles the end of the word (`a-1e+5`).
                return Err(self.bad());
            } else {
                break;
            }
        }
        let text = &self.src[start..end];
        Ok(Expr::Bare(text.into(), self.li.span(start, end)))
    }
}
