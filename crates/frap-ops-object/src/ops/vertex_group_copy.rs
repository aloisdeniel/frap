// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_copy()`, tier T1 (`tests/golden/cases/object/vgroups_m8/
//! copy_lock_move_sort.py`). Port of `vertex_group_copy_exec` / `vgroup_duplicate`
//! (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): the active group is
//! duplicated as `<name>_copy` (the name kept when it already contains `_copy`, then made
//! unique), with its lock, appended and made active; every vertex with a weight in the active
//! group gets the same weight in the copy (the edit BMesh in Edit Mode).
//!
//! Poll `vertex_group_poll`: a mesh with at least one vertex group.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Weights};

fn copy_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_copy")
}

/// `vertex_group_poll`.
const COPY_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: copy_poll,
};

/// `SNPRINTF_UTF8(name, "%s_copy", dg->name)` into a 64-byte buffer.
fn copy_name(name: &str) -> String {
    if name.contains("_copy") {
        return name.to_owned();
    }
    frap_rna::naming::truncate_utf8(&format!("{name}_copy"), deform::GROUP_NAME_MAX).to_owned()
}

#[operator(poll = COPY_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupCopy) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let idg = {
        let c = cx.ctx();
        let Some(o) = c.main.objects.get(ob) else {
            return Ok(OpStatus::Cancelled);
        };
        weights::active_group(&c, o)
    };
    // `BLI_findlink` failing: nothing to copy, but the operator still finishes.
    let Some(idg) = idg else {
        return Ok(OpStatus::Finished);
    };
    let m = cx.mesh_mut(me)?;
    let src = m.vertex_group_names[idg].clone();
    let locked = m.vertex_group_locked(idg);
    let name = deform::unique_group_name(&m.vertex_group_names, None, &copy_name(&src));
    m.vertex_group_names.push(name);
    let icdg = m.vertex_group_names.len() - 1;
    m.set_vertex_group_locked(icdg, locked);
    cx.object_mut(ob)?.vertex_groups_active_index = icdg as i32;
    if let Some(mut w) = Weights::load(cx, me) {
        for dv in &mut w.dv {
            if let Some(i) = deform::find(dv, idg as u32) {
                let weight = dv.weights[i].1;
                *deform::ensure(dv, icdg as u32) = weight;
            }
        }
        w.store(cx)?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_names() {
        assert_eq!(copy_name("Group"), "Group_copy");
        assert_eq!(copy_name("Group_copy"), "Group_copy");
        assert_eq!(copy_name("a_copy_b"), "a_copy_b");
        assert_eq!(copy_name(&"x".repeat(70)).len(), 63);
    }
}
