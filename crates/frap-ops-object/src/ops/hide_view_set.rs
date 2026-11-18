// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.hide_view_set(unselected=False)`, tier T0
//! (`tests/golden/cases/scenes/hide_view.py`; M3 WP 3.10, M34W0-4). Port of
//! `object_hide_view_set_exec` and `object_hide_poll` (`editors/object/object_edit.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): in the context view layer, every **visible** base that is
//! selected (`unselected=True`: not selected) is deselected and hidden (`hide_get()` becomes
//! True; `Object.hide_viewport` is untouched). Nothing hidden → `CANCELLED`. The poll is
//! `ED_operator_view3d_active`, which frap's virtual 3D View always passes (§8); the
//! `N object(s) hidden` INFO report is only made when the operator is invoked from the UI.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::BaseFlag;
use frap_spec_ops_object::object;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::HideViewSet) -> OpResult {
    let unselected = p.unselected.value;
    let (scene, layer, hide) = {
        let c = cx.ctx();
        let vis = c.base_visibility();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let hide: Vec<IdRef<ObjectId>> = vl
            .bases
            .iter()
            .filter(|b| vis.get(&b.object).is_some_and(|v| v.visible))
            .filter(|b| b.selected() != unselected)
            .map(|b| b.object)
            .collect();
        (c.win.scene, layer, hide)
    };
    if hide.is_empty() {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: nothing hidden: no visible {} object",
                if unselected { "unselected" } else { "selected" }
            ),
        );
        return Ok(OpStatus::Cancelled);
    }
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        for ob in &hide {
            if let Some(b) = s.view_layers[layer].base_mut(*ob) {
                b.flag.remove(BaseFlag::SELECTED);
                b.flag.insert(BaseFlag::HIDDEN);
            }
        }
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
