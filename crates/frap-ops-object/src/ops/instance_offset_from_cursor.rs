// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.instance_offset_from_cursor()`, tier T0
//! (`tests/golden/cases/object/transform_m8/instance_offset.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/object.py (`DupliOffsetFromCursor`),
//! Blender 5.2.4, GPL-2.0-or-later: `context.collection.instance_offset =
//! scene.cursor.location` (the active layer collection's collection, the scene collection
//! included). No poll; every mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

/// `context.collection.instance_offset`: the active layer collection's collection (the
/// scene's master collection when that is the active one).
pub(crate) fn offset_of(c: &frap_scene::Context) -> [f32; 3] {
    match c.collection() {
        Some(r) => c
            .main
            .collections
            .get(r)
            .map_or([0.0; 3], |col| col.instance_offset),
        None => c.scene().collection.instance_offset,
    }
}

/// Sets `context.collection.instance_offset`.
pub(crate) fn set_offset(cx: &mut OpCtx, v: [f32; 3]) {
    let (col, scene) = {
        let c = cx.ctx();
        (c.collection(), c.win.scene)
    };
    let (main, log) = cx.main_mut();
    match col {
        Some(r) => {
            if let Some(col) = main.collections.get_mut(r, log) {
                col.instance_offset = v;
            }
        }
        None => {
            if let Some(s) = main.scenes.get_mut(scene, log) {
                s.collection.instance_offset = v;
            }
        }
    }
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::InstanceOffsetFromCursor) -> OpResult {
    let v = cx.ctx().cursor().location;
    set_offset(cx, v);
    Ok(OpStatus::Finished)
}
