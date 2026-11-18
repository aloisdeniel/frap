// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit Mode bookkeeping shared by the operator crates (the `object.mode_set` handler lives in
//! `frap-ops-object`, the `.frap` loader in `frap-ops-io`; both reach it through `frap-op`).
//!
//! **Functional stub** created by M2 W0 (WP 2.0): `reenter_after_load` keeps the M1 behaviour
//! (objects saved in Edit Mode come back in Object Mode with a WARNING). WP 2.2 (Edit-mode
//! integration, ARCHITECTURE §22.4, acceptance "`.frap` re-enters Edit Mode on load") makes it
//! rebuild the `EditState` (`BMesh::from_mesh` per unique mesh) instead.

use frap_rna::ReportType;
use frap_scene::ObjectMode;

use crate::ctx::OpCtx;
use crate::def::OpError;

/// After `wm.open_mainfile` replaced the state (`EditState` empty): objects whose `mode` is
/// `EDIT` re-enter Edit Mode.
pub fn reenter_after_load(cx: &mut OpCtx) -> Result<(), OpError> {
    let editing: Vec<_> = cx
        .main()
        .objects
        .iter()
        .filter(|(_, o)| o.mode == ObjectMode::Edit)
        .map(|(r, o)| (r, o.header.name.clone()))
        .collect();
    for (r, name) in editing {
        cx.object_mut(r)?.mode = ObjectMode::Object;
        cx.report(
            ReportType::Warning,
            format!(
                "frap: {name:?} was saved in Edit Mode; it is loaded in Object Mode (Edit Mode arrives in M2)"
            ),
        );
    }
    Ok(())
}
