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

/// CPython's `PyArg_ParseTupleAndKeywords` checks for `pyrna_struct_keyframe_parse`
/// (blender/source/blender/python/intern/bpy_rna_anim.cc, Blender 5.2.4: format
/// `s|$iO&sO!s`, keywords `data_path, index, frame, group, options, keytype`), with CPython
/// 3.13's messages (verified on 5.2.4): `takes at most 6 arguments (7 given)`, `argument 1
/// must be str, not None`, `'float' object cannot be interpreted as an integer`, `must be real
/// number, not str`, `argument 5 must be set, not list`.
fn keyframe_parse_check(
    fname: &str,
    params: &[PropDef],
    args: &[CallArg],
    npos: usize,
) -> Result<(), FrapError> {
    const KWLIST: [&str; 6] = ["data_path", "index", "frame", "group", "options", "keytype"];
    if args.len() > KWLIST.len() {
        return Err(type_err(format!(
            "{fname}() takes at most {} arguments ({} given)",
            KWLIST.len(),
            args.len()
        )));
    }
    if npos > 1 {
        return Err(type_err(format!(
            "{fname}() takes at most 1 positional argument ({npos} given)"
        )));
    }
    let tn = |i: &Input| match i {
        Input::None => "None",
        other => other.py_type_name(),
    };
    for (n, name) in KWLIST.iter().enumerate() {
        let found = if n < npos {
            args.get(n)
        } else {
            args.iter().find(|a| a.name.as_deref() == Some(*name))
        };
        let Some(a) = found else { continue };
        if !params.iter().any(|p| p.identifier == *name) {
            continue;
        }
        let input = a.value.to_input()?;
        let err = match (*name, &input) {
            ("data_path" | "group" | "keytype", Input::Str(_) | Input::Bare(_)) => None,
            ("data_path" | "group" | "keytype", other) => Some(type_err(format!(
                "{fname}() argument {} must be str, not {}",
                n + 1,
                tn(other)
            ))),
            ("index", Input::Int(_) | Input::Bool(_)) => None,
            ("index", other) => Some(type_err(format!(
                "'{}' object cannot be interpreted as an integer",
                tn(other)
            ))),
            ("frame", Input::None | Input::Int(_) | Input::Float(_) | Input::Bool(_)) => None,
            ("frame", other) => Some(type_err(format!("must be real number, not {}", tn(other)))),
            ("options", Input::Set(_)) => None,
            ("options", other) => Some(type_err(format!(
                "{fname}() argument {} must be set, not {}",
                n + 1,
                tn(other)
            ))),
            _ => None,
        };
        if let Some(e) = err {
            return Err(e.with_span(a.span));
        }
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
    // `bpy_struct.keyframe_insert/keyframe_delete` parse with `PyArg_ParseTupleAndKeywords`
    // (`pyrna_struct_keyframe_parse`, format `s|$iO&sO!s`): CPython's messages (WP 6.4).
    let keyframe_c =
        python_only && !cpython && matches!(def.identifier, "keyframe_insert" | "keyframe_delete");
    if keyframe_c {
        keyframe_parse_check(&fname, params, args, npos)?;
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
                let mut input = a.value.to_input()?;
                if keyframe_c {
                    match (p.identifier, &input) {
                        // `PyC_ParseOptionalFloat`: `None` is the current frame (unset).
                        ("frame", Input::None) => {
                            values.insert(p.identifier, Value::default_for(p));
                            continue;
                        }
                        // A non-finite frame is kept as given (no clamping).
                        ("frame", Input::Float(f)) if !f.is_finite() => {
                            values.insert(p.identifier, Value::Float(*f as f32));
                            is_set.insert(i);
                            continue;
                        }
                        ("frame" | "index", Input::Bool(b)) => input = Input::Int(i64::from(*b)),
                        _ => {}
                    }
                }
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
        self.host.tag_animation_call(&this, def.identifier);
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

    /// `bpy_prop_collection_idprop.add/remove/clear/move` (`bpy_rna.cc`,
    /// `pyrna_prop_collection_idprop_*`; messages verified on 5.2.4). The collection's set
    /// hook receives `("add",)`, `("remove", i)`, `("clear",)` or `("move", a, b)` (the
    /// owner's hook stores the items: the glTF add-on's variant groups, M7 WP 7.5); `add()`
    /// returns the appended item.
    fn call_idprop(
        &mut self,
        c: CollPlace,
        which: CollFunc,
        args: Vec<CallArg>,
    ) -> Result<Py, FrapError> {
        let name = which.name();
        let qual = format!("bpy_prop_collection_idprop.{name}()");
        if args.iter().any(|a| a.name.is_some()) {
            return Err(type_err(format!("{qual} takes no keyword arguments")));
        }
        let n = args.len();
        // `PyLong_AsLong` / `PyArg_ParseTuple("i")`: ints and bools only.
        let int = |p: &Py| match p {
            Py::Int(i) => Some(*i),
            Py::Bool(b) => Some(i64::from(*b)),
            _ => None,
        };
        let mut op = vec![Value::Str(SmolStr::new_static(name))];
        match which {
            CollFunc::Add | CollFunc::Clear if n > 0 => {
                return Err(type_err(format!("{qual} takes no arguments ({n} given)")));
            }
            CollFunc::Remove => {
                if n != 1 {
                    return Err(type_err(format!(
                        "{qual} takes exactly one argument ({n} given)"
                    )));
                }
                let k = int(&args[0].value).ok_or_else(|| {
                    type_err("bpy_prop_collection.remove(): expected one int argument")
                })?;
                op.push(Value::Int(k));
            }
            CollFunc::Move => {
                let bad = || type_err("bpy_prop_collection.move(): expected two ints as arguments");
                if n != 2 {
                    return Err(bad());
                }
                for a in &args {
                    let k = int(&a.value)
                        .filter(|k| i32::try_from(*k).is_ok())
                        .ok_or_else(bad)?;
                    op.push(Value::Int(k));
                }
            }
            _ => {}
        }
        resolve::set_prop(&mut self.host, &c.ptr, c.prop, Value::Tuple(op))?;
        if which != CollFunc::Add {
            return Ok(Py::None);
        }
        let len = with_coll(&self.host, &c.ptr, c.prop, &mut |v| Ok(v.len()))?;
        let last = len.checked_sub(1).ok_or_else(|| {
            type_err("bpy_prop_collection.add(): not supported for this collection")
        })?;
        Ok(
            match resolve::step(&self.host, Target::Coll(c), &PathSeg::Index(last as i64))? {
                Target::None => Py::None,
                t => Py::Rna(t),
            },
        )
    }

    fn call_coll(
        &mut self,
        c: CollPlace,
        which: CollFunc,
        args: Vec<CallArg>,
    ) -> Result<Py, FrapError> {
        if matches!(
            which,
            CollFunc::Add | CollFunc::Remove | CollFunc::Clear | CollFunc::Move
        ) {
            return self.call_idprop(c, which, args);
        }
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
                if which == CollFunc::Find {
                    // `pyrna_prop_collection_find`: the first item whose name property equals
                    // the key, enabled or not (Mix `data_type='RGBA'`: `inputs.find('A')` is the
                    // disabled `A_Float`, index 2, where `inputs['A']` is `A_Color`). ID
                    // collections that name no items through `key_of` are searched by `find`.
                    let found = with_coll(host, &c.ptr, c.prop, &mut |v| {
                        let first =
                            (0..v.len()).find(|&i| v.key_of(i).is_some_and(|k| k.as_str() == key));
                        Ok(first.or_else(|| v.find(&key)))
                    })?;
                    return Ok(Py::Int(found.map_or(-1, |i| i as i64)));
                }
                let (named, found) =
                    with_coll(host, &c.ptr, c.prop, &mut |v| Ok((v.named(), v.find(&key))))?;
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
            CollFunc::Add | CollFunc::Remove | CollFunc::Clear | CollFunc::Move => {
                unreachable!("handled by call_idprop")
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
        // Blender's `foreach_get/set` take positional arguments only (verified on 5.2.4:
        // `foreach_get(attr='co', seq=…)` and `foreach_get('co', seq=…)` raise).
        if args.iter().any(|a| a.name.is_some()) {
            return Err(type_err(format!(
                "{}() takes no keyword arguments",
                which.name()
            )));
        }
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
        // frap answers a two-argument `foreach_get` with the buffer: an empty list stays `[]`.
        if positional.len() == 2 && positional[1].items().is_some_and(|v| v.is_empty()) {
            return Ok(match positional[1] {
                Py::List(_) if !set => Py::List(Vec::new()),
                _ => Py::None,
            });
        }
        // A buffer for an empty collection raises before the attribute is looked up
        // (verified on 5.2.4: a new mesh's `vertices.foreach_get('co', [0.0] * 3)` and
        // `foreach_set('foo', [0])`, the `groups.foreach_get('weight', [0.0])` of a vertex in
        // no group).
        if positional.len() == 2
            && end == start
            && let Some(n) = positional[1].items().map(|v| v.len())
        {
            return Err(type_err(format!(
                "{}(..) sequence length mismatch given {n}, needed 0",
                which.name()
            )));
        }
        let host = &self.host;
        let item_type = with_coll(host, &c.ptr, c.prop, &mut |v| Ok(v.item_type()))?;
        let data_root = c.ptr.root == Root::Data && c.ptr.path.is_empty();
        let owner = if data_root {
            "BlendData"
        } else {
            c.ptr.srna.identifier
        };
        // The items of an ID-pointer collection, which may be empty slots (`mesh.materials`
        // after `append(None)`): Blender skips them, they take no buffer values (verified on
        // 5.2.4: `[Material, None]` takes 1 roughness, `[Material, None, M2]` 2; a collection
        // whose first item is empty crashes Blender, frap skips it too, D-176).
        // Only ID-pointer arrays (`IDMaterials`) hold empty slots; the ID lists of
        // `scene.objects` and the like are read item by item below, in one pass (M8S10-5).
        let slotted = matches!(
            c.prop.kind,
            PropKind::Collection {
                api: Some("IDMaterials"),
                ..
            }
        );
        let slots: Option<Vec<Option<RnaPtr>>> = if slotted {
            Some(resolve::coll_items(&self.host, &c, (start, end))?)
        } else {
            None
        };
        let present = slots
            .as_ref()
            .map_or(end - start, |s| s.iter().flatten().count());
        let first_item = |this: &Self| -> Result<Option<RnaPtr>, FrapError> {
            match &slots {
                Some(s) => Ok(s.iter().flatten().next().cloned()),
                None if end > start => this.foreach_item(&c, 0),
                None => Ok(None),
            }
        };
        // Blender looks the property up on the first item's own type (verified on 5.2.4:
        // `lights.foreach_get('energy', …)`, `modifiers.foreach_get('levels', …)` with a
        // SUBSURF modifier first); every item must then have it.
        let first_type = if item_type.find_prop(&attr).is_none() {
            first_item(self)?.map(|p| p.srna)
        } else {
            None
        };
        let refined = first_type.is_some_and(|t| t.find_prop(&attr).is_some());
        let Some(prop) = item_type
            .find_prop(&attr)
            .or_else(|| first_type.and_then(|t| t.find_prop(&attr)))
        else {
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
        // A variable-length array (`MeshPolygon.vertices`): each face holds `loop_total`
        // values, and a buffer must hold their sum (verified on 5.2.4: the cube's 24 values,
        // a 6-sided cone's 24 in 7 faces; fewer raise, more are accepted).
        let lens: Option<Vec<usize>> = match item_type.find_prop("loop_total") {
            Some(total) if resolve::variable_length(prop) => {
                match with_coll(host, &c.ptr, c.prop, &mut |v| {
                    v.foreach_get(total, start..end).map_err(FrapError::from)
                })? {
                    Bulk::I32(t) => Some(t.iter().map(|&n| n.max(0) as usize).collect()),
                    _ => None,
                }
            }
            _ => None,
        };
        let needed = lens.as_ref().map_or(present * k, |l| l.iter().sum());
        // Blender's raw-array fast path takes exactly `needed` values; the item-by-item path
        // takes at least `needed` (and `foreach_set` writes the first ones).
        let raw = raw_array_access(c.ptr.srna.identifier, c.prop.identifier, prop.identifier);
        let fits = |n: usize| if raw { n == needed } else { n >= needed };
        let is_enum = matches!(prop.kind, PropKind::Enum { .. });
        if !set {
            let bulk =
                self.foreach_read(&c, prop, (start, end), is_enum || refined, slots.as_deref())?;
            if positional.len() == 1 {
                // A variable-length array (`polygons.foreach_get('vertices')`) answers flat,
                // paged by whole faces.
                let mut j = bulk_json(&bulk, prop, start, self.opts.max_array, lens.as_deref());
                if let Some(s) = &slots {
                    slot_range(&mut j, s, start);
                }
                return Ok(Py::Json(j));
            }
            // A string is a sequence of its characters: Blender checks its size first
            // (verified on 5.2.4: `foreach_get('co', 'abc')` raises the size error).
            let n = match positional[1] {
                Py::Str(t) => t.chars().count(),
                other => match other.items() {
                    Some(v) => v.len(),
                    None => return Err(type_err("couldn't access the py sequence")),
                },
            };
            if !fits(n) {
                return Err(runtime_err("internal error setting the array"));
            }
            // The values are written into the buffer, which must be mutable (verified on
            // 5.2.4: a tuple or a string of the right size raises; a short one raises the
            // size error).
            let Py::List(seq) = positional[1] else {
                return Err(type_err("couldn't access the py sequence"));
            };
            let mut seq = seq.clone();
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
        let Some(seq) = positional[1].items() else {
            return Err(type_err("couldn't access the py sequence"));
        };
        // Every item is converted before the size or the read-only flag is looked at
        // (verified on 5.2.4: `foreach_set('co', [[1, 1, 1]] * 8)`, `('vertices', [..., 1.5])`,
        // `('select', [2] * 8)` and the read-only `('index', [0.5] * 30)` raise TypeError).
        let int_range = raw_int_range(c.ptr.srna.identifier, c.prop.identifier, prop.identifier);
        let mut bulk = foreach_buffer(prop, &seq, int_range)?;
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
        // A longer buffer is accepted and its first `needed` values are written, except by
        // Blender's raw-array fast path (verified on 5.2.4: the cube's
        // `polygons.foreach_set('vertices', <25 values>)`, `vertices.foreach_set('co',
        // <27 values>)` are accepted; `edges.foreach_set('vertices', <25 values>)` and
        // `attributes['position'].data.foreach_set('vector', <25 values>)` raise).
        if !fits(seq.len()) {
            return Err(runtime_err("internal error setting the array"));
        }
        match &mut bulk {
            Bulk::F32(v) => v.truncate(needed),
            Bulk::I32(v) => v.truncate(needed),
            Bulk::Bool(v) => v.truncate(needed),
            _ => {}
        }
        // Enum values are numbers here: every one must name an item.
        let enum_vals: Option<Vec<Value>> = match (&prop.kind, &bulk) {
            (PropKind::Enum { items, .. }, Bulk::I32(v)) => Some(
                v.iter()
                    .map(|&x| enum_from_int(prop, items, x))
                    .collect::<Option<_>>()
                    .ok_or_else(|| runtime_err("internal error setting the array"))?,
            ),
            _ => None,
        };
        // A hook-served collection (`Object.vertex_groups`) has no mutable binding.
        let hooked =
            frap_scene::find_hook(c.ptr.srna, c.prop.identifier).is_some_and(|h| h.get.is_some());
        let whole_rna = c.range.is_none()
            && enum_vals.is_none()
            && !refined
            && !hooked
            && slots.is_none()
            && !data_root;
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
        // Item by item (slices, `bpy.data` collections, hook collections, enums, items of
        // several types). Indexed from the slice's own start (`polygons[1:3][0]` is face 1;
        // `start` is 0 without a slice). Every item must have the property: Blender raises the
        // size error at the first one without it (after writing the ones before, which frap
        // does not, D-173).
        let mut items = Vec::with_capacity(present);
        let ptrs = match slots {
            Some(s) => s,
            None => self.foreach_items(&c, (start, end))?,
        };
        for ptr in ptrs {
            let Some(ptr) = ptr else {
                continue;
            };
            let p = if refined {
                ptr.srna
                    .find_prop(prop.identifier)
                    .ok_or_else(|| runtime_err("internal error setting the array"))?
            } else {
                prop
            };
            items.push((ptr, p));
        }
        let mut at = 0;
        for (n, (ptr, prop)) in items.into_iter().enumerate() {
            let v = match &enum_vals {
                Some(vals) => vals[n].clone(),
                None => {
                    let len = lens.as_ref().map_or(k, |l| l[n]);
                    let v = bulk_item(&bulk, prop, at..at + len);
                    at += len;
                    v
                }
            };
            resolve::set_prop(&mut self.host, &ptr, prop, v)?;
        }
        Ok(Py::None)
    }

    /// Item `n` of a collection or slice (`polygons[1:3]`: `n` counts from the slice start);
    /// `None` for an empty slot of an ID-pointer collection (`mesh.materials`).
    fn foreach_item(&self, c: &CollPlace, n: usize) -> Result<Option<RnaPtr>, FrapError> {
        match resolve::step(
            &self.host,
            Target::Coll(c.clone()),
            &PathSeg::Index(n as i64),
        )? {
            Target::Struct(ptr) => Ok(Some(ptr)),
            Target::None => Ok(None),
            _ => Err(resolve::stale("foreach on a non-struct item")),
        }
    }

    /// Items `start..end` (absolute) of a collection or slice, resolved in one pass (a
    /// hook-served list is built once, M8S10-5). An empty item is an error here: only
    /// `IDMaterials` holds empty slots, and its callers pass them as `slots`.
    fn foreach_items(
        &self,
        c: &CollPlace,
        range: (usize, usize),
    ) -> Result<Vec<Option<RnaPtr>>, FrapError> {
        let items = resolve::coll_items(&self.host, c, range)?;
        if items.iter().any(Option::is_none) {
            return Err(resolve::stale("foreach on an empty item"));
        }
        Ok(items)
    }

    /// The values of `prop` over items `start..end` (`foreach_get`). The collection's bulk
    /// read answers when it can; ID items (`scene.objects`, `view_layer.objects`,
    /// `collection.all_objects`), hook-served properties and enums (as their item numbers,
    /// `material_slots.foreach_get('link', …)`) are read item by item (verified on 5.2.4).
    /// `item_by_item` skips the bulk read (enums, items of several types, each read through
    /// its own type's property of that name). `slots` are the items of an ID-pointer
    /// collection (empty slots are skipped).
    fn foreach_read(
        &self,
        c: &CollPlace,
        prop: &'static PropDef,
        (start, end): (usize, usize),
        item_by_item: bool,
        slots: Option<&[Option<RnaPtr>]>,
    ) -> Result<Bulk, FrapError> {
        let host = &self.host;
        // Blender answers a property that has no bulk form (strings, pointers) with the
        // same error as a wrong buffer length.
        let no_bulk = || runtime_err("internal error setting the array");
        if !item_by_item && slots.is_none() {
            let r = with_coll(host, &c.ptr, c.prop, &mut |v| {
                v.foreach_get(prop, start..end).map_err(FrapError::from)
            });
            match r {
                Ok(b) => return Ok(b),
                // A property the items' binding does not serve (`pose.bones` 'ik_stretch'):
                // read item by item, which says it is unbound.
                Err(e)
                    if e.exception == PyExc::TypeError
                        || e.code == frap_rna::codes::RNA_NO_ATTRIBUTE => {}
                Err(e) => return Err(e),
            }
        }
        let mut out = match &prop.kind {
            PropKind::Float { .. } => Bulk::F32(Vec::new()),
            PropKind::Int { .. } | PropKind::Enum { .. } => Bulk::I32(Vec::new()),
            PropKind::Bool { .. } => Bulk::Bool(Vec::new()),
            _ => return Err(no_bulk()),
        };
        let ptrs = match slots {
            Some(s) => s.to_vec(),
            None => self.foreach_items(c, (start, end))?,
        };
        for ptr in ptrs {
            let Some(ptr) = ptr else {
                continue;
            };
            // Items of several types (`modifiers`): each one's own property of that name.
            let prop = if item_by_item {
                ptr.srna.find_prop(prop.identifier).ok_or_else(no_bulk)?
            } else {
                prop
            };
            let v = resolve::with_struct(host, &ptr, &mut |s| match resolve::get_field(
                host, s, &ptr, prop,
            )? {
                frap_rna::Field::Value(v) => Ok(v),
                _ => Err(no_bulk()),
            })?;
            if !push_bulk(&mut out, prop, &v) {
                return Err(no_bulk());
            }
        }
        Ok(out)
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

/// The value of one item whose scalars are `b[r]` (one scalar for a non-array property).
fn bulk_item(b: &Bulk, prop: &PropDef, r: std::ops::Range<usize>) -> Value {
    let scalar = matches!(prop.array, frap_rna::ArrayShape::Scalar);
    match b {
        Bulk::F32(v) if scalar => Value::Float(v[r.start]),
        Bulk::F32(v) => {
            let mut data: smallvec::SmallVec<[f32; 16]> = v[r].iter().copied().collect();
            prop.reorder_raw(&mut data, false);
            Value::FloatArray {
                data,
                shape: prop.array,
                tag: frap_rna::MathTag::for_subtype(prop.subtype),
            }
        }
        Bulk::I32(v) if scalar => Value::Int(v[r.start] as i64),
        Bulk::I32(v) => Value::IntArray(v[r].iter().map(|x| *x as i64).collect()),
        Bulk::Bool(v) if scalar => Value::Bool(v[r.start]),
        Bulk::Bool(v) => Value::BoolArray(v[r].iter().copied().collect()),
        _ => Value::None,
    }
}

/// `(collection owner, collection, item property)` that Blender 5.2.4 reads and writes through
/// its raw-array fast path (`rna_raw_access` → `RNA_property_collection_raw_array`: a DNA array
/// iterated in place, a property without its own getter). There a `foreach_get` /
/// `foreach_set` buffer must hold exactly the values, while the item-by-item path takes a
/// longer one. Each entry was verified on 5.2.4 by a buffer one value too long (raises
/// `RuntimeError: internal error setting the array` and writes nothing), enum properties
/// included (DNA `char` fields: keyframe `interpolation` / `easing` / `type` / handle types,
/// curve-map and Bézier point handle types; `CurveProfile` points' handle types take the
/// longer buffer); the other properties
/// of these collections (`edges.foreach_set('select', …)`, `polygons…('material_index', …)`,
/// `color_attributes[…].data…('color_srgb', …)`, `ByteColorAttribute` data, `ShapeKey.data`,
/// `MeshUVLoopLayer.data`, `MeshLoopColorLayer.data`) and ID collections take the longer one.
const RAW_ARRAY: &[(&str, &str, &str)] = &[
    ("Mesh", "edges", "vertices"),
    ("Mesh", "polygons", "loop_start"),
    ("Mesh", "loop_triangles", "loops"),
    ("Mesh", "loop_triangle_polygons", "value"),
    ("Mesh", "vertex_normals", "vector"),
    ("Mesh", "polygon_normals", "vector"),
    ("Mesh", "corner_normals", "vector"),
    ("MeshVertex", "groups", "group"),
    ("MeshVertex", "groups", "weight"),
    ("MeshUVLoopLayer", "uv", "vector"),
    ("MeshUVLoopLayer", "pin", "value"),
    ("MeshSkinVertexLayer", "data", "radius"),
    ("ShapeKey", "points", "co"),
    ("FloatAttribute", "data", "value"),
    ("IntAttribute", "data", "value"),
    ("Int2Attribute", "data", "value"),
    ("Short2Attribute", "data", "value"),
    ("ByteIntAttribute", "data", "value"),
    ("BoolAttribute", "data", "value"),
    ("QuaternionAttribute", "data", "value"),
    ("Float4x4Attribute", "data", "value"),
    ("Float2Attribute", "data", "vector"),
    ("FloatVectorAttribute", "data", "vector"),
    ("FloatColorAttribute", "data", "color"),
    ("FCurve", "keyframe_points", "amplitude"),
    ("FCurve", "keyframe_points", "back"),
    ("FCurve", "keyframe_points", "period"),
    ("FCurve", "keyframe_points", "interpolation"),
    ("FCurve", "keyframe_points", "easing"),
    ("FCurve", "keyframe_points", "type"),
    ("FCurve", "keyframe_points", "handle_left_type"),
    ("FCurve", "keyframe_points", "handle_right_type"),
    ("CurveMap", "points", "handle_type"),
    ("Spline", "bezier_points", "handle_left_type"),
    ("Spline", "bezier_points", "handle_right_type"),
    ("CurveMap", "points", "location"),
    ("CurveProfile", "points", "location"),
];

/// The integer range of a raw array narrower than 32 bits (`int8_t` for INT8 attribute data,
/// `int16_t` for INT16_2D): Blender converts each item to that type and refuses one out of
/// its range with `TypeError: couldn't access the py sequence` (verified on 5.2.4:
/// `foreach_set('value', [128] * 8)` on an INT8 attribute, `[32768] * 16` on an INT16_2D
/// one raise; `127`, `-128`, `32767` are written).
fn raw_int_range(owner: &str, coll: &str, prop: &str) -> Option<(i64, i64)> {
    match (owner, coll, prop) {
        ("ByteIntAttribute", "data", "value") => Some((i8::MIN.into(), i8::MAX.into())),
        ("Short2Attribute", "data", "value") => Some((i16::MIN.into(), i16::MAX.into())),
        _ => None,
    }
}

/// Whether `owner.coll.foreach_*(prop, …)` goes through Blender's raw-array fast path
/// ([`RAW_ARRAY`]).
fn raw_array_access(owner: &str, coll: &str, prop: &str) -> bool {
    RAW_ARRAY.contains(&(owner, coll, prop))
}

/// The one-argument `foreach_get` answer over an ID-pointer collection with empty slots
/// (`mesh.materials` after `append(None)`): empty slots take no values, so `range` counts
/// slots, not values (its end is the slot after the last one the page covered, empty slots
/// included, so the next page starts there), and `slots` lists the slot of each value item
/// (M8S10-2). `slots` are the items `start..`. Every `IDMaterials` answer carries `slots`.
fn slot_range(j: &mut serde_json::Value, slots: &[Option<RnaPtr>], start: usize) {
    let filled: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_some())
        .map(|(n, _)| start + n)
        .collect();
    let kept = j["range"][1].as_u64().unwrap_or(0) as usize - start;
    let truncated = j["truncated"].as_bool().unwrap_or(false);
    let end = match kept {
        n if truncated && n > 0 => filled[n - 1] + 1,
        _ => start + slots.len(),
    };
    j["range"] = serde_json::json!([start, end]);
    j["slots"] = serde_json::json!(filled[..kept.min(filled.len())]);
}

/// A `foreach_set` buffer as `prop`'s scalars. Blender converts every item first and answers
/// an item of the wrong kind with `TypeError: couldn't access the py sequence` (verified on
/// 5.2.4): a float property takes numbers and booleans, an int or enum property integers in
/// the 32-bit range and booleans (not `1.5`, not `2**40`), a boolean property booleans and
/// the integers 0 and 1 (not `2`, `-1` or `1.0`); a nested list or a string is refused.
/// `int_range` narrows the integer range to the raw array's item type ([`raw_int_range`]).
fn foreach_buffer(
    prop: &PropDef,
    seq: &[Py],
    int_range: Option<(i64, i64)>,
) -> Result<Bulk, FrapError> {
    let bad = || type_err("couldn't access the py sequence");
    Ok(match &prop.kind {
        PropKind::Float { .. } => Bulk::F32(
            seq.iter()
                .map(|x| match x {
                    Py::Bool(b) => Ok(*b as i32 as f32),
                    Py::Int(i) => Ok(*i as f32),
                    Py::Float(f) => Ok(*f as f32),
                    _ => Err(bad()),
                })
                .collect::<Result<_, _>>()?,
        ),
        PropKind::Int { .. } | PropKind::Enum { .. } => Bulk::I32(
            seq.iter()
                .map(|x| match x {
                    Py::Bool(b) => Ok(*b as i32),
                    Py::Int(i) if int_range.is_some_and(|(lo, hi)| !(lo..=hi).contains(i)) => {
                        Err(bad())
                    }
                    Py::Int(i) => i32::try_from(*i).map_err(|_| bad()),
                    _ => Err(bad()),
                })
                .collect::<Result<_, _>>()?,
        ),
        PropKind::Bool { .. } => Bulk::Bool(
            seq.iter()
                .map(|x| match x {
                    Py::Bool(b) => Ok(*b),
                    Py::Int(0) => Ok(false),
                    Py::Int(1) => Ok(true),
                    _ => Err(bad()),
                })
                .collect::<Result<_, _>>()?,
        ),
        // A property without a bulk form (strings, pointers): Blender's size error (verified
        // on 5.2.4: `objects.foreach_set('name', ['a', 'b', 'c'])`).
        _ => return Err(runtime_err("internal error setting the array")),
    })
}

/// The number Blender's `foreach_get` gives an enum value: the item's `value` (an enum flag:
/// the bits of its items).
fn enum_to_int(items: &[frap_rna::EnumItem], v: &Value) -> Option<i32> {
    let value_of = |s: &str| {
        items
            .iter()
            .find(|it| it.identifier == s)
            .map(|it| it.value)
    };
    match v {
        Value::Enum(s) => value_of(s),
        Value::EnumSet(set) => set.iter().try_fold(0, |acc, s| Some(acc | value_of(s)?)),
        _ => None,
    }
}

/// The enum value a `foreach_set` number names; `None` when no item has it (frap refuses it,
/// D-174).
fn enum_from_int(prop: &PropDef, items: &[frap_rna::EnumItem], x: i32) -> Option<Value> {
    let named = || items.iter().filter(|it| !it.identifier.is_empty());
    if prop.flags.contains(PropFlags::ENUM_FLAG) {
        let mut rest = x;
        let mut set = std::collections::BTreeSet::new();
        for it in named().filter(|it| it.value != 0 && x & it.value == it.value) {
            set.insert(SmolStr::from(it.identifier));
            rest &= !it.value;
        }
        (rest == 0).then_some(Value::EnumSet(set))
    } else {
        named()
            .find(|it| it.value == x)
            .map(|it| Value::Enum(it.identifier.into()))
    }
}

/// Appends one item's value of `prop` to a bulk buffer; false when it has no bulk form.
fn push_bulk(out: &mut Bulk, prop: &PropDef, v: &Value) -> bool {
    match (out, v) {
        (Bulk::I32(b), Value::Enum(_) | Value::EnumSet(_)) => {
            let PropKind::Enum { items, .. } = &prop.kind else {
                return false;
            };
            match enum_to_int(items, v) {
                Some(i) => b.push(i),
                None => return false,
            }
        }
        (out, v) => return out.push_value(prop, v).is_ok(),
    }
    true
}
