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
//!
//! **Shape keys** (WP 6.3, `BM_mesh_bm_from_me` / `bm_to_mesh_shape`): entering loads every
//! key block as a shape layer (tagged with the block's uid) and takes the vertex positions
//! from the object's active block (`active_shape_key_index`;
//! `object_shapenr_basis_index_ensured` makes "none" the basis), which the BMesh keeps
//! (`bm->shapenr`); leaving writes the edited positions into that block, offsets the blocks
//! that use an edited basis by the edit (edit-mode basis propagation, skipped once a vertex
//! was created from an example), and keeps the mesh positions at the reference key's when
//! another block was edited (`frap_bmesh::shape`). Blocks added or removed through RNA while
//! editing are matched by uid.

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
        let mut bm = BMesh::from_mesh(mesh, &FromMeshParams::default());
        if let Some(k) = mesh.shape_keys {
            // `BM_mesh_bm_from_me`'s uid check (blocks made by `object.join` or a reader).
            if cx.main().shape_keys.get(k).is_some_and(|k| !k.uids_valid()) {
                let (main, log) = cx.main_mut();
                if let Some(key) = main.shape_keys.get_mut(k, log) {
                    key.ensure_uids();
                }
            }
            if let Some(key) = cx
                .main()
                .shape_keys
                .get(k)
                .filter(|k| !k.key_blocks.is_empty())
            {
                key.load_into_bmesh(&mut bm, active_block(cx, ob));
            }
        }
        // `BKE_editmesh_looptris_and_normals_calc` after `EDBM_mesh_make`: the BMesh normals
        // are recomputed BMesh-style, so a zero-area face has a zero normal (the Mesh's
        // (0, 0, 1) fallback does not reach the vertex normals the edit tools read: an
        // `inset(depth=…)` after an in-place `extrude_faces_indiv`, M8 stabilization round 3).
        bm.normals_update();
        bm
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

/// `ob->shapenr` (`active_shape_key_index + 1`).
fn shapenr(cx: &OpCtx, ob: IdRef<ObjectId>) -> i32 {
    cx.main()
        .objects
        .get(ob)
        .map_or(0, |o| o.active_shape_key_index.saturating_add(1))
}

/// The 0-based active key block of `ob` on entering (`object_shapenr_basis_index_ensured`:
/// `shapenr == 0` means the basis; a negative one none).
fn active_block(cx: &OpCtx, ob: IdRef<ObjectId>) -> Option<usize> {
    match shapenr(cx, ob) {
        0 => Some(0),
        n => usize::try_from(n - 1).ok(),
    }
}

/// `EDBM_mesh_load_ex` + free: writes the edit BMesh of `me` back and drops it. The edited
/// block is the one the BMesh was made with (`bm->shapenr`); `ob_shapenr` is the `shapenr` of
/// the object leaving Edit Mode, whose 0 makes it the basis (the #42360 workaround).
fn load(cx: &mut OpCtx, me: IdRef<MeshId>, ob_shapenr: i32) -> Result<(), OpError> {
    let em = cx.split_edit().1.meshes.remove(&me.key());
    if let Some(em) = em
        && let Some(template) = cx.main().meshes.get(me)
    {
        let params = ToMeshParams {
            calc_object_remap: true,
            update_shapekey_indices: false,
        };
        let mut loaded = em.bm.to_mesh(template, &params);
        if let Some(k) = template.shape_keys {
            let mut bm = Arc::unwrap_or_clone(em.bm);
            let (main, log) = cx.main_mut();
            if let Some(key) = main
                .shape_keys
                .get_mut(k, log)
                .filter(|k| !k.key_blocks.is_empty())
            {
                if ob_shapenr == 0 {
                    bm.shapenr = 1;
                }
                if let Some(pos) = key.write_back_from_bmesh(&mut bm) {
                    loaded.set_positions(pos);
                }
            }
        }
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
            let nr = shapenr(cx, ob);
            load(cx, me, nr)?;
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
        let user = cx
            .main()
            .objects
            .iter()
            .find(|(_, o)| o.data == Some(me.any()))
            .map(|(r, _)| r);
        let nr = user.map_or(0, |ob| shapenr(cx, ob));
        load(cx, me, nr)?;
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
