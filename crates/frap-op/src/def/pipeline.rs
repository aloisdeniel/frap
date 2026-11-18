// SPDX-License-Identifier: GPL-3.0-or-later
//! The invocation pipeline (ARCHITECTURE §10.2) as the session drives it, plus the operator
//! metadata queries of §8 (`bpy.ops.x.y.poll()`, `.get_rna_type()`, `.idname()`,
//! `.idname_py()`).
//!
//! **Order (verified on Blender 5.2.4).** `bpy.ops.mesh.bevel(foo=1)` in Object Mode raises
//! `RuntimeError: Operator bpy.ops.mesh.bevel.poll() failed, context is incorrect`, not the
//! unknown-keyword `TypeError`: `pyop_call` (`bpy_operator.cc`) polls **before** it converts the
//! keyword arguments. [`prepare`](crate::pipeline::prepare) therefore runs: duplicate keywords (a Python `SyntaxError`),
//! lookup, positional arguments (Python-side `_parse_args`), poll, keyword conversion. The
//! session ensures evaluation (§10.2 step 4) before calling [`prepare`](crate::pipeline::prepare), opens the transaction,
//! calls [`execute`](crate::pipeline::execute) and builds the response.
//!
//! **Macros.** `MESH_OT_extrude_region_move` has one POINTER prop per step, named after the
//! step's `bl_idname`; `TRANSFORM_OT_translate={"value": (0, 0, 1)}` (bpy form) and
//! `TRANSFORM_OT_translate.value=0,0,1` (short form; `frap-lang` merges dotted keys into one
//! dict, and [`convert_kwargs`](crate::pipeline::convert_kwargs) accepts dotted keys too) convert into `OpArgs::nested`. Steps run
//! in order without their own poll (`wm_macro_exec`); a cancelled step stops the macro, which
//! still reports `FINISHED` when an earlier step finished (`wm_macro_end`, verified with a
//! Python macro on 5.2.4).

use std::collections::HashMap;
use std::f64::consts::TAU;
use std::sync::{Mutex, OnceLock};

use frap_rna::{
    ArrayShape, CoerceCx, Defaults, EnumDefault, EnumItem, FloatStorage, FrapError, Input,
    MathValue, OpArgs, OperatorSpec, PollCell, PropDef, PropFlags, PropKind, PyExc, Report,
    ReportType, RnaErrorKind, Subtype, Unit, Value, coerce, py_float_repr,
};
use frap_scene::{Context, Host, ObjectMode};
use serde_json::{Map, Value as J, json};
use smol_str::SmolStr;

use crate::ctx::{OpCtx, OpOutput};
use crate::def::{
    DynEnumFn, EnumItemOwned, OpError, OpKind, OpResult, OpStatus, OperatorDef, PollReq, Support,
};
use crate::errors::*;
use crate::registry::{Lookup, Registry, check_poll, unknown_error};
use crate::services::Services;

/// Hint code: a whole-number value above 2π given to an angle property (§13.2).
pub const ANGLE_LIKELY_DEGREES: &str = "ANGLE_LIKELY_DEGREES";

/// Blender's execution contexts, accepted as positional arguments and ignored (§10.2 step 2).
pub const EXEC_CONTEXTS: &[&str] = &[
    "INVOKE_DEFAULT",
    "INVOKE_REGION_WIN",
    "INVOKE_REGION_CHANNELS",
    "INVOKE_REGION_PREVIEW",
    "INVOKE_AREA",
    "INVOKE_SCREEN",
    "EXEC_DEFAULT",
    "EXEC_REGION_WIN",
    "EXEC_REGION_CHANNELS",
    "EXEC_REGION_PREVIEW",
    "EXEC_AREA",
    "EXEC_SCREEN",
];

const PREFIX: &str = "Converting py args to operator properties: ";

/// One entry of the response's `hints` (§13.2): frap help, never Blender behaviour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    /// `ANGLE_LIKELY_DEGREES`, …
    pub code: &'static str,
    /// Human-readable help naming the fix.
    pub message: String,
}

impl Hint {
    /// `{"code": …, "message": …}`
    pub fn to_json(&self) -> J {
        json!({"code": self.code, "message": self.message})
    }
}

/// Output of keyword conversion.
#[derive(Clone, Debug, Default)]
pub struct Converted {
    /// Values in spec order (defaults for unset props), `is_set`, macro `nested` args.
    pub args: OpArgs,
    /// INFO reports for clamped values (deviation D-03) and `INVOKE_*` contexts (D-02).
    pub reports: Vec<Report>,
    /// frap hints (`ANGLE_LIKELY_DEGREES`).
    pub hints: Vec<Hint>,
}

/// An operator call that passed lookup, poll and argument conversion.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The implemented operator.
    pub def: &'static OperatorDef,
    /// Converted arguments.
    pub args: OpArgs,
    /// Reports produced before exec (they precede exec's reports in the response).
    pub reports: Vec<Report>,
    /// Hints for the response.
    pub hints: Vec<Hint>,
}

// ------------------------------------------------------------------------------- arguments

/// `f(a=1, a=2)` is a Python `SyntaxError` before anything runs.
pub fn check_duplicate_keywords(kwargs: &[(String, Input)]) -> Result<(), FrapError> {
    for (i, (k, _)) in kwargs.iter().enumerate() {
        if kwargs[..i].iter().any(|(k2, _)| k2 == k) {
            return Err(FrapError::new(
                PyExc::SyntaxError,
                frap_rna::ErrorCode("SYNTAX"),
                format!("keyword argument repeated: {k}"),
            )
            .with_detail("argument", k.as_str()));
        }
    }
    Ok(())
}

/// §10.2 step 2: execution-context strings are accepted and ignored (`INVOKE_*` adds an INFO
/// report, deviation D-02); any other positional argument is `OP_ARG_POSITIONAL` (D-01).
pub fn check_positional(
    spec: &OperatorSpec,
    positional: &[Input],
) -> Result<Vec<Report>, FrapError> {
    let mut reports = Vec::new();
    for p in positional {
        match p {
            Input::Str(s) if EXEC_CONTEXTS.contains(&s.as_str()) => {
                if s.starts_with("INVOKE") {
                    reports.push(Report::new(
                        ReportType::Info,
                        format!("frap: {s} runs as EXEC_DEFAULT (headless)"),
                    ));
                }
            }
            other => {
                return Err(FrapError::new(
                    PyExc::TypeError,
                    OP_ARG_POSITIONAL,
                    format!(
                        "frap: positional argument {} is not accepted; operator properties are keyword-only",
                        other.py_type_name()
                    ),
                )
                .with_detail("operator", spec.idname)
                .with_hint("pass properties by keyword, e.g. size=2"));
            }
        }
    }
    Ok(reports)
}

/// Short-form macro keys: `TRANSFORM_OT_translate.value=…` joins the `TRANSFORM_OT_translate`
/// dict (created if absent, at the position of the first dotted key).
fn merge_dotted(kwargs: Vec<(String, Input)>) -> Result<Vec<(String, Input)>, FrapError> {
    let mut out: Vec<(String, Input)> = Vec::with_capacity(kwargs.len());
    for (k, v) in kwargs {
        let Some((head, member)) = k.split_once('.') else {
            out.push((k, v));
            continue;
        };
        let pos = match out.iter().position(|(k2, _)| k2 == head) {
            Some(p) => p,
            None => {
                out.push((head.to_owned(), Input::Dict(Default::default())));
                out.len() - 1
            }
        };
        if let Input::Dict(d) = &mut out[pos].1 {
            if d.contains_key(member) {
                return Err(FrapError::new(
                    PyExc::SyntaxError,
                    frap_rna::ErrorCode("SYNTAX"),
                    format!("keyword argument repeated: {k}"),
                )
                .with_detail("argument", k.as_str()));
            }
            d.insert(member.to_owned(), v);
        }
        // Not a dict (`X=5 X.value=1`): the conversion of `X` reports the type error.
    }
    Ok(out)
}

/// Interned dynamic enum item tables (bounded: one per distinct item list).
fn intern_items(items: &[EnumItemOwned]) -> &'static [EnumItem] {
    type Key = Vec<(SmolStr, SmolStr, SmolStr)>;
    static CACHE: OnceLock<Mutex<HashMap<Key, &'static [EnumItem]>>> = OnceLock::new();
    let key: Key = items
        .iter()
        .map(|i| (i.identifier.clone(), i.name.clone(), i.description.clone()))
        .collect();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(t) = cache.get(&key) {
        return t;
    }
    let leak = |s: &SmolStr| -> &'static str { Box::leak(s.to_string().into_boxed_str()) };
    let table: &'static [EnumItem] = Box::leak(
        items
            .iter()
            .enumerate()
            .map(|(n, i)| EnumItem {
                identifier: leak(&i.identifier),
                name: leak(&i.name),
                description: leak(&i.description),
                value: n as i32,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    cache.insert(key, table);
    table
}

/// A copy of an enum `PropDef` with its items replaced by the dynamic ones.
/// Without a context, a dynamic enum whose dump has no static items (the superset is empty,
/// e.g. `object.collection_link.collection`, items = the collection names) accepts the given
/// identifier; the operator checks it against its context items with Blender's message
/// (WP 1.20).
fn open_dynamic(def: &'static PropDef, input: &Input) -> Option<PropDef> {
    let PropKind::Enum {
        items,
        dynamic: Some(_),
        ..
    } = &def.kind
    else {
        return None;
    };
    if !items.is_empty() {
        return None;
    }
    let id = match input {
        Input::Str(s) => SmolStr::new(s),
        Input::Bare(s) => s.clone(),
        _ => return None,
    };
    let item = EnumItemOwned {
        identifier: id.clone(),
        name: id,
        description: SmolStr::default(),
    };
    with_items(def, intern_items(&[item]))
}

fn with_items(def: &'static PropDef, items: &'static [EnumItem]) -> Option<PropDef> {
    let PropKind::Enum {
        default, dynamic, ..
    } = &def.kind
    else {
        return None;
    };
    let default = match default {
        EnumDefault::Item(s) => EnumDefault::Item(s),
        EnumDefault::Flags(f) => EnumDefault::Flags(f),
    };
    Some(PropDef {
        identifier: def.identifier,
        name: def.name,
        description: def.description,
        kind: PropKind::Enum {
            items,
            default,
            dynamic: *dynamic,
        },
        subtype: def.subtype,
        unit: def.unit,
        array: def.array,
        flags: def.flags,
        deprecated: def.deprecated,
    })
}

/// Numbers of an input as written: `(value, text)`; `None` when it is not numeric.
fn input_numbers(input: &Input) -> Option<(Vec<(f64, String)>, bool)> {
    let one = |i: &Input| -> Option<(f64, String)> {
        match i {
            Input::Int(v) => Some((*v as f64, v.to_string())),
            Input::Float(v) => Some((*v, py_float_repr(*v))),
            _ => None,
        }
    };
    match input {
        Input::Int(_) | Input::Float(_) => one(input).map(|n| (vec![n], true)),
        Input::Tuple(v) | Input::List(v) => {
            let n: Option<Vec<_>> = v.iter().map(one).collect();
            n.map(|n| (n, false))
        }
        Input::Math(MathValue::Euler(e, _)) => {
            Some((e.iter().map(|x| (*x, py_float_repr(*x))).collect(), false))
        }
        Input::Math(MathValue::Vector(e)) => {
            Some((e.iter().map(|x| (*x, py_float_repr(*x))).collect(), false))
        }
        _ => None,
    }
}

/// `ANGLE_LIKELY_DEGREES` (§13.2): an angle property (unit ROTATION, subtype ANGLE/EULER) given
/// a whole number whose magnitude exceeds 2π.
pub fn angle_hint(def: &PropDef, input: &Input) -> Option<Hint> {
    let angular = matches!(def.kind, PropKind::Float { .. })
        && (def.unit == Unit::Rotation || matches!(def.subtype, Subtype::Angle | Subtype::Euler));
    if !angular {
        return None;
    }
    let (nums, scalar) = input_numbers(input)?;
    let (_, t) = nums
        .iter()
        .find(|(v, _)| v.is_finite() && v.fract() == 0.0 && v.abs() > TAU)?;
    let shown = if scalar {
        t.clone()
    } else {
        nums.iter()
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    Some(Hint {
        code: ANGLE_LIKELY_DEGREES,
        message: format!(
            "{}={shown} is in radians ({t} rad); did you mean radians({t})?",
            def.identifier
        ),
    })
}

/// Error context while converting nested macro arguments.
#[derive(Copy, Clone)]
struct Site<'a> {
    /// Top-level operator idname (`detail.operator`).
    operator: &'static str,
    /// Top-level keyword when converting a macro sub-dict (`detail.argument`).
    top_arg: Option<&'a str>,
}

impl Site<'_> {
    fn decorate(&self, e: FrapError, spec: &OperatorSpec, arg: &str) -> FrapError {
        match self.top_arg {
            None => e
                .with_detail("operator", self.operator)
                .with_detail("argument", arg),
            Some(top) => e
                .with_detail("operator", self.operator)
                .with_detail("argument", top)
                .with_detail("sub_operator", spec.idname)
                .with_detail("sub_argument", arg),
        }
    }
}

/// The macro sub-operator spec a POINTER prop stands for (`fixed_type` = a step's `bl_idname`).
fn sub_operator(spec: &OperatorSpec, def: &PropDef) -> Option<&'static OperatorSpec> {
    let PropKind::Pointer { fixed_type, is_id } = def.kind else {
        return None;
    };
    if is_id {
        return None;
    }
    let is_step = spec.macro_steps.contains(&fixed_type);
    let reg = Registry::try_global().ok()?;
    match reg.spec_by_bl_idname(fixed_type) {
        Some(s) if is_step || fixed_type.contains("_OT_") => Some(s),
        _ => None,
    }
}

fn dyn_enums_of(spec: &OperatorSpec) -> &'static [(&'static str, DynEnumFn)] {
    Registry::try_global()
        .ok()
        .and_then(|r| r.get_by_bl_idname(spec.bl_idname))
        .map_or(&[], |d| d.dyn_enums)
}

fn convert_struct(
    spec: &'static OperatorSpec,
    dyn_enums: &[(&'static str, DynEnumFn)],
    ctx: Option<&Context>,
    kwargs: Vec<(String, Input)>,
    site: Site<'_>,
    reports: &mut Vec<Report>,
    hints: &mut Vec<Hint>,
) -> Result<OpArgs, FrapError> {
    let mut kw: Vec<Option<(String, Input)>> = kwargs.into_iter().map(Some).collect();
    let mut args = OpArgs::default();
    let cx = CoerceCx::OpArgs {
        owner: spec.bl_idname,
    };
    // Blender (`pyrna_pydict_to_props`) walks the RNA props in order, then complains about
    // the first leftover keyword.
    for (index, def) in spec.props.iter().enumerate() {
        let found = kw
            .iter_mut()
            .find(|e| e.as_ref().is_some_and(|(k, _)| k == def.identifier))
            .and_then(Option::take);
        if let Some(sub) = sub_operator(spec, def) {
            let given = match found {
                None => None,
                Some((_, Input::Dict(d))) => Some(d.into_iter().collect()),
                Some((_, other)) => {
                    let e = FrapError::new(
                        PyExc::TypeError,
                        OP_ARG_TYPE,
                        format!(
                            "{PREFIX}{}.{} expected a {} type, not {}",
                            spec.bl_idname,
                            def.identifier,
                            sub.bl_idname,
                            other.py_type_name()
                        ),
                    );
                    return Err(site.decorate(e, spec, def.identifier));
                }
            };
            if given.is_some() {
                args.is_set.insert(index);
            }
            let sub_site = Site {
                operator: site.operator,
                top_arg: Some(site.top_arg.unwrap_or(def.identifier)),
            };
            let nested = convert_struct(
                sub,
                dyn_enums_of(sub),
                ctx,
                given.unwrap_or_default(),
                sub_site,
                reports,
                hints,
            )?;
            args.values.insert(def.identifier, Value::None);
            args.nested.insert(sub.bl_idname, nested);
            continue;
        }
        // frap's own operators (`frap.*`, no Blender counterpart) document an unset number as
        // `=None` (`render_contact_sheet(frame=None)`, §19.6): an explicit `None` is unset
        // there. Blender's operators keep Blender's TypeError.
        let found = found.filter(|(_, input)| {
            !(matches!(input, Input::None)
                && spec.idname.starts_with("frap.")
                && matches!(def.kind, PropKind::Int { .. } | PropKind::Float { .. }))
        });
        let Some((_, input)) = found else {
            args.values.insert(def.identifier, Value::default_for(def));
            continue;
        };
        let dynamic = match (ctx, dyn_enums.iter().find(|(p, _)| *p == def.identifier)) {
            (Some(c), Some((_, f))) => with_items(def, intern_items(&f(c))),
            (None, _) => open_dynamic(def, &input),
            _ => None,
        };
        let target: &PropDef = dynamic.as_ref().unwrap_or(def);
        let written = input.clone();
        let c = coerce(target, input, &cx).map_err(|e| {
            let e = e.into_frap(PREFIX, |k| match k {
                RnaErrorKind::Enum => OP_ARG_ENUM,
                RnaErrorKind::Length => OP_ARG_LENGTH,
                _ => OP_ARG_TYPE,
            });
            site.decorate(e, spec, def.identifier)
        })?;
        if c.clamped {
            reports.push(Report::new(
                ReportType::Info,
                frap_rna::clamp_report(def, &written),
            ));
        }
        if let Some(h) = angle_hint(def, &written) {
            hints.push(h);
        }
        args.values.insert(def.identifier, c.value);
        args.is_set.insert(index);
    }
    if let Some((k, _)) = kw.into_iter().flatten().next() {
        let valid: Vec<&str> = spec.props.iter().map(|p| p.identifier).collect();
        let e = FrapError::new(
            PyExc::TypeError,
            OP_ARG_UNKNOWN,
            format!("Converting py args to operator properties:: keyword \"{k}\" unrecognized"),
        )
        .with_detail(
            "did_you_mean",
            crate::registry::suggest(&k, valid.iter().copied())
                .into_iter()
                .map(J::from)
                .collect::<Vec<_>>(),
        )
        .with_detail("valid", valid.into_iter().map(J::from).collect::<Vec<_>>());
        return Err(site.decorate(e, spec, &k));
    }
    Ok(args)
}

/// §10.2 step 3: converts keyword arguments against `spec.props` with Blender's messages.
/// Dynamic enums listed in `dyn_enums` are validated against their items in `ctx` (without a
/// context, against the static superset). Macro sub-dicts (and dotted short-form keys)
/// recurse into `OpArgs::nested`, keyed by the step's `bl_idname`; steps the caller did not
/// mention get their defaults.
pub fn convert_kwargs(
    spec: &'static OperatorSpec,
    dyn_enums: &[(&'static str, DynEnumFn)],
    ctx: Option<&Context>,
    kwargs: Vec<(String, Input)>,
) -> Result<Converted, FrapError> {
    check_duplicate_keywords(&kwargs)?;
    let kwargs = merge_dotted(kwargs)?;
    let mut out = Converted::default();
    out.args = convert_struct(
        spec,
        dyn_enums,
        ctx,
        kwargs,
        Site {
            operator: spec.idname,
            top_arg: None,
        },
        &mut out.reports,
        &mut out.hints,
    )?;
    Ok(out)
}

/// [`convert_kwargs`] for an implemented operator in a context (its dynamic enums checked).
pub fn convert(
    def: &OperatorDef,
    ctx: &Context,
    kwargs: Vec<(String, Input)>,
) -> Result<Converted, FrapError> {
    convert_kwargs(def.spec, def.dyn_enums, Some(ctx), kwargs)
}

/// §10.2 steps 1, 2, 5 and 3 in Blender's order (module docs): everything before the
/// transaction. The caller has ensured evaluation (step 4).
pub fn prepare(
    idname: &str,
    ctx: &Context,
    positional: Vec<Input>,
    kwargs: Vec<(String, Input)>,
) -> Result<Prepared, FrapError> {
    check_duplicate_keywords(&kwargs)?;
    let def = crate::registry::lookup(idname)?;
    let mut reports = check_positional(def.spec, &positional)?;
    check_poll(def, ctx)?;
    let c = convert(def, ctx, kwargs)?;
    reports.extend(c.reports);
    Ok(Prepared {
        def,
        args: c.args,
        reports,
        hints: c.hints,
    })
}

// ------------------------------------------------------------------------------- execution

/// Step properties a macro definition sets (`RNA_boolean_set(otmacro->ptr, …)` in
/// `WM_operatortype_macro_define` callers): they win over the caller's nested arguments
/// (verified on 5.2.4: `duplicate_move_linked(OBJECT_OT_duplicate={'linked': False})` still
/// links). `(macro, step, prop, value)`; the operator dumps do not record them (WP 1.20).
#[rustfmt::skip]
pub const MACRO_STEP_FORCED: &[(&str, &str, &str, bool)] = &[
    ("OBJECT_OT_duplicate_move_linked", "OBJECT_OT_duplicate", "linked", true),
    // `mesh_ops.cc`: `RNA_boolean_set(otmacro->ptr, "single_side", true)` (WP 2.6).
    ("MESH_OT_offset_edge_loops_slide", "TRANSFORM_OT_edge_slide", "single_side", true),
    // `ED_operatormacros_mesh` (editors/mesh/mesh_ops.cc, WP 2.5): every extrude macro's
    // transform step runs without proportional editing and mesh mirror; `extrude_manifold`
    // also dissolves orthogonal edges and auto-merges.
    ("MESH_OT_extrude_region_move", "TRANSFORM_OT_translate", "use_proportional_edit", false),
    ("MESH_OT_extrude_region_move", "TRANSFORM_OT_translate", "mirror", false),
    ("MESH_OT_extrude_manifold", "MESH_OT_extrude_region", "use_dissolve_ortho_edges", true),
    ("MESH_OT_extrude_manifold", "TRANSFORM_OT_translate", "use_proportional_edit", false),
    ("MESH_OT_extrude_manifold", "TRANSFORM_OT_translate", "mirror", false),
    ("MESH_OT_extrude_manifold", "TRANSFORM_OT_translate", "use_automerge_and_split", true),
    ("MESH_OT_extrude_context_move", "TRANSFORM_OT_translate", "use_proportional_edit", false),
    ("MESH_OT_extrude_context_move", "TRANSFORM_OT_translate", "mirror", false),
    ("MESH_OT_extrude_region_shrink_fatten", "TRANSFORM_OT_shrink_fatten", "use_proportional_edit", false),
    ("MESH_OT_extrude_region_shrink_fatten", "TRANSFORM_OT_shrink_fatten", "mirror", false),
    ("MESH_OT_extrude_faces_move", "TRANSFORM_OT_shrink_fatten", "use_proportional_edit", false),
    ("MESH_OT_extrude_faces_move", "TRANSFORM_OT_shrink_fatten", "mirror", false),
    ("MESH_OT_extrude_edges_move", "TRANSFORM_OT_translate", "use_proportional_edit", false),
    ("MESH_OT_extrude_edges_move", "TRANSFORM_OT_translate", "mirror", false),
    ("MESH_OT_extrude_vertices_move", "TRANSFORM_OT_translate", "use_proportional_edit", false),
    ("MESH_OT_extrude_vertices_move", "TRANSFORM_OT_translate", "mirror", false),
];

/// The boolean step properties `macro` forces on `step` ([`MACRO_STEP_FORCED`]).
pub fn macro_step_forced(macro_bl: &str, step_bl: &str) -> Vec<(&'static str, bool)> {
    MACRO_STEP_FORCED
        .iter()
        .filter(|(m, s, _, _)| *m == macro_bl && *s == step_bl)
        .map(|&(_, _, p, v)| (p, v))
        .collect()
}

/// §10.2 step 7: runs an operator. A macro runs its steps in order with their nested args
/// (defaults for steps without one), stops at the first cancelled step and reports `FINISHED`
/// if an earlier step finished (`wm_macro_exec`/`wm_macro_end`); its own `exec` then runs as a
/// post-step hook. Errors propagate (the session rolls back).
pub fn execute(def: &'static OperatorDef, cx: &mut OpCtx, args: &OpArgs) -> OpResult {
    if def.kind == OpKind::Normal {
        return (def.exec)(cx, args);
    }
    let reg = Registry::global();
    let mut finished = false;
    for step in def.spec.macro_steps {
        let Some(step_def) = reg.get_by_bl_idname(step) else {
            return Err(OpError::NotImplemented(format!(
                "{} (step of macro {}) exists in Blender 5.2 but is not implemented by frap",
                idname_py_from_bl(step),
                def.spec.idname
            )));
        };
        let defaults;
        let mut sub = match args.nested.get(step) {
            Some(a) => a,
            None => {
                defaults = convert_kwargs(step_def.spec, &[], None, Vec::new())
                    .map_err(|e| OpError::Runtime(e.message))?
                    .args;
                &defaults
            }
        };
        let forced_args;
        let forced = macro_step_forced(def.spec.bl_idname, step);
        if !forced.is_empty() {
            let mut a = sub.clone();
            for (prop, v) in forced {
                if let Some(i) = a.values.get_index_of(prop) {
                    a.values.insert(prop, frap_rna::Value::Bool(v));
                    a.is_set.insert(i);
                }
            }
            forced_args = a;
            sub = &forced_args;
        }
        match cx.run_def(step_def, sub, true)? {
            OpStatus::Finished => finished = true,
            OpStatus::Cancelled => {
                return Ok(if finished {
                    OpStatus::Finished
                } else {
                    OpStatus::Cancelled
                });
            }
        }
    }
    match (def.exec)(cx, args)? {
        OpStatus::Cancelled if !finished => Ok(OpStatus::Cancelled),
        _ => Ok(OpStatus::Finished),
    }
}

/// The outcome of [`invoke`].
#[derive(Debug)]
pub struct Invocation {
    /// The operator that ran.
    pub def: &'static OperatorDef,
    /// Exec outcome.
    pub status: OpResult,
    /// Reports (conversion reports first) and `Response.result`.
    pub output: OpOutput,
    /// Hints from argument conversion.
    pub hints: Vec<Hint>,
}

/// [`prepare`] + [`execute`] **without a transaction**: what an operator calling another
/// operator by name, and the framework tests, need. The session wraps the two steps in its
/// transaction instead.
pub fn invoke(
    host: &mut dyn Host,
    services: &mut dyn Services,
    idname: &str,
    positional: Vec<Input>,
    kwargs: Vec<(String, Input)>,
) -> Result<Invocation, FrapError> {
    let p = prepare(idname, &host.ctx(), positional, kwargs)?;
    let mut cx = OpCtx::new(host, services, p.def.spec);
    for r in p.reports {
        cx.report(r.level, r.message);
    }
    let status = execute(p.def, &mut cx, &p.args);
    Ok(Invocation {
        def: p.def,
        status,
        output: cx.finish(),
        hints: p.hints,
    })
}

// ------------------------------------------------------------------------------- metadata

/// `bpy.ops.mesh.foo.idname()` → `"MESH_OT_foo"` (pure string transformation, also for unknown
/// operators, verified).
pub fn idname_bl(idname: &str) -> String {
    match idname.split_once('.') {
        Some((ns, op)) => format!("{}_OT_{op}", ns.to_uppercase()),
        None => idname.to_owned(),
    }
}

/// `bpy.ops.mesh.foo.idname_py()` → `"mesh.foo"` (also accepts `MESH_OT_foo`).
pub fn idname_py(idname: &str) -> String {
    if idname.contains('.') {
        idname.to_owned()
    } else {
        idname_py_from_bl(idname)
    }
}

fn idname_py_from_bl(bl: &str) -> String {
    match bl.split_once("_OT_") {
        Some((ns, op)) => format!("{}.{op}", ns.to_lowercase()),
        None => bl.to_owned(),
    }
}

/// Answer of `bpy.ops.x.y.poll()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollAnswer {
    /// The poll result.
    pub ok: bool,
    /// Custom failure message, if the poll sets one.
    pub message: Option<String>,
    /// Every declared requirement.
    pub requires: Vec<PollReq>,
    /// The failing requirements.
    pub unmet: Vec<PollReq>,
    /// The fix.
    pub hint: Option<String>,
    /// For an operator frap does not implement: the dumped poll-matrix context (`"A"`…`"E"`)
    /// the answer was read from (an estimate).
    pub estimated_from: Option<&'static str>,
}

impl PollAnswer {
    /// `detail` of the `op_meta` response (`requires`, `unmet`, `message`, `estimated_from`).
    pub fn detail(&self) -> Map<String, J> {
        let mut m = Map::new();
        m.insert(
            "requires".into(),
            self.requires.iter().map(|r| J::from(r.code())).collect(),
        );
        m.insert(
            "unmet".into(),
            self.unmet.iter().map(|r| J::from(r.code())).collect(),
        );
        if let Some(msg) = &self.message {
            m.insert("message".into(), J::from(msg.as_str()));
        }
        if let Some(c) = self.estimated_from {
            m.insert("estimated_from".into(), json!({"poll_matrix": c}));
        }
        m
    }
}

/// Which dumped poll context (A–E, [OBJ §0]) the current context resembles.
fn poll_column(ctx: &Context) -> (usize, &'static str) {
    let active = ctx
        .active_object()
        .and_then(|r| ctx.main.objects.get(r).map(|o| (r, o)));
    match active {
        Some((_, ob)) if ob.mode == ObjectMode::Edit && ob.object_type() == "ARMATURE" => (4, "E"),
        Some((_, ob)) if ob.mode == ObjectMode::Edit => (2, "C"),
        Some((_, ob)) if ob.mode == ObjectMode::Pose => (3, "D"),
        // A is "mesh Cube active and selected"; any other active object in Object Mode is
        // closer to A than to B (nothing selected, no active object).
        Some(_) => (0, "A"),
        None => (1, "B"),
    }
}

/// `bpy.ops.<idname>.poll()`: the declarative poll of an implemented operator; for a dumped
/// operator frap does not implement, the dumped poll matrix (VIEW_3D column, deviation D-10) in
/// the closest context A–E. Unknown → `AttributeError: Polling operator "bpy.ops.x.y" error,
/// could not be found` (verified).
pub fn poll(idname: &str, ctx: &Context) -> Result<PollAnswer, FrapError> {
    match Registry::global().lookup(idname) {
        Lookup::Implemented(def) => Ok(match (def.poll.check)(ctx) {
            Ok(()) => PollAnswer {
                ok: true,
                message: None,
                requires: def.poll.requires.to_vec(),
                unmet: Vec::new(),
                hint: None,
                estimated_from: None,
            },
            Err(f) => PollAnswer {
                ok: false,
                message: f.message,
                requires: def.poll.requires.to_vec(),
                unmet: f.unmet.to_vec(),
                hint: f.hint,
                estimated_from: None,
            },
        }),
        Lookup::KnownNotImplemented(spec) => {
            let (col, name) = poll_column(ctx);
            let ok = spec
                .poll_matrix
                .is_none_or(|m| m.view3d[col] == PollCell::Y);
            Ok(PollAnswer {
                ok,
                message: None,
                requires: Vec::new(),
                unmet: Vec::new(),
                hint: None,
                estimated_from: Some(name),
            })
        }
        Lookup::Unknown { suggestions } => Err(unknown_error("Polling", idname, &suggestions)),
    }
}

/// `bpy.ops.<idname>.get_rna_type()`: the operator's spec (and implementation, if any).
#[derive(Copy, Clone, Debug)]
pub struct RnaType {
    /// The dumped spec.
    pub spec: &'static OperatorSpec,
    /// The implementation, if frap has one.
    pub def: Option<&'static OperatorDef>,
}

impl RnaType {
    /// The describe fragment (`identifier`, `properties`, `frap.status`, …).
    pub fn to_json(&self) -> J {
        let spec = self.spec;
        let frap = match self.def {
            Some(d) => json!({
                "status": "implemented",
                "tier": format!("{:?}", d.tier),
                "kind": format!("{:?}", d.kind),
                "support": support_json(d.support),
                "requires": d.poll.requires.iter().map(|r| r.code()).collect::<Vec<_>>(),
                "notes": d.notes,
                "dynamic_enums": d.dyn_enums.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            }),
            None => json!({"status": "not_implemented"}),
        };
        json!({
            "identifier": spec.bl_idname,
            "idname": spec.idname,
            "name": spec.label,
            "description": spec.description,
            "bl_options": spec.options.names(),
            "macro_steps": spec.macro_steps,
            "properties": spec.props.iter().map(prop_json).collect::<Vec<_>>(),
            "signature": signature(spec),
            "frap": frap,
        })
    }
}

fn support_json(s: Support) -> J {
    match s {
        Support::Full => json!({"level": "full"}),
        Support::Partial(n) => json!({"level": "partial", "note": n}),
        Support::HeadlessExtended(n) => json!({"level": "headless_extended", "note": n}),
    }
}

/// `get_rna_type()`: unknown → `KeyError: 'get_rna_type("MESH_OT_foo") not found'` (verified).
pub fn get_rna_type(idname: &str) -> Result<RnaType, FrapError> {
    let reg = Registry::global();
    match reg.lookup(idname) {
        Lookup::Implemented(d) => Ok(RnaType {
            spec: d.spec,
            def: Some(d),
        }),
        Lookup::KnownNotImplemented(spec) => Ok(RnaType { spec, def: None }),
        Lookup::Unknown { suggestions } => Err(FrapError::new(
            PyExc::KeyError,
            OP_UNKNOWN,
            format!("get_rna_type(\"{}\") not found", idname_bl(idname)),
        )
        .with_detail("operator", idname)
        .with_detail(
            "did_you_mean",
            suggestions.iter().map(|s| J::from(*s)).collect::<Vec<_>>(),
        )),
    }
}

// ------------------------------------------------------------------------------- formatting

fn num_json(v: f64) -> J {
    if v.is_finite() {
        J::from(v)
    } else if v.is_nan() {
        json!({"$float": "nan"})
    } else if v > 0.0 {
        json!({"$float": "inf"})
    } else {
        json!({"$float": "-inf"})
    }
}

/// Shortest decimal of a stored value (`0.001`, not `0.0010000000474974513`).
fn stored(v: f64, storage: FloatStorage) -> f64 {
    match storage {
        FloatStorage::F32 => frap_math::fmt::shortest_f64(v as f32),
        _ => v,
    }
}

fn array_json(a: ArrayShape) -> J {
    match a {
        ArrayShape::Scalar => J::Null,
        ArrayShape::Fixed(n) => json!([n]),
        ArrayShape::Matrix(r, c) => json!([r, c]),
        ArrayShape::Dynamic => J::from("dynamic"),
    }
}

/// JSON metadata of one operator property (Blender's `bpy.types.Property` fields).
pub fn prop_json(p: &PropDef) -> J {
    let mut m = Map::new();
    m.insert("identifier".into(), J::from(p.identifier));
    m.insert("name".into(), J::from(p.name));
    m.insert("description".into(), J::from(p.description));
    m.insert("type".into(), J::from(p.type_str()));
    m.insert("subtype".into(), J::from(p.subtype.as_str()));
    let unit = if p.unit == Unit::Rotation {
        "ROTATION (radians)".to_owned()
    } else {
        p.unit.as_str().to_owned()
    };
    m.insert("unit".into(), J::from(unit));
    m.insert("array".into(), array_json(p.array));
    match &p.kind {
        PropKind::Bool { default } => {
            m.insert(
                "default".into(),
                match default {
                    Defaults::Scalar(b) => J::from(*b),
                    Defaults::Array(a) => J::from(a.to_vec()),
                },
            );
        }
        PropKind::Int {
            default,
            hard,
            soft,
        } => {
            m.insert(
                "default".into(),
                match default {
                    Defaults::Scalar(i) => J::from(*i),
                    Defaults::Array(a) => J::from(a.to_vec()),
                },
            );
            m.insert("hard_min".into(), J::from(hard.0));
            m.insert("hard_max".into(), J::from(hard.1));
            m.insert("soft_min".into(), J::from(soft.0));
            m.insert("soft_max".into(), J::from(soft.1));
        }
        PropKind::Float {
            default,
            hard,
            soft,
            storage,
            ..
        } => {
            let f = |v: f64| num_json(stored(v, *storage));
            m.insert(
                "default".into(),
                match default {
                    Defaults::Scalar(v) => f(*v),
                    Defaults::Array(a) => J::Array(a.iter().map(|v| f(*v)).collect()),
                },
            );
            m.insert("hard_min".into(), f(hard.0));
            m.insert("hard_max".into(), f(hard.1));
            m.insert("soft_min".into(), f(soft.0));
            m.insert("soft_max".into(), f(soft.1));
        }
        PropKind::String { default, .. } => {
            m.insert("default".into(), J::from(*default));
        }
        PropKind::Enum {
            items,
            default,
            dynamic,
        } => {
            m.insert(
                "default".into(),
                match default {
                    EnumDefault::Item(s) => J::from(*s),
                    EnumDefault::Flags(f) => J::from(f.to_vec()),
                },
            );
            m.insert(
                "enum_items".into(),
                items
                    .iter()
                    .filter(|i| !i.identifier.is_empty())
                    .map(|i| J::from(i.identifier))
                    .collect(),
            );
            m.insert(
                "is_enum_flag".into(),
                J::from(p.flags.contains(PropFlags::ENUM_FLAG)),
            );
            if let Some(d) = dynamic {
                m.insert("dynamic_enum".into(), J::from(*d));
            }
        }
        PropKind::Pointer { fixed_type, .. } => {
            m.insert("fixed_type".into(), J::from(*fixed_type));
        }
        PropKind::Collection { item_type, .. } => {
            m.insert("fixed_type".into(), J::from(*item_type));
        }
        _ => {}
    }
    m.insert(
        "hidden".into(),
        J::from(p.flags.contains(PropFlags::HIDDEN)),
    );
    m.insert(
        "skip_save".into(),
        J::from(p.flags.contains(PropFlags::SKIP_SAVE)),
    );
    J::Object(m)
}

fn py_str(s: &str) -> String {
    if s.contains('\'') && !s.contains('"') {
        format!("\"{s}\"")
    } else {
        format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

fn py_tuple(items: Vec<String>) -> String {
    if items.len() == 1 {
        format!("({},)", items[0])
    } else {
        format!("({})", items.join(", "))
    }
}

/// Python repr of a property's default, as in Blender's API reference.
fn default_repr(p: &PropDef) -> String {
    let b = |v: bool| if v { "True" } else { "False" }.to_owned();
    match &p.kind {
        PropKind::Bool { default } => match default {
            Defaults::Scalar(v) => b(*v),
            Defaults::Array(a) => py_tuple(a.iter().map(|v| b(*v)).collect()),
        },
        PropKind::Int { default, .. } => match default {
            Defaults::Scalar(v) => v.to_string(),
            Defaults::Array(a) => py_tuple(a.iter().map(i64::to_string).collect()),
        },
        PropKind::Float {
            default, storage, ..
        } => {
            let f = |v: f64| py_float_repr(stored(v, *storage));
            match (default, p.array) {
                (Defaults::Scalar(v), _) => f(*v),
                (Defaults::Array(a), ArrayShape::Matrix(_, c)) if c > 0 => py_tuple(
                    a.chunks(c as usize)
                        .map(|row| py_tuple(row.iter().map(|v| f(*v)).collect()))
                        .collect(),
                ),
                (Defaults::Array(a), _) => py_tuple(a.iter().map(|v| f(*v)).collect()),
            }
        }
        PropKind::String { default, .. } => py_str(default),
        PropKind::Enum { default, .. } => match default {
            EnumDefault::Item(s) => py_str(s),
            EnumDefault::Flags([]) => "set()".into(),
            EnumDefault::Flags(f) => format!(
                "{{{}}}",
                f.iter().map(|s| py_str(s)).collect::<Vec<_>>().join(", ")
            ),
        },
        _ => "None".into(),
    }
}

/// The Blender call signature with defaults, as in the API reference:
/// `bpy.ops.mesh.primitive_cube_add(size=2.0, calc_uvs=True, …, scale=(0.0, 0.0, 0.0))`.
pub fn signature(spec: &OperatorSpec) -> String {
    let args: Vec<String> = spec
        .props
        .iter()
        .map(|p| format!("{}={}", p.identifier, default_repr(p)))
        .collect();
    format!("bpy.ops.{}({})", spec.idname, args.join(", "))
}
