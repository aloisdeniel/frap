// SPDX-License-Identifier: GPL-3.0-or-later
//! `TimelineMarker.frame` (WP 7.6): any `int` (the RNA range is the whole `int` range). Its
//! update callback only sends notifiers (`rna_TimelineMarker_update`): moving a marker does not
//! switch the scene camera (verified on 5.2.4: the camera stays until the next `frame_set`).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::timeline_marker::with_marker_mut;

#[rna_hook(srna = "TimelineMarker", prop = "frame", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let Value::Int(f) = v else {
        return Ok(());
    };
    let frame = f.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let (main, log) = h.main_mut();
    with_marker_mut(main, log, this, |m| m.frame = frame)
}
