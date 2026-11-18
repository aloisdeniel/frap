// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.hide_view_clear(select=True)`, tier T0
//! (`tests/golden/cases/scenes/hide_view.py`; M3 WP 3.10, M34W0-4). Port of
//! `object_hide_view_clear_exec` and `object_hide_poll` (`editors/object/object_edit.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): every hidden base of the context view layer
//! (`hide_get()`) is shown again and, with `select=True`, selected, even when it is not
//! selectable (`base_select` is bypassed, as in Blender). No hidden base → `CANCELLED`. The poll
//! is `ED_operator_view3d_active`, which frap's virtual 3D View always passes (§8).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::BaseFlag;
use frap_spec_ops_object::object;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::HideViewClear) -> OpResult {
    let select = p.select.value;
    let (scene, layer, shown) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let shown: Vec<IdRef<ObjectId>> = vl
            .bases
            .iter()
            .filter(|b| b.flag.contains(BaseFlag::HIDDEN))
            .map(|b| b.object)
            .collect();
        (c.win.scene, layer, shown)
    };
    if shown.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing to show: no object is hidden in the view layer",
        );
        return Ok(OpStatus::Cancelled);
    }
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        for ob in &shown {
            if let Some(b) = s.view_layers[layer].base_mut(*ob) {
                b.flag.remove(BaseFlag::HIDDEN);
                if select {
                    b.flag.insert(BaseFlag::SELECTED);
                }
            }
        }
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
