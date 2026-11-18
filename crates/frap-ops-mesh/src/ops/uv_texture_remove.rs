// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.uv_texture_remove()`, tier T1
//! (`tests/golden/cases/edit/normals/uv_texture_*.py`). Port of `mesh_uv_texture_remove_exec`
//! and `uv_texture_remove_poll` (`editors/mesh/mesh_data.cc`) with `BKE_attribute_remove`,
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll: the active object is an editable mesh whose active UV map exists (in Edit Mode, as a
//! layer of the edit BMesh). Removes the **active UV map** and its `.pn.` pin layer; the
//! active and render UV maps move to the UV map now at their clamped index (none when no UV
//! map is left). The active attribute keeps its index, as in Blender.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;

use super::geometry__attribute_add::{attrs, common};
use super::geometry__color_attribute_remove::remove_keep_active_index;

const IDNAME: &str = "mesh.uv_texture_remove";

/// Whether the active UV map name is a UV map of `store`.
fn active_uv_found(store: &dyn attrs::Store, names: &attrs::Names) -> bool {
    let list = store.layers();
    names.uv_active.as_deref().is_some_and(|n| {
        list.iter()
            .any(|l| l.name == n && attrs::is_uv(&l.name, l.domain, l.data_type))
    })
}

/// `uv_texture_remove_poll` (on the mesh in `Main`; `exec` re-checks the edit BMesh).
fn remove_poll(c: &Context) -> Result<(), PollFail> {
    poll::active_editable_mesh(c)?;
    let me = common::context_mesh_data(c).expect("checked by active_editable_mesh");
    let names = attrs::Names::of(me);
    let ok = if poll::edit_object(c).is_some() {
        names.uv_active.is_some()
    } else {
        active_uv_found(me, &names)
    };
    if ok {
        Ok(())
    } else {
        Err(common::poll_fail(
            "the mesh has no active UV map: bpy.ops.mesh.uv_texture_add()",
        ))
    }
}

const REMOVE_POLL: Poll = Poll {
    requires: common::ATTRIBUTES_REQUIRES,
    check: remove_poll,
};

#[operator(poll = REMOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::UvTextureRemove) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    if common::read(cx, me, active_uv_found) != Some(true) {
        return Err(common::exec_poll_failed(IDNAME));
    }
    let r = common::with_store(cx, me, |store, names, _| {
        let name = names.uv_active.clone().unwrap_or_default();
        remove_keep_active_index(store, names, &name)
    })?;
    match r {
        Ok(true) => Ok(OpStatus::Finished),
        Ok(false) => Ok(OpStatus::Cancelled),
        Err(m) => Err(OpError::Runtime(m)),
    }
}
