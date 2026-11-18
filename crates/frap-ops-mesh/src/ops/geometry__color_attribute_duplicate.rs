// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.color_attribute_duplicate()`, tier T1
//! (`tests/golden/cases/edit/normals/color_attribute_*.py`). Port of
//! `geometry_color_attribute_duplicate_exec` and `geometry_color_attributes_duplicate_poll`
//! (`editors/geometry/geometry_attributes.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Object Mode only (`Operation is not allowed in edit mode`). Copies the **active colour
//! attribute** (poll: it names a colour attribute) to a new attribute at the end of the list,
//! named after it and made unique (`Attribute` → `Attribute.001`), which becomes the active
//! colour; the default colour and the active attribute are unchanged.

use frap_mesh::Mesh;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_spec_ops_mesh::geometry;

use super::geometry__attribute_add::{attrs, common};
use super::geometry__color_attribute_remove::active_color_found;

const IDNAME: &str = "geometry.color_attribute_duplicate";

/// `geometry_color_attributes_duplicate_poll`.
fn duplicate_poll(c: &Context) -> Result<(), PollFail> {
    common::attributes_poll(c)?;
    common::not_in_edit(c, IDNAME)?;
    let me = common::context_mesh_data(c).expect("checked by attributes_poll");
    if active_color_found(me, &attrs::Names::of(me)) {
        Ok(())
    } else {
        Err(common::poll_fail(
            "bpy.ops.geometry.color_attribute_add() or set mesh.color_attributes.active_color",
        ))
    }
}

const DUPLICATE_POLL: Poll = Poll {
    requires: common::ATTRIBUTES_REQUIRES,
    check: duplicate_poll,
};

#[operator(poll = DUPLICATE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: geometry::ColorAttributeDuplicate) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let m: &mut Mesh = cx.mesh_mut(me)?;
    let Some(src) = m.color_active.clone() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(a) = m.attributes().get(&src).cloned() else {
        return Ok(OpStatus::Cancelled);
    };
    let name = attrs::unique_name(m, &m.vertex_group_names.clone(), &src);
    attrs::Store::insert(m, name.clone(), a.domain, a.data);
    m.color_active = Some(name);
    Ok(OpStatus::Finished)
}
