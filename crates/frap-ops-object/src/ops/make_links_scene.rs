// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.make_links_scene(scene=…)`, tier T0
//! (`tests/golden/cases/object/data_m8/links_scene.py`). Port of `make_links_scene_exec`
//! (`editors/object/object_relations.cc`) and `RNA_scene_local_itemf` / `rna_id_itemf`
//! (`windowmanager/intern/wm_operators.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4:
//! - `scene` items: the local scenes of `bpy.data.scenes`, by name, in list order; an item's
//!   value is its position in the whole list; unset, the value is 0: the **first** scene;
//! - every selected object (`selected_bases`, editable or not) is linked to the target
//!   scene's master collection (`scene.collection.objects`), once;
//! - the context scene: `RuntimeError: Error: Cannot link objects into the same scene`; a
//!   linked scene: `Cannot link objects into a linked scene`;
//! - no poll; always `FINISHED` otherwise (also with nothing selected).

use frap_op::EnumItemOwned;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{PyExc, RnaError, RnaErrorKind};
use frap_scene::Context;
use frap_scene::id::layers;
use frap_spec_ops_object::object;

use super::move_to_collection::common::link_object;

/// `RNA_scene_local_itemf`: the local scenes, identifier and name = the scene name.
fn scene_items(c: &Context) -> Vec<EnumItemOwned> {
    c.main
        .scenes
        .iter()
        .filter(|(_, s)| s.header.library.is_none())
        .map(|(_, s)| EnumItemOwned {
            identifier: s.header.name.clone(),
            name: s.header.name.clone(),
            description: Default::default(),
        })
        .collect()
}

fn enum_error(wanted: &str, items: &[EnumItemOwned]) -> OpError {
    let list = items
        .iter()
        .map(|i| format!("'{}'", i.identifier))
        .collect::<Vec<_>>()
        .join(", ");
    let mut e = RnaError::new(
        RnaErrorKind::Enum,
        PyExc::TypeError,
        format!(
            "Converting py args to operator properties: enum \"{wanted}\" not found in ({list})"
        ),
    );
    e.detail.insert(
        "valid".into(),
        items
            .iter()
            .map(|i| i.identifier.as_str())
            .collect::<Vec<_>>()
            .into(),
    );
    OpError::Rna(e)
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    dyn_enums = [("scene", scene_items)],
    notes = ["scene items: the local scenes by name; unset is the first scene of bpy.data.scenes"]
)]
fn exec(cx: &mut OpCtx, p: object::MakeLinksScene) -> OpResult {
    let (target, current, selected) = {
        let c = cx.ctx();
        let items = scene_items(&c);
        if p.scene.is_set && !items.iter().any(|i| i.identifier == p.scene.value) {
            return Err(enum_error(&p.scene.value, &items));
        }
        // `BLI_findlink(&bmain->scenes, RNA_enum_get(op->ptr, "scene"))`.
        let target = if p.scene.is_set {
            c.main.scenes.find_in(&p.scene.value, None)
        } else {
            c.main.scenes.at(0)
        };
        (target, c.win.scene, c.selected_objects())
    };
    let Some(target) = target else {
        return Err(OpError::Runtime("Could not find scene".into()));
    };
    if target == current {
        return Err(OpError::Runtime(
            "Cannot link objects into the same scene".into(),
        ));
    }
    let (main, log) = cx.main_mut();
    if main
        .scenes
        .get(target)
        .is_some_and(|s| s.header.library.is_some())
    {
        return Err(OpError::Runtime(
            "Cannot link objects into a linked scene".into(),
        ));
    }
    let mut changed = false;
    for ob in selected {
        changed |= link_object(main, log, target, None, ob);
    }
    if changed {
        layers::sync_all(main, log);
    }
    Ok(OpStatus::Finished)
}
