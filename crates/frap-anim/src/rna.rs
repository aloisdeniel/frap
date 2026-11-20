// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA bindings of the animation data (WP 6.1): `Action` (slots, layers, strips, channelbags,
//! groups, F-curves, keyframe points; `Action.fcurves` → `RNA_REMOVED_IN_5X` with the 5.x path
//! as hint), `AnimData`, `NlaTrack`, `NlaStrip`. Stubs until WP 6.1 lands (M5+M6 W0).

use frap_rna::*;

use crate::action::Action;

/// `Action` property reads.
pub(crate) fn action_get<'a>(a: &'a Action, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    let _ = (a, prop);
    todo!("M6 WP 6.1: Action RNA bindings")
}

/// `Action` property writes.
pub(crate) fn action_set(
    a: &mut Action,
    prop: &'static PropDef,
    v: Value,
) -> Result<SetOutcome, RnaError> {
    let _ = (a, prop, v);
    todo!("M6 WP 6.1: Action RNA bindings")
}

/// `Action` nested structs.
pub(crate) fn action_struct_mut<'a>(
    a: &'a mut Action,
    prop: &'static PropDef,
) -> Result<RnaMutGuard<'a>, RnaError> {
    let _ = (a, prop);
    todo!("M6 WP 6.1: Action RNA bindings")
}

/// `Action` collections.
pub(crate) fn action_coll_mut<'a>(
    a: &'a mut Action,
    prop: &'static PropDef,
) -> Result<RnaCollMutGuard<'a>, RnaError> {
    let _ = (a, prop);
    todo!("M6 WP 6.1: Action RNA bindings")
}
