// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh Edit Mode handler of `MODE_HANDLERS` (ARCHITECTURE §6.3, §8; WP 2.2, M2). Port of
//! `editmode_enter_ex` / `editmode_exit_ex` (`editors/object/object_edit.cc`) and
//! `EDBM_mesh_make` / `EDBM_mesh_load_ex` (`editors/mesh/editmesh_utils.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! - **Enter** (multi-object edit: `object.mode_set` passes the active object, then every
//!   selected object of its type): each mesh gets `BMesh::from_mesh` (normals computed) into
//!   `EditState.meshes`, `bm->selectmode` from `tool_settings.mesh_select_mode`, then the
//!   selection is flushed for that mode (`EDBM_selectmode_flush`). An object whose mesh is
//!   already in Edit Mode (a second user of the same mesh) is skipped and keeps its mode, as
//!   `BKE_object_is_in_editmode` makes Blender do; so are linked objects and linked meshes.
//!   When the first (active) object cannot enter, nothing does and the operator is cancelled.
//! - **Exit**: the BMesh is written back with `BMesh::to_mesh` (the original mesh is the
//!   template for everything but geometry), dropped, and `Object.mode` goes back to `OBJECT`.
//!   An object in Edit Mode whose mesh has no edit BMesh (it shares the mesh with an object
//!   that already left) only gets its mode reset.
//!
//! `bm_gen` starts from the process-global generation counter, so a BMesh entered again never
//! reuses the generation of an earlier one (evaluation caches key on it).

use std::sync::Arc;

use frap_bmesh::{BMesh, FromMeshParams, SelectMode, ToMeshParams};
use frap_op::prelude::*;
use frap_op::{MODE_HANDLERS, ModeHandler};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::{EditMesh, ObjectMode, ViewCache};

/// `tool_settings.mesh_select_mode` (`ts->selectmode`) as BMesh select-mode bits; Blender never
/// stores an empty mode (its setter refuses one), frap falls back to vertex mode.
pub(crate) fn select_mode_of(bits: [bool; 3]) -> SelectMode {
    let mut m = SelectMode::empty();
    for (on, b) in bits
        .iter()
        .zip([SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE])
    {
        if *on {
            m |= b;
        }
    }
    if m.is_empty() { SelectMode::VERT } else { m }
}

/// Why an object cannot enter Edit Mode (`editmode_enter_ex` returning false).
enum Skip {
    /// Not a mesh object with mesh data, or gone: nothing happens.
    Silently,
    /// The mesh is already in Edit Mode (another user of it entered): Blender returns
    /// "entered" without touching this object.
    AlreadyEditing,
    /// Linked object or linked mesh (`Unable to enter edit-mode on library data …`).
    Library(String),
}

/// Checks one object; `Ok(mesh)` when it can enter.
fn can_enter(cx: &mut OpCtx, ob: IdRef<ObjectId>) -> Result<IdRef<MeshId>, Skip> {
    let (c, edit) = cx.split_edit();
    let o = c.main.objects.get(ob).ok_or(Skip::Silently)?;
    let me = o
        .data
        .and_then(|d| d.typed::<MeshId>())
        .ok_or(Skip::Silently)?;
    let mesh = c.main.meshes.get(me).ok_or(Skip::Silently)?;
    if o.header.library.is_some() || mesh.header.library.is_some() {
        return Err(Skip::Library(o.header.name.to_string()));
    }
    if edit.meshes.contains_key(&me.key()) {
        // `BKE_object_is_in_editmode`: another object in Edit Mode uses this mesh. An entry no
        // object in Edit Mode uses any more is stale (cannot happen through the guarded paths)
        // and is replaced.
        let users_editing = c
            .main
            .objects
            .iter()
            .any(|(r, x)| r != ob && x.mode == ObjectMode::Edit && x.data == Some(me.any()));
        if users_editing {
            return Err(Skip::AlreadyEditing);
        }
        edit.meshes.remove(&me.key());
    }
    Ok(me)
}

/// `editmode_enter_ex` for one mesh object that can enter.
fn enter_one(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    mode: SelectMode,
) -> Result<(), OpError> {
    if cx
        .main()
        .objects
        .get(ob)
        .is_some_and(|o| o.mode != ObjectMode::Edit)
    {
        cx.object_mut(ob)?.mode = ObjectMode::Edit;
        cx.main_mut().1.context_changed = true;
    }
    let mut bm = {
        let mesh = cx
            .main()
            .meshes
            .get(me)
            .ok_or_else(|| OpError::Runtime("mesh no longer exists".into()))?;
        BMesh::from_mesh(mesh, &FromMeshParams::default())
    };
    // `EDBM_mesh_make`: the scene's select mode, then a flush for it (the mode may differ from
    // the one the selection was made in).
    bm.select_mode = mode;
    bm.select_flush_mode();
    let (_, edit) = cx.split_edit();
    edit.meshes.insert(
        me.key(),
        EditMesh {
            bm: Arc::new(bm),
            bm_gen: frap_rna::id::next_generation(),
            view: ViewCache::default(),
        },
    );
    Ok(())
}

/// Enters mesh Edit Mode for `objects` (the active object first).
fn enter(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    let mode = select_mode_of(cx.ctx().tool_settings().mesh_select_mode);
    for (i, &ob) in objects.iter().enumerate() {
        match can_enter(cx, ob) {
            Ok(me) => enter_one(cx, ob, me, mode)?,
            Err(Skip::AlreadyEditing) => {}
            Err(Skip::Silently) if i > 0 => {}
            Err(Skip::Library(name)) if i > 0 => {
                cx.report(
                    ReportType::Warning,
                    format!("Unable to enter edit-mode on library data for object '{name}'"),
                );
            }
            Err(Skip::Silently) => return Ok(OpStatus::Cancelled),
            Err(Skip::Library(name)) => {
                cx.report(
                    ReportType::Warning,
                    format!("Unable to enter edit-mode on library data for object '{name}'"),
                );
                return Ok(OpStatus::Cancelled);
            }
        }
    }
    Ok(OpStatus::Finished)
}

/// `EDBM_mesh_load_ex` + free: writes the edit BMesh of `me` back and drops it.
fn load(cx: &mut OpCtx, me: IdRef<MeshId>) -> Result<(), OpError> {
    let em = cx.split_edit().1.meshes.remove(&me.key());
    if let Some(em) = em
        && let Some(template) = cx.main().meshes.get(me)
    {
        let params = ToMeshParams {
            calc_object_remap: true,
            update_shapekey_indices: false,
        };
        let loaded = em.bm.to_mesh(template, &params);
        let m = cx.mesh_mut(me)?;
        // Keep the header `mesh_mut` just bumped (generation, name).
        let header = m.header.clone();
        *m = loaded;
        m.header = header;
    }
    Ok(())
}

/// Leaves mesh Edit Mode for `objects`: `EDBM_mesh_load_ex` + free, `Object.mode = 'OBJECT'`.
/// Afterwards no edit BMesh outlives every object in Edit Mode that uses its mesh: an orphan
/// (its mesh no longer the data of an object in Edit Mode) is written back and dropped too, so
/// the mesh never stays locked against writes in Object Mode.
fn exit(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    for &ob in objects {
        let Some(o) = cx.main().objects.get(ob) else {
            continue;
        };
        if o.mode != ObjectMode::Edit {
            continue;
        }
        if let Some(me) = o.data.and_then(|d| d.typed::<MeshId>()) {
            load(cx, me)?;
        }
        cx.object_mut(ob)?.mode = ObjectMode::Object;
        cx.main_mut().1.context_changed = true;
    }
    let orphans: Vec<IdRef<MeshId>> = {
        let (c, edit) = cx.split_edit();
        edit.meshes
            .keys()
            .map(|k| IdRef::<MeshId>::from_key(*k))
            .filter(|me| {
                !c.main
                    .objects
                    .iter()
                    .any(|(_, o)| o.mode == ObjectMode::Edit && o.data == Some(me.any()))
            })
            .collect()
    };
    for me in orphans {
        load(cx, me)?;
    }
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(MODE_HANDLERS)]
static EDIT_MESH: ModeHandler = ModeHandler {
    mode: ObjectMode::Edit,
    object_type: "MESH",
    enter,
    exit,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_mode_follows_tool_settings() {
        assert_eq!(select_mode_of([true, false, false]), SelectMode::VERT);
        assert_eq!(
            select_mode_of([false, true, true]),
            SelectMode::EDGE | SelectMode::FACE
        );
        assert_eq!(select_mode_of([false, false, false]), SelectMode::VERT);
    }
}
