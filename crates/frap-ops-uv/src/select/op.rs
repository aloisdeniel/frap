// SPDX-License-Identifier: GPL-3.0-or-later
//! Operator glue of the UV selection operators: the objects they visit
//! (`BKE_view_layer_array_from_objects_in_edit_mode_unique_data_with_uvs`), the edit BMesh of
//! each, and the tool settings they write.

use frap_op::OpCtx;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};
use std::sync::Arc;

use frap_bmesh::BMesh;
use frap_scene::generated::plain::ToolSettings;

use super::{Ts, UvSel, active_uv};

/// The edit BMesh of `me` (shared, read-only).
pub(crate) fn edit_bm(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<Arc<BMesh>> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| em.bm.clone())
}

/// The meshes in Edit Mode with a UV map (active first, unique data).
pub(crate) fn objects(cx: &mut OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    frap_op::helpers::iter_edit_meshes(cx)
        .into_iter()
        .filter(|&(_, me)| edit_bm(cx, me).is_some_and(|bm| active_uv(&bm).is_some()))
        .collect()
}

/// The context scene's UV selection settings.
pub(crate) fn ts(cx: &OpCtx) -> Ts {
    Ts::of(&cx.ctx())
}

/// Runs `f` on the UV selection of `me`'s edit BMesh (`None` without a UV map).
pub(crate) fn edit<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    ts: Ts,
    f: impl FnOnce(&mut UvSel) -> R,
) -> Result<Option<R>, OpError> {
    if edit_bm(cx, me).is_none_or(|bm| active_uv(&bm).is_none()) {
        return Ok(None);
    }
    let bm = cx.bmesh_mut(me)?;
    Ok(UvSel::new(ts, bm).map(|mut s| f(&mut s)))
}

/// Runs `f` on the UV selection of a copy of `me`'s edit BMesh (read-only passes).
pub(crate) fn read<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    ts: Ts,
    f: impl FnOnce(&mut UvSel) -> R,
) -> Option<R> {
    let bm = edit_bm(cx, me)?;
    let mut copy = (*bm).clone();
    UvSel::new(ts, &mut copy).map(|mut s| f(&mut s))
}

/// Writes the context scene's tool settings.
pub(crate) fn set_tool_settings(cx: &mut OpCtx, f: impl FnOnce(&mut ToolSettings)) {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        f(&mut s.tool_settings);
    }
}
