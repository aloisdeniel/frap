// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit Mode bookkeeping shared by the operator crates (the `object.mode_set` handler lives in
//! `frap-ops-object/src/modes/edit_mesh.rs`, the `.frap` loader in `frap-ops-io`; both reach it
//! through `frap-op`). Owned by WP 2.2 (Edit-mode integration, ARCHITECTURE §22.4).
//!
//! `.frap` re-enters Edit Mode on load, as Blender does for a `.blend` saved in Edit Mode
//! (`ED_editors_init` calls `editmode_enter_ex` for objects whose `mode` is `EDIT`): the
//! objects keep their `EDIT` mode and each mesh gets its edit BMesh back from the saved mesh
//! (which `wm.save_as_mainfile` wrote from the edit-mode view, selection included). The
//! selection history is not stored in a `.frap` (Blender keeps it in `Mesh.mselect`), so it
//! restarts empty. Objects of a type without an Edit Mode handler (armatures before M6) come
//! back in Object Mode with a WARNING.

use frap_rna::{IdRef, ObjectId, ReportType};
use frap_scene::ObjectMode;

use crate::ctx::OpCtx;
use crate::def::{OpError, OpStatus};
use crate::helpers::mode_handler;

/// After `wm.open_mainfile` replaced the state (`EditState` empty): objects whose `mode` is
/// `EDIT` re-enter Edit Mode through the `EDIT` mode handler of their type, the active object
/// first, then the others in `bpy.data.objects` order.
pub fn reenter_after_load(cx: &mut OpCtx) -> Result<(), OpError> {
    let (editing, active) = {
        let c = cx.ctx();
        let editing: Vec<(IdRef<ObjectId>, &'static str)> = c
            .main
            .objects
            .iter()
            .filter(|(_, o)| o.mode == ObjectMode::Edit)
            .map(|(r, o)| (r, o.object_type()))
            .collect();
        (editing, c.active_object())
    };
    let mut ordered = editing;
    ordered.sort_by_key(|(r, _)| Some(*r) != active);
    for (r, ty) in ordered {
        let why = match mode_handler(ObjectMode::Edit, ty) {
            Ok(h) => {
                if matches!((h.enter)(cx, &[r])?, OpStatus::Finished) {
                    continue;
                }
                "it could not re-enter Edit Mode".to_owned()
            }
            Err(_) => format!("no Edit Mode for {ty} objects yet"),
        };
        let name = cx
            .main()
            .objects
            .get(r)
            .map(|o| o.header.name.to_string())
            .unwrap_or_default();
        if cx
            .main()
            .objects
            .get(r)
            .is_some_and(|o| o.mode != ObjectMode::Object)
        {
            cx.object_mut(r)?.mode = ObjectMode::Object;
        }
        cx.report(
            ReportType::Warning,
            format!("frap: {name:?} was saved in Edit Mode; it is loaded in Object Mode ({why})"),
        );
    }
    Ok(())
}
