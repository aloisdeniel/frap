// SPDX-License-Identifier: GPL-3.0-or-later
//! Semantic IR (ARCHITECTURE §11.2, CONTRACT frozen at M1 W0; M1 W0 added the variants the M0
//! parser did not produce yet; WP 1.3 added `PathSeg::KeyLib`). Knows nothing about operators or
//! RNA.

use frap_rna::Span;
use smol_str::SmolStr;

/// Which syntax a statement was written in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Form {
    /// `bpy.ops.mesh.primitive_cube_add(size=2)`
    Bpy,
    /// `mesh.primitive_cube_add size=2`
    Short,
}

/// One statement.
#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    /// What to do.
    pub cmd: Command,
    /// Whole-statement span.
    pub span: Span,
    /// 1-based source line of the first token.
    pub line: u32,
    /// Syntax used.
    pub form: Form,
}

/// What a statement does.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// `bpy.ops.ns.op(...)` | `ns.op k=v`
    Invoke {
        /// "mesh.primitive_cube_add"
        idname: SmolStr,
        /// Positional arguments (only execution-context strings are accepted, §10.2).
        positional: Vec<Expr>,
        /// Keyword arguments, in source order.
        args: Vec<Arg>,
    },
    /// `bpy.ops.ns.op.poll()` | `.get_rna_type()` | `.idname()` | `.idname_py()`
    OpMeta {
        /// `ns.op`
        idname: SmolStr,
        /// Which query.
        what: OpMetaKind,
    },
    /// Bare path: read.
    Get {
        /// Path to read.
        target: PathExpr,
    },
    /// `path = value`
    Set {
        /// Path to write.
        target: PathExpr,
        /// Assignment operator.
        op: AssignOp,
        /// Value.
        value: Expr,
    },
    /// A path ending in an RNA function call (`objects['Cube'].select_set(True)`).
    Call {
        /// The function path (last segment is the function name).
        target: PathExpr,
        /// Arguments.
        args: Vec<Arg>,
    },
    /// `print(a, b)`
    Print {
        /// Printed expressions.
        values: Vec<Expr>,
    },
    /// `import bpy`, `from mathutils import Vector`, `pass`.
    Noop {
        /// Why nothing happens.
        reason: NoopReason,
    },
}

/// Assignment operators.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    Add,
    /// `-=`
    Sub,
    /// `*=`
    Mul,
    /// `/=`
    Div,
}

/// Operator metadata queries (§8, §11.2).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpMetaKind {
    /// `.poll()` → `False`/`True` with `detail.unmet`
    Poll,
    /// `.get_rna_type()` → the operator's describe fragment
    GetRnaType,
    /// `.idname()` → `"MESH_OT_bevel"`
    Idname,
    /// `.idname_py()` → `"mesh.bevel"`
    IdnamePy,
}

/// Binary operators of the expression subset (§11.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `@`
    MatMul,
    /// `**`
    Pow,
}

/// Built-in callables and constants (`mathutils`, `math`, `float`, `int`; §11.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Builtin {
    /// `Vector(...)`
    Vector,
    /// `Euler(...)`
    Euler,
    /// `Quaternion(...)`
    Quaternion,
    /// `Matrix(...)`
    Matrix,
    /// `Matrix.Translation(...)`
    MatrixTranslation,
    /// `Matrix.Rotation(...)`
    MatrixRotation,
    /// `Matrix.Scale(...)`
    MatrixScale,
    /// `Matrix.Identity(...)`
    MatrixIdentity,
    /// `Color(...)`
    Color,
    /// `math.pi` (no arguments)
    Pi,
    /// `math.radians` / `radians`
    Radians,
    /// `math.degrees` / `degrees`
    Degrees,
    /// `math.sqrt`
    Sqrt,
    /// `math.sin`
    Sin,
    /// `math.cos`
    Cos,
    /// `math.tan`
    Tan,
    /// `float(...)` (incl. `float('inf')`)
    Float,
    /// `int(...)`
    Int,
    /// `set()`
    Set,
}

/// Reasons for a no-op statement.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NoopReason {
    /// `import ...` / `from ... import ...`
    Import,
    /// `pass`
    Pass,
    /// Blank line or comment only.
    Blank,
}

/// A call argument.
#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    /// Keyword, if any.
    pub name: Option<SmolStr>,
    /// Value expression.
    pub value: Expr,
    /// Span of the whole argument (`size=2`).
    pub span: Span,
}

/// Literal values.
#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    /// Integer.
    Int(i64),
    /// Float.
    Float(f64),
    /// String.
    Str(String),
    /// Bool.
    Bool(bool),
    /// None.
    None,
}

/// Unary operators.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnOp {
    /// `-x`
    Neg,
    /// `+x`
    Pos,
}

/// Expressions (§11.2).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Expr {
    /// Literal.
    Lit(Lit, Span),
    /// Short form only: unquoted word (enum id, name).
    Bare(SmolStr, Span),
    /// A path read.
    Path(PathExpr),
    /// `(a, b)`
    Tuple(Vec<Expr>, Span),
    /// `[a, b]`
    List(Vec<Expr>, Span),
    /// `{a, b}`
    Set(Vec<Expr>, Span),
    /// `{"k": v}` (macro sub-props)
    Dict(Vec<(Expr, Expr)>, Span),
    /// `-x`
    Unary(UnOp, Box<Expr>, Span),
    /// `a + b`, `a @ b`, …
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    /// `Vector(...)`, `math.radians(...)`, `math.pi`, `float('inf')`
    Builtin {
        /// Which built-in.
        name: Builtin,
        /// Arguments (empty for constants).
        args: Vec<Arg>,
        /// Span.
        span: Span,
    },
    /// `Vector(...).normalized()`
    Method {
        /// Receiver.
        recv: Box<Expr>,
        /// Method name.
        name: SmolStr,
        /// Arguments.
        args: Vec<Arg>,
        /// Span.
        span: Span,
    },
}

/// `bpy.data.objects['Cube'].location`
#[derive(Clone, Debug, PartialEq)]
pub struct PathExpr {
    /// Root.
    pub root: PathRoot,
    /// Segments.
    pub segs: Vec<PathSeg>,
    /// Span.
    pub span: Span,
}

/// Path roots.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PathRoot {
    /// `bpy.data` | `D` | `data`
    BpyData,
    /// `bpy.context` | `C` | `context`
    BpyContext,
}

/// Path segments.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PathSeg {
    /// `.name`
    Attr(SmolStr),
    /// `['Name']`
    Key(String),
    /// `['Name', 'lib.blend']` / `['Name', None]`: a `(name, library filepath)` tuple key
    /// (§5.2; added by WP 1.3, additive on a `#[non_exhaustive]` enum, rule 2.2.4).
    KeyLib(String, Option<String>),
    /// `[3]`
    Index(i64),
    /// `[a:b]`
    Slice(Option<i64>, Option<i64>),
    /// `(args)`: a call (nested calls are evaluated innermost first, §11.2)
    Call(Vec<Arg>),
}

impl PathExpr {
    /// Source-like text of the path (`bpy.data.objects['Cube'].location`), used in messages.
    pub fn display(&self) -> String {
        let mut s = match self.root {
            PathRoot::BpyData => "bpy.data".to_owned(),
            PathRoot::BpyContext => "bpy.context".to_owned(),
        };
        for seg in &self.segs {
            match seg {
                PathSeg::Attr(a) => {
                    s.push('.');
                    s.push_str(a);
                }
                PathSeg::Key(k) => s.push_str(&format!("[{}]", frap_rna::ptr::py_str_repr(k))),
                PathSeg::KeyLib(k, lib) => s.push_str(&format!(
                    "[{}, {}]",
                    frap_rna::ptr::py_str_repr(k),
                    lib.as_deref()
                        .map(frap_rna::ptr::py_str_repr)
                        .unwrap_or_else(|| "None".to_owned())
                )),
                PathSeg::Index(i) => s.push_str(&format!("[{i}]")),
                PathSeg::Slice(a, b) => s.push_str(&format!(
                    "[{}:{}]",
                    a.map(|x| x.to_string()).unwrap_or_default(),
                    b.map(|x| x.to_string()).unwrap_or_default()
                )),
                PathSeg::Call(args) => s.push_str(&format!("({} args)", args.len())),
            }
        }
        s
    }
}

/// Resets every span to `Span::default()`, so IR from two syntaxes can be compared (§11.5).
pub trait EraseSpans {
    /// Erase spans in place.
    fn erase_spans(&mut self);
}

impl EraseSpans for Expr {
    fn erase_spans(&mut self) {
        match self {
            Expr::Lit(_, s) | Expr::Bare(_, s) => *s = Span::default(),
            Expr::Path(p) => p.erase_spans(),
            Expr::Tuple(v, s) | Expr::List(v, s) | Expr::Set(v, s) => {
                *s = Span::default();
                v.iter_mut().for_each(EraseSpans::erase_spans);
            }
            Expr::Dict(v, s) => {
                *s = Span::default();
                for (k, x) in v {
                    k.erase_spans();
                    x.erase_spans();
                }
            }
            Expr::Unary(_, e, s) => {
                *s = Span::default();
                e.erase_spans();
            }
            Expr::Binary(_, a, b, s) => {
                *s = Span::default();
                a.erase_spans();
                b.erase_spans();
            }
            Expr::Builtin { args, span, .. } => {
                *span = Span::default();
                args.iter_mut().for_each(EraseSpans::erase_spans);
            }
            Expr::Method {
                recv, args, span, ..
            } => {
                *span = Span::default();
                recv.erase_spans();
                args.iter_mut().for_each(EraseSpans::erase_spans);
            }
        }
    }
}
impl EraseSpans for PathExpr {
    fn erase_spans(&mut self) {
        self.span = Span::default();
        for seg in &mut self.segs {
            if let PathSeg::Call(args) = seg {
                args.iter_mut().for_each(EraseSpans::erase_spans);
            }
        }
    }
}
impl EraseSpans for Arg {
    fn erase_spans(&mut self) {
        self.span = Span::default();
        self.value.erase_spans();
    }
}
impl EraseSpans for Command {
    fn erase_spans(&mut self) {
        match self {
            Command::Invoke {
                positional, args, ..
            } => {
                positional.iter_mut().for_each(EraseSpans::erase_spans);
                args.iter_mut().for_each(EraseSpans::erase_spans);
            }
            Command::Get { target } => target.erase_spans(),
            Command::Set { target, value, .. } => {
                target.erase_spans();
                value.erase_spans();
            }
            Command::Call { target, args } => {
                target.erase_spans();
                args.iter_mut().for_each(EraseSpans::erase_spans);
            }
            Command::Print { values } => values.iter_mut().for_each(EraseSpans::erase_spans),
            Command::OpMeta { .. } | Command::Noop { .. } => {}
        }
    }
}
