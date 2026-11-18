// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.join()`, tier T0 (`tests/golden/cases/object/join/*.py`). Port of
//! `object_join_exec`, `object_join_poll` and `ED_mesh_join_objects_exec`
//! (`editors/object/object_add.cc`, `editors/mesh/meshtools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//! - poll: an active object with data, local, of a joinable type (mesh; armatures in M6),
//!   else `failed, context is incorrect`;
//! - the active object must be among `context.selected_editable_objects`, else `CANCELLED`
//!   with WARNING `Active object is not a selected mesh`; when the selected meshes add no
//!   vertex to it, `CANCELLED` with WARNING `No mesh data to join`; in Edit Mode `Error: This
//!   data does not support joining in edit mode`;
//! - the other selected mesh objects (base order; non-mesh ones are ignored and stay selected)
//!   are merged into the active mesh in its object space (see `join/mesh.rs`), an object
//!   sharing the active mesh included, then deleted: their meshes stay as orphans, their
//!   children lose their parent and keep their local transform;
//! - materials: the slot lists are merged by material (the active object's effective slots
//!   first; an object without slots counts as one empty slot; nothing at all when no object
//!   has a slot), every slot links `DATA`, faces are remapped; `material_index` is written when
//!   more than one slot results;
//! - when the active object's parent is joined into it, it is unparented keeping its world
//!   transform.
//!
//! Shape keys arrive in M6: joining meshes with shape keys answers `OP_NOT_IMPLEMENTED`.

mod mesh;

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MaterialId, MeshId, ObjectId};
use frap_scene::id::slots;
use frap_scene::id::transform::world_matrix;
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;

use self::mesh::{Part, join};
use super::move_to_collection::common::free_and_unlink;

/// `object_join_poll`: an active object with data, local, of a joinable type.
fn join_poll(c: &Context) -> Result<(), PollFail> {
    let Some((_, ob)) = poll::active(c) else {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "make the mesh to join into active: bpy.context.view_layer.objects.active = \
             bpy.data.objects['Name']",
        ));
    };
    if ob.header.library.is_some() {
        return Err(poll::fail(
            PollReq::ActiveEditable,
            format!(
                "bpy.data.objects[{}].make_local()",
                poll::py_name(&ob.header.name)
            ),
        ));
    }
    if ob.data.is_none() || !matches!(ob.object_type(), "MESH" | "ARMATURE") {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make the mesh to join into active: bpy.context.view_layer.objects.active = \
             bpy.data.objects['Name']",
        ));
    }
    Ok(())
}

/// `object_join_poll`
const JOIN_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: join_poll,
};

#[operator(
    poll = JOIN_POLL,
    tier = T0,
    support = Partial("meshes only: armatures arrive in M6; meshes with shape keys or with one attribute name stored with two domains/types answer OP_NOT_IMPLEMENTED"),
    notes = ["CANCELLED with Blender's WARNING reports when the active object is not a selected mesh or nothing would be joined"]
)]
fn exec(cx: &mut OpCtx, _p: object::Join) -> OpResult {
    let (active, selected, scene) = {
        let c = cx.ctx();
        let (a, _) = poll::active(&c).ok_or_else(|| OpError::Runtime("no active object".into()))?;
        (a, c.selected_editable_objects(), c.win.scene)
    };
    let main = cx.main();
    let ob = main
        .objects
        .get(active)
        .ok_or_else(|| OpError::Runtime("the active object was removed".into()))?;
    if ob.mode == ObjectMode::Edit {
        return Err(OpError::Runtime(
            "This data does not support joining in edit mode".into(),
        ));
    }
    if ob
        .data
        .and_then(|d| main.header(d))
        .is_some_and(|h| h.library.is_some())
    {
        return Err(OpError::Runtime("Cannot edit external library data".into()));
    }
    if ob.object_type() == "ARMATURE" {
        return Err(OpError::NotImplemented(
            "frap: joining armatures arrives with armatures in M6".into(),
        ));
    }
    if ob.object_type() != "MESH" {
        cx.report(ReportType::Warning, "Active object is not a mesh");
        return Ok(OpStatus::Cancelled);
    }
    let me_active: IdRef<MeshId> = ob
        .data
        .and_then(|d| d.typed())
        .ok_or_else(|| OpError::Runtime("the active object has no mesh".into()))?;
    // Count and check (`CTX_DATA_BEGIN … selected_editable_objects`).
    let meshes: Vec<(IdRef<ObjectId>, IdRef<MeshId>)> = selected
        .iter()
        .filter_map(|r| {
            let o = main.objects.get(*r)?;
            (o.object_type() == "MESH").then_some((*r, o.data?.typed()?))
        })
        .collect();
    let totvert: usize = meshes
        .iter()
        .filter_map(|(_, me)| main.meshes.get(*me))
        .map(|m| m.verts_num())
        .sum();
    let join_parent = ob
        .parent
        .is_some_and(|p| meshes.iter().any(|(o, _)| *o == p));
    if !meshes.iter().any(|(o, _)| *o == active) {
        cx.report(ReportType::Warning, "Active object is not a selected mesh");
        return Ok(OpStatus::Cancelled);
    }
    let active_verts = main.meshes.get(me_active).map_or(0, |m| m.verts_num());
    if totvert == 0 || totvert == active_verts {
        cx.report(ReportType::Warning, "No mesh data to join");
        return Ok(OpStatus::Cancelled);
    }
    if totvert > i32::MAX as usize {
        cx.report(
            ReportType::Warning,
            format!(
                "Joining results in {totvert} vertices, limit is {}",
                i32::MAX
            ),
        );
        return Ok(OpStatus::Cancelled);
    }
    if meshes
        .iter()
        .any(|(_, me)| main.meshes.get(*me).is_some_and(|m| m.shape_keys.is_some()))
    {
        return Err(OpError::NotImplemented(
            "frap: joining meshes with shape keys arrives with shape keys in M6".into(),
        ));
    }
    // The active object first, then the others in base order.
    let order: Vec<(IdRef<ObjectId>, IdRef<MeshId>)> = std::iter::once((active, me_active))
        .chain(meshes.iter().copied().filter(|(o, _)| *o != active))
        .collect();
    let active_world = world_matrix(&main.objects, active);
    let inv = active_world.inverse();
    // Materials: union of the effective slot materials, an object without slots counting as
    // one empty slot; nothing when no object has a slot.
    let slot_lists: Vec<Vec<Option<IdRef<MaterialId>>>> = order
        .iter()
        .map(|(o, _)| {
            main.objects
                .get(*o)
                .map(|x| slots::slot_materials(main, x))
                .unwrap_or_default()
        })
        .collect();
    let any_slots = slot_lists.iter().any(|l| !l.is_empty());
    let mut matar: Vec<Option<IdRef<MaterialId>>> = Vec::new();
    let mut matmaps: Vec<Vec<i32>> = Vec::new();
    for list in &slot_lists {
        let list: &[Option<IdRef<MaterialId>>] = if list.is_empty() && any_slots {
            &[None]
        } else {
            list
        };
        let map = list
            .iter()
            .map(|m| match matar.iter().position(|x| x == m) {
                Some(i) => i as i32,
                None => {
                    matar.push(*m);
                    matar.len() as i32 - 1
                }
            })
            .collect();
        matmaps.push(map);
    }
    let joined = {
        let parts: Vec<Part> = order
            .iter()
            .zip(matmaps)
            .filter_map(|((o, me), matmap)| {
                let mesh = main.meshes.get(*me)?;
                let xform = (*o != active).then(|| inv * world_matrix(&main.objects, *o));
                Some(Part {
                    mesh,
                    xform,
                    matmap,
                })
            })
            .collect();
        join(&parts, matar.len() > 1).map_err(OpError::NotImplemented)?
    };
    let others: Vec<IdRef<ObjectId>> = order.iter().skip(1).map(|(o, _)| *o).collect();
    let (main, log) = cx.main_mut();
    if join_parent && let Some(o) = main.objects.get_mut(active, log) {
        o.parent = None;
        o.set_matrix_world(&active_world, None);
    }
    if let Some(m) = main.meshes.get_mut(me_active, log) {
        let header = m.header.clone();
        *m = joined;
        m.header = header;
        m.materials = matar.clone();
    }
    // Every slot of the active object links `DATA` now; objects sharing the mesh follow its
    // new slot count.
    if let Some(o) = main.objects.get_mut(active, log) {
        o.material_slot_overrides.clear();
        let n = matar.len() as i32;
        o.active_material_index = o.active_material_index.clamp(0, (n - 1).max(0));
    }
    for o in others {
        free_and_unlink(main, log, scene, o);
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
