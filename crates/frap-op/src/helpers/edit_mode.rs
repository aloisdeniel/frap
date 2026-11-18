// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit Mode bookkeeping shared by the operator crates (the `object.mode_set` handler lives in
//! `frap-ops-object/src/modes/edit_mesh.rs`, the `.frap` loader in `frap-ops-io`; both reach it
//! through `frap-op`). Owned by WP 2.2 (Edit-mode integration, ARCHITECTURE §22.4).
//!
//! `.frap` re-enters Edit Mode on load, as Blender does for a `.blend` saved in Edit Mode: a
//! port of the mode loop of `ED_editors_init` (`editors/util/ed_util.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). In `bpy.data.objects` order, each object saved in Edit Mode
//! - keeps it untouched when its mesh already has an edit BMesh (a second user of a mesh that
//!   re-entered: "for multi-edit mode we may already have mode data");
//! - is otherwise reset to Object Mode, and re-enters only when there is an active object of
//!   the same type, the object and its data are local, it is in the scene and not hidden in
//!   the viewport (`hide_viewport`); verified on 5.2.4: a file saved with the camera active
//!   loads both edited meshes in Object Mode, a hidden edit object loads in Object Mode.
//!
//! Each mesh gets its edit BMesh back from the saved mesh (which `wm.save_as_mainfile` wrote
//! from the edit-mode view, selection included). The selection history is not stored in a
//! `.frap` (Blender keeps it in `Mesh.mselect`), so it restarts empty. Objects of a type
//! without an Edit Mode handler (armatures before M6) come back in Object Mode with a WARNING.

use frap_rna::{IdRef, ObjectId, ReportType};
use frap_scene::Main;
use frap_scene::ObjectMode;
use frap_scene::id::{Collection, Scene};

use crate::ctx::OpCtx;
use crate::def::{OpError, OpStatus};
use crate::helpers::mode_handler;

/// `BKE_scene_has_object`: `ob` is in the scene's collection hierarchy (excluded collections
/// included).
fn scene_has_object(main: &Main, scene: &Scene, ob: IdRef<ObjectId>) -> bool {
    fn walk(main: &Main, c: &Collection, ob: IdRef<ObjectId>, depth: usize) -> bool {
        depth <= 256
            && (c.objects.contains(&ob)
                || c.children
                    .iter()
                    .filter_map(|ch| main.collections.get(*ch))
                    .any(|cc| walk(main, cc, ob, depth + 1)))
    }
    walk(main, &scene.collection, ob, 0)
}

/// After `wm.open_mainfile` replaced the state (`EditState` empty): the objects saved in Edit
/// Mode re-enter it through the `EDIT` mode handler of their type when `ED_editors_init`
/// would (see the module docs).
pub fn reenter_after_load(cx: &mut OpCtx) -> Result<(), OpError> {
    let (editing, active_type) = {
        let c = cx.ctx();
        let editing: Vec<IdRef<ObjectId>> = c
            .main
            .objects
            .iter()
            .filter(|(_, o)| o.mode == ObjectMode::Edit)
            .map(|(r, _)| r)
            .collect();
        let active_type = c
            .active_object()
            .and_then(|r| c.main.objects.get(r))
            .map(|o| o.object_type());
        (editing, active_type)
    };
    for r in editing {
        let (ty, name, has_mode_data, eligible) = {
            let (c, edit) = cx.split_edit();
            let Some(o) = c.main.objects.get(r) else {
                continue;
            };
            // `BKE_object_has_mode_data(ob, OB_MODE_EDIT)`.
            let has_mode_data = o.data.is_some_and(|d| {
                edit.meshes.contains_key(&d.key) || edit.armatures.contains_key(&d.key)
            });
            let ty = o.object_type();
            let data_local = o
                .data
                .and_then(|d| c.main.header(d))
                .is_none_or(|h| h.library.is_none());
            let in_scene = scene_has_object(c.main, c.scene(), r);
            let eligible = active_type == Some(ty)
                && o.header.library.is_none()
                && data_local
                && in_scene
                && !o.hide_viewport;
            (ty, o.header.name.to_string(), has_mode_data, eligible)
        };
        if has_mode_data {
            continue;
        }
        // "Reset object to Object mode, so that code below can properly re-switch it".
        cx.object_mut(r)?.mode = ObjectMode::Object;
        if !eligible {
            continue;
        }
        let why = match mode_handler(ObjectMode::Edit, ty) {
            Ok(h) => {
                if matches!((h.enter)(cx, &[r])?, OpStatus::Finished) {
                    continue;
                }
                "it could not re-enter Edit Mode".to_owned()
            }
            Err(_) => format!("no Edit Mode for {ty} objects yet"),
        };
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
