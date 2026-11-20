// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_clean(group_select_mode='', limit=0.0, keep_single=False)`,
//! tier T0. Port of `vertex_group_clean_exec`, `vgroup_clean_subset` and
//! `vgroup_parray_remove_zero` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): on every object in Edit Mode (or every selected mesh object), the
//! weights `<= limit` of the subset (the active group unless asked otherwise) are removed from
//! each vertex (the selected ones in Edit Mode), the last one kept with `keep_single`. Mesh
//! X-mirror cleaning is not done (frap meshes have no `use_mirror_x`). Poll
//! `vertex_group_poll`.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn clean_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_clean")
}

/// `vertex_group_poll`.
const CLEAN_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: clean_poll,
};

/// `rna_vertex_group_with_single_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    common::subset_items(c, true)
}

#[operator(poll = CLEAN_POLL, tier = T0, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupClean) -> OpResult {
    let mode = if p.group_select_mode.value.is_empty() {
        "ACTIVE"
    } else {
        p.group_select_mode.value.as_str()
    };
    let (limit, keep_single) = (p.limit.value, p.keep_single.value);
    let objects = common::objects_for_wpaint(&cx.ctx());
    for ob in objects {
        let (validmap, me) = {
            let c = cx.ctx();
            let Some(o) = c.main.objects.get(ob) else {
                continue;
            };
            let Some(me) = common::mesh_of(&c, o) else {
                continue;
            };
            (common::subset(&c, o, mode).0, me)
        };
        let sel = common::use_vert_sel(cx, ob);
        common::for_each_dvert(cx, me, sel, |dv| {
            deform::remove_zero(dv, &validmap, limit, keep_single);
        })?;
    }
    Ok(OpStatus::Finished)
}
