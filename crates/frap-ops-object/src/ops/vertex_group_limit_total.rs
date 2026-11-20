// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_limit_total(group_select_mode='', limit=4)`, tier T0 (the
//! glTF exporter's 4-influence limit). Port of `vertex_group_limit_total_exec` and
//! `vgroup_limit_total_subset` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): on every object in Edit Mode (or every selected mesh object), each vertex
//! (the selected ones in Edit Mode) keeps at most `limit` weights of the subset (`ALL` unless
//! asked otherwise), dropping the lowest; its weight list is re-packed as Blender's (other
//! groups first, then the subset's in descending order). INFO `N vertex weights limited`.
//! Always `FINISHED`. Poll `vertex_group_poll`.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn limit_total_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_limit_total")
}

/// `vertex_group_poll`.
const LIMIT_TOTAL_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: limit_total_poll,
};

/// `rna_vertex_group_select_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    common::subset_items(c, false)
}

#[operator(poll = LIMIT_TOTAL_POLL, tier = T0, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupLimitTotal) -> OpResult {
    let mode = if p.group_select_mode.value.is_empty() {
        "ALL"
    } else {
        p.group_select_mode.value.as_str()
    };
    let limit = p.limit.value.max(0) as usize;
    let objects = common::objects_for_wpaint(&cx.ctx());
    let mut removed = 0usize;
    for ob in objects {
        let (validmap, subset_count, me) = {
            let c = cx.ctx();
            let Some(o) = c.main.objects.get(ob) else {
                continue;
            };
            let Some(me) = common::mesh_of(&c, o) else {
                continue;
            };
            let (v, k) = common::subset(&c, o, mode);
            (v, k, me)
        };
        let sel = common::use_vert_sel(cx, ob);
        common::for_each_dvert(cx, me, sel, |dv| {
            removed += deform::limit_total(dv, &validmap, subset_count, limit);
        })?;
    }
    if removed > 0 {
        cx.report(
            ReportType::Info,
            format!("{removed} vertex weights limited"),
        );
    }
    Ok(OpStatus::Finished)
}
