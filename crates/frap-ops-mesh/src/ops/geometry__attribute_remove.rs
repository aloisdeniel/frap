// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.attribute_remove()`, tier T1 (`tests/golden/cases/edit/normals/
//! attribute_*.py`). Port of `geometry_attribute_remove_exec` and
//! `geometry_attributes_remove_poll` (`editors/geometry/geometry_attributes.cc`) with
//! `BKE_attribute_remove` (`blenkernel/intern/attribute.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Removes the **active attribute** (poll: there is one). `position`, `.edge_verts`,
//! `.corner_vert` and `.corner_edge` are refused (`Attribute 'position' is required and cannot
//! be removed`). Removing the active/default colour or the active/render UV map moves those to
//! the attribute now at their clamped index; a UV map's `.pn.` pin layer goes with it. The
//! active attribute then moves one index down (unless it was the first) and to the nearest
//! attribute that is not internal (`BKE_attributes_active_index_validate`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_spec_ops_mesh::geometry;

use super::geometry__attribute_add::{attrs, common};

const IDNAME: &str = "geometry.attribute_remove";

/// `geometry_attributes_remove_poll`: an active attribute (checked on the mesh in `Main`; in
/// Edit Mode `exec` checks the edit BMesh).
fn remove_poll(c: &Context) -> Result<(), PollFail> {
    common::attributes_poll(c)?;
    let me = common::context_mesh_data(c).expect("checked by attributes_poll");
    let names = attrs::Names::of(me);
    let ok = if poll::edit_object(c).is_some() {
        names.active.is_some()
    } else {
        attrs::active_name(me, &names).is_some()
    };
    if ok {
        Ok(())
    } else {
        Err(common::poll_fail(
            "bpy.ops.geometry.attribute_add(...) or set mesh.attributes.active first",
        ))
    }
}

const REMOVE_POLL: Poll = Poll {
    requires: common::ATTRIBUTES_REQUIRES,
    check: remove_poll,
};

/// Removes the active attribute and moves the activity (module docs); `Ok(false)` when there
/// is no active attribute.
pub(crate) fn remove_active(
    store: &mut dyn attrs::Store,
    names: &mut attrs::Names,
) -> Result<bool, String> {
    let Some(name) = attrs::active_name(store, names) else {
        return Ok(false);
    };
    let before = store.layers();
    let mut index = attrs::index_of(&before, Some(&name));
    attrs::remove(store, names, &name)?;
    if index > 0 {
        index -= 1;
    }
    let after = store.layers();
    let index = attrs::validate_index(&after, index);
    names.active = attrs::name_at(&after, index, store.is_edit());
    Ok(true)
}

#[operator(poll = REMOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: geometry::AttributeRemove) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let has_active = common::read(cx, me, |s, n| attrs::active_name(s, n).is_some());
    if has_active != Some(true) {
        return Err(common::exec_poll_failed(IDNAME));
    }
    match common::with_store(cx, me, |s, n, _| remove_active(s, n))? {
        Ok(true) => Ok(OpStatus::Finished),
        Ok(false) => Ok(OpStatus::Cancelled),
        Err(m) => Err(OpError::Runtime(m)),
    }
}
