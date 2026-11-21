// SPDX-License-Identifier: GPL-3.0-or-later
//! `TimelineMarker.select` (WP 7.6): the `SELECT` bit of `TimeMarker::flag`.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::timeline_marker::with_marker_mut;

#[rna_hook(srna = "TimelineMarker", prop = "select", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let Value::Bool(b) = v else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    with_marker_mut(main, log, this, |m| m.select = b)
}
