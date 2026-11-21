// SPDX-License-Identifier: GPL-3.0-or-later
//! `TimelineMarker.name` (WP 7.6): stored as given, cut to 63 bytes on a character boundary
//! (`TimeMarker::name[64]`); not made unique (verified on 5.2.4: two markers named `B`).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};
use smol_str::SmolStr;

use crate::Host;
use crate::id::timeline_marker::{NAME_MAX_BYTES, with_marker_mut};

#[rna_hook(srna = "TimelineMarker", prop = "name", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let name = v.as_str().unwrap_or_default().to_owned();
    let (main, log) = h.main_mut();
    with_marker_mut(main, log, this, |m| {
        m.name = SmolStr::new(frap_rna::naming::truncate_utf8(&name, NAME_MAX_BYTES));
    })
}
