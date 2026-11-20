// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_add()`, tier T0 (`tests/golden/cases/weights_shapes/
//! vgroup_ops.py`). Port of `vertex_group_add_exec` (`BKE_object_defgroup_add`: a group named
//! `Group`, made unique, becomes active) and of the shared vertex-group helpers of
//! `editors/object/object_vgroup.cc` and `blenkernel/intern/object_deform.cc` (Blender 5.2.4,
//! GPL-2.0-or-later) the other `vertex_group_*` operators use ([`common`]).
//!
//! Poll `vertex_group_supported_poll`: an active, local, editable object of a type with vertex
//! groups (meshes in frap; Blender also has lattices and grease pencil) whose data is local.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

/// Helpers shared by the `vertex_group_*` operators (WP 6.3).
pub(crate) mod common {
    use frap_bmesh::shape::deform;
    use frap_bmesh::{BMesh, ElemFlag};
    use frap_mesh::DeformVert;
    use frap_op::prelude::*;
    use frap_op::{PollFail, PollReq};
    use frap_rna::{IdRef, MeshId, ObjectId};
    use frap_scene::id::Object;
    use frap_scene::{Context, ObjectMode};
    use smallvec::smallvec;

    fn msg(idname: &str, why: &str) -> Option<String> {
        Some(format!("Operator bpy.ops.{idname}.poll() {why}"))
    }

    /// `OB_TYPE_SUPPORT_VGROUP` for the types frap has.
    pub(crate) fn supports_vgroup(ob: &Object) -> bool {
        ob.object_type() == "MESH"
    }

    /// `vertex_group_supported_poll_ex` with Blender's messages.
    pub(crate) fn supported_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
        let Some((_, ob)) = poll::active(c) else {
            return Err(PollFail {
                message: msg(idname, "No active editable object"),
                unmet: smallvec![PollReq::ActiveObject],
                hint: Some(
                    "make a mesh active: bpy.context.view_layer.objects.active = \
                     bpy.data.objects['Name']"
                        .into(),
                ),
            });
        };
        if ob.header.library.is_some() {
            return Err(PollFail {
                message: msg(idname, "No active editable object"),
                unmet: smallvec![PollReq::ActiveEditable],
                hint: Some(format!(
                    "bpy.data.objects[{}].make_local()",
                    poll::py_name(&ob.header.name)
                )),
            });
        }
        if !supports_vgroup(ob) {
            return Err(PollFail {
                message: msg(idname, "Object type does not support vertex groups"),
                unmet: smallvec![PollReq::ActiveType("MESH")],
                hint: Some(
                    "vertex groups belong to meshes: make a mesh object active".into(),
                ),
            });
        }
        if ob
            .data
            .and_then(|d| c.main.header(d))
            .is_none_or(|h| h.library.is_some())
        {
            return Err(PollFail {
                message: msg(idname, "Object type \"%s\" does not have editable data"),
                unmet: smallvec![PollReq::ActiveEditable],
                hint: None,
            });
        }
        Ok(())
    }

    /// The vertex-group names of an object.
    pub(crate) fn names<'a>(c: &Context<'a>, ob: &Object) -> &'a [smol_str::SmolStr] {
        mesh_of(c, ob)
            .and_then(|me| c.main.meshes.get(me))
            .map_or(&[], |m| m.vertex_group_names.as_slice())
    }

    /// `vertex_group_poll_ex`: [`supported_poll`] and at least one group.
    pub(crate) fn group_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
        supported_poll(c, idname)?;
        let (_, ob) = poll::active(c).expect("invariant: checked by supported_poll");
        if names(c, ob).is_empty() {
            return Err(PollFail {
                message: msg(idname, "Object has no vertex groups"),
                unmet: smallvec![],
                hint: Some("add one first: bpy.ops.object.vertex_group_add()".into()),
            });
        }
        Ok(())
    }

    /// `vertex_group_vert_select_poll` (`vertex_group_vert_poll_ex(C, true, 0)`): [`supported_poll`]
    /// and the object in Edit Mode (weight paint vertex selection does not exist in frap).
    pub(crate) fn vert_select_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
        supported_poll(c, idname)?;
        let (_, ob) = poll::active(c).expect("invariant: checked by supported_poll");
        if ob.mode != ObjectMode::Edit {
            return Err(PollFail {
                message: None,
                unmet: smallvec![PollReq::ModeEditMesh],
                hint: Some("bpy.ops.object.mode_set(mode='EDIT')".into()),
            });
        }
        Ok(())
    }

    /// `vertex_group_vert_select_unlocked_poll`: [`vert_select_poll`] (frap has no group locks).
    pub(crate) fn vert_select_unlocked_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
        vert_select_poll(c, idname)
    }

    /// The mesh of a mesh object.
    pub(crate) fn mesh_of(c: &Context, ob: &Object) -> Option<IdRef<MeshId>> {
        let me = ob.data?.typed::<MeshId>()?;
        c.main.meshes.get(me).map(|_| me)
    }

    /// The active object, its mesh and its active group (`actdef - 1`, `None` when out of
    /// range).
    pub(crate) fn active_group(cx: &OpCtx) -> Option<(IdRef<ObjectId>, IdRef<MeshId>, Option<usize>)> {
        let c = cx.ctx();
        let (ob, o) = poll::active(&c)?;
        let me = mesh_of(&c, o)?;
        let n = names(&c, o).len();
        let act = usize::try_from(o.vertex_groups_active_index)
            .ok()
            .filter(|&a| a < n);
        Some((ob, me, act))
    }

    /// Whether `me` is in Edit Mode.
    pub(crate) fn in_edit(cx: &mut OpCtx, me: IdRef<MeshId>) -> bool {
        cx.split_edit().1.meshes.contains_key(&me.key())
    }

    /// `vgroup_parray_alloc` + a pass: runs `f` on the weights of every vertex (only the
    /// selected ones with `use_vert_sel`; the edit BMesh in Edit Mode). False when the mesh
    /// has no weight layer (nothing ran).
    pub(crate) fn for_each_dvert(
        cx: &mut OpCtx,
        me: IdRef<MeshId>,
        use_vert_sel: bool,
        mut f: impl FnMut(&mut DeformVert),
    ) -> Result<bool, OpError> {
        if in_edit(cx, me) {
            let has = {
                let (_, edit) = cx.split_edit();
                edit.meshes
                    .get(&me.key())
                    .is_some_and(|em| em.bm.vdata.deform.is_some())
            };
            if !has {
                return Ok(false);
            }
            let bm = cx.bmesh_mut(me)?;
            for v in bm.vert_ids() {
                if use_vert_sel && !bm.vert(v).flag.contains(ElemFlag::SELECT) {
                    continue;
                }
                if let Some(dv) = bm.vdata.deform.as_mut().and_then(|d| d.get_mut(v.slot())) {
                    f(dv);
                }
            }
            return Ok(true);
        }
        if cx
            .main()
            .meshes
            .get(me)
            .is_none_or(|m| m.deform_verts().is_none())
        {
            return Ok(false);
        }
        let m = cx.mesh_mut(me)?;
        let sel = deform::mesh_vert_selection(m);
        for (i, dv) in m.deform_verts_mut().iter_mut().enumerate() {
            if use_vert_sel && !sel.get(i).copied().unwrap_or(false) {
                continue;
            }
            f(dv);
        }
        Ok(true)
    }

    /// The same as [`for_each_dvert`] for reading.
    pub(crate) fn read_dverts(cx: &mut OpCtx, me: IdRef<MeshId>, use_vert_sel: bool) -> Option<Vec<DeformVert>> {
        if in_edit(cx, me) {
            let (_, edit) = cx.split_edit();
            let em = edit.meshes.get(&me.key())?;
            let d = em.bm.vdata.deform.as_ref()?;
            return Some(
                em.bm
                    .vert_ids()
                    .into_iter()
                    .filter(|&v| !use_vert_sel || em.bm.vert(v).flag.contains(ElemFlag::SELECT))
                    .map(|v| d.get(v.slot()).cloned().unwrap_or_default())
                    .collect(),
            );
        }
        let m = cx.main().meshes.get(me)?;
        let dv = m.deform_verts()?;
        let sel = deform::mesh_vert_selection(m);
        Some(
            dv.iter()
                .enumerate()
                .filter(|(i, _)| !use_vert_sel || sel.get(*i).copied().unwrap_or(false))
                .map(|(_, d)| d.clone())
                .collect(),
        )
    }

    /// `vertex_group_use_vert_sel`: Edit Mode (weight-paint selection masking does not exist
    /// in frap).
    pub(crate) fn use_vert_sel(cx: &OpCtx, ob: IdRef<ObjectId>) -> bool {
        cx.main()
            .objects
            .get(ob)
            .is_some_and(|o| o.mode == ObjectMode::Edit)
    }

    /// `BKE_object_defgroup_add_name(ob, "Group")`: a new group, made active. Returns its index.
    pub(crate) fn add_group(cx: &mut OpCtx, ob: IdRef<ObjectId>, me: IdRef<MeshId>) -> Result<usize, OpError> {
        let m = cx.mesh_mut(me)?;
        let name = deform::unique_group_name(&m.vertex_group_names, None, "Group");
        m.vertex_group_names.push(name);
        let n = m.vertex_group_names.len();
        cx.object_mut(ob)?.vertex_groups_active_index = n as i32 - 1;
        Ok(n - 1)
    }

    /// `BKE_object_defgroup_remove`: group `g`'s weights (from the edit BMesh in Edit Mode)
    /// and name go, higher groups shift down, the active index follows.
    pub(crate) fn remove_group(cx: &mut OpCtx, ob: IdRef<ObjectId>, me: IdRef<MeshId>, g: usize) -> Result<(), OpError> {
        let edit = in_edit(cx, me);
        if edit {
            let bm: &mut BMesh = cx.bmesh_mut(me)?;
            deform::remove_group_bmesh(bm, g as u32);
        }
        let m = cx.mesh_mut(me)?;
        if !edit {
            deform::remove_group_mesh(m, g as u32);
        }
        if g < m.vertex_group_names.len() {
            m.vertex_group_names.remove(g);
        }
        let empty = m.vertex_group_names.is_empty();
        let o = cx.object_mut(ob)?;
        if o.vertex_groups_active_index + 1 > g as i32 {
            o.vertex_groups_active_index -= 1;
        }
        if !empty && o.vertex_groups_active_index < 0 {
            o.vertex_groups_active_index = 0;
        }
        Ok(())
    }

    /// `BKE_object_defgroup_clear`: removes group `g` from every vertex with weights (the
    /// selected ones with `use_selection`). True when a vertex with weights was visited.
    pub(crate) fn clear_group(cx: &mut OpCtx, me: IdRef<MeshId>, g: usize, use_selection: bool) -> Result<bool, OpError> {
        if in_edit(cx, me) {
            let has = {
                let (_, edit) = cx.split_edit();
                edit.meshes
                    .get(&me.key())
                    .is_some_and(|em| em.bm.vdata.deform.is_some())
            };
            if !has {
                return Ok(false);
            }
            let bm = cx.bmesh_mut(me)?;
            return Ok(deform::clear_group_bmesh(bm, g as u32, use_selection));
        }
        if cx
            .main()
            .meshes
            .get(me)
            .is_none_or(|m| m.deform_verts().is_none())
        {
            return Ok(false);
        }
        let m = cx.mesh_mut(me)?;
        Ok(deform::clear_group_mesh(m, g as u32, use_selection))
    }

    /// The `group_select_mode` items of the active object (`vgroup_selection_itemf_helper`):
    /// `ACTIVE` when `with_active`, `BONE_DEFORM` when an armature deforms it, `ALL`.
    pub(crate) fn subset_items(c: &Context, with_active: bool) -> Vec<frap_op::EnumItemOwned> {
        let item = |id: &str, name: &str, desc: &str| frap_op::EnumItemOwned {
            identifier: id.into(),
            name: name.into(),
            description: desc.into(),
        };
        let mut v = Vec::new();
        if with_active {
            v.push(item("ACTIVE", "Active Group", "The active Vertex Group"));
        }
        if poll::active(c).is_some_and(|(_, o)| deformed_by_armature(c, o).is_some()) {
            v.push(item(
                "BONE_DEFORM",
                "Deform Pose Bones",
                "All Vertex Groups assigned to Deform Bones",
            ));
        }
        v.push(item("ALL", "All Groups", "All Vertex Groups"));
        v
    }

    /// `BKE_modifiers_is_deformed_by_armature`: the armature of the first ARMATURE modifier
    /// with an object, else an armature parent with `parent_type == 'ARMATURE'`.
    pub(crate) fn deformed_by_armature(c: &Context, ob: &Object) -> Option<IdRef<ObjectId>> {
        for m in &ob.modifiers {
            if m.settings.type_identifier() == "ARMATURE"
                && let Some(a) = m
                    .settings
                    .id_refs()
                    .into_iter()
                    .find_map(|(k, r)| (k == "object").then_some(r).flatten())
                && let Some(r) = a.typed::<ObjectId>()
                && c.main.objects.get(r).is_some_and(|o| o.object_type() == "ARMATURE")
            {
                return Some(r);
            }
        }
        let p = ob.parent?;
        (ob.parent_type.as_str() == "ARMATURE"
            && c.main.objects.get(p).is_some_and(|o| o.object_type() == "ARMATURE"))
        .then_some(p)
    }

    /// `BKE_object_defgroup_validmap_get`: the groups named after a deform bone of the
    /// deforming armature.
    pub(crate) fn validmap_deform(c: &Context, ob: &Object) -> Vec<bool> {
        let names = names(c, ob);
        let arm = deformed_by_armature(c, ob)
            .and_then(|a| c.main.objects.get(a))
            .and_then(|a| a.data)
            .and_then(|d| d.typed())
            .and_then(|d| c.main.armatures.get(d));
        names
            .iter()
            .map(|n| arm.is_some_and(|a| a.bone(n).is_some_and(|b| b.use_deform)))
            .collect()
    }

    /// `BKE_object_defgroup_subset_from_select_type`: the groups in the subset and their count.
    pub(crate) fn subset(c: &Context, ob: &Object, mode: &str) -> (Vec<bool>, usize) {
        let n = names(c, ob).len();
        match mode {
            "ACTIVE" => {
                let mut v = vec![false; n];
                match usize::try_from(ob.vertex_groups_active_index)
                    .ok()
                    .filter(|&a| a < n)
                {
                    Some(a) => {
                        v[a] = true;
                        (v, 1)
                    }
                    None => (v, 0),
                }
            }
            "BONE_DEFORM" => {
                let v = validmap_deform(c, ob);
                let k = v.iter().filter(|x| **x).count();
                (v, k)
            }
            _ => (vec![true; n], n),
        }
    }

    /// `objects_in_mode_or_selected(C, vertex_group_supported_poll_ex)`: the objects in Edit
    /// Mode (unique data) when the active one is, else the selected objects with vertex-group
    /// support (unique data).
    pub(crate) fn objects_for_wpaint(c: &Context) -> Vec<IdRef<ObjectId>> {
        let active_edit = poll::active(c).is_some_and(|(_, o)| o.mode == ObjectMode::Edit);
        let candidates = if active_edit {
            c.objects_in_mode_unique_data()
        } else {
            c.selected_editable_objects()
        };
        let mut seen = Vec::new();
        candidates
            .into_iter()
            .filter(|r| {
                let Some(o) = c.main.objects.get(*r) else {
                    return false;
                };
                if !supports_vgroup(o)
                    || o.header.library.is_some()
                    || o.data.and_then(|d| c.main.header(d)).is_none_or(|h| h.library.is_some())
                {
                    return false;
                }
                if seen.contains(&o.data) {
                    return false;
                }
                seen.push(o.data);
                true
            })
            .collect()
    }
}

fn add_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::supported_poll(c, "object.vertex_group_add")
}

/// `vertex_group_supported_poll`.
const ADD_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: add_poll,
};

#[operator(poll = ADD_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupAdd) -> OpResult {
    let c = cx.ctx();
    let Some((ob, o)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(me) = common::mesh_of(&c, o) else {
        return Ok(OpStatus::Cancelled);
    };
    common::add_group(cx, ob, me)?;
    Ok(OpStatus::Finished)
}
