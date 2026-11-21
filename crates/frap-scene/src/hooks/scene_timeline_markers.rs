// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.timeline_markers` (WP 7.6): the scene's markers in list order, keyed by name (the
//! first match: names repeat) and addressed by index
//! ([`crate::id::timeline_marker::TimelineMarkerColl`]). Its functions are `TimelineMarkers.new`,
//! `remove` and `clear` (`frap-ops-data/src/funcs/timeline_markers_*.rs`).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::id::paths::{internal, scene_of};
use crate::id::timeline_marker::TimelineMarkerColl;

#[rna_hook(srna = "Scene", prop = "timeline_markers", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let sc = h
        .main()
        .scenes
        .get(scene_of(this)?)
        .ok_or_else(|| internal("Scene.timeline_markers: the scene was removed"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(TimelineMarkerColl {
        markers: &sc.timeline_markers,
    }))))
}
