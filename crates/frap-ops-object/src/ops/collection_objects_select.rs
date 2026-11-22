// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.collection_objects_select()`, tier T0, *headless-extended* (deviations
//! D-150; `tests/golden/cases/object/select_m8/collection_objects*.py`). Ported from
//! blender/source/blender/editors/object/object_collection.cc (`select_grouped_exec`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! Blender reads the `collection` context member, which only the Outliner and the Properties
//! editor provide: in a 3D View (and in `blender -b`) it is missing and the operator cancels.
//! frap runs it as from the Outliner on the active collection (`bpy.context.collection`, the
//! scene collection included). Verified on 5.2.4 (with `temp_override(collection=…)`): poll
//! `ED_operator_objectmode`; the visible, unselected, selectable bases whose object is in the
//! collection or one of its child collections are selected; always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::Main;
use frap_spec_ops_object::object;

use super::move_to_collection::common::{SceneColl, get};
use super::select_grouped::bases::Bases;

/// `BKE_collection_has_object_recursive`: the objects of a collection and its children.
fn recursive_objects(
    main: &Main,
    scene: IdRef<frap_rna::SceneId>,
    c: SceneColl,
) -> Vec<IdRef<ObjectId>> {
    fn go(
        main: &Main,
        scene: IdRef<frap_rna::SceneId>,
        c: SceneColl,
        out: &mut Vec<IdRef<ObjectId>>,
        depth: usize,
    ) {
        let Some(col) = get(main, scene, c) else {
            return;
        };
        if depth > 256 {
            return;
        }
        out.extend(col.objects.iter().copied());
        for &ch in &col.children {
            go(main, scene, Some(ch), out, depth + 1);
        }
    }
    let mut out = Vec::new();
    go(main, scene, c, &mut out, 0);
    out
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    notes = ["headless-extended (D-150): runs on the active collection, as Blender does from the Outliner (a 3D View has no collection context and cancels)"]
)]
fn exec(cx: &mut OpCtx, _p: object::CollectionObjectsSelect) -> OpResult {
    let objects = {
        let c = cx.ctx();
        recursive_objects(c.main, c.win.scene, c.collection())
    };
    let mut b = Bases::load(cx);
    for i in b.visible() {
        let base = b.list[i];
        if !base.selected && base.selectable && objects.contains(&base.ob) {
            b.select(i, true);
        }
    }
    b.commit(cx);
    Ok(OpStatus::Finished)
}
