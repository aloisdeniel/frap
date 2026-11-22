// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.instance_offset_to_cursor()`, tier T0
//! (`tests/golden/cases/object/transform_m8/instance_offset.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/object.py (`DupliOffsetToCursor`),
//! Blender 5.2.4, GPL-2.0-or-later: `scene.cursor.location = context.collection
//! .instance_offset` (the cursor's rotation is kept). No poll; every mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use crate::support::cursor;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::InstanceOffsetToCursor) -> OpResult {
    let (v, scene) = {
        let c = cx.ctx();
        (
            super::instance_offset_from_cursor::offset_of(&c),
            c.win.scene,
        )
    };
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.cursor.location = v;
        cursor::sync_matrix(&mut s.cursor);
    }
    Ok(OpStatus::Finished)
}
