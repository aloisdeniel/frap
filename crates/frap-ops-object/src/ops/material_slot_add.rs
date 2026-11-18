// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_add()`, tier T0 (`tests/golden/cases/object/slots/*.py`).
//! Port of `material_slot_add_exec`, `BKE_object_material_slot_add` and
//! `BKE_object_material_assign(…, BKE_MAT_ASSIGN_USERPREF)` (`editors/render/
//! render_shading.cc`, `blenkernel/intern/material.cc`, Blender 5.2.4, GPL-2.0-or-later).
//! Verified on 5.2.4:
//! - only the active object: an empty slot is appended to its data (`mesh.materials`), so
//!   every object sharing the mesh gets the slot too; the new slot becomes the active one
//!   (`active_material_index`), the other users keep theirs;
//! - the new slot's `link` copies the active slot's (`OBJECT` stays `OBJECT`), `DATA` without
//!   slots; growing the list drops a user's hidden object-linked entries past its old count;
//! - poll (`object_materials_supported_poll`): an active object (`Context missing active
//!   object`), local, not hidden (`Cannot edit hidden object`; `hide_set` does not count), of a
//!   type with materials (meshes in frap; cameras, lights and empties fail with `failed, context
//!   is incorrect`). It also passes in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

/// Material-slot helpers shared by the `material_slot_*` operators (WP 1.20).
pub(crate) mod common {
    use frap_mesh::{AttrData, Mesh};
    use frap_op::prelude::*;
    use frap_op::{PollFail, PollReq};
    use frap_rna::{IdRef, MeshId, ObjectId};
    use frap_scene::id::{Object, SlotLink, SlotOverride, slots};
    use frap_scene::{ChangeLog, Context, Main, ObjectMode};

    /// Blender's face material-index attribute.
    pub(crate) const MATERIAL_INDEX: &str = "material_index";

    fn with_message(mut f: PollFail, idname: &str, why: &str) -> PollFail {
        f.message = Some(format!("Operator bpy.ops.{idname}.poll() {why}"));
        f
    }

    /// `OB_TYPE_SUPPORT_MATERIAL` for the object types frap has (curves, surfaces, text,
    /// metaballs, grease pencil and volumes do not exist in frap; point clouds keep no
    /// material list yet).
    pub(crate) fn supports_material(ob: &Object) -> bool {
        ob.object_type() == "MESH"
    }

    /// `object_materials_supported_poll_ex(C, ob)` without the messages: `ob` is local, not
    /// hidden, of a type with materials, and its active slot is object-linked or its data is
    /// local.
    pub(crate) fn materials_supported(c: &Context, ob: &Object) -> bool {
        if ob.header.library.is_some() || (ob.hide_viewport && ob.mode != ObjectMode::Edit) {
            return false;
        }
        if !supports_material(ob) {
            return false;
        }
        let act = ob.active_material_index.max(0) as usize;
        let object_linked = act < slots::slot_count(c.main, ob)
            && ob
                .material_slot_overrides
                .get(act)
                .is_some_and(|s| s.link == SlotLink::Object);
        object_linked
            || ob
                .data
                .and_then(|d| c.main.header(d))
                .is_some_and(|h| h.library.is_none())
    }

    /// `object_materials_supported_poll` for `idname`, with Blender's messages
    /// (`ED_operator_object_active_editable_ex`).
    pub(crate) fn supported_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
        poll::active_editable(c).map_err(|f| {
            let why = match poll::active(c) {
                None => "Context missing active object",
                Some((_, ob)) if ob.header.library.is_some() => {
                    "Cannot edit library linked or non-editable override object"
                }
                Some(_) => "Cannot edit hidden object",
            };
            with_message(f, idname, why)
        })?;
        let (_, ob) = poll::active(c).expect("invariant: checked by active_editable");
        if !supports_material(ob) {
            return Err(poll::fail(
                PollReq::ActiveType("MESH"),
                "make a mesh object active: bpy.context.view_layer.objects.active = \
                 bpy.data.objects['Name']",
            ));
        }
        if !materials_supported(c, ob) {
            return Err(poll::fail(
                PollReq::ActiveEditable,
                "the object's data is linked from a library",
            ));
        }
        Ok(())
    }

    /// The removing operators' poll: [`supported_poll`], not in Edit Mode (`Unable to remove
    /// material slot in edit mode`) and at least one slot.
    pub(crate) fn remove_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
        supported_poll(c, idname)?;
        let (_, ob) = poll::active(c).expect("invariant: checked by supported_poll");
        if ob.mode == ObjectMode::Edit {
            return Err(with_message(
                poll::fail(
                    PollReq::ModeObject,
                    "bpy.ops.object.mode_set(mode='OBJECT')",
                ),
                idname,
                "Unable to remove material slot in edit mode",
            ));
        }
        if slots::slot_count(c.main, ob) == 0 {
            let mut f = poll::fail(
                PollReq::ActiveObject,
                "the active object has no material slot: bpy.ops.object.material_slot_add()",
            );
            f.unmet.clear();
            return Err(f);
        }
        Ok(())
    }

    /// `object_array_for_shading_edit_mode_disabled`: the selected editable objects (base
    /// order) not in Edit Mode whose materials can be edited. The active object is not added
    /// when it is not selected (verified on 5.2.4).
    pub(crate) fn shading_objects(c: &Context) -> Vec<IdRef<ObjectId>> {
        c.selected_editable_objects()
            .into_iter()
            .filter(|r| {
                c.main
                    .objects
                    .get(*r)
                    .is_some_and(|o| o.mode != ObjectMode::Edit && materials_supported(c, o))
            })
            .collect()
    }

    /// The mesh of a mesh object.
    pub(crate) fn mesh_of(main: &Main, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
        main.objects.get(ob)?.data?.typed::<MeshId>()
    }

    /// Objects whose data is `me`, in `bpy.data.objects` order.
    pub(crate) fn users_of(main: &Main, me: IdRef<MeshId>) -> Vec<IdRef<ObjectId>> {
        main.objects
            .iter()
            .filter(|(_, o)| o.data == Some(me.any()))
            .map(|(r, _)| r)
            .collect()
    }

    /// `ob->actcol` (1-based, clamped to the slot count; 0 without slots).
    pub(crate) fn actcol(ob: &Object, totcol: usize) -> usize {
        ((ob.active_material_index.max(0) as usize) + 1).min(totcol)
    }

    /// The face material indices of a mesh (`None` without the attribute).
    pub(crate) fn material_indices(me: &Mesh) -> Option<&[i32]> {
        match me.attributes().get(MATERIAL_INDEX).map(|a| &a.data) {
            Some(AttrData::Int(v)) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// `BKE_object_material_slot_used(ob, slot)` for a mesh: a face uses the 0-based `slot`
    /// (without the attribute every face uses slot 0).
    pub(crate) fn slot_used(me: &Mesh, slot: usize) -> bool {
        match material_indices(me) {
            Some(mi) => mi.iter().any(|&i| i >= 0 && i as usize == slot),
            None => slot == 0,
        }
    }

    /// `BKE_object_material_slot_add`: appends an empty slot to the data of `ob` and makes it
    /// active; its link copies the active slot's. Returns false when `ob` has no mesh.
    pub(crate) fn slot_add(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>) -> bool {
        let Some(me) = mesh_of(main, ob) else {
            return false;
        };
        let Some(o) = main.objects.get(ob) else {
            return false;
        };
        let old = slots::slot_count(main, o);
        let link = if old > 0 {
            o.material_slot_overrides
                .get(actcol(o, old) - 1)
                .map_or(SlotLink::Data, |s| s.link)
        } else {
            SlotLink::Data
        };
        // `BKE_object_material_resize` on growth: entries past the old count are dropped.
        for u in users_of(main, me) {
            if main
                .objects
                .get(u)
                .is_some_and(|x| x.material_slot_overrides.len() > old)
                && let Some(x) = main.objects.get_mut(u, log)
            {
                x.material_slot_overrides.truncate(old);
            }
        }
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.materials.push(None);
        }
        if let Some(o) = main.objects.get_mut(ob, log) {
            if link == SlotLink::Object {
                *slots::override_mut(o, old) = SlotOverride {
                    link,
                    material: None,
                };
            }
            o.active_material_index = old as i32;
        }
        true
    }

    /// `BKE_object_material_slot_remove`: removes the active slot of `ob` from its data and
    /// from every object sharing the data (their object-linked entries shift down, their
    /// active slot is clamped), and decrements the face material indices above it
    /// (`BKE_mesh_material_index_remove`). Returns false without a slot.
    pub(crate) fn slot_remove(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>) -> bool {
        let Some(me) = mesh_of(main, ob) else {
            return false;
        };
        let Some(o) = main.objects.get(ob) else {
            return false;
        };
        let tot = slots::slot_count(main, o);
        if tot == 0 {
            return false;
        }
        let idx = actcol(o, tot) - 1;
        let new_tot = tot - 1;
        for u in users_of(main, me) {
            let Some(x) = main.objects.get_mut(u, log) else {
                continue;
            };
            if idx < x.material_slot_overrides.len() {
                x.material_slot_overrides.remove(idx);
            }
            if new_tot == 0 {
                x.material_slot_overrides.clear();
            }
            let act = ((x.active_material_index.max(0) as usize) + 1).min(new_tot);
            x.active_material_index = act.saturating_sub(1) as i32;
        }
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.materials.remove(idx);
            let has = material_indices(m).is_some();
            if has
                && let Some(AttrData::Int(v)) = m
                    .attributes_mut()
                    .get_mut(MATERIAL_INDEX)
                    .map(|a| &mut a.data)
            {
                for i in std::sync::Arc::make_mut(v).iter_mut() {
                    if *i > 0 && *i as usize >= idx {
                        *i -= 1;
                    }
                }
            }
        }
        true
    }

    /// `ob->actcol = slot + 1` then [`slot_remove`].
    pub(crate) fn slot_remove_at(
        main: &mut Main,
        log: &mut ChangeLog,
        ob: IdRef<ObjectId>,
        slot: usize,
    ) -> bool {
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.active_material_index = slot as i32;
        }
        slot_remove(main, log, ob)
    }
}

fn add_poll(c: &Context) -> Result<(), PollFail> {
    common::supported_poll(c, "object.material_slot_add")
}

/// `object_materials_supported_poll`
const ADD_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: add_poll,
};

#[operator(poll = ADD_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotAdd) -> OpResult {
    let Some(ob) = cx.ctx().object() else {
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    common::slot_add(main, log, ob);
    Ok(OpStatus::Finished)
}
