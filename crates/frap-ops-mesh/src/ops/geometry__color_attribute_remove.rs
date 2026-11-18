// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.geometry.color_attribute_remove()`, tier T1
//! (`tests/golden/cases/edit/normals/color_attribute_*.py`). Port of
//! `geometry_color_attribute_remove_exec` and `geometry_color_attributes_remove_poll`
//! (`editors/geometry/geometry_attributes.cc`) with `BKE_attribute_remove`, Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Removes the **active colour attribute** (poll: it names a colour attribute). The active
//! and default colour names then move to the colour attribute now at their clamped index
//! (none when no colour attribute is left). The active attribute index is not adjusted, so
//! the activity moves to whatever attribute now has its index, as in Blender.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_spec_ops_mesh::geometry;

use super::geometry__attribute_add::{attrs, common};

const IDNAME: &str = "geometry.color_attribute_remove";

/// Whether the active colour name is a colour attribute of `store`.
pub(crate) fn active_color_found(store: &dyn attrs::Store, names: &attrs::Names) -> bool {
    let list = store.layers();
    names.color_active.as_deref().is_some_and(|n| {
        list.iter()
            .any(|l| l.name == n && attrs::is_color(l.domain, l.data_type))
    })
}

/// `geometry_color_attributes_remove_poll` (on the mesh in `Main`; `exec` re-checks the edit
/// BMesh).
fn remove_poll(c: &Context) -> Result<(), PollFail> {
    common::attributes_poll(c)?;
    let me = common::context_mesh_data(c).expect("checked by attributes_poll");
    let names = attrs::Names::of(me);
    let ok = if poll::edit_object(c).is_some() {
        names.color_active.is_some()
    } else {
        active_color_found(me, &names)
    };
    if ok {
        Ok(())
    } else {
        Err(common::poll_fail(
            "bpy.ops.geometry.color_attribute_add() or set mesh.color_attributes.active_color",
        ))
    }
}

const REMOVE_POLL: Poll = Poll {
    requires: common::ATTRIBUTES_REQUIRES,
    check: remove_poll,
};

/// Removes `name` keeping the active attribute's *index* (`BKE_attribute_remove` does not
/// touch it).
pub(crate) fn remove_keep_active_index(
    store: &mut dyn attrs::Store,
    names: &mut attrs::Names,
    name: &str,
) -> Result<bool, String> {
    let before = store.layers();
    let index = attrs::index_of(&before, names.active.as_deref());
    let removed = attrs::remove(store, names, name)?;
    if removed && index >= 0 {
        names.active = attrs::name_at(&store.layers(), index, store.is_edit());
    }
    Ok(removed)
}

#[operator(poll = REMOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: geometry::ColorAttributeRemove) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    if common::read(cx, me, active_color_found) != Some(true) {
        return Err(common::exec_poll_failed(IDNAME));
    }
    let r = common::with_store(cx, me, |store, names, _| {
        let name = names.color_active.clone().unwrap_or_default();
        remove_keep_active_index(store, names, &name)
    })?;
    match r {
        Ok(true) => Ok(OpStatus::Finished),
        Ok(false) => Ok(OpStatus::Cancelled),
        Err(m) => Err(OpError::Runtime(m)),
    }
}
