// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.hide_render_clear_all()`, tier T0
//! (`tests/golden/cases/object/select_m8/hide_render_clear_all.py`). Ported from
//! blender/scripts/startup/bl_operators/object.py (`ClearAllRestrictRender`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4: no poll (any mode); `hide_render = False` on every
//! object of `context.scene.objects` (the objects of the scene's collections, excluded ones
//! included; the objects' own flag only, collections keep theirs); always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_spec_ops_object::object;

use super::move_to_collection::common::{all_scene_collections, get};

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::HideRenderClearAll) -> OpResult {
    let objects: Vec<IdRef<ObjectId>> = {
        let c = cx.ctx();
        let scene = c.win.scene;
        let mut v: Vec<IdRef<ObjectId>> = Vec::new();
        for col in all_scene_collections(c.main, scene) {
            for &ob in get(c.main, scene, col).map(|x| &x.objects[..]).unwrap_or(&[]) {
                if !v.contains(&ob) {
                    v.push(ob);
                }
            }
        }
        v
    };
    let (main, log) = cx.main_mut();
    for ob in objects {
        if main.objects.get(ob).is_some_and(|o| o.hide_render)
            && let Some(o) = main.objects.get_mut(ob, log)
        {
            o.hide_render = false;
        }
    }
    Ok(OpStatus::Finished)
}
