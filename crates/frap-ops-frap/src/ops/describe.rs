// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.frap.describe(target='mesh.bevel')`, tier T0: the `--describe` manifest fragment
//! of an operator, a struct or a struct member, inside a session (ARCHITECTURE §14, §22.4
//! WP 2.13; spec `spec/frap/ops/frap.json`). Blender has no such operator.
//!
//! The manifest lives in `frap-schema` (L11), above the operator crates, so this operator
//! calls it through `frap_op::DESCRIBE_PROVIDERS` (`frap-schema/src/provider.rs`). `target`
//! takes every form of `frap --describe=<target>` and the `frap_describe` tool:
//! `op:mesh.bevel`, `mesh.bevel`, `bpy.ops.mesh.bevel`, `struct:Object`, `Object`,
//! `Object.location`, `Object.select_set`, `errors`, `protocol`, … The operator changes
//! nothing and answers in `result`:
//!
//! - `target`: the target as given;
//! - `describe`: the fragment, exactly what `frap --describe=<target>` prints.
//!
//! An unknown target (operator, struct, member or target form) raises `KeyError` with code
//! `DESCRIBE_TARGET` and `detail.did_you_mean`, as `frap --describe` does. `target` is required
//! (its spec flags are `REQUIRED`, `NEVER_NULL`): leaving it out, passing `None` (Blender's
//! "doesn't support None from string types") or passing an empty or whitespace-only string
//! raises `TypeError` (`OP_ARG_TYPE`) instead of answering the whole manifest (about 5 MB),
//! which only `target='manifest'` asks for. A build that
//! does not link `frap-schema` (no registered provider) answers `OP_NOT_IMPLEMENTED`.

use frap_op::errors::OP_ARG_TYPE;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{FrapError, PyExc};
use frap_spec_ops_frap::frap;

/// The message when no provider is registered (the binary does not link `frap-schema`).
const NO_PROVIDER: &str = "frap.describe: this build does not include the --describe manifest (frap-schema); run `frap --describe=<target>` instead";

/// The hint of both `target` errors.
const TARGET_HINT: &str = "bpy.ops.frap.describe(target='mesh.bevel'); targets: op:<idname>, struct:<Struct>, <Struct>.<prop>, errors, manifest";

/// `TypeError` for a call without `target` (Blender's wording for a missing required parameter).
fn missing_target() -> FrapError {
    FrapError::new(
        PyExc::TypeError,
        OP_ARG_TYPE,
        "frap.describe(): required parameter \"target\" not specified",
    )
    .with_hint(TARGET_HINT)
}

/// `TypeError` for an empty or whitespace-only `target`: `frap --describe=` would answer the
/// whole manifest, which a session asks for only with `target='manifest'`.
fn empty_target() -> FrapError {
    FrapError::new(
        PyExc::TypeError,
        OP_ARG_TYPE,
        "frap.describe(): parameter \"target\" is empty; target='manifest' asks for the whole manifest",
    )
    .with_hint(TARGET_HINT)
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    notes = [
        "frap extension: answers result.describe, the fragment `frap --describe=<target>` prints, and changes nothing",
        "targets: op:<idname> (also mesh.bevel, bpy.ops.mesh.bevel), struct:<Struct> (also Object), <Struct>.<prop>, errors, protocol, …; an unknown one is KeyError DESCRIBE_TARGET with detail.did_you_mean",
    ]
)]
fn exec(cx: &mut OpCtx, p: frap::Describe) -> OpResult {
    if !p.target.is_set {
        return Err(OpError::Frap(Box::new(missing_target())));
    }
    let target = p.target.value;
    if target.trim().is_empty() {
        return Err(OpError::Frap(Box::new(empty_target())));
    }
    let provider =
        frap_op::describe_provider().ok_or_else(|| OpError::NotImplemented(NO_PROVIDER.into()))?;
    let fragment = (provider.describe)(&target).map_err(|e| OpError::Frap(Box::new(e)))?;
    cx.result("target", target.as_str());
    cx.result("describe", Value::Json(Box::new(fragment)));
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_target_is_a_type_error() {
        let e = missing_target();
        assert_eq!(e.exception, PyExc::TypeError);
        assert_eq!(e.code, OP_ARG_TYPE);
        assert!(e.message.contains("\"target\""));
        assert!(e.hint.as_deref().unwrap().contains("target='mesh.bevel'"));
    }

    #[test]
    fn empty_target_is_a_type_error() {
        let e = empty_target();
        assert_eq!(e.exception, PyExc::TypeError);
        assert_eq!(e.code, OP_ARG_TYPE);
        assert!(e.message.contains("target='manifest'"));
        assert!(e.hint.as_deref().unwrap().contains("manifest"));
    }
}
