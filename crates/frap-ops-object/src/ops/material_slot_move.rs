// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_move(direction='UP')`, tier T0
//! (`tests/golden/cases/nodes/slots/material_slot_move*.py`, WP 3.1, M3+M4 W0 assignment).
//!
//! Verified on 5.2.4:
//! - swaps the active slot of the active object with the one above (`UP`, the default) or
//!   below (`DOWN`) and moves the active index with it; at the first (last) slot, or with
//!   fewer than two slots, nothing changes and the operator answers `{'CANCELLED'}`;
//! - the swap permutes the object's own slot entries (`link`, object-linked material), the
//!   data's material list (so every object sharing the mesh sees the new order, while their own
//!   object-linked entries and active index stay) and remaps the face `material_index` values
//!   of the two slots; without the attribute it is created first (all faces use slot 0, so
//!   moving slot 0 assigns its faces to the other slot);
//! - in Edit Mode the edit mesh's face indices are remapped instead;
//! - poll: `object_materials_supported_poll` (also in Edit Mode).

// Ported from blender/source/blender/editors/render/render_shading.cc
// (`material_slot_move_exec`) and blender/source/blender/blenkernel/intern/material.cc,
// mesh.cc (`BKE_object_material_remap`, `BKE_mesh_material_remap`), Blender 5.2.4,
// GPL-2.0-or-later.

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::slots;
use frap_spec_ops_object::object;
use frap_spec_ops_object::object::material_slot_move::Direction;

use super::material_slot_add::common::{self, MATERIAL_INDEX};

const IDNAME: &str = "object.material_slot_move";

fn move_poll(c: &Context) -> Result<(), PollFail> {
    common::supported_poll(c, IDNAME)
}

/// `object_materials_supported_poll`.
const MOVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: move_poll,
};

/// `MAT_NR_REMAP` of `BKE_mesh_material_remap` for the swap of slots `a` and `b` (indices at
/// or past `totcol` are left alone).
fn remap(n: i32, a: usize, b: usize) -> i32 {
    match usize::try_from(n) {
        Ok(i) if i == a => b as i32,
        Ok(i) if i == b => a as i32,
        _ => n,
    }
}

#[operator(poll = MOVE_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::MaterialSlotMove) -> OpResult {
    let Some(ob) = cx.ctx().object() else {
        return Ok(OpStatus::Cancelled);
    };
    let c = cx.ctx();
    let Some(o) = c.main.objects.get(ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let totcol = slots::slot_count(c.main, o);
    if totcol < 2 {
        return Ok(OpStatus::Cancelled);
    }
    // `BKE_object_material_active_index_sanitize`, then the 1-based `actcol`.
    let actcol = common::actcol(o, totcol).max(1);
    let (a, b, new_act) = match p.direction.value {
        Direction::Up if actcol > 1 => (actcol - 2, actcol - 1, actcol - 1),
        Direction::Down if actcol < totcol => (actcol - 1, actcol, actcol + 1),
        _ => return Ok(OpStatus::Cancelled),
    };
    let Some(me) = common::mesh_of(c.main, ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let in_edit = {
        let (_, edit) = cx.split_edit();
        edit.meshes.contains_key(&me.key())
    };

    // `BLI_array_permute(ob->mat / ob->matbits)`: the object's own entries.
    let o = cx.object_mut(ob)?;
    if o.material_slot_overrides.len() > a {
        slots::override_mut(o, b);
        o.material_slot_overrides.swap(a, b);
    }
    o.active_material_index = (new_act - 1) as i32;

    // `BLI_array_permute(*matar)` and `BKE_mesh_material_remap`.
    let m = cx.mesh_mut(me)?;
    if m.materials.len() > b {
        m.materials.swap(a, b);
    }
    if in_edit {
        let bm = cx.bmesh_mut(me)?;
        for f in bm.face_ids() {
            let face = bm.face_mut(f);
            face.mat_nr = remap(i32::from(face.mat_nr), a, b) as i16;
        }
    } else {
        let faces = m.faces_num();
        if common::material_indices(m).is_none() {
            m.attributes_mut().insert(Attribute {
                name: MATERIAL_INDEX.into(),
                domain: Domain::Face,
                data: AttrData::Int(Arc::new(vec![0; faces])),
            });
        }
        if let Some(AttrData::Int(v)) = m
            .attributes_mut()
            .get_mut(MATERIAL_INDEX)
            .map(|x| &mut x.data)
        {
            for i in Arc::make_mut(v).iter_mut() {
                *i = remap(*i, a, b);
            }
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::remap;

    #[test]
    fn remap_swaps_the_two_slots_only() {
        assert_eq!(remap(0, 0, 1), 1);
        assert_eq!(remap(1, 0, 1), 0);
        assert_eq!(remap(2, 0, 1), 2);
        assert_eq!(remap(-1, 0, 1), -1);
        assert_eq!(remap(3, 2, 3), 2);
    }
}
