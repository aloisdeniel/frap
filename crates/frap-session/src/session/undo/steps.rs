// SPDX-License-Identifier: GPL-3.0-or-later
//! The undo step types frap keeps (`ED_undosys_type_init`, `editors/undo/
//! undo_system_types.cc`, Blender 5.2.4, GPL-2.0-or-later): **Edit Armature**, **Edit Mesh**
//! and the global **Memfile** fallback, chosen from the context in that order
//! (`BKE_undosys_type_from_context`). Ported from blender/source/blender/editors/mesh/
//! editmesh_undo.cc, editors/armature/editarmature_undo.cc, editors/undo/memfile_undo.cc and
//! the helpers of editors/undo/ed_undo.cc.
//!
//! Every step shares its data with the session state (`Arc`s, ARCHITECTURE §12: a snapshot is
//! O(#IDs)):
//!
//! - **Memfile**: the whole `State`. Decoding restores `bpy.data` (keeping `filepath` and
//!   `is_dirty`, which belong to the window manager in Blender), the edit data stored with it,
//!   the window's scene and view layer (`setup_app_data`'s `track_undo_scene` with one
//!   window: the step's scene), the pending-animation flags; the 3D View (screen data) and the
//!   preferences are not undone. Blender re-reads only the datablocks whose memfile chunks
//!   changed, so a change that was never pushed and never evaluated can survive a memfile undo
//!   there; frap always restores the stored state (deviations D-145).
//! - **Edit Mesh** (`MeshUndoStep`): per object in Edit Mode (the active one first, one per
//!   mesh: `ED_undo_editmode_objects_from_view_layer`) the edit BMesh, the mesh's vertex
//!   groups and the object's active group, plus the scene's mesh select mode and UV sync
//!   selection. Decoding puts exactly these objects in Edit Mode
//!   (`ED_undo_object_editmode_restore_helper`: others leave it, written back and deselected),
//!   replaces their BMesh with the stored one taken through `BM_mesh_bm_to_me` /
//!   `BM_mesh_bm_from_me` as `undomesh_from_editmesh` / `undomesh_to_editmesh` do (compact
//!   element pools, selection history and active face kept, normals recomputed), makes the
//!   first object active and restores the select mode. Nothing else is undone.
//! - **Edit Armature** (`ArmatureUndoStep`): per armature in Edit Mode the edit bones (with
//!   the active one) and the bone collections, restored the same way.

use std::sync::Arc;

use frap_bmesh::{BMElem, BMesh, EdgeId, FaceId, FromMeshParams, ToMeshParams, VertId};
use frap_op::OpCtx;
use frap_rna::{ArmatureId, IdKey, IdRef, MeshId, ObjectId, SceneId};
use frap_scene::id::BoneCollection;
use frap_scene::{ChangeLog, EditArmature, EditMesh, ObjectMode, ViewCache};
use smol_str::SmolStr;

use super::stack::StepData;
use crate::session::{Session, State};

/// The type of a step about to be pushed (`UndoType`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `BKE_UNDOSYS_TYPE_MEMFILE`.
    Memfile,
    /// `ED_mesh_undosys_type` ("Edit Mesh").
    EditMesh,
    /// `ED_armature_undosys_type` ("Edit Armature").
    EditArmature,
}

/// One object of an Edit Mesh step (`MeshUndoStep_Elem`).
#[derive(Clone, Debug)]
pub(crate) struct MeshElem {
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    bm: Arc<BMesh>,
    vertex_group_names: Vec<SmolStr>,
    vertex_group_locks: Vec<SmolStr>,
    vertex_groups_active_index: i32,
}

/// `MeshUndoStep`.
#[derive(Clone, Debug)]
pub(crate) struct EditMeshStep {
    scene: IdRef<SceneId>,
    /// `MeshUndoStep_SceneData::selectmode`.
    select_mode: [bool; 3],
    /// `UV_FLAG_SELECT_SYNC` of `uv_flag`.
    uv_select_sync: bool,
    elems: Vec<MeshElem>,
}

/// One object of an Edit Armature step.
#[derive(Clone, Debug)]
pub(crate) struct ArmatureElem {
    ob: IdRef<ObjectId>,
    arm: IdRef<ArmatureId>,
    edit: Arc<EditArmature>,
    collections: Vec<BoneCollection>,
    active_collection: Option<usize>,
}

/// `ArmatureUndoStep`.
#[derive(Clone, Debug)]
pub(crate) struct EditArmatureStep {
    scene: IdRef<SceneId>,
    elems: Vec<ArmatureElem>,
}

/// A step's data.
#[derive(Clone)]
pub(crate) enum Data {
    /// `MemFileUndoStep`: the whole state.
    Memfile(Box<State>),
    /// `MeshUndoStep`.
    EditMesh(EditMeshStep),
    /// `ArmatureUndoStep`.
    EditArmature(EditArmatureStep),
}

impl std::fmt::Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Data::Memfile(_) => f.write_str("Memfile"),
            Data::EditMesh(s) => f.debug_tuple("EditMesh").field(&s.elems.len()).finish(),
            Data::EditArmature(s) => f.debug_tuple("EditArmature").field(&s.elems.len()).finish(),
        }
    }
}

impl StepData for Data {
    fn is_memfile(&self) -> bool {
        matches!(self, Data::Memfile(_))
    }
}

#[cfg(test)]
impl Data {
    /// The type of the data.
    pub(crate) fn kind(&self) -> Kind {
        match self {
            Data::Memfile(_) => Kind::Memfile,
            Data::EditMesh(_) => Kind::EditMesh,
            Data::EditArmature(_) => Kind::EditArmature,
        }
    }
}

/// `BKE_view_layer_edit_object_get`: the active object when it is in Edit Mode, with its
/// type.
fn edit_object(state: &State) -> Option<(IdRef<ObjectId>, &'static str)> {
    let ctx = frap_scene::HostRead::ctx(state);
    let ob = ctx.edit_object()?;
    Some((ob, state.main.objects.get(ob)?.object_type()))
}

/// `BKE_undosys_type_from_context`: the first type whose poll passes: `editarm_object_from_
/// context` (an armature in Edit Mode), `editmesh_object_from_context` (a mesh with an edit
/// BMesh), then `memfile_undosys_poll` (global undo, refused in Edit Mode once a step exists:
/// `ED_undo_is_memfile_compatible`). `None`: nothing can be pushed.
pub(crate) fn kind_from_context(state: &State, stack_has_active: bool) -> Option<Kind> {
    if let Some((ob, ty)) = edit_object(state) {
        let data = state.main.objects.get(ob).and_then(|o| o.data);
        match ty {
            "ARMATURE" if data.is_some_and(|d| state.edit.armatures.contains_key(&d.key)) => {
                return Some(Kind::EditArmature);
            }
            "MESH" if data.is_some_and(|d| state.edit.meshes.contains_key(&d.key)) => {
                return Some(Kind::EditMesh);
            }
            _ => {}
        }
        return (!stack_has_active).then_some(Kind::Memfile);
    }
    Some(Kind::Memfile)
}

/// `ED_undo_editmode_objects_from_view_layer`: the active base (in Edit Mode) first, then every
/// base of the view layer whose object has its type and is in Edit Mode, one per data.
fn editmode_objects(state: &State) -> Vec<IdRef<ObjectId>> {
    let ctx = frap_scene::HostRead::ctx(state);
    let Some(active) = ctx.active_object() else {
        return Vec::new();
    };
    let Some(ao) = state.main.objects.get(active) else {
        return Vec::new();
    };
    if ao.mode != ObjectMode::Edit {
        return Vec::new();
    }
    let ty = ao.object_type();
    let mut data = vec![ao.data];
    let mut out = vec![active];
    for base in &ctx.view_layer().bases {
        let Some(o) = state.main.objects.get(base.object) else {
            continue;
        };
        if o.mode == ObjectMode::Edit && o.object_type() == ty && !data.contains(&o.data) {
            data.push(o.data);
            out.push(base.object);
        }
    }
    out
}

/// Encodes a step of `kind` from the current state (`undosys_step_encode`).
pub(crate) fn encode(state: &State, kind: Kind) -> Data {
    let scene = state.win.scene;
    match kind {
        Kind::Memfile => Data::Memfile(Box::new(state.clone())),
        Kind::EditMesh => {
            let ts = frap_scene::HostRead::ctx(state).tool_settings();
            let elems = editmode_objects(state)
                .into_iter()
                .filter_map(|ob| {
                    let o = state.main.objects.get(ob)?;
                    let me = o.data?.typed::<MeshId>()?;
                    let em = state.edit.meshes.get(&me.key())?;
                    let mesh = state.main.meshes.get(me)?;
                    Some(MeshElem {
                        ob,
                        me,
                        bm: Arc::clone(&em.bm),
                        vertex_group_names: mesh.vertex_group_names.clone(),
                        vertex_group_locks: mesh.vertex_group_locks.clone(),
                        vertex_groups_active_index: o.vertex_groups_active_index,
                    })
                })
                .collect();
            Data::EditMesh(EditMeshStep {
                scene,
                select_mode: ts.mesh_select_mode,
                uv_select_sync: ts.use_uv_select_sync,
                elems,
            })
        }
        Kind::EditArmature => {
            let elems = editmode_objects(state)
                .into_iter()
                .filter_map(|ob| {
                    let o = state.main.objects.get(ob)?;
                    let arm = o.data?.typed::<ArmatureId>()?;
                    let edit = state.edit.armatures.get(&arm.key())?;
                    let a = state.main.armatures.get(arm)?;
                    Some(ArmatureElem {
                        ob,
                        arm,
                        edit: Arc::clone(edit),
                        collections: a.collections.clone(),
                        active_collection: a.active_collection,
                    })
                })
                .collect();
            Data::EditArmature(EditArmatureStep { scene, elems })
        }
    }
}

/// Decodes `data` into the session (`undosys_step_decode`).
pub(crate) fn decode(session: &mut Session, data: &Data) {
    #[cfg(test)]
    if PANIC_ON_DECODE.with(std::cell::Cell::get) {
        panic!("a test's decode bug");
    }
    match data {
        Data::Memfile(snap) => decode_memfile(session, snap),
        Data::EditMesh(step) => decode_edit_mesh(session, step),
        Data::EditArmature(step) => decode_edit_armature(session, step),
    }
}

#[cfg(test)]
thread_local! {
    /// Tests: [`decode`] panics (an internal bug while a step is loaded).
    pub(crate) static PANIC_ON_DECODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `memfile_undosys_step_decode`.
fn decode_memfile(session: &mut Session, snap: &State) {
    let st = &mut session.host.state;
    let filepath = st.main.filepath.take();
    let is_dirty = st.main.is_dirty;
    st.main = snap.main.clone();
    st.main.filepath = filepath;
    st.main.is_dirty = is_dirty;
    st.edit = snap.edit.clone();
    st.win.scene = snap.win.scene;
    st.win.view_layer = snap.win.view_layer.clone();
    st.time_dirty = snap.time_dirty;
    st.anim_tagged = snap.anim_tagged.clone();
}

/// The index of the window's view layer in its scene.
fn view_layer_index(state: &State) -> usize {
    state
        .main
        .scenes
        .get(state.win.scene)
        .and_then(|s| {
            s.view_layers
                .iter()
                .position(|v| v.name == state.win.view_layer)
        })
        .unwrap_or(0)
}

/// `ED_undo_object_editmode_restore_helper` + `ED_undo_object_set_active_or_warn`: the objects
/// of the step are put in Edit Mode (their edit data is set by the caller), every other object
/// in Edit Mode of the active type leaves it (written back by its mode handler with
/// `EM_FREEDATA`) and is deselected, and the first object becomes active.
fn restore_editmode_objects(
    session: &mut Session,
    objects: &[IdRef<ObjectId>],
    datas: &[IdKey],
    object_type: &str,
) {
    let current = editmode_objects(&session.host.state);
    let leaving: Vec<IdRef<ObjectId>> = current
        .into_iter()
        .filter(|ob| {
            session
                .host
                .state
                .main
                .objects
                .get(*ob)
                .and_then(|o| o.data)
                .is_none_or(|d| !datas.contains(&d.key))
        })
        .collect();
    if !leaving.is_empty() {
        let ty = session
            .host
            .state
            .main
            .objects
            .get(leaving[0])
            .map_or(object_type, |o| o.object_type());
        if let (Ok(h), Ok(def)) = (
            frap_op::helpers::mode_handler(ObjectMode::Edit, ty),
            frap_op::lookup("ed.undo"),
        ) {
            let mut cx = OpCtx::new(&mut session.host, &mut session.services, def.spec);
            // A failure leaves the object as it is ("failed to enter edit-mode … undo state
            // invalid" is only logged by Blender either).
            let _ = (h.exit)(&mut cx, &leaving);
            let _ = cx.finish();
        }
        let vl = view_layer_index(&session.host.state);
        let scene = session.host.state.win.scene;
        let mut log = ChangeLog::default();
        for ob in leaving {
            let _ = frap_scene::id::layers::select_set(
                &mut session.host.state.main,
                &mut log,
                scene,
                vl,
                ob,
                false,
            );
        }
    }
    let mut log = ChangeLog::default();
    for &ob in objects {
        let main = &mut session.host.state.main;
        if main
            .objects
            .get(ob)
            .is_some_and(|o| o.mode != ObjectMode::Edit)
            && let Some(o) = main.objects.get_mut(ob, &mut log)
        {
            o.mode = ObjectMode::Edit;
        }
    }
    // `ED_undo_object_set_active_or_warn`.
    if let Some(&first) = objects.first() {
        let vl = view_layer_index(&session.host.state);
        let scene = session.host.state.win.scene;
        let main = &mut session.host.state.main;
        let has_base = main
            .scenes
            .get(scene)
            .and_then(|s| s.view_layers.get(vl))
            .is_some_and(|l| l.base(first).is_some());
        let active = main
            .scenes
            .get(scene)
            .and_then(|s| s.view_layers.get(vl))
            .and_then(|l| l.active);
        if has_base
            && active != Some(first)
            && let Some(s) = main.scenes.get_mut(scene, &mut log)
        {
            s.view_layers[vl].active = Some(first);
        }
    }
}

/// `undomesh_from_editmesh` + `undomesh_to_editmesh`: the BMesh through a `Mesh` and back
/// (`BM_mesh_bm_to_me` / `BM_mesh_bm_from_me`), so the restored BMesh has compact pools like
/// Blender's; the selection history and the active face follow their elements, the select
/// mode and `shapenr` are kept, and the normal spaces are dirty. A BMesh with shape-key layers
/// is restored as stored (frap's conversion does not carry key blocks).
fn restored_bmesh(stored: &Arc<BMesh>, template: &frap_mesh::Mesh) -> Arc<BMesh> {
    if !stored.shape_layers.is_empty() || stored.shape_keyindex.is_some() {
        return Arc::clone(stored);
    }
    let params = ToMeshParams {
        calc_object_remap: false,
        update_shapekey_indices: false,
    };
    let mesh = stored.to_mesh(template, &params);
    let mut bm = BMesh::from_mesh(
        &mesh,
        &FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            use_shapekey: false,
            active_shapekey: stored.shapenr,
        },
    );
    let index_of = |slots: Vec<usize>, cap: usize| {
        let mut idx = vec![u32::MAX; cap];
        for (i, s) in slots.into_iter().enumerate() {
            idx[s] = i as u32;
        }
        idx
    };
    let vi = index_of(
        stored.vert_ids().into_iter().map(VertId::slot).collect(),
        stored.verts.slots.len(),
    );
    let ei = index_of(
        stored.edge_ids().into_iter().map(EdgeId::slot).collect(),
        stored.edges.slots.len(),
    );
    let fi = index_of(
        stored.face_ids().into_iter().map(FaceId::slot).collect(),
        stored.faces.slots.len(),
    );
    let live = |m: &[u32], s: usize| m.get(s).copied().filter(|i| *i != u32::MAX);
    bm.select_history = stored
        .select_history
        .iter()
        .filter_map(|e| match *e {
            BMElem::Vert(v) => live(&vi, v.slot()).map(|i| BMElem::Vert(VertId(i))),
            BMElem::Edge(x) => live(&ei, x.slot()).map(|i| BMElem::Edge(EdgeId(i))),
            BMElem::Face(f) => live(&fi, f.slot()).map(|i| BMElem::Face(FaceId(i))),
        })
        .collect();
    bm.act_face = stored
        .act_face
        .and_then(|f| live(&fi, f.slot()))
        .map(FaceId);
    bm.select_mode = stored.select_mode;
    bm.shapenr = stored.shapenr;
    bm.lnor_spacearr.invalidate_all();
    Arc::new(bm)
}

/// `mesh_undosys_step_decode`.
fn decode_edit_mesh(session: &mut Session, step: &EditMeshStep) {
    // `ED_undo_object_editmode_validate_scene_from_windows`: one window, showing the current
    // scene; a step made in another scene is decoded in the current one.
    let _ = step.scene;
    let objects: Vec<IdRef<ObjectId>> = step.elems.iter().map(|e| e.ob).collect();
    let datas: Vec<IdKey> = step.elems.iter().map(|e| e.me.key()).collect();
    restore_editmode_objects(session, &objects, &datas, "MESH");
    let mut log = ChangeLog::default();
    let st = &mut session.host.state;
    for e in &step.elems {
        let Some(template) = st.main.meshes.get(e.me) else {
            continue;
        };
        let bm = restored_bmesh(&e.bm, template);
        // `obedit->shapenr = em->bm->shapenr` (`active_shape_key_index` is `shapenr - 1`, and
        // only means something with key blocks).
        let shape_index = (!bm.shape_layers.is_empty()).then(|| (bm.shapenr - 1).max(0));
        st.edit.meshes.insert(
            e.me.key(),
            EditMesh {
                bm,
                bm_gen: frap_rna::id::next_generation(),
                view: ViewCache::default(),
            },
        );
        if let Some(mesh) = st.main.meshes.get(e.me)
            && (mesh.vertex_group_names != e.vertex_group_names
                || mesh.vertex_group_locks != e.vertex_group_locks)
            && let Some(mesh) = st.main.meshes.get_mut(e.me, &mut log)
        {
            mesh.vertex_group_names = e.vertex_group_names.clone();
            mesh.vertex_group_locks = e.vertex_group_locks.clone();
        }
        if let Some(o) = st.main.objects.get(e.ob)
            && (o.vertex_groups_active_index != e.vertex_groups_active_index
                || shape_index.is_some_and(|i| o.active_shape_key_index != i))
            && let Some(o) = st.main.objects.get_mut(e.ob, &mut log)
        {
            o.vertex_groups_active_index = e.vertex_groups_active_index;
            if let Some(i) = shape_index {
                o.active_shape_key_index = i;
            }
        }
    }
    // The scene's selection settings follow the mesh data.
    let scene = st.win.scene;
    let differs = st.main.scenes.get(scene).is_some_and(|s| {
        s.tool_settings.mesh_select_mode != step.select_mode
            || s.tool_settings.use_uv_select_sync != step.uv_select_sync
    });
    if differs && let Some(s) = st.main.scenes.get_mut(scene, &mut log) {
        s.tool_settings.mesh_select_mode = step.select_mode;
        s.tool_settings.use_uv_select_sync = step.uv_select_sync;
    }
}

/// `armature_undosys_step_decode`.
fn decode_edit_armature(session: &mut Session, step: &EditArmatureStep) {
    let _ = step.scene;
    let objects: Vec<IdRef<ObjectId>> = step.elems.iter().map(|e| e.ob).collect();
    let datas: Vec<IdKey> = step.elems.iter().map(|e| e.arm.key()).collect();
    restore_editmode_objects(session, &objects, &datas, "ARMATURE");
    let mut log = ChangeLog::default();
    let st = &mut session.host.state;
    for e in &step.elems {
        st.edit.armatures.insert(e.arm.key(), Arc::clone(&e.edit));
        if let Some(a) = st.main.armatures.get(e.arm)
            && (a.collections != e.collections || a.active_collection != e.active_collection)
            && let Some(a) = st.main.armatures.get_mut(e.arm, &mut log)
        {
            a.collections = e.collections.clone();
            a.active_collection = e.active_collection;
        }
    }
}
