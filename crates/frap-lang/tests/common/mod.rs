// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared test helpers: a compact, deterministic IR dump for `insta` snapshots, and span
//! invariants checked over every statement and error.

#![allow(dead_code)]

use std::fmt::Write;

use frap_lang::*;
use frap_rna::{FrapError, Span};

/// `@line:col/col_chars+len`
pub fn sp(s: &Span) -> String {
    format!("@{}:{}/{}+{}", s.line, s.col, s.col_chars, s.len)
}

/// Renders the result of `parse_complete(src)` (source, then one block per statement).
pub fn render(src: &str) -> String {
    let mut o = String::new();
    for l in src.lines() {
        let _ = writeln!(o, "> {l}");
    }
    let _ = writeln!(o, "---");
    for (k, r) in parse_complete(src).iter().enumerate() {
        match r {
            Ok(s) => {
                let _ = writeln!(
                    o,
                    "stmt {} {:?} line {} {}",
                    k + 1,
                    s.form,
                    s.line,
                    sp(&s.span)
                );
                cmd(&mut o, &s.cmd, 1);
            }
            Err(e) => error(&mut o, k + 1, e),
        }
    }
    o
}

pub fn error(o: &mut String, k: usize, e: &FrapError) {
    let span = e.span.as_ref().map(sp).unwrap_or_else(|| "NO SPAN".into());
    let _ = writeln!(o, "error {k} {} {} {span}", e.exception.as_str(), e.code.0);
    let _ = writeln!(o, "  message: {}", e.message);
    if let Some(h) = &e.hint {
        let _ = writeln!(o, "  hint: {h}");
    }
    if !e.detail.is_empty() {
        let kv: Vec<String> = e.detail.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let _ = writeln!(o, "  detail: {}", kv.join(" "));
    }
}

fn ind(o: &mut String, d: usize) {
    for _ in 0..d {
        o.push_str("  ");
    }
}

pub fn cmd(o: &mut String, c: &Command, d: usize) {
    ind(o, d);
    match c {
        Command::Invoke {
            idname,
            positional,
            args,
        } => {
            let _ = writeln!(o, "Invoke {idname}");
            for p in positional {
                ind(o, d + 1);
                let _ = writeln!(o, "positional");
                expr(o, p, d + 2);
            }
            for a in args {
                arg(o, a, d + 1);
            }
        }
        Command::OpMeta { idname, what } => {
            let _ = writeln!(o, "OpMeta {idname} {what:?}");
        }
        Command::Get { target } => {
            let _ = writeln!(o, "Get");
            path(o, target, d + 1);
        }
        Command::Set { target, op, value } => {
            let _ = writeln!(o, "Set {op:?}");
            path(o, target, d + 1);
            expr(o, value, d + 1);
        }
        Command::Call { target, args } => {
            let _ = writeln!(o, "Call");
            path(o, target, d + 1);
            for a in args {
                arg(o, a, d + 1);
            }
        }
        Command::Print { values } => {
            let _ = writeln!(o, "Print");
            for v in values {
                expr(o, v, d + 1);
            }
        }
        Command::Noop { reason } => {
            let _ = writeln!(o, "Noop {reason:?}");
        }
        other => {
            let _ = writeln!(o, "{other:?}");
        }
    }
}

pub fn arg(o: &mut String, a: &Arg, d: usize) {
    ind(o, d);
    match &a.name {
        Some(n) => {
            let _ = writeln!(o, "arg {n} {}", sp(&a.span));
        }
        None => {
            let _ = writeln!(o, "arg (positional) {}", sp(&a.span));
        }
    }
    expr(o, &a.value, d + 1);
}

pub fn path(o: &mut String, p: &PathExpr, d: usize) {
    ind(o, d);
    let root = match p.root {
        PathRoot::BpyData => "bpy.data",
        PathRoot::BpyContext => "bpy.context",
    };
    let _ = writeln!(o, "Path {root} {}", sp(&p.span));
    for s in &p.segs {
        ind(o, d + 1);
        match s {
            PathSeg::Attr(a) => {
                let _ = writeln!(o, ".{a}");
            }
            PathSeg::Key(k) => {
                let _ = writeln!(o, "[{k:?}]");
            }
            PathSeg::KeyLib(k, lib) => {
                let _ = writeln!(o, "[{k:?}, {lib:?}]");
            }
            PathSeg::Index(i) => {
                let _ = writeln!(o, "[{i}]");
            }
            PathSeg::Slice(a, b) => {
                let _ = writeln!(o, "[{a:?}:{b:?}]");
            }
            PathSeg::Call(args) => {
                let _ = writeln!(o, "(call)");
                for a in args {
                    arg(o, a, d + 2);
                }
            }
            other => {
                let _ = writeln!(o, "{other:?}");
            }
        }
    }
}

pub fn expr(o: &mut String, e: &Expr, d: usize) {
    ind(o, d);
    match e {
        Expr::Lit(l, s) => {
            let t = match l {
                Lit::Int(v) => format!("Int {v}"),
                Lit::Float(v) => format!("Float {v:?}"),
                Lit::Str(v) => format!("Str {v:?}"),
                Lit::Bool(v) => format!("Bool {v}"),
                Lit::None => "None".into(),
            };
            let _ = writeln!(o, "{t} {}", sp(s));
        }
        Expr::Bare(b, s) => {
            let _ = writeln!(o, "Bare {b} {}", sp(s));
        }
        Expr::Path(p) => path(o, p, d),
        Expr::Tuple(v, s) | Expr::List(v, s) | Expr::Set(v, s) => {
            let k = match e {
                Expr::Tuple(..) => "Tuple",
                Expr::List(..) => "List",
                _ => "Set",
            };
            let _ = writeln!(o, "{k} {}", sp(s));
            for x in v {
                expr(o, x, d + 1);
            }
        }
        Expr::Dict(v, s) => {
            let _ = writeln!(o, "Dict {}", sp(s));
            for (k, x) in v {
                ind(o, d + 1);
                let _ = writeln!(o, "entry");
                expr(o, k, d + 2);
                expr(o, x, d + 2);
            }
        }
        Expr::Unary(op, x, s) => {
            let _ = writeln!(o, "{op:?} {}", sp(s));
            expr(o, x, d + 1);
        }
        Expr::Binary(op, a, b, s) => {
            let _ = writeln!(o, "{op:?} {}", sp(s));
            expr(o, a, d + 1);
            expr(o, b, d + 1);
        }
        Expr::Builtin { name, args, span } => {
            let _ = writeln!(o, "Builtin {name:?} {}", sp(span));
            for a in args {
                arg(o, a, d + 1);
            }
        }
        Expr::Method {
            recv,
            name,
            args,
            span,
        } => {
            let _ = writeln!(o, "Method {name} {}", sp(span));
            expr(o, recv, d + 1);
            for a in args {
                arg(o, a, d + 1);
            }
        }
        other => {
            let _ = writeln!(o, "{other:?}");
        }
    }
}

// ---------------------------------------------------------------------- span invariants

/// Checks that `s` is a valid span of `src`: in bounds, on char boundaries, and with `line`,
/// `col` (bytes) and `col_chars` (code points) consistent with `start` (first line 1).
pub fn check_span(src: &str, s: &Span, what: &str) {
    let (start, len) = (s.start as usize, s.len as usize);
    assert!(
        start <= src.len(),
        "{what}: start {start} > len {} in {src:?}",
        src.len()
    );
    assert!(
        start + len <= src.len(),
        "{what}: end {} > len {} in {src:?}",
        start + len,
        src.len()
    );
    assert!(
        src.is_char_boundary(start),
        "{what}: start not a char boundary in {src:?}"
    );
    let line_start = src[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = src[..start].matches('\n').count() as u32 + 1;
    assert_eq!(s.line, line, "{what}: line in {src:?}");
    assert_eq!(
        s.col as usize,
        start - line_start + 1,
        "{what}: col in {src:?}"
    );
    assert_eq!(
        s.col_chars as usize,
        src[line_start..start].chars().count() + 1,
        "{what}: col_chars in {src:?}"
    );
}

fn check_path(src: &str, p: &PathExpr) {
    check_span(src, &p.span, "path");
    for seg in &p.segs {
        if let PathSeg::Call(args) = seg {
            args.iter().for_each(|a| check_arg(src, a));
        }
    }
}

fn check_arg(src: &str, a: &Arg) {
    check_span(src, &a.span, "arg");
    check_expr(src, &a.value);
}

fn check_expr(src: &str, e: &Expr) {
    match e {
        Expr::Lit(_, s) | Expr::Bare(_, s) => check_span(src, s, "expr"),
        Expr::Path(p) => check_path(src, p),
        Expr::Tuple(v, s) | Expr::List(v, s) | Expr::Set(v, s) => {
            check_span(src, s, "seq");
            v.iter().for_each(|x| check_expr(src, x));
        }
        Expr::Dict(v, s) => {
            check_span(src, s, "dict");
            for (k, x) in v {
                check_expr(src, k);
                check_expr(src, x);
            }
        }
        Expr::Unary(_, x, s) => {
            check_span(src, s, "unary");
            check_expr(src, x);
        }
        Expr::Binary(_, a, b, s) => {
            check_span(src, s, "binary");
            check_expr(src, a);
            check_expr(src, b);
        }
        Expr::Builtin { args, span, .. } => {
            check_span(src, span, "builtin");
            args.iter().for_each(|a| check_arg(src, a));
        }
        Expr::Method {
            recv, args, span, ..
        } => {
            check_span(src, span, "method");
            check_expr(src, recv);
            args.iter().for_each(|a| check_arg(src, a));
        }
        _ => {}
    }
}

/// Parses `src` and checks every span of every statement and error.
pub fn check_all_spans(src: &str) {
    for r in parse_complete(src) {
        match r {
            Ok(s) => {
                check_span(src, &s.span, "stmt");
                assert_eq!(s.line, s.span.line);
                match &s.cmd {
                    Command::Invoke {
                        positional, args, ..
                    } => {
                        positional.iter().for_each(|x| check_expr(src, x));
                        args.iter().for_each(|a| check_arg(src, a));
                    }
                    Command::Get { target } => check_path(src, target),
                    Command::Set { target, value, .. } => {
                        check_path(src, target);
                        check_expr(src, value);
                    }
                    Command::Call { target, args } => {
                        check_path(src, target);
                        args.iter().for_each(|a| check_arg(src, a));
                    }
                    Command::Print { values } => values.iter().for_each(|x| check_expr(src, x)),
                    _ => {}
                }
            }
            Err(e) => {
                let s = e
                    .span
                    .unwrap_or_else(|| panic!("error without span for {src:?}: {e}"));
                check_span(src, &s, "error");
            }
        }
    }
}

/// Maps `Bare(x)` to `Lit::Str(x)`: the two coerce to the same value (§11.5).
pub fn bare_to_str(e: &mut Expr) {
    match e {
        Expr::Bare(b, s) => *e = Expr::Lit(Lit::Str(b.to_string()), *s),
        Expr::Tuple(v, _) | Expr::List(v, _) | Expr::Set(v, _) => {
            v.iter_mut().for_each(bare_to_str)
        }
        Expr::Dict(v, _) => v.iter_mut().for_each(|(k, x)| {
            bare_to_str(k);
            bare_to_str(x);
        }),
        Expr::Unary(_, x, _) => bare_to_str(x),
        _ => {}
    }
}

/// Erased `Command` of the only statement in `src` (panics on errors).
pub fn ir(src: &str) -> (Command, Form) {
    let mut v = parse_complete(src);
    assert_eq!(v.len(), 1, "{src}");
    let s = v
        .remove(0)
        .unwrap_or_else(|e| panic!("{src}: {e} {:?} {:?}", e.hint, e.span));
    let mut c = s.cmd;
    c.erase_spans();
    (c, s.form)
}

/// The only statement's error.
pub fn err(src: &str) -> FrapError {
    let mut v = parse_complete(src);
    assert_eq!(v.len(), 1, "{src}");
    match v.remove(0) {
        Ok(s) => panic!("{src}: expected an error, got {:?}", s.cmd),
        Err(e) => e,
    }
}
