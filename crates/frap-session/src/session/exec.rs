// SPDX-License-Identifier: GPL-3.0-or-later
//! Statement execution (ARCHITECTURE §10.2, §11.6, §12, §13.2): one response per statement;
//! mutations run in one transaction, pure reads without one.

use frap_lang::{
    Arg, AssignOp, BinOp, Command, Expr, Lit, OpMetaKind, PathExpr, PathSeg, Stmt, UnOp,
};
use frap_op::{OpCtx, OpStatus};
use frap_rna::value::coerce_element;
use frap_rna::{
    ArrayShape, CoerceCx, FrapError, MathTag, PropDef, PropFlags, PropKind, PyExc, Report,
    ReportType, RnaErrorKind, Root, Span, Unit, Value, coerce,
};
use frap_scene::HostRead;
use serde_json::{Map, Value as J, json};
use smol_str::SmolStr;

use super::eval::{self, ArgVal, Py};
use super::funcs::CallArg;
use super::{Fail, Session};
use crate::errors::*;
use crate::resolve::{self, ArrayKind, Elem, PropPlace, Target};
use crate::response::{Response, Status, info, value_json};

/// The response `kind` of a command.
pub(crate) fn stmt_kind(cmd: &Command) -> &'static str {
    match cmd {
        Command::Invoke { .. } => "op",
        Command::OpMeta { .. } => "op_meta",
        Command::Get { .. } => "get",
        Command::Set { .. } => "set",
        Command::Call { .. } => "call",
        Command::Print { .. } => "print",
        _ => "noop",
    }
}

/// The operator idname of an `op`/`op_meta` command.
pub(crate) fn stmt_op(cmd: &Command) -> Option<SmolStr> {
    match cmd {
        Command::Invoke { idname, .. } | Command::OpMeta { idname, .. } => Some(idname.clone()),
        _ => None,
    }
}

/// Parses a path for the front-end-free API (`Session::get/set/call`, §11.6).
pub(crate) fn parse_path(src: &str) -> Result<PathExpr, FrapError> {
    let mut stmts = frap_lang::parse_complete(src);
    let not_a_path = || {
        FrapError::new(
            PyExc::SyntaxError,
            frap_rna::ErrorCode("UNSUPPORTED_SYNTAX"),
            format!("frap: {src:?} is not a path such as bpy.data.objects['Cube'].location"),
        )
    };
    if stmts.len() != 1 {
        return Err(not_a_path());
    }
    match stmts.remove(0)? {
        Stmt {
            cmd: Command::Get { target },
            ..
        } => Ok(target),
        _ => Err(not_a_path()),
    }
}

/// Where an assigned value comes from.
pub(crate) enum ValueSrc<'a> {
    /// An expression of the statement.
    Expr(&'a Expr),
    /// An already-evaluated value (`Session::set`).
    Value(Py),
}

/// Where call arguments come from.
pub(crate) enum ArgsSrc<'a> {
    /// Arguments of the statement.
    Exprs(&'a [Arg]),
    /// Already-evaluated arguments (`Session::call`).
    Values(Vec<CallArg>),
}

/// True when evaluating the path calls a function (and so needs a transaction, §12).
pub(crate) fn path_has_call(p: &PathExpr) -> bool {
    p.segs.iter().any(|s| matches!(s, PathSeg::Call(_)))
}

fn expr_has_call(e: &Expr) -> bool {
    match e {
        Expr::Path(p) => path_has_call(p),
        Expr::Tuple(v, _) | Expr::List(v, _) | Expr::Set(v, _) => v.iter().any(expr_has_call),
        Expr::Dict(v, _) => v.iter().any(|(k, x)| expr_has_call(k) || expr_has_call(x)),
        Expr::Unary(_, x, _) => expr_has_call(x),
        Expr::Binary(_, a, b, _) => expr_has_call(a) || expr_has_call(b),
        Expr::Builtin { args, .. } => args.iter().any(|a| expr_has_call(&a.value)),
        Expr::Method { recv, args, .. } => {
            expr_has_call(recv) || args.iter().any(|a| expr_has_call(&a.value))
        }
        _ => false,
    }
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Lit(_, s)
        | Expr::Bare(_, s)
        | Expr::Tuple(_, s)
        | Expr::List(_, s)
        | Expr::Set(_, s)
        | Expr::Dict(_, s)
        | Expr::Unary(_, _, s)
        | Expr::Binary(_, _, _, s) => *s,
        Expr::Path(p) => p.span,
        Expr::Builtin { span, .. } | Expr::Method { span, .. } => *span,
        _ => Span::default(),
    }
}

fn binop_of(op: AssignOp) -> Option<BinOp> {
    match op {
        AssignOp::Add => Some(BinOp::Add),
        AssignOp::Sub => Some(BinOp::Sub),
        AssignOp::Mul => Some(BinOp::Mul),
        AssignOp::Div => Some(BinOp::Div),
        _ => None,
    }
}

fn read_only(owner: &str, prop: &str) -> FrapError {
    FrapError::new(
        PyExc::AttributeError,
        RNA_READONLY,
        format!("bpy_struct: attribute \"{prop}\" from \"{owner}\" is read-only"),
    )
}

const SET_PREFIX: &str = "bpy_struct: item.attr = val: ";

/// A set hook's `TypeError` mirrors a `pyrna_py_to_prop` check (`PROP_ID_SELF_CHECK`, the
/// pointer type, a `None` refusal), which Blender reports under the assignment prefix
/// (verified on 5.2.4: `bpy_struct: item.attr = val: Object.data expected a Mesh type, not
/// Camera`).
fn set_hook_prefix(mut e: FrapError) -> FrapError {
    if e.exception == PyExc::TypeError && e.code == RNA_TYPE && !e.message.starts_with(SET_PREFIX) {
        e.message = frap_rna::error::join_prefix(SET_PREFIX, &e.message);
    }
    e
}

fn set_code(k: RnaErrorKind) -> frap_rna::ErrorCode {
    match k {
        RnaErrorKind::Enum => RNA_ENUM,
        RnaErrorKind::Length => RNA_LENGTH,
        _ => RNA_TYPE,
    }
}

/// `ANGLE_LIKELY_DEGREES` (§13.2): a whole number above 2π assigned to a ROTATION property.
fn angle_hint(prop: &PropDef, v: &Py) -> Option<J> {
    if prop.unit != Unit::Rotation {
        return None;
    }
    let items = v.items().unwrap_or_else(|| vec![v.clone()]);
    let suspicious: Vec<f64> = items
        .iter()
        .filter_map(Py::number)
        .filter(|x| x.fract() == 0.0 && x.abs() > std::f64::consts::TAU)
        .collect();
    let first = suspicious.first()?;
    Some(json!({
        "code": "ANGLE_LIKELY_DEGREES",
        "message": format!(
            "{}={} is in radians ({first} rad); did you mean radians({first})?",
            prop.identifier,
            frap_rna::describe_input(&v.to_input().ok()?)
        ),
    }))
}

impl Session {
    pub(crate) fn dispatch(&mut self, stmt: &Stmt) -> Result<Response, Fail> {
        match &stmt.cmd {
            Command::Invoke {
                idname,
                positional,
                args,
            } => self.exec_invoke(stmt, idname, positional, args),
            Command::OpMeta { idname, what } => self.exec_op_meta(stmt, idname, *what),
            Command::Get { target } => self.get_path(stmt.line, target),
            Command::Set { target, op, value } => {
                self.set_path(stmt.line, target, *op, ValueSrc::Expr(value))
            }
            Command::Call { target, args } => {
                self.call_path(stmt.line, target, ArgsSrc::Exprs(args))
            }
            Command::Print { values } => self.exec_print(stmt, values),
            Command::Noop { .. } => Ok(self.base("noop", stmt.line, Status::Finished)),
            _ => Err(Fail::new(
                "noop",
                FrapError::new(
                    PyExc::NotImplementedError,
                    NOT_IMPLEMENTED,
                    "frap: this statement kind is not implemented yet",
                ),
            )),
        }
    }

    // ------------------------------------------------------------------ evaluation

    /// Evaluates an expression (nested calls run here, inside the statement's transaction).
    pub(crate) fn eval_expr(&mut self, e: &Expr) -> Result<Py, FrapError> {
        let sp = expr_span(e);
        let r = (|| -> Result<Py, FrapError> {
            Ok(match e {
                Expr::Lit(l, _) => match l {
                    Lit::Int(i) => Py::Int(*i),
                    Lit::Float(f) => Py::Float(*f),
                    Lit::Str(s) => Py::Str(s.clone()),
                    Lit::Bool(b) => Py::Bool(*b),
                    Lit::None => Py::None,
                },
                Expr::Bare(s, _) => Py::Bare(s.clone()),
                Expr::Tuple(v, _) => Py::Tuple(self.eval_all(v)?),
                Expr::List(v, _) => Py::List(self.eval_all(v)?),
                Expr::Set(v, _) => Py::Set(self.eval_all(v)?),
                Expr::Dict(v, _) => {
                    let mut d = indexmap::IndexMap::new();
                    for (k, x) in v {
                        let key = match self.eval_expr(k)? {
                            Py::Str(s) => s,
                            Py::Bare(s) => s.to_string(),
                            other => {
                                return Err(FrapError::new(
                                    PyExc::TypeError,
                                    RNA_TYPE,
                                    format!(
                                        "frap: dict keys must be strings, not {}",
                                        other.type_name()
                                    ),
                                ));
                            }
                        };
                        let val = self.eval_expr(x)?;
                        d.insert(key, val);
                    }
                    Py::Dict(d)
                }
                Expr::Unary(op, x, _) => eval::unary(*op == UnOp::Neg, self.eval_expr(x)?)?,
                Expr::Binary(op, a, b, _) => {
                    let a = self.eval_expr(a)?;
                    let b = self.eval_expr(b)?;
                    eval::binop(*op, &a, &b, false)?
                }
                Expr::Builtin { name, args, .. } => {
                    let args = self.eval_arg_vals(args)?;
                    eval::builtin(*name, &args)?
                }
                Expr::Method {
                    recv, name, args, ..
                } => {
                    let recv = self.eval_expr(recv)?;
                    let args = self.eval_arg_vals(args)?;
                    eval::method(recv, name, &args)?
                }
                Expr::Path(p) => {
                    let t = self.eval_target(p)?;
                    eval::target_py(&self.host, t)?
                }
                _ => {
                    return Err(FrapError::new(
                        PyExc::NotImplementedError,
                        NOT_IMPLEMENTED,
                        "frap: this expression is not implemented yet",
                    ));
                }
            })
        })();
        r.map_err(|err| {
            if sp == Span::default() {
                err
            } else {
                err.with_span(sp)
            }
        })
    }

    fn eval_all(&mut self, v: &[Expr]) -> Result<Vec<Py>, FrapError> {
        v.iter().map(|x| self.eval_expr(x)).collect()
    }

    fn eval_arg_vals(&mut self, args: &[Arg]) -> Result<Vec<ArgVal>, FrapError> {
        args.iter()
            .map(|a| {
                Ok(ArgVal {
                    name: a.name.clone(),
                    value: self.eval_expr(&a.value)?,
                })
            })
            .collect()
    }

    pub(crate) fn eval_args(&mut self, args: &[Arg]) -> Result<Vec<CallArg>, FrapError> {
        args.iter()
            .map(|a| {
                Ok(CallArg {
                    name: a.name.clone(),
                    value: self.eval_expr(&a.value)?,
                    span: a.span,
                })
            })
            .collect()
    }

    /// Resolves a path, calling the functions it contains (innermost first, §11.2).
    pub(crate) fn eval_target(&mut self, p: &PathExpr) -> Result<Target, FrapError> {
        self.eval_segs(p, p.segs.len())
    }

    /// Resolves the first `n` segments of a path.
    fn eval_segs(&mut self, p: &PathExpr, n: usize) -> Result<Target, FrapError> {
        let mut t = resolve::root(p.root);
        for seg in &p.segs[..n] {
            t = match seg {
                PathSeg::Call(args) => {
                    let Target::Func(f) = t else {
                        return Err(FrapError::new(
                            PyExc::TypeError,
                            RNA_TYPE,
                            format!("'{}' object is not callable", resolve::py_type_name(&t)),
                        ));
                    };
                    let args = self.eval_args(args)?;
                    let (v, _) = self.call_func(f, args)?;
                    super::funcs::py_target(v)
                }
                seg => resolve::step(&self.host, t, seg).map_err(|e| e.with_span(p.span))?,
            };
        }
        Ok(t)
    }

    // ------------------------------------------------------------------ get / print

    pub(crate) fn get_path(&mut self, line: u32, p: &PathExpr) -> Result<Response, Fail> {
        let caps = self.caps();
        if !path_has_call(p) {
            // A pure read: no transaction, the state stays bit-identical (§12).
            let t = resolve::resolve(&self.host, p)
                .map_err(|e| Fail::new("get", e.with_span(p.span)))?;
            let (value, rna) =
                resolve::encode(&self.host, &t, caps).map_err(|e| Fail::new("get", e))?;
            let mut r = self.base("get", line, Status::Finished);
            r.path = resolve::repr(&t, &self.host.state.main);
            r.value = Some(value);
            r.rna = rna;
            return Ok(r);
        }
        let txn = self.transact(|s| {
            let t = s.eval_target(p)?;
            let (value, rna) = resolve::encode(&s.host, &t, caps)?;
            let path = resolve::repr(&t, &s.host.state.main);
            Ok(((value, rna, path), false))
        });
        let (value, rna, path) = txn.result.map_err(|e| Fail {
            kind: "get",
            op: None,
            err: e,
            reports: txn.reports.clone(),
        })?;
        let mut r = self.base("get", line, Status::Finished);
        r.path = path;
        r.value = Some(value);
        r.rna = rna;
        r.reports = txn.reports;
        if !txn.log.added.is_empty() || !txn.log.modified.is_empty() || !txn.log.removed.is_empty()
        {
            r.changes = Some(self.changes_json(&txn.log));
        }
        Ok(r)
    }

    fn exec_print(&mut self, stmt: &Stmt, values: &[Expr]) -> Result<Response, Fail> {
        let caps = self.caps();
        let body = |s: &mut Session| -> Result<Vec<J>, FrapError> {
            let mut out = Vec::with_capacity(values.len());
            for e in values {
                out.push(match e {
                    Expr::Path(p) => {
                        let t = s.eval_target(p)?;
                        resolve::encode(&s.host, &t, caps)?.0
                    }
                    e => s.eval_expr(e)?.to_json(&s.host, caps)?,
                });
            }
            Ok(out)
        };
        let (vals, reports, changes) = if values.iter().any(expr_has_call) {
            let txn = self.transact(|s| body(s).map(|v| (v, false)));
            let log = txn.log;
            let vals = txn.result.map_err(|e| Fail {
                kind: "print",
                op: None,
                err: e,
                reports: txn.reports.clone(),
            })?;
            let changes = (!log.added.is_empty()
                || !log.modified.is_empty()
                || !log.removed.is_empty()
                || !log.renamed.is_empty())
            .then(|| self.changes_json(&log));
            (vals, txn.reports, changes)
        } else {
            (body(self).map_err(|e| Fail::new("print", e))?, vec![], None)
        };
        let mut r = self.base("print", stmt.line, Status::Finished);
        r.value = Some(J::Array(vals));
        r.reports = reports;
        r.changes = changes;
        Ok(r)
    }

    // ------------------------------------------------------------------ set

    pub(crate) fn set_path(
        &mut self,
        line: u32,
        target: &PathExpr,
        op: AssignOp,
        src: ValueSrc<'_>,
    ) -> Result<Response, Fail> {
        let txn = self.transact(|s| {
            let Some((last, _)) = target.segs.split_last() else {
                let root = if target.root == frap_lang::PathRoot::BpyData {
                    "bpy.data"
                } else {
                    "bpy.context"
                };
                return Err(FrapError::new(
                    PyExc::SyntaxError,
                    frap_rna::ErrorCode("SYNTAX"),
                    format!("cannot assign to {root} itself"),
                ));
            };
            let rhs = |s: &mut Session| match &src {
                ValueSrc::Expr(e) => s.eval_expr(e),
                ValueSrc::Value(v) => Ok(v.clone()),
            };
            // Python evaluates the right-hand side of `=` first; `+=` reads the target first.
            let (place, v) = match binop_of(op) {
                None => {
                    let v = rhs(s)?;
                    let parent = s.eval_segs(target, target.segs.len() - 1)?;
                    (s.assign_target(parent, last, v.type_name())?, v)
                }
                Some(bop) => {
                    let parent = s.eval_segs(target, target.segs.len() - 1)?;
                    let place = s.assign_target(parent, last, "value")?;
                    let cur = eval::target_py(&s.host, Target::Prop(place.clone()))?;
                    let v = rhs(s)?;
                    let v = eval::binop(bop, &cur, &v, true)?;
                    (place, v)
                }
            };
            let out = s.assign(&place, v)?;
            let path = resolve::repr(&Target::Prop(place), &s.host.state.main);
            Ok(((out, path), false))
        });
        let ((value, reports, hints), path) = txn.result.map_err(|e| Fail {
            kind: "set",
            op: None,
            err: e.with_span(target.span),
            reports: txn.reports.clone(),
        })?;
        let mut r = self.base("set", line, Status::Finished);
        r.path = path;
        r.value = Some(value);
        r.reports = txn.reports;
        r.reports.extend(reports);
        r.hints = hints;
        r.changes = Some(self.changes_json(&txn.log));
        Ok(r)
    }

    /// The property an assignment writes, with Blender's errors for everything else.
    fn assign_target(
        &self,
        parent: Target,
        last: &PathSeg,
        value_type: &str,
    ) -> Result<PropPlace, FrapError> {
        // Python objects without `__dict__` (numbers, strings, mathutils values, lists) word
        // their AttributeError differently on assignment; RNA structs (also those reached
        // through a pointer property or a context member) do not.
        let parent_is_python = match &parent {
            Target::Prop(p) => {
                !matches!(p.prop.kind, PropKind::Pointer { .. })
                    && !(p.ptr.root == Root::Context && p.ptr.path.is_empty())
            }
            Target::Value(_) | Target::List { .. } => true,
            _ => false,
        };
        let parent_is_coll = matches!(parent, Target::Coll(_));
        let t = match resolve::step(&self.host, parent, last) {
            Ok(t) => t,
            Err(mut e) => {
                // Python's setattr wording, also on `None` (`scene.camera.lens = 1` with no
                // camera, verified on 5.2.4).
                if (parent_is_python || e.message.starts_with("'NoneType'"))
                    && e.exception == PyExc::AttributeError
                    && e.message.contains("object has no attribute")
                {
                    e.message
                        .push_str(" and no __dict__ for setting new attributes");
                }
                return Err(e);
            }
        };
        match t {
            Target::Prop(p) => {
                if p.elem == Elem::Whole {
                    self.check_writable(&p)?;
                }
                Ok(p)
            }
            Target::Coll(c) => Err(read_only(c.ptr.srna.identifier, c.prop.identifier)),
            _ if parent_is_coll => Err(FrapError::new(
                PyExc::TypeError,
                RNA_TYPE,
                format!(
                    "bpy_prop_collection[key] = value: invalid, expected a StructRNA type or None, not a {value_type}"
                ),
            )),
            other => Err(FrapError::new(
                PyExc::TypeError,
                RNA_TYPE,
                format!("frap: cannot assign to a {}", resolve::py_type_name(&other)),
            )),
        }
    }

    fn check_writable(&self, p: &PropPlace) -> Result<(), FrapError> {
        let prop = p.prop;
        if p.ptr.root == Root::Context && p.ptr.path.is_empty() {
            let msg = if prop.flags.contains(PropFlags::CONTEXT_MEMBER) {
                format!(
                    "bpy_struct: Context property \"{}\" is read-only",
                    prop.identifier
                )
            } else {
                format!(
                    "bpy_struct: attribute \"{}\" from \"Context\" is read-only",
                    prop.identifier
                )
            };
            let mut e = FrapError::new(PyExc::AttributeError, CONTEXT_READONLY, msg);
            e = e.with_hint(match prop.identifier {
                "object" | "active_object" => {
                    "set bpy.context.view_layer.objects.active = bpy.data.objects['Name']"
                }
                "selected_objects" | "selected_editable_objects" => {
                    "select with bpy.data.objects['Name'].select_set(True) or bpy.ops.object.select_all(action='DESELECT')"
                }
                "scene" => "set bpy.context.window.scene = bpy.data.scenes['Name']",
                "mode" => "bpy.ops.object.mode_set(mode='EDIT')",
                _ => "bpy.context is read-only; assign through bpy.data instead",
            });
            return Err(e);
        }
        if prop.flags.contains(PropFlags::READONLY)
            || matches!(prop.kind, PropKind::Collection { .. })
        {
            let owner = if p.ptr.root == Root::Data && p.ptr.path.is_empty() {
                "BlendData"
            } else {
                p.ptr.srna.identifier
            };
            return Err(read_only(owner, prop.identifier));
        }
        Ok(())
    }

    /// Coerces and writes; returns the echoed value, frap's reports and hints.
    fn assign(&mut self, p: &PropPlace, v: Py) -> Result<(J, Vec<Report>, Vec<J>), FrapError> {
        let prop = p.prop;
        let owner = p.ptr.srna.identifier;
        let mut reports = Vec::new();
        let hints: Vec<J> = angle_hint(prop, &v).into_iter().collect();
        if let Some(d) = prop.deprecated {
            // Python's DeprecationWarning text (verified: `Material.use_nodes = False`).
            reports.push(Report::new(
                ReportType::Warning,
                format!(
                    "'{}.{}' is expected to be removed in Blender {}.{}",
                    owner, prop.identifier, d.removal_version[0], d.removal_version[1]
                ),
            ));
        }
        if p.elem == Elem::Whole {
            let input = v.to_input()?;
            let c = coerce(prop, input.clone(), &CoerceCx::RnaSet { owner })
                .map_err(|e| e.into_frap(SET_PREFIX, set_code))?;
            if c.clamped {
                reports.push(info(frap_rna::clamp_report(prop, &input)));
            }
            resolve::set_prop(&mut self.host, &p.ptr, prop, c.value.clone())
                .map_err(set_hook_prefix)?;
            return Ok((value_json(&c.value, &self.host.state.main), reports, hints));
        }
        // One element (or a slice) of an array: read, replace, write the whole array back.
        let whole = PropPlace {
            elem: Elem::Whole,
            ..p.clone()
        };
        self.check_writable(&whole)?;
        let mut cur = resolve::read_value(&self.host, &whole)?;
        let kind = ArrayKind::of(prop);
        let len = match &cur {
            Value::FloatArray { data, .. } => data.len(),
            Value::IntArray(a) => a.len(),
            Value::BoolArray(a) => a.len(),
            _ => return Err(resolve::stale("element write on a non-array")),
        };
        // One element, coerced and clamped like a whole-array write (same INFO report).
        let element = |v: &Py, reports: &mut Vec<Report>| -> Result<Value, FrapError> {
            let input = v.to_input()?;
            let c = coerce_element(prop, input.clone()).map_err(FrapError::from)?;
            if c.clamped {
                reports.push(info(frap_rna::clamp_report(prop, &input)));
            }
            Ok(c.value)
        };
        let echo = match p.elem {
            // A matrix row out of range (in-range rows resolve to `Elem::Row`).
            Elem::Index(_) if matches!(kind, ArrayKind::Matrix(..)) => {
                return Err(kind.write_index_error());
            }
            Elem::Index(i) => {
                let j = resolve::norm_index(i, len).ok_or_else(|| kind.write_index_error())?;
                let c = element(&v, &mut reports)?;
                put(&mut cur, j, &c);
                c
            }
            Elem::Component(_, j) => {
                let c = element(&v, &mut reports)?;
                put(&mut cur, j, &c);
                c
            }
            Elem::RowCol(r, col) => {
                let ArrayKind::Matrix(_, cols) = kind else {
                    return Err(resolve::stale("row of a non-matrix"));
                };
                let k = resolve::norm_index(col, cols)
                    .ok_or_else(|| ArrayKind::Vector(cols).write_index_error())?;
                let x = v.number().ok_or_else(|| {
                    FrapError::new(
                        PyExc::TypeError,
                        RNA_TYPE,
                        "vector[index] = x: assigned value not a number",
                    )
                })?;
                put(&mut cur, r * cols + k, &Value::Float(x as f32));
                Value::Float(x as f32)
            }
            Elem::Row(_) | Elem::Slice(..) if let ArrayKind::Matrix(rows, cols) = kind => {
                // `mathutils` row assignment: `matrix[i] = row`, `matrix[a:b] = rows`
                // (messages verified on 5.2.4).
                let (r0, r1, what) = match p.elem {
                    Elem::Row(r) => (r, r + 1, "matrix[i] = value assignment"),
                    Elem::Slice(a, b) => {
                        let (s, e) = resolve::norm_slice(a, b, rows);
                        (s, e, "matrix[slice] = value")
                    }
                    _ => return Err(resolve::stale("matrix element")),
                };
                let type_err = || FrapError::new(PyExc::TypeError, RNA_TYPE, what);
                let row_vals = |row: &Py| -> Result<Vec<f32>, FrapError> {
                    let items = row.items().ok_or_else(type_err)?;
                    if items.len() != cols {
                        return Err(FrapError::new(
                            PyExc::ValueError,
                            RNA_LENGTH,
                            format!(
                                "{what}: sequence length is {}, expected {cols}",
                                items.len()
                            ),
                        ));
                    }
                    items
                        .iter()
                        .map(|x| x.number().map(|f| f as f32).ok_or_else(type_err))
                        .collect()
                };
                let new_rows: Vec<Vec<f32>> = if matches!(p.elem, Elem::Row(_)) {
                    vec![row_vals(&v)?]
                } else {
                    let items = v.items().ok_or_else(type_err)?;
                    if items.len() != r1 - r0 {
                        return Err(FrapError::new(
                            PyExc::ValueError,
                            RNA_LENGTH,
                            format!(
                                "{what}: sequence size is {}, expected {}",
                                items.len(),
                                r1 - r0
                            ),
                        ));
                    }
                    items.iter().map(row_vals).collect::<Result<_, _>>()?
                };
                let mut echo = Vec::with_capacity(new_rows.len());
                for (k, row) in new_rows.into_iter().enumerate() {
                    for (j, x) in row.iter().enumerate() {
                        put(&mut cur, (r0 + k) * cols + j, &Value::Float(*x));
                    }
                    echo.push(Value::FloatArray {
                        data: row.into_iter().collect(),
                        shape: ArrayShape::Fixed(cols as u16),
                        tag: MathTag::Vector,
                    });
                }
                if matches!(p.elem, Elem::Row(_)) {
                    echo.pop().expect("invariant: one row")
                } else {
                    Value::Tuple(echo)
                }
            }
            Elem::Row(_) | Elem::Slice(..) => {
                let (s, e) = match (p.elem, kind) {
                    (Elem::Row(r), ArrayKind::Matrix(_, cols)) => (r * cols, (r + 1) * cols),
                    (Elem::Slice(a, b), _) => resolve::norm_slice(a, b, len),
                    _ => return Err(resolve::stale("row of a non-matrix")),
                };
                let lower = match kind {
                    ArrayKind::PropArray => "bpy_prop_array[slice]",
                    ArrayKind::Euler => "euler[slice]",
                    ArrayKind::Quaternion => "quaternion[slice]",
                    ArrayKind::Color => "color[slice]",
                    _ => "vector[slice]",
                };
                let items = v.items().ok_or_else(|| {
                    FrapError::new(PyExc::TypeError, RNA_TYPE, format!("{lower} = seq"))
                })?;
                if items.len() != e - s {
                    return Err(if kind == ArrayKind::PropArray {
                        FrapError::new(
                            PyExc::TypeError,
                            RNA_LENGTH,
                            "bpy_prop_array[slice] = value: re-sizing bpy_struct arrays isn't supported",
                        )
                    } else {
                        FrapError::new(
                            PyExc::ValueError,
                            RNA_LENGTH,
                            format!(
                                "{lower} = seq: sequence size is {}, expected {}",
                                items.len(),
                                e - s
                            ),
                        )
                    });
                }
                let mut vals = Vec::with_capacity(items.len());
                for (k, it) in items.iter().enumerate() {
                    let c = element(it, &mut reports)?;
                    put(&mut cur, s + k, &c);
                    vals.push(c);
                }
                Value::Tuple(vals)
            }
            Elem::Whole => unreachable!("handled above"),
        };
        resolve::set_prop(&mut self.host, &p.ptr, prop, cur)?;
        Ok((value_json(&echo, &self.host.state.main), reports, hints))
    }

    // ------------------------------------------------------------------ call

    pub(crate) fn call_path(
        &mut self,
        line: u32,
        target: &PathExpr,
        src: ArgsSrc<'_>,
    ) -> Result<Response, Fail> {
        let caps = self.caps();
        let mut src = Some(src);
        let txn = self.transact(|s| {
            let t = s.eval_target(target)?;
            let Target::Func(f) = t else {
                return Err(FrapError::new(
                    PyExc::TypeError,
                    RNA_TYPE,
                    format!("'{}' object is not callable", resolve::py_type_name(&t)),
                ));
            };
            let args = match src.take().expect("invariant: one call") {
                ArgsSrc::Exprs(a) => s.eval_args(a)?,
                ArgsSrc::Values(v) => v,
            };
            let func = f.repr(&s.host.state.main);
            let (v, ret) = s.call_func(f, args)?;
            let rna = ret.map(|p| resolve::rna_tag(p, None));
            let value = v.to_json(&s.host, caps)?;
            Ok(((func, value, rna), false))
        });
        let (func, value, rna) = txn.result.map_err(|e| Fail {
            kind: "call",
            op: None,
            err: e.with_span(target.span),
            reports: txn.reports.clone(),
        })?;
        let mut r = self.base("call", line, Status::Finished);
        r.func = Some(func);
        r.value = Some(value);
        r.rna = rna;
        r.reports = txn.reports;
        r.changes = Some(self.changes_json(&txn.log));
        Ok(r)
    }

    // ------------------------------------------------------------------ operators

    fn exec_invoke(
        &mut self,
        stmt: &Stmt,
        idname: &SmolStr,
        positional: &[Expr],
        args: &[Arg],
    ) -> Result<Response, Fail> {
        // Arguments may contain nested calls: evaluate them inside the statement's transaction
        // (the operator call itself runs in a nested transaction of the same snapshot).
        let has_calls =
            positional.iter().any(expr_has_call) || args.iter().any(|a| expr_has_call(&a.value));
        let fail = |e: FrapError| Fail {
            kind: "op",
            op: Some(idname.clone()),
            err: e,
            reports: vec![],
        };
        if !has_calls {
            let pos = positional
                .iter()
                .map(|e| self.eval_expr(e))
                .collect::<Result<Vec<_>, _>>()
                .map_err(fail)?;
            let mut kw = Vec::with_capacity(args.len());
            for a in args {
                let v = self.eval_expr(&a.value).map_err(fail)?;
                kw.push((a.name.as_deref().unwrap_or("").to_owned(), v, a.span));
            }
            return self
                .invoke_values(stmt.line, idname, pos, kw)
                .map_err(|mut f| {
                    f.err = f.err.with_span(stmt.span);
                    f
                });
        }
        // One outer transaction covers argument evaluation and the operator: the operator's
        // own (nested) transaction then sees the nested calls' changes and reports, and any
        // failure or panic restores the statement's snapshot.
        let txn = self.transact(|s| {
            let pos = positional
                .iter()
                .map(|e| s.eval_expr(e))
                .collect::<Result<Vec<_>, _>>()?;
            let mut kw = Vec::with_capacity(args.len());
            for a in args {
                let v = s.eval_expr(&a.value)?;
                kw.push((a.name.as_deref().unwrap_or("").to_owned(), v, a.span));
            }
            let r = s.invoke_values(stmt.line, idname, pos, kw);
            // Anything but FINISHED rolls the whole statement back.
            let rollback = !matches!(&r, Ok(r) if r.status == Status::Finished);
            Ok((r, rollback))
        });
        let mut leftover = txn.reports;
        match txn.result {
            Ok(Ok(r)) => Ok(r),
            Ok(Err(mut f)) => {
                f.err = f.err.with_span(stmt.span);
                leftover.append(&mut f.reports);
                f.reports = leftover;
                Err(f)
            }
            Err(e) => {
                let mut f = fail(e.with_span(stmt.span));
                f.reports = leftover;
                Err(f)
            }
        }
    }

    /// The §10.2 pipeline after argument evaluation: lookup, conversion, poll, transaction,
    /// exec, response.
    pub(crate) fn invoke_values(
        &mut self,
        line: u32,
        idname: &str,
        positional: Vec<Py>,
        kwargs: Vec<(String, Py, Span)>,
    ) -> Result<Response, Fail> {
        let op = Some(SmolStr::new(idname));
        let fail = |e: FrapError| Fail {
            kind: "op",
            op: op.clone(),
            err: e,
            reports: vec![],
        };
        // §10.2 step 1: lookup.
        let def = frap_op::lookup(idname).map_err(fail)?;
        // Steps 2–3: arguments, before poll.
        let spans: Vec<(String, Span)> = kwargs.iter().map(|(k, _, s)| (k.clone(), *s)).collect();
        let pos = positional
            .iter()
            .map(Py::to_input)
            .collect::<Result<Vec<_>, _>>()
            .map_err(fail)?;
        let mut kw = Vec::with_capacity(kwargs.len());
        for (k, v, _) in kwargs {
            kw.push((k, v.to_input().map_err(fail)?));
        }
        let (args, mut reports) = frap_op::convert_args(def.spec, pos, kw).map_err(|e| {
            let arg_span = e
                .detail
                .get("argument")
                .and_then(|a| a.as_str())
                .and_then(|a| spans.iter().find(|(k, _)| k == a))
                .map(|(_, s)| *s);
            fail(match arg_span {
                Some(mut s) if s != Span::default() => {
                    // An unknown keyword points at the keyword itself (§13.5).
                    if e.code == frap_op::errors::OP_ARG_UNKNOWN {
                        s.len = e
                            .detail
                            .get("argument")
                            .and_then(|a| a.as_str())
                            .map_or(s.len, |a| a.len() as u32);
                    }
                    e.with_span(s)
                }
                _ => e,
            })
        })?;
        // Step 4 (ensure evaluated) happens inside the operators that read evaluated data.
        // Step 5: poll.
        frap_op::check_poll(def, &self.host.state.ctx()).map_err(fail)?;
        // Steps 6–8: transaction + exec.
        let txn = self.transact(|s| {
            let mut cx = OpCtx::new(&mut s.host, &mut s.services, def.spec);
            let r = (def.exec)(&mut cx, &args);
            let out = cx.finish();
            match r {
                Ok(OpStatus::Finished) => Ok((out, false)),
                Ok(OpStatus::Cancelled) => Ok((out, true)),
                Err(e) => {
                    s.host.reports.extend(out.reports);
                    Err(e.into_frap())
                }
            }
        });
        reports.extend(txn.reports);
        let out = match txn.result {
            Ok(out) => out,
            Err(e) => {
                return Err(Fail {
                    kind: "op",
                    op,
                    err: e,
                    reports,
                });
            }
        };
        reports.extend(out.reports);
        // Step 9: response.
        let status = if txn.cancelled {
            Status::Cancelled
        } else {
            Status::Finished
        };
        let mut r = self.base("op", line, status);
        r.op = op;
        r.reports = reports;
        if status == Status::Finished {
            let names = &self.host.state.main;
            let mut res = Map::new();
            for (k, v) in &out.result {
                res.insert(k.to_string(), value_json(v, names));
            }
            r.result = Some(res);
            r.changes = Some(self.changes_json(&txn.log));
        }
        Ok(r)
    }

    fn exec_op_meta(
        &mut self,
        stmt: &Stmt,
        idname: &SmolStr,
        what: OpMetaKind,
    ) -> Result<Response, Fail> {
        let fail = |e: FrapError| Fail {
            kind: "op_meta",
            op: Some(idname.clone()),
            err: e,
            reports: vec![],
        };
        let mut r = self.base("op_meta", stmt.line, Status::Finished);
        r.op = Some(idname.clone());
        let bl_idname = || {
            let (ns, name) = idname.split_once('.').unwrap_or(("", idname));
            format!("{}_OT_{name}", ns.to_ascii_uppercase())
        };
        match what {
            OpMetaKind::Idname => r.value = Some(J::from(bl_idname())),
            OpMetaKind::IdnamePy => r.value = Some(J::from(idname.as_str())),
            OpMetaKind::Poll => {
                let def = frap_op::lookup(idname).map_err(|mut e| {
                    if e.code == frap_op::errors::OP_UNKNOWN {
                        e.message = format!(
                            "Polling operator \"bpy.ops.{idname}\" error, could not be found"
                        );
                    }
                    fail(e)
                })?;
                match frap_op::check_poll(def, &self.host.state.ctx()) {
                    Ok(()) => r.value = Some(J::from(true)),
                    Err(e) => {
                        r.value = Some(J::from(false));
                        let mut res = Map::new();
                        res.insert("message".into(), J::from(e.message));
                        for k in ["requires", "unmet"] {
                            if let Some(v) = e.detail.get(k) {
                                res.insert(k.into(), v.clone());
                            }
                        }
                        res.insert("hint".into(), e.hint.map(J::from).unwrap_or(J::Null));
                        r.result = Some(res);
                    }
                }
            }
            OpMetaKind::GetRnaType => {
                let def = frap_op::lookup(idname).map_err(fail)?;
                let spec = def.spec;
                let props: Vec<J> = spec
                    .props
                    .iter()
                    .map(|p| {
                        let mut j = resolve::rna_tag(p, None);
                        if let J::Object(m) = &mut j {
                            m.insert("identifier".into(), J::from(p.identifier));
                            m.insert("name".into(), J::from(p.name));
                            m.insert(
                                "default".into(),
                                value_json(&Value::default_for(p), &self.host.state.main),
                            );
                            if p.array_len() > 1 {
                                m.insert("array_length".into(), J::from(p.array_len()));
                            }
                        }
                        j
                    })
                    .collect();
                r.value = Some(json!({
                    "$struct": spec.bl_idname, "idname": spec.idname, "name": spec.label,
                    "description": spec.description, "properties": props,
                }));
            }
            _ => {
                return Err(fail(FrapError::new(
                    PyExc::NotImplementedError,
                    NOT_IMPLEMENTED,
                    "frap: this operator query is not implemented yet",
                )));
            }
        }
        Ok(r)
    }
}

fn put(arr: &mut Value, i: usize, v: &Value) {
    match (arr, v) {
        (Value::FloatArray { data, .. }, Value::Float(f)) => data[i] = *f,
        (Value::FloatArray { data, .. }, Value::Double(f)) => data[i] = *f as f32,
        (Value::IntArray(a), Value::Int(x)) => a[i] = *x,
        (Value::BoolArray(a), Value::Bool(x)) => a[i] = *x,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_hint_flags_whole_numbers_above_two_pi() {
        let euler = frap_spec_rna::VIEW3_D_CURSOR
            .find_prop("rotation_euler")
            .expect("prop");
        let h = angle_hint(euler, &Py::Tuple(vec![Py::Int(0), Py::Int(0), Py::Int(90)])).unwrap();
        assert_eq!(h["code"], "ANGLE_LIKELY_DEGREES");
        assert_eq!(
            h["message"],
            "rotation_euler=(0, 0, 90) is in radians (90 rad); did you mean radians(90)?"
        );
        assert!(angle_hint(euler, &Py::Tuple(vec![Py::Float(1.5); 3])).is_none());
        let loc = frap_spec_rna::OBJECT.find_prop("location").expect("prop");
        assert!(angle_hint(loc, &Py::Tuple(vec![Py::Int(90); 3])).is_none());
    }
}
