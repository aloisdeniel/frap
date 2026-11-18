// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_assign()`, tier T1
//! (`tests/golden/cases/edit/normals/material_slot_assign*.py`). Port of
//! `material_slot_assign_exec` and `object_array_for_shading_edit_mode_enabled`
//! (`editors/render/render_shading.cc`, `editors/object/object_edit.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Assigns the active object's **active material slot** to the selected faces of every mesh in
//! Edit Mode (unique data, active first): for each such object with slots, the slot index is
//! the active object's active slot when that slot holds the same material on this object,
//! else the first slot holding that material (objects without it are skipped). Leaving Edit
//! Mode writes `material_index` (removed when every face uses slot 0).
//!
//! In Object Mode no object is in Edit Mode, so nothing is assigned: `CANCELLED`, as in
//! Blender (frap adds a WARNING with the fix). Poll: `object_materials_supported_poll`.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MaterialId};
use frap_scene::Context;
use frap_scene::id::slots;
use frap_spec_ops_object::object;

use super::material_slot_add::common;

const IDNAME: &str = "object.material_slot_assign";

fn assign_poll(c: &Context) -> Result<(), PollFail> {
    common::supported_poll(c, IDNAME)
}

/// `object_materials_supported_poll`.
const ASSIGN_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: assign_poll,
};

#[operator(poll = ASSIGN_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotAssign) -> OpResult {
    let c = cx.ctx();
    // `BKE_object_material_get(obact, obact->actcol)`.
    let active = poll::active(&c).map(|(_, o)| {
        let totcol = slots::slot_count(c.main, o);
        let actcol = common::actcol(o, totcol);
        let mat: Option<IdRef<MaterialId>> = if actcol >= 1 {
            slots::slot(c.main, o, actcol - 1).1
        } else {
            None
        };
        (actcol, mat)
    });
    let (actcol, mat_active) = active.unwrap_or((0, None));
    let mut plan = Vec::new();
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let c = cx.ctx();
        let Some(o) = c.main.objects.get(ob) else {
            continue;
        };
        if !common::materials_supported(&c, o) {
            continue;
        }
        let totcol = slots::slot_count(c.main, o);
        if totcol == 0 {
            continue;
        }
        let material = |i: usize| slots::slot(c.main, o, i).1;
        let mat_nr = if actcol >= 1 && actcol <= totcol && material(actcol - 1) == mat_active {
            actcol - 1
        } else {
            match (0..totcol).find(|&i| material(i) == mat_active) {
                Some(i) => i,
                None => continue,
            }
        };
        plan.push((me, mat_nr as i16));
    }
    let mut changed_multi = false;
    for (me, mat_nr) in plan {
        let selected = {
            let (_, edit) = cx.split_edit();
            edit.meshes.get(&me.key()).is_some_and(|em| {
                em.bm
                    .face_ids()
                    .iter()
                    .any(|&f| em.bm.face(f).flag.contains(ElemFlag::SELECT))
            })
        };
        if !selected {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        for f in bm.face_ids() {
            if bm.face(f).flag.contains(ElemFlag::SELECT) {
                bm.face_mut(f).mat_nr = mat_nr;
            }
        }
        changed_multi = true;
    }
    if !changed_multi {
        cx.report(
            ReportType::Warning,
            "frap: nothing assigned: select faces in Edit Mode \
             (bpy.ops.object.mode_set(mode='EDIT')) of an object whose slots hold the active \
             slot's material",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
