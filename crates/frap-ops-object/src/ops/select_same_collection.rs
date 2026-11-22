// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_same_collection(collection='')`, tier T0
//! (`tests/golden/cases/object/select_m8/same_collection*.py`). Ported from
//! blender/source/blender/editors/object/object_select.cc
//! (`object_select_same_collection_exec`), Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `objects_selectable_poll`; no visible object, or no collection of that name in
//!   `bpy.data.collections` (the scene collection is not one) → `PASS_THROUGH`, which frap
//!   answers `FINISHED` with nothing changed (as `select_all`);
//! - otherwise the visible, unselected, selectable bases whose object is directly in the
//!   collection are selected (its child collections do not count); always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use super::select_grouped::SELECTABLE;
use super::select_grouped::bases::Bases;

#[operator(
    poll = SELECTABLE,
    tier = T0,
    notes = ["no visible object or no such collection: Blender passes through (frap: FINISHED, nothing changes)"]
)]
fn exec(cx: &mut OpCtx, p: object::SelectSameCollection) -> OpResult {
    let mut b = Bases::load(cx);
    if b.visible().is_empty() {
        return Ok(OpStatus::Finished);
    }
    let objects = {
        let main = cx.main();
        match main.collections.find(&p.collection.value) {
            Some(c) => main
                .collections
                .get(c)
                .map(|col| col.objects.clone())
                .unwrap_or_default(),
            None => {
                cx.report(
                    ReportType::Warning,
                    format!(
                        "frap: no collection named {} in bpy.data.collections: nothing selected",
                        poll::py_name(&p.collection.value)
                    ),
                );
                return Ok(OpStatus::Finished);
            }
        }
    };
    for i in b.visible() {
        let base = b.list[i];
        if !base.selected && base.selectable && objects.contains(&base.ob) {
            b.select(i, true);
        }
    }
    b.commit(cx);
    Ok(OpStatus::Finished)
}
