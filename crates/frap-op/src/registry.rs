// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime registry and the framework part of the invocation pipeline (ARCHITECTURE §10.1,
//! §10.2 steps 1–3 and 5). The session owns transactions and responses (steps 4, 6–9).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use frap_rna::{
    CoerceCx, FrapError, Input, OpArgs, OperatorSpec, PyExc, RnaErrorKind, Value, coerce,
};
use frap_scene::Context;
use serde_json::Value as J;

use frap_rna::{Report, ReportType};

use crate::def::{OPERATORS, OperatorDef};
use crate::errors::*;

/// Result of a registry lookup.
pub enum Lookup {
    /// Implemented by frap.
    Implemented(&'static OperatorDef),
    /// Dumped from Blender 5.2 (`frap-spec-catalog`) but not implemented: `OP_NOT_IMPLEMENTED`.
    KnownNotImplemented(&'static OperatorSpec),
    /// Unknown to Blender 5.2 and frap.
    Unknown {
        /// Close idnames.
        suggestions: Vec<&'static str>,
    },
}

/// Index over the `OPERATORS` slice, built once per process.
pub struct Registry {
    by_idname: BTreeMap<&'static str, &'static OperatorDef>,
}

impl Registry {
    /// The process-wide registry. Panics if two operators share an idname (caught by tests).
    pub fn global() -> &'static Registry {
        static R: OnceLock<Registry> = OnceLock::new();
        R.get_or_init(|| {
            let mut by_idname = BTreeMap::new();
            for def in OPERATORS.iter() {
                if by_idname.insert(def.spec.idname, def).is_some() {
                    panic!("duplicate operator registration: {}", def.spec.idname);
                }
            }
            Registry { by_idname }
        })
    }
    /// Every registered idname, sorted.
    pub fn idnames(&self) -> Vec<&'static str> {
        self.by_idname.keys().copied().collect()
    }
    /// Lookup by `ns.op`.
    pub fn lookup(&self, idname: &str) -> Lookup {
        match self.by_idname.get(idname) {
            Some(d) => Lookup::Implemented(d),
            None => Lookup::Unknown {
                suggestions: suggest(idname, self.by_idname.keys().copied()),
            },
        }
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j] + (ca != *cb) as usize)
                    .min(prev[j + 1] + 1)
                    .min(cur[j] + 1),
            );
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Up to three candidates within a small edit distance, closest first.
pub fn suggest<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let max = (word.chars().count() / 3).max(2);
    let mut v: Vec<(usize, &str)> = candidates
        .map(|c| (levenshtein(word, c), c))
        .filter(|(d, _)| *d <= max)
        .collect();
    v.sort();
    v.into_iter().take(3).map(|(_, c)| c).collect()
}

const PREFIX: &str = "Converting py args to operator properties: ";
const EXEC_CONTEXTS: &[&str] = &[
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

/// §10.2 step 1: lookup, with Blender's error for unknown operators.
pub fn lookup(idname: &str) -> Result<&'static OperatorDef, FrapError> {
    match Registry::global().lookup(idname) {
        Lookup::Implemented(d) => Ok(d),
        Lookup::KnownNotImplemented(spec) => Err(FrapError::new(
            PyExc::NotImplementedError,
            OP_NOT_IMPLEMENTED,
            format!(
                "{} exists in Blender 5.2 but is not implemented by frap",
                spec.idname
            ),
        )
        .with_detail("operator", spec.idname)),
        Lookup::Unknown { suggestions } => {
            let mut e = FrapError::new(
                PyExc::AttributeError,
                OP_UNKNOWN,
                format!("Calling operator \"bpy.ops.{idname}\" error, could not be found"),
            );
            e = e.with_detail(
                "did_you_mean",
                suggestions.iter().map(|s| J::from(*s)).collect::<Vec<_>>(),
            );
            Err(e.with_hint(
                "M0 implements only mesh.primitive_cube_add and export_scene.gltf; see frap --describe (M1)",
            ))
        }
    }
}

/// §10.2 steps 2–3: positional arguments and keyword conversion (before poll, as in Blender).
/// Returns the converted arguments plus INFO reports for clamped values.
pub fn convert_args(
    spec: &'static OperatorSpec,
    positional: Vec<Input>,
    kwargs: Vec<(String, Input)>,
) -> Result<(OpArgs, Vec<Report>), FrapError> {
    let mut reports = Vec::new();
    for p in positional {
        match &p {
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
    // Duplicate keywords are a Python SyntaxError before the call happens.
    for (i, (k, _)) in kwargs.iter().enumerate() {
        if kwargs[..i].iter().any(|(k2, _)| k2 == k) {
            return Err(FrapError::new(
                PyExc::SyntaxError,
                frap_rna::ErrorCode("SYNTAX"),
                format!("keyword argument repeated: {k}"),
            ));
        }
    }
    let mut kw: Vec<Option<(String, Input)>> = kwargs.into_iter().map(Some).collect();
    let mut args = OpArgs::default();
    let cx = CoerceCx::OpArgs {
        owner: spec.bl_idname,
    };
    // Blender (pyrna_pydict_to_props) walks the RNA props in order, then complains about leftovers.
    for (index, def) in spec.props.iter().enumerate() {
        let found = kw
            .iter_mut()
            .find(|e| e.as_ref().is_some_and(|(k, _)| k == def.identifier))
            .and_then(Option::take);
        match found {
            Some((_, input)) => {
                let written = input.clone();
                let c = coerce(def, input, &cx).map_err(|e| {
                    e.into_frap(PREFIX, |k| match k {
                        RnaErrorKind::Enum => OP_ARG_ENUM,
                        RnaErrorKind::Length => OP_ARG_LENGTH,
                        _ => OP_ARG_TYPE,
                    })
                    .with_detail("operator", spec.idname)
                    .with_detail("argument", def.identifier)
                })?;
                if c.clamped {
                    reports.push(Report::new(
                        ReportType::Info,
                        frap_rna::clamp_report(def, &written),
                    ));
                }
                args.values.insert(def.identifier, c.value);
                args.is_set.insert(index);
            }
            None => {
                args.values.insert(def.identifier, Value::default_for(def));
            }
        }
    }
    if let Some((k, _)) = kw.into_iter().flatten().next() {
        let valid: Vec<&str> = spec.props.iter().map(|p| p.identifier).collect();
        return Err(FrapError::new(
            PyExc::TypeError,
            OP_ARG_UNKNOWN,
            format!("Converting py args to operator properties:: keyword \"{k}\" unrecognized"),
        )
        .with_detail("operator", spec.idname)
        .with_detail("argument", k.as_str())
        .with_detail(
            "did_you_mean",
            suggest(&k, valid.iter().copied())
                .into_iter()
                .map(J::from)
                .collect::<Vec<_>>(),
        )
        .with_detail("valid", valid.into_iter().map(J::from).collect::<Vec<_>>()));
    }
    Ok((args, reports))
}

/// §10.2 step 5: poll, with Blender's message and frap's structured help.
pub fn check_poll(def: &OperatorDef, ctx: &Context) -> Result<(), FrapError> {
    (def.poll.check)(ctx).map_err(|f| {
        let msg = f.message.unwrap_or_else(|| {
            format!(
                "Operator bpy.ops.{}.poll() failed, context is incorrect",
                def.spec.idname
            )
        });
        let mut e = FrapError::new(PyExc::RuntimeError, OP_POLL_FAILED, msg)
            .with_detail(
                "requires",
                def.poll
                    .requires
                    .iter()
                    .map(|r| J::from(format!("{r:?}")))
                    .collect::<Vec<_>>(),
            )
            .with_detail(
                "unmet",
                f.unmet
                    .iter()
                    .map(|r| J::from(format!("{r:?}")))
                    .collect::<Vec<_>>(),
            );
        e.hint = f.hint;
        e
    })
}
