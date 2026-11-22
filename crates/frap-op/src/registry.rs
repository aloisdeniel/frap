// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime registry and the framework part of the invocation pipeline (ARCHITECTURE §10.1,
//! §10.2 steps 1–3 and 5). The session owns transactions and responses (steps 4, 6–9).
//!
//! The registry indexes three sources:
//! - the `OPERATORS` slice (implemented operators, one `OperatorDef` per file);
//! - the typed specs of the current milestone (`frap-spec-ops-<group>::ALL_OPS`);
//! - every other dumped Blender 5.2 operator (`frap_spec_catalog::catalog()`).
//!
//! A dumped operator without an `OperatorDef` is `KnownNotImplemented` (`OP_NOT_IMPLEMENTED`);
//! a name in none of them is `Unknown` (`OP_UNKNOWN`). Building the registry fails if two
//! `OperatorDef`s share an idname, or if an implemented operator has no typed spec (it is still
//! served by the catalog, or it carries a hand-written copy of its generated spec).

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use frap_rna::{FrapError, Input, OpArgs, OperatorSpec, PyExc};
use frap_scene::Context;
use serde_json::Value as J;

use frap_rna::Report;

use crate::def::{OPERATORS, OperatorDef};
use crate::errors::*;

/// Result of a registry lookup.
pub enum Lookup {
    /// Implemented by frap.
    Implemented(&'static OperatorDef),
    /// Dumped from Blender 5.2 (`frap-spec-catalog` or a typed spec) but not implemented:
    /// `OP_NOT_IMPLEMENTED`.
    KnownNotImplemented(&'static OperatorSpec),
    /// Unknown to Blender 5.2 and frap.
    Unknown {
        /// Close idnames (implemented or known), closest first.
        suggestions: Vec<&'static str>,
    },
}

/// Index over the `OPERATORS` slice and the dumped specs, built once per process.
pub struct Registry {
    by_idname: BTreeMap<&'static str, &'static OperatorDef>,
    /// Dumped operators without an implementation.
    known: BTreeMap<&'static str, &'static OperatorSpec>,
    /// Every spec (implemented or not) by `bl_idname` (macro steps name their sub-operators so).
    by_bl_idname: HashMap<&'static str, &'static OperatorSpec>,
    /// Implemented operators by `bl_idname`.
    defs_by_bl_idname: HashMap<&'static str, &'static OperatorDef>,
}

impl Registry {
    /// Every typed `OperatorSpec` of the current milestone (§4.2), across the ops groups.
    pub fn typed_specs() -> impl Iterator<Item = &'static OperatorSpec> {
        let groups: [&'static [&'static OperatorSpec]; 9] = [
            &frap_spec_ops_object::ALL_OPS,
            &frap_spec_ops_transform::ALL_OPS,
            &frap_spec_ops_mesh::ALL_OPS,
            &frap_spec_ops_uv::ALL_OPS,
            &frap_spec_ops_anim::ALL_OPS,
            &frap_spec_ops_data::ALL_OPS,
            &frap_spec_ops_io::ALL_OPS,
            &frap_spec_ops_render::ALL_OPS,
            &frap_spec_ops_frap::ALL_OPS,
        ];
        groups.into_iter().flatten().copied()
    }

    /// Builds a registry from implemented operators, the typed specs and the catalog.
    ///
    /// Fails (every problem listed, one per line) when two definitions share an idname, when an
    /// implemented operator is still served by the catalog (no typed spec: raise its
    /// OPERATORS.md milestone and run `cargo xtask codegen`), or when an implemented operator
    /// uses another spec than the generated static of its idname.
    pub fn build(
        defs: impl IntoIterator<Item = &'static OperatorDef>,
        typed: impl IntoIterator<Item = &'static OperatorSpec>,
        catalog: impl IntoIterator<Item = &'static OperatorSpec>,
    ) -> Result<Registry, String> {
        let mut problems: Vec<String> = Vec::new();
        let mut by_idname: BTreeMap<&'static str, &'static OperatorDef> = BTreeMap::new();
        let mut dup: BTreeMap<&'static str, usize> = BTreeMap::new();
        for def in defs {
            if by_idname.insert(def.spec.idname, def).is_some() {
                *dup.entry(def.spec.idname).or_insert(1) += 1;
            }
        }
        for (idname, n) in &dup {
            problems.push(format!(
                "duplicate operator registration: {idname} ({n} OperatorDefs share this idname)"
            ));
        }
        let typed: BTreeMap<&'static str, &'static OperatorSpec> =
            typed.into_iter().map(|s| (s.idname, s)).collect();
        let catalog: BTreeMap<&'static str, &'static OperatorSpec> =
            catalog.into_iter().map(|s| (s.idname, s)).collect();
        let mut known = BTreeMap::new();
        for (idname, spec) in typed.iter().chain(catalog.iter()) {
            match by_idname.get(idname) {
                None => {
                    known.insert(*idname, *spec);
                }
                Some(def) => {
                    if catalog.contains_key(idname) && !typed.contains_key(idname) {
                        problems.push(format!(
                            "operator {idname} is implemented but has no typed spec (it is \
                             served by frap-spec-catalog): raise its OPERATORS.md milestone \
                             and run `cargo xtask codegen`"
                        ));
                    } else if !std::ptr::eq(def.spec, *spec) {
                        problems.push(format!(
                            "operator {idname} does not use its generated spec static \
                             ({} of frap-spec-ops-*)",
                            spec.bl_idname
                        ));
                    }
                }
            }
        }
        if !problems.is_empty() {
            problems.sort();
            return Err(problems.join("\n"));
        }
        let mut by_bl_idname = HashMap::new();
        let mut defs_by_bl_idname = HashMap::new();
        for spec in known.values() {
            by_bl_idname.insert(spec.bl_idname, *spec);
        }
        for def in by_idname.values() {
            by_bl_idname.insert(def.spec.bl_idname, def.spec);
            defs_by_bl_idname.insert(def.spec.bl_idname, *def);
        }
        Ok(Registry {
            by_idname,
            known,
            by_bl_idname,
            defs_by_bl_idname,
        })
    }

    /// The process-wide registry, or why it cannot be built.
    pub fn try_global() -> Result<&'static Registry, &'static str> {
        static R: OnceLock<Result<Registry, String>> = OnceLock::new();
        R.get_or_init(|| {
            Registry::build(
                OPERATORS.iter(),
                Registry::typed_specs(),
                frap_spec_catalog::catalog().iter().copied(),
            )
        })
        .as_ref()
        .map_err(String::as_str)
    }

    /// The process-wide registry. Panics if it cannot be built (duplicate idnames, implemented
    /// operator without a typed spec); the registry tests catch this in CI.
    pub fn global() -> &'static Registry {
        match Registry::try_global() {
            Ok(r) => r,
            Err(e) => panic!("frap operator registry: {e}"),
        }
    }

    /// Every implemented idname, sorted.
    pub fn idnames(&self) -> Vec<&'static str> {
        self.by_idname.keys().copied().collect()
    }

    /// Every dumped idname without an implementation, sorted.
    pub fn known_idnames(&self) -> Vec<&'static str> {
        self.known.keys().copied().collect()
    }

    /// The implemented operator `ns.op`.
    pub fn get(&self, idname: &str) -> Option<&'static OperatorDef> {
        self.by_idname.get(idname).copied()
    }

    /// The spec of `ns.op`, implemented or not.
    pub fn spec(&self, idname: &str) -> Option<&'static OperatorSpec> {
        self.get(idname)
            .map(|d| d.spec)
            .or_else(|| self.known.get(idname).copied())
    }

    /// The spec of `NS_OT_op`, implemented or not.
    pub fn spec_by_bl_idname(&self, bl_idname: &str) -> Option<&'static OperatorSpec> {
        self.by_bl_idname.get(bl_idname).copied()
    }

    /// The implemented operator `NS_OT_op` (macro steps).
    pub fn get_by_bl_idname(&self, bl_idname: &str) -> Option<&'static OperatorDef> {
        self.defs_by_bl_idname.get(bl_idname).copied()
    }

    /// Lookup by `ns.op`.
    pub fn lookup(&self, idname: &str) -> Lookup {
        if let Some(d) = self.by_idname.get(idname) {
            return Lookup::Implemented(d);
        }
        if let Some(s) = self.known.get(idname) {
            return Lookup::KnownNotImplemented(s);
        }
        Lookup::Unknown {
            suggestions: suggest(
                idname,
                self.by_idname.keys().chain(self.known.keys()).copied(),
            ),
        }
    }
}

/// Up to three candidates within a small edit distance, closest first (ties by name).
pub fn suggest<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    frap_rna::error::suggest(word, candidates)
}

/// The unknown-operator error (`Calling …` for calls, `Polling …` for `poll()`; verified on
/// 5.2.4), with `did_you_mean`.
pub(crate) fn unknown_error(verb: &str, idname: &str, suggestions: &[&str]) -> FrapError {
    let e = FrapError::new(
        PyExc::AttributeError,
        OP_UNKNOWN,
        format!("{verb} operator \"bpy.ops.{idname}\" error, could not be found"),
    )
    .with_detail("operator", idname)
    .with_detail(
        "did_you_mean",
        suggestions.iter().map(|s| J::from(*s)).collect::<Vec<_>>(),
    );
    match suggestions.first() {
        Some(s) => e.with_hint(format!("did you mean bpy.ops.{s}?")),
        None => e.with_hint("frap --describe lists every Blender 5.2 operator"),
    }
}

/// The known-but-unimplemented error (`NotImplementedError`, `OP_NOT_IMPLEMENTED`) with the
/// Blender signature in `detail.signature` and the OPERATORS.md milestone in
/// `detail.planned` (`"M8"`, `"M9"` = post-M8, `"OUT"`; M8 W0, decision log M8W0-4): the hint
/// names what the row says, and for an operator without a row it says so (rule 3 makes it
/// `OUT`) instead of pointing at a row that does not exist.
pub(crate) fn not_implemented_error(spec: &OperatorSpec) -> FrapError {
    let row = crate::planned::row(spec.idname).filter(|r| r.section == 'A');
    let planned = row.map_or("OUT", |r| r.milestone);
    let status = match row {
        Some(r) if r.milestone == "OUT" => "OPERATORS.md marks it OUT (not planned)".to_owned(),
        Some(r) if r.milestone == crate::planned::POST_M8 => {
            "OPERATORS.md plans it after M8 (M9, the post-M8 backlog)".to_owned()
        }
        Some(r) => format!("OPERATORS.md plans it for {}", r.milestone),
        None => "OPERATORS.md does not list it, so it is OUT (not planned, rule 3)".to_owned(),
    };
    FrapError::new(
        PyExc::NotImplementedError,
        OP_NOT_IMPLEMENTED,
        format!(
            "{} exists in Blender 5.2 but is not implemented by frap",
            spec.idname
        ),
    )
    .with_detail("operator", spec.idname)
    .with_detail("signature", crate::def::pipeline::signature(spec))
    .with_detail("planned", planned)
    .with_hint(format!(
        "frap --describe op:{} shows its status; {status}",
        spec.idname
    ))
}

/// An operator Blender registers only while a condition holds, e.g. the glTF add-on's
/// variant operators while `KHR_materials_variants_ui` is on (`variant_register`). While
/// `registered` answers false, calling or polling it is Blender's unknown-operator error
/// (`Calling operator "bpy.ops.X" error, could not be found`), with `when` as the hint.
/// Extension point (M8 W0, decision log M8W0-6): the operator's file registers its gate.
#[derive(Debug)]
pub struct OpGate {
    /// `ns.op`
    pub idname: &'static str,
    /// Whether the operator is registered in this context.
    pub registered: fn(&Context) -> bool,
    /// How to register it (`error.hint` while it is not).
    pub when: &'static str,
}

/// Operators registered only under a condition ([`OpGate`]).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static OP_GATES: [OpGate];

/// The gate of `idname`, if it has one.
pub fn gate(idname: &str) -> Option<&'static OpGate> {
    OP_GATES.iter().find(|g| g.idname == idname)
}

/// [`lookup`] in a context: an operator whose [`OpGate`] is closed is unknown, as in Blender.
pub fn lookup_in(idname: &str, ctx: &Context) -> Result<&'static OperatorDef, FrapError> {
    if let Some(g) = gate(idname)
        && !(g.registered)(ctx)
    {
        let mut e = unknown_error("Calling", idname, &[]);
        e.hint = Some(g.when.to_owned());
        return Err(e);
    }
    lookup(idname)
}

/// §10.2 step 1: lookup, with Blender's error for unknown operators and `OP_NOT_IMPLEMENTED`
/// for dumped operators frap does not implement.
pub fn lookup(idname: &str) -> Result<&'static OperatorDef, FrapError> {
    match Registry::global().lookup(idname) {
        Lookup::Implemented(d) => Ok(d),
        Lookup::KnownNotImplemented(spec) => Err(not_implemented_error(spec)),
        Lookup::Unknown { suggestions } => Err(unknown_error("Calling", idname, &suggestions)),
    }
}

/// §10.2 steps 2–3 without a context: positional arguments and keyword conversion. Dynamic
/// enums are checked against their static superset (use [`crate::def::pipeline::prepare`] or
/// [`crate::def::pipeline::convert`] to validate them against the context). Returns the converted
/// arguments plus INFO reports for clamped values and the frap hints for the response
/// (`ANGLE_LIKELY_DEGREES`, §13.2).
pub fn convert_args(
    spec: &'static OperatorSpec,
    positional: Vec<Input>,
    kwargs: Vec<(String, Input)>,
) -> Result<(OpArgs, Vec<Report>, Vec<crate::def::pipeline::Hint>), FrapError> {
    let mut reports = crate::def::pipeline::check_positional(spec, &positional)?;
    let c = crate::def::pipeline::convert_kwargs(spec, &[], None, kwargs)?;
    reports.extend(c.reports);
    Ok((c.args, reports, c.hints))
}

/// §10.2 step 5: poll, with Blender's message and frap's structured help
/// (`detail.requires`, `detail.unmet` as [`crate::PollReq::code`]s, and `hint`).
pub fn check_poll(def: &OperatorDef, ctx: &Context) -> Result<(), FrapError> {
    (def.poll.check)(ctx).map_err(|f| {
        let msg = f.message.unwrap_or_else(|| {
            format!(
                "Operator bpy.ops.{}.poll() failed, context is incorrect",
                def.spec.idname
            )
        });
        let mut e = FrapError::new(PyExc::RuntimeError, OP_POLL_FAILED, msg)
            .with_detail("operator", def.spec.idname)
            .with_detail(
                "requires",
                def.poll
                    .requires
                    .iter()
                    .map(|r| J::from(r.code()))
                    .collect::<Vec<_>>(),
            )
            .with_detail(
                "unmet",
                f.unmet
                    .iter()
                    .map(|r| J::from(r.code()))
                    .collect::<Vec<_>>(),
            );
        e.hint = f.hint;
        e
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_closest_first() {
        let c = ["size", "calc_uvs", "scale", "align"];
        assert_eq!(suggest("sizee", c.iter().copied()), vec!["size"]);
        assert_eq!(suggest("scal", c.iter().copied()), vec!["scale"]);
        assert!(suggest("zzzzzzzz", c.iter().copied()).is_empty());
    }

    #[test]
    fn catalog_has_unimplemented_blender_ops() {
        let r = Registry::global();
        assert!(matches!(
            r.lookup("mesh.bevel"),
            Lookup::KnownNotImplemented(s) if s.bl_idname == "MESH_OT_bevel"
        ));
        assert!(r.spec_by_bl_idname("TRANSFORM_OT_translate").is_some());
    }
}
