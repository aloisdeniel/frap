// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers of the evaluated-data hooks of this directory. Not a concept: the leading
//! underscore keeps it out of the §10.5 file checks.

use std::sync::Arc;

use frap_rna::{IdRef, ObjectId, PyExc, RnaError, RnaErrorKind, RnaPtr, Root};
use frap_scene::{EvalMode, EvaluatedObject, HostRead};

/// An inconsistency between the hook target and the data (should not happen).
pub(crate) fn internal(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("frap internal: {msg}"),
    )
}

/// The object at the root of `this` (`Object.dimensions`, `Object.bound_box`).
pub(crate) fn object_of(this: &RnaPtr) -> Result<IdRef<ObjectId>, RnaError> {
    match this.root {
        Root::Id(a) => a.typed().ok_or_else(|| internal("not an object")),
        _ => Err(internal(format!(
            "{} is not rooted at an object",
            this.srna.identifier
        ))),
    }
}

/// The evaluated object, always fresh with respect to the originals (D-04, §7.3). Get hooks
/// only see `&dyn HostRead`, so they read through the per-thread read cache
/// (`EvalCache::evaluated_for_read`, keyed by generation fingerprint): the modifier stack is
/// evaluated once per change, not once per read (§21.7; M8 WP 8.8 replaced a throwaway cache
/// that re-evaluated every modifier on every `dimensions` / `bound_box` read).
pub(crate) fn evaluated_read(
    h: &dyn HostRead,
    ob: IdRef<ObjectId>,
) -> Result<Arc<EvaluatedObject>, RnaError> {
    frap_eval::EvalCache::evaluated_for_read(h, ob, EvalMode::Viewport)
        .map_err(|e| internal(e.message))
}
