// SPDX-License-Identifier: GPL-3.0-or-later
//! Function calls (ARCHITECTURE §9.5, §13.3): RNA functions bound through `FuncArgs` with
//! Blender's `pyrna_func_call` rules and messages, dispatched to `RNA_FUNCS`; the Python-level
//! `bpy_prop_collection` methods (`get`, `find`, `keys`, `values`, `items`, `foreach_get`,
//! `foreach_set`) and `bpy_struct` methods (`path_from_id`, `path_resolve`) are implemented
//! here. Messages verified on Blender 5.2.4.

use frap_lang::{Command, PathSeg};
use frap_rna::{
    ArrayShape, BitSet, Bulk, CoerceCx, FrapError, FuncDef, FuncFlags, Input, MathTag, PropDef,
    PropFlags, PropKind, PyExc, Report, RnaErrorKind, RnaPtr, Root, Span, Value, coerce,
};
use frap_scene::{FuncArgs, RNA_FUNCS, find_func};
use indexmap::IndexMap;
use smol_str::SmolStr;

use super::Session;
use super::eval::{Py, value_py};
use crate::errors::*;
use crate::resolve::{
    self, CollFunc, CollPlace, FuncTarget, StructFunc, Target, bulk_json, with_coll,
};
use crate::response::info;

/// One evaluated call argument.
#[derive(Clone, Debug)]
pub struct CallArg {
    /// Keyword (`None` = positional).
    pub name: Option<SmolStr>,
    /// Value.
    pub value: Py,
    /// Source span.
    pub span: Span,
}

/// A call result as a target, so a path can continue after it (`objects.get('Cube').name`).
pub(crate) fn py_target(v: Py) -> Target {
    match v {
        Py::None => Target::None,
        Py::Rna(t) => t,
        Py::Bool(b) => Target::Value(Value::Bool(b)),
        Py::Int(i) => Target::Value(Value::Int(i)),
        Py::Float(f) => Target::Value(Value::Double(f)),
        Py::Str(s) => Target::Value(Value::Str(s.into())),
        Py::Bare(s) => Target::Value(Value::Str(s)),
        Py::List(items) | Py::Tuple(items) | Py::Set(items) | Py::PropArray(items) => {
            Target::List {
                what: "list",
                items: items.into_iter().map(py_target).collect(),
            }
        }
        // mathutils values keep their type, so a path continues with `.x`, `.order`, `[i]`.
        Py::Vector(d) => math_target(d, MathTag::Vector, None),
        Py::Euler(d, o) => math_target(d.to_vec(), MathTag::Euler(o), None),
        Py::Quaternion(d) => math_target(d.to_vec(), MathTag::Quaternion, None),
        Py::Color(d) => math_target(d.to_vec(), MathTag::Color, None),
        Py::Matrix(rows) => {
            let r = rows.len();
            let c = rows.first().map_or(0, Vec::len);
            math_target(rows.concat(), MathTag::Matrix, Some((r, c)))
        }
        other => match other.items() {
            Some(items) => Target::List {
                what: "tuple",
                items: items.into_iter().map(py_target).collect(),
            },
            None => Target::Value(Value::Str(other.type_name().into())),
        },
    }
}

fn math_target(data: Vec<f32>, tag: MathTag, matrix: Option<(usize, usize)>) -> Target {
    let shape = match matrix {
        Some((r, c)) => ArrayShape::Matrix(r as u16, c as u16),
        None => ArrayShape::Fixed(data.len() as u16),
    };
    Target::Value(Value::FloatArray {
        data: data.into_iter().collect(),
        shape,
        tag,
    })
}

fn type_err(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::TypeError, frap_rna::codes::RNA_FUNC_ARGS, msg)
}

fn runtime_err(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::RuntimeError, frap_rna::codes::RNA_RUNTIME, msg)
}

/// CPython's call checks for a method defined in Python (`def from_pydata(self, vertices,
/// edges, faces, shade_flat=True)`), in CPython's order and wording (verified on 5.2.4):
/// `Mesh.from_pydata() takes from 4 to 5 positional arguments but 6 were given`,
/// `… got an unexpected keyword argument 'foo'`, `… got multiple values for argument 'x'`,
/// `… missing 1 required positional argument: 'faces'`. Counts include `self`.
fn cpython_arity(
    fname: &str,
    params: &[PropDef],
    args: &[CallArg],
    npos: usize,
) -> Result<(), FrapError> {
    let required = params
        .iter()
        .filter(|p| p.flags.contains(PropFlags::REQUIRED))
        .count();
    if npos > params.len() {
        let (lo, hi) = (required + 1, params.len() + 1);
        let takes = if lo == hi {
            format!("{hi} positional argument{}", if hi == 1 { "" } else { "s" })
        } else {
            format!("from {lo} to {hi} positional arguments")
        };
        return Err(type_err(format!(
            "{fname}() takes {takes} but {} were given",
            npos + 1
        )));
    }
    for a in args.iter().filter_map(|a| a.name.as_deref()) {
        match params.iter().position(|p| p.identifier == a) {
            None => {
                return Err(type_err(format!(
                    "{fname}() got an unexpected keyword argument '{a}'"
                )));
            }
            Some(i) if i < npos => {
                return Err(type_err(format!(
                    "{fname}() got multiple values for argument '{a}'"
                )));
            }
            Some(_) => {}
        }
    }
    let missing: Vec<String> = params
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            p.flags.contains(PropFlags::REQUIRED)
                && *i >= npos
                && !args.iter().any(|a| a.name.as_deref() == Some(p.identifier))
        })
        .map(|(_, p)| format!("'{}'", p.identifier))
        .collect();
    if !missing.is_empty() {
        let list = match missing.as_slice() {
            [one] => one.clone(),
            [a, b] => format!("{a} and {b}"),
            [init @ .., last] => format!("{}, and {last}", init.join(", ")),
            [] => String::new(),
        };
        return Err(type_err(format!(
            "{fname}() missing {} required positional argument{}: {list}",
            missing.len(),
            if missing.len() == 1 { "" } else { "s" }
        )));
    }
    Ok(())
}

/// Binds positional and keyword arguments against `FuncDef.params` (Blender's
/// `pyrna_func_call` order of checks), coercing each one.
pub(crate) fn bind_args(
    owner: &str,
    def: &'static FuncDef,
    args: &[CallArg],
    reports: &mut Vec<Report>,
) -> Result<FuncArgs, FrapError> {
    let params = def.params;
    let python_only = def.flags.contains(FuncFlags::PYTHON_ONLY);
    let npos = args.iter().filter(|a| a.name.is_none()).count();
    if args
        .iter()
        .skip_while(|a| a.name.is_none())
        .any(|a| a.name.is_none())
    {
        return Err(FrapError::new(
            PyExc::SyntaxError,
            frap_rna::ErrorCode("SYNTAX"),
            "positional argument follows keyword argument",
        ));
    }
    // A method defined in Python (`Mesh.from_pydata` in `_bpy_types.py`), not a `bpy_struct`
    // C method: CPython's own argument errors (WP 1.22, verified on 5.2.4).
    let cpython = python_only
        && !frap_spec_rna::BPY_STRUCT_FUNCS
            .iter()
            .any(|f| std::ptr::eq(f, def));
    let fname = if cpython {
        format!("{owner}.{}", def.identifier)
    } else if python_only {
        format!("bpy_struct.{}()", def.identifier)
    } else {
        format!("{owner}.{}", def.identifier)
    };
    if cpython {
        cpython_arity(&fname, params, args, npos)?;
    }
    if let Some(k) = def.keyword_only_from {
        let k = k as usize;
        if npos > k {
            return Err(type_err(format!(
                "{fname}() takes at most {k} positional argument{} ({npos} given)",
                if k == 1 { "" } else { "s" }
            )));
        }
    }
    if args.len() > params.len() {
        return Err(type_err(format!(
            "{fname}(): takes at most {} arguments, got {}",
            params.len(),
            args.len()
        )));
    }
    let mut values = IndexMap::new();
    let mut is_set = BitSet::new();
    let mut used = vec![false; args.len()];
    for (i, p) in params.iter().enumerate() {
        let found = if i < npos {
            // New in 2.8x (`pyrna_func_call`): optional parameters must be keywords.
            if !p.flags.contains(PropFlags::REQUIRED) && !python_only {
                return Err(type_err(format!(
                    "{fname}(): required parameter \"{}\" to be a keyword argument!",
                    p.identifier
                )));
            }
            used[i] = true;
            Some(&args[i])
        } else {
            args.iter().enumerate().find_map(|(j, a)| {
                (a.name.as_deref() == Some(p.identifier)).then(|| {
                    used[j] = true;
                    a
                })
            })
        };
        match found {
            Some(a) => {
                let input = a.value.to_input()?;
                // A `bpy_struct` C method parses its enum arguments with
                // `pyrna_enum_value_from_id` / `pyrna_enum_bitfield_from_set` (WP 6.4,
                // `keyframe_insert(keytype=…, options=…)`): `ValueError: <fname>: 'X' not found
                // in ('A', 'B')`.
                if python_only
                    && !cpython
                    && let PropKind::Enum { items, .. } = &p.kind
                {
                    let given: Vec<&str> = match &input {
                        Input::Str(s) => vec![s.as_str()],
                        Input::Set(v) | Input::List(v) | Input::Tuple(v) => v
                            .iter()
                            .filter_map(|x| match x {
                                Input::Str(s) => Some(s.as_str()),
                                _ => None,
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                    if let Some(bad) = given
                        .iter()
                        .find(|g| !items.iter().any(|it| it.identifier == **g))
                    {
                        let all: Vec<String> = items
                            .iter()
                            .map(|it| format!("'{}'", it.identifier))
                            .collect();
                        return Err(FrapError::new(
                            PyExc::ValueError,
                            RNA_ENUM,
                            format!("{fname}: '{bad}' not found in ({})", all.join(", ")),
                        )
                        .with_span(a.span));
                    }
                }
                // `pyrna_func_call` names keyword arguments by keyword; a Python method's
                // own checks have no prefix.
                let prefix = match (cpython, &a.name) {
                    (true, _) => String::new(),
                    (false, Some(_)) => format!(
                        "{fname}(): error with keyword argument \"{}\" -  ",
                        p.identifier
                    ),
                    (false, None) => format!(
                        "{fname}(): error with argument {}, \"{}\" -  ",
                        i + 1,
                        p.identifier
                    ),
                };
                let c = coerce(p, input.clone(), &CoerceCx::RnaSet { owner: "Function" }).map_err(
                    |e| {
                        e.into_frap(&prefix, |k| match k {
                            RnaErrorKind::Enum => RNA_ENUM,
                            RnaErrorKind::Length => RNA_LENGTH,
                            _ => frap_rna::codes::RNA_FUNC_ARGS,
                        })
                        .with_span(a.span)
                    },
                )?;
                if c.clamped {
                    reports.push(info(frap_rna::clamp_report(p, &input)));
                }
                values.insert(p.identifier, c.value);
                is_set.insert(i);
            }
            None if p.flags.contains(PropFlags::REQUIRED) => {
                return Err(type_err(if python_only {
                    format!(
                        "{fname}() missing required argument '{}' (pos {})",
                        p.identifier,
                        i + 1
                    )
                } else {
                    format!(
                        "{fname}(): required parameter \"{}\" not specified",
                        p.identifier
                    )
                }));
            }
            None => {
                values.insert(p.identifier, Value::default_for(p));
            }
        }
    }
    let bad: Vec<&str> = args
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .filter_map(|(a, _)| a.name.as_deref())
        .collect();
    if !bad.is_empty() && python_only {
        return Err(type_err(format!(
            "{fname}() got an unexpected keyword argument '{}'",
            bad[0]
        )));
    }
    if !bad.is_empty() {
        let expected: Vec<&str> = params.iter().map(|p| p.identifier).collect();
        let sugg: Vec<serde_json::Value> = bad
            .iter()
            .flat_map(|b| frap_op::suggest(b, expected.iter().copied()))
            .map(serde_json::Value::from)
            .collect();
        return Err(type_err(format!(
            "{fname}(): was called with invalid keyword argument(s) ({}), expected ({})",
            bad.join(", "),
            expected.join(", ")
        ))
        .with_detail("did_you_mean", sugg)
        .with_detail(
            "valid",
            expected
                .iter()
                .map(|e| serde_json::Value::from(*e))
                .collect::<Vec<_>>(),
        ));
    }
    Ok(FuncArgs {
        func: def,
        values,
        is_set,
    })
}

impl Session {
    /// Calls a function target; returns the value and the RNA definition of the return value.
    pub(crate) fn call_func(
        &mut self,
        f: FuncTarget,
        args: Vec<CallArg>,
    ) -> Result<(Py, Option<&'static PropDef>), FrapError> {
        match f {
            FuncTarget::Rna { this, def } => self.call_rna(this, def, args),
            FuncTarget::Coll { coll, which } => Ok((self.call_coll(coll, which, args)?, None)),
            FuncTarget::Struct { this, which } => Ok((self.call_struct(this, which, args)?, None)),
            FuncTarget::Math { recv, prop, name } => {
                let args: Vec<super::eval::ArgVal> = args
                    .into_iter()
                    .map(|a| super::eval::ArgVal {
                        name: a.name,
                        value: a.value,
                    })
                    .collect();
                Ok((
                    super::eval::method(super::eval::value_py(prop, recv), name, &args)?,
                    None,
                ))
            }
        }
    }

    fn call_rna(
        &mut self,
        this: RnaPtr,
        def: &'static FuncDef,
        args: Vec<CallArg>,
    ) -> Result<(Py, Option<&'static PropDef>), FrapError> {
        let owner = match (this.root, this.path.is_empty()) {
            (Root::Data, true) => "BlendData",
            (Root::Context, true) => "Context",
            _ => this.srna.identifier,
        };
        let mut reports = Vec::new();
        let mut fa = bind_args(owner, def, &args, &mut reports)?;
        crate::edit::check_func(&self.host, &this, def)?;
        self.host.reports.extend(reports);
        let imp = if def.flags.contains(FuncFlags::PYTHON_ONLY)
            && frap_spec_rna::BPY_STRUCT_FUNCS
                .iter()
                .any(|f| std::ptr::eq(f, def))
        {
            RNA_FUNCS
                .iter()
                .find(|r| r.srna == "bpy_struct" && r.func == def.identifier)
        } else {
            find_func(this.srna, def.identifier)
        };
        let Some(imp) = imp else {
            return Err(FrapError::new(
                PyExc::NotImplementedError,
                NOT_IMPLEMENTED,
                format!(
                    "frap: {owner}.{}() exists in Blender 5.2 but is not implemented by frap yet",
                    def.identifier
                ),
            ));
        };
        let v = (imp.imp)(&mut self.host, &this, &mut fa).map_err(|e| {
            e.into_frap("", |k| match k {
                RnaErrorKind::Other => frap_rna::codes::RNA_RUNTIME,
                k => k.default_code(),
            })
        })?;
        let ret = def.returns.first();
        let py = match v {
            Value::Id(Some(id)) => Py::Rna(Target::Struct(RnaPtr::id(
                id,
                resolve::id_srna(&self.host, id)?,
            ))),
            Value::Ptr(p) => Py::Rna(Target::Struct(*p)),
            Value::Coll(p) => Py::Rna(Target::Value(Value::Coll(p))),
            Value::Tuple(items) if def.returns.len() > 1 => Py::Tuple(
                items
                    .into_iter()
                    .zip(def.returns)
                    .map(|(v, p)| value_py(Some(p), v))
                    .collect(),
            ),
            v => value_py(ret, v),
        };
        Ok((py, ret))
    }

    // ------------------------------------------------------------------ bpy_prop_collection

    fn coll_owner(c: &CollPlace) -> String {
        let owner = match (c.ptr.root, c.ptr.path.is_empty()) {
            (Root::Data, true) => "BlendData",
            (Root::Context, true) => "Context",
            _ => c.ptr.srna.identifier,
        };
        format!("{owner}.{}", c.prop.identifier)
    }

    fn call_coll(
        &mut self,
        c: CollPlace,
        which: CollFunc,
        args: Vec<CallArg>,
    ) -> Result<Py, FrapError> {
        let host = &self.host;
        let (start, end) = match c.range {
            Some(r) => r,
            None => (0, with_coll(host, &c.ptr, c.prop, &mut |v| Ok(v.len()))?),
        };
        let item = |i: usize| -> Result<Py, FrapError> {
            Ok(
                match resolve::step(host, Target::Coll(c.clone()), &PathSeg::Index(i as i64))? {
                    Target::None => Py::None,
                    t => Py::Rna(t),
                },
            )
        };
        let positional: Vec<&Py> = args
            .iter()
            .filter(|a| a.name.is_none())
            .map(|a| &a.value)
            .collect();
        match which {
            CollFunc::Get | CollFunc::Find => {
                let name = which.name();
                if positional.is_empty() {
                    return Err(type_err(format!(
                        "{name}() takes at least 1 argument (0 given)"
                    )));
                }
                if which == CollFunc::Get && args.len() > 2 {
                    return Err(type_err(format!(
                        "get() takes at most 2 arguments ({} given)",
                        args.len()
                    )));
                }
                if which == CollFunc::Find && args.len() > 1 {
                    return Err(type_err(format!(
                        "bpy_prop_collection.find() takes exactly one argument ({} given)",
                        args.len()
                    )));
                }
                let key = match positional[0] {
                    Py::Str(s) => s.clone(),
                    Py::Bare(s) => s.to_string(),
                    // `get((name, library))`: the tuple key of linked IDs (§5.2).
                    Py::Tuple(t) if which == CollFunc::Get && t.len() == 2 => {
                        let name = match &t[0] {
                            Py::Str(s) => s.clone(),
                            other => {
                                return Err(type_err(format!(
                                    "bpy_prop_collection.get((name, lib)): name must be a string, not {}",
                                    other.type_name()
                                )));
                            }
                        };
                        let lib = match &t[1] {
                            Py::None => None,
                            Py::Str(s) => Some(s.clone()),
                            other => {
                                return Err(type_err(format!(
                                    "bpy_prop_collection.get((name, lib)): lib must be a string or None, not {}",
                                    other.type_name()
                                )));
                            }
                        };
                        return match resolve::step(
                            host,
                            Target::Coll(c.clone()),
                            &PathSeg::KeyLib(name, lib),
                        ) {
                            Ok(Target::None) => Ok(Py::None),
                            Ok(t) => Ok(Py::Rna(t)),
                            Err(e) if e.exception == PyExc::KeyError => {
                                Ok(positional.get(1).map_or(Py::None, |d| (*d).clone()))
                            }
                            Err(e) => Err(e),
                        };
                    }
                    other => {
                        return Err(type_err(format!(
                            "bpy_prop_collection.{name}(key, ...): key must be a string, not {}",
                            other.type_name()
                        )));
                    }
                };
                let (named, found) =
                    with_coll(host, &c.ptr, c.prop, &mut |v| Ok((v.named(), v.find(&key))))?;
                if which == CollFunc::Find {
                    return Ok(Py::Int(found.map_or(-1, |i| i as i64)));
                }
                if !named {
                    return Err(type_err(format!(
                        "bpy_prop_collection.get: {} does not support string lookups",
                        Self::coll_owner(&c)
                    )));
                }
                match found {
                    Some(i) => item(i),
                    None => Ok(positional.get(1).map_or(Py::None, |d| (*d).clone())),
                }
            }
            CollFunc::Keys | CollFunc::Values | CollFunc::Items => {
                if !args.is_empty() {
                    return Err(type_err(format!(
                        "bpy_prop_collection.{}() takes no arguments ({} given)",
                        which.name(),
                        args.len()
                    )));
                }
                let keys: Vec<Option<SmolStr>> = with_coll(host, &c.ptr, c.prop, &mut |v| {
                    Ok((start..end).map(|i| v.key_of(i)).collect())
                })?;
                Ok(Py::List(match which {
                    CollFunc::Keys => keys
                        .into_iter()
                        .flatten()
                        .map(|k| Py::Str(k.to_string()))
                        .collect(),
                    CollFunc::Values => (start..end).map(item).collect::<Result<_, _>>()?,
                    _ => keys
                        .into_iter()
                        .zip(start..end)
                        .map(|(k, i)| {
                            let key = k.map_or(Py::Int(i as i64), |k| Py::Str(k.to_string()));
                            Ok(Py::Tuple(vec![key, item(i)?]))
                        })
                        .collect::<Result<_, FrapError>>()?,
                }))
            }
            CollFunc::ForeachGet | CollFunc::ForeachSet => {
                self.foreach(c, which, (start, end), args)
            }
        }
    }

    fn foreach(
        &mut self,
        c: CollPlace,
        which: CollFunc,
        (start, end): (usize, usize),
        args: Vec<CallArg>,
    ) -> Result<Py, FrapError> {
        let set = which == CollFunc::ForeachSet;
        let positional: Vec<&Py> = args.iter().map(|a| &a.value).collect();
        // frap extension (D-09): foreach_get(attr) returns a bulk array.
        if positional.len() != 2 && (set || positional.len() != 1) {
            return Err(type_err(format!(
                "foreach_get/set() takes exactly 2 arguments ({} given)",
                positional.len()
            )));
        }
        let attr = match positional[0] {
            Py::Str(s) => s.clone(),
            Py::Bare(s) => s.to_string(),
            other => {
                return Err(type_err(format!(
                    "foreach_get/set() argument 1 must be str, not {}",
                    other.type_name()
                )));
            }
        };
        // An empty buffer returns None before the attribute is looked up (verified on 5.2.4:
        // `vertices.foreach_get('foo', [])` and `foreach_set('co', [])` are no-ops).
        if positional.len() == 2 && positional[1].items().is_some_and(|v| v.is_empty()) {
            return Ok(Py::None);
        }
        let host = &self.host;
        let item_type = with_coll(host, &c.ptr, c.prop, &mut |v| Ok(v.item_type()))?;
        let owner = match (c.ptr.root, c.ptr.path.is_empty()) {
            (Root::Data, true) => "BlendData",
            _ => c.ptr.srna.identifier,
        };
        let Some(prop) = item_type.find_prop(&attr) else {
            return Err(FrapError::new(
                PyExc::AttributeError,
                RNA_NO_ATTRIBUTE,
                format!(
                    "{}(..) '{owner}.{:>200}[...]' elements have no attribute '{attr}'",
                    which.name(),
                    c.prop.identifier
                ),
            ));
        };
        let k = prop.array_len();
        let needed = (end - start) * k;
        if !set {
            // Blender answers a property that has no bulk form (strings, pointers) with the
            // same error as a wrong buffer length.
            let bulk = with_coll(host, &c.ptr, c.prop, &mut |v| {
                v.foreach_get(prop, start..end).map_err(|e| {
                    if e.kind == RnaErrorKind::Type {
                        runtime_err("internal error setting the array")
                    } else {
                        FrapError::from(e)
                    }
                })
            })?;
            if positional.len() == 1 {
                return Ok(Py::Json(bulk_json(&bulk, prop, start, self.opts.max_array)));
            }
            let Some(mut seq) = positional[1].items() else {
                return Err(type_err("couldn't access the py sequence"));
            };
            if seq.len() < needed {
                return Err(runtime_err("internal error setting the array"));
            }
            let vals: Vec<Py> = match &bulk {
                Bulk::F32(v) => v.iter().map(|x| Py::Float(*x as f64)).collect(),
                Bulk::I32(v) => v.iter().map(|x| Py::Int(*x as i64)).collect(),
                Bulk::Bool(v) => v.iter().map(|x| Py::Bool(*x)).collect(),
                _ => vec![],
            };
            for (dst, src) in seq.iter_mut().zip(vals) {
                *dst = src;
            }
            return Ok(Py::List(seq));
        }
        if prop.flags.contains(PropFlags::READONLY) {
            // Blender ignores writes to a read-only property here (`foreach_set('index', …)`
            // returns None, verified); frap says so.
            self.host.reports.push(Report::new(
                frap_rna::ReportType::Warning,
                format!(
                    "frap: foreach_set: {}.{} is read-only; nothing was written",
                    item_type.identifier, prop.identifier
                ),
            ));
            return Ok(Py::None);
        }
        let Some(seq) = positional[1].items() else {
            return Err(type_err("couldn't access the py sequence"));
        };
        if seq.len() != needed {
            return Err(runtime_err("internal error setting the array"));
        }
        let bulk = match prop.kind {
            PropKind::Float { .. } => Bulk::F32(
                seq.iter()
                    .map(|x| x.number().map(|f| f as f32))
                    .collect::<Option<_>>()
                    .ok_or_else(|| type_err("expected a sequence of numbers"))?,
            ),
            PropKind::Int { .. } => Bulk::I32(
                seq.iter()
                    .map(|x| x.number().map(|f| f as i32))
                    .collect::<Option<_>>()
                    .ok_or_else(|| type_err("expected a sequence of numbers"))?,
            ),
            PropKind::Bool { .. } => Bulk::Bool(
                seq.iter()
                    .map(|x| x.number().map(|f| f != 0.0))
                    .collect::<Option<_>>()
                    .ok_or_else(|| type_err("expected a sequence of booleans"))?,
            ),
            _ => {
                return Err(type_err(
                    "foreach_get/foreach_set: property type does not match the buffer type",
                ));
            }
        };
        let whole_rna = c.range.is_none() && !(c.ptr.root == Root::Data && c.ptr.path.is_empty());
        if whole_rna {
            let r = resolve::with_coll_mut(&mut self.host, &c.ptr, c.prop, &mut |cm| {
                cm.foreach_set(prop, &bulk).map_err(FrapError::from)
            });
            match r {
                Ok(()) => return Ok(Py::None),
                // A hook-served collection has no mutable binding: fall back to item writes.
                Err(e)
                    if ![
                        frap_rna::codes::RNA_NOT_IMPLEMENTED,
                        frap_rna::codes::RNA_NO_ATTRIBUTE,
                        NOT_IMPLEMENTED,
                    ]
                    .contains(&e.code) =>
                {
                    return Err(e);
                }
                Err(_) => {}
            }
        }
        // Item by item (slices, `bpy.data` collections, hook collections).
        for (n, i) in (start..end).enumerate() {
            let t = resolve::step(
                &self.host,
                Target::Coll(c.clone()),
                &PathSeg::Index(i as i64),
            )?;
            let Target::Struct(ptr) = t else {
                return Err(resolve::stale("foreach_set on a non-struct item"));
            };
            let v = bulk_item(&bulk, prop, n);
            resolve::set_prop(&mut self.host, &ptr, prop, v)?;
        }
        Ok(Py::None)
    }

    // ------------------------------------------------------------------ bpy_struct

    fn call_struct(
        &mut self,
        this: RnaPtr,
        which: StructFunc,
        args: Vec<CallArg>,
    ) -> Result<Py, FrapError> {
        let srna = this.srna.identifier;
        let arg = |i: usize, name: &str| {
            args.iter()
                .find(|a| a.name.as_deref() == Some(name))
                .or_else(|| args.iter().filter(|a| a.name.is_none()).nth(i))
                .map(|a| a.value.clone())
        };
        let idprops = || {
            resolve::idprops::owner(&this)
                .map(|id| resolve::idprops::all(&self.host, id))
                .unwrap_or_default()
        };
        match which {
            StructFunc::Keys => Ok(Py::List(
                idprops().into_iter().map(|(k, _)| Py::Str(k)).collect(),
            )),
            StructFunc::Values => Ok(Py::List(
                idprops()
                    .into_iter()
                    .map(|(_, v)| super::eval::value_py(None, v))
                    .collect(),
            )),
            StructFunc::Items => Ok(Py::List(
                idprops()
                    .into_iter()
                    .map(|(k, v)| Py::Tuple(vec![Py::Str(k), super::eval::value_py(None, v)]))
                    .collect(),
            )),
            StructFunc::Get => {
                let key = match arg(0, "key") {
                    Some(Py::Str(k)) => k,
                    Some(Py::Bare(k)) => k.to_string(),
                    Some(other) => {
                        return Err(type_err(format!(
                            "{srna}.get(): argument 1 must be str, not {}",
                            other.type_name()
                        )));
                    }
                    None => {
                        return Err(type_err(
                            "get() takes at least 1 argument (0 given)".to_owned(),
                        ));
                    }
                };
                let default = arg(1, "default").unwrap_or(Py::None);
                Ok(idprops()
                    .into_iter()
                    .find(|(k, _)| *k == key)
                    .map_or(default, |(_, v)| super::eval::value_py(None, v)))
            }
            StructFunc::PathFromId => {
                let prop = match arg(0, "property") {
                    None => String::new(),
                    Some(Py::Str(s)) => s,
                    Some(other) => {
                        return Err(type_err(format!(
                            "{srna}.path_from_id(): argument 1 must be str, not {}",
                            other.type_name()
                        )));
                    }
                };
                let base = this.path_from_id().unwrap_or_default();
                if !prop.is_empty() && this.srna.find_prop(&prop).is_none() {
                    return Err(FrapError::new(
                        PyExc::AttributeError,
                        RNA_NO_ATTRIBUTE,
                        format!("{srna}.path_from_id(\"{prop}\") not found"),
                    ));
                }
                if base.is_empty() && prop.is_empty() {
                    return Err(FrapError::new(
                        PyExc::ValueError,
                        RNA_RUNTIME_CODE,
                        format!(
                            "{srna}.path_from_id() does not support path creation for this type"
                        ),
                    ));
                }
                Ok(Py::Str(match (base.is_empty(), prop.is_empty()) {
                    (true, _) => prop,
                    (false, true) => base,
                    (false, false) => format!("{base}.{prop}"),
                }))
            }
            StructFunc::PathResolve => {
                let Some(Py::Str(path)) = arg(0, "path") else {
                    return Err(type_err(format!(
                        "{srna}.path_resolve(): argument 1 must be str"
                    )));
                };
                let not_found = || {
                    FrapError::new(
                        PyExc::ValueError,
                        RNA_NO_ATTRIBUTE,
                        format!("{srna}.path_resolve(\"{path}\") could not be resolved"),
                    )
                };
                let sep = if path.starts_with('[') { "" } else { "." };
                let src = format!("{}{sep}{path}", this.repr(&self.host.state.main));
                let stmts = frap_lang::parse_complete(&src);
                let [
                    Ok(frap_lang::Stmt {
                        cmd: Command::Get { target },
                        ..
                    }),
                ] = stmts.as_slice()
                else {
                    return Err(not_found());
                };
                if crate::session::exec::path_has_call(target) {
                    return Err(not_found());
                }
                // Every resolution failure, also an element index checked on read
                // (`location[9]`), is Blender's ValueError (verified on 5.2.4).
                let t = resolve::resolve(&self.host, target).map_err(|_| not_found())?;
                super::eval::target_py(&self.host, t).map_err(|_| not_found())
            }
        }
    }
}

/// `RuntimeError`-class code of a `ValueError` raised by a `bpy_struct` method.
const RNA_RUNTIME_CODE: frap_rna::ErrorCode = frap_rna::codes::RNA_RUNTIME;

fn bulk_item(b: &Bulk, prop: &PropDef, i: usize) -> Value {
    let n = prop.array_len();
    let scalar = matches!(prop.array, frap_rna::ArrayShape::Scalar);
    let r = i * n..i * n + n;
    match b {
        Bulk::F32(v) if scalar => Value::Float(v[r.start]),
        Bulk::F32(v) => Value::FloatArray {
            data: v[r].iter().copied().collect(),
            shape: prop.array,
            tag: frap_rna::MathTag::for_subtype(prop.subtype),
        },
        Bulk::I32(v) if scalar => Value::Int(v[r.start] as i64),
        Bulk::I32(v) => Value::IntArray(v[r].iter().map(|x| *x as i64).collect()),
        Bulk::Bool(v) if scalar => Value::Bool(v[r.start]),
        Bulk::Bool(v) => Value::BoolArray(v[r].iter().copied().collect()),
        _ => Value::None,
    }
}
