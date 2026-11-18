// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.collection_link(collection=…)`, tier T0
//! (`tests/golden/cases/object/collection/*.py`). Port of `collection_link_exec` and
//! `RNA_collection_local_itemf` (`editors/object/object_collection.cc`,
//! `makesrna/intern/rna_object.cc`, Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//! - `collection` is a dynamic enum of the local collections of `bpy.data`, by name, in
//!   `bpy.data.collections` order (an unknown name is Blender's `enum "X" not found in (…)`);
//! - unset, it is the first collection (enum value 0);
//! - links the active object (`context.object`) to it, whether or not the collection is in a
//!   scene; already linked → `FINISHED` without a change; no active object → `CANCELLED`;
//! - poll `ED_operator_objectmode`.
//!
//! Blender also refuses a collection the object instances (a dependency cycle); frap has no
//! collection instancing before M8, so that check cannot fail.

use frap_op::EnumItemOwned;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{PyExc, RnaError, RnaErrorKind};
use frap_scene::Context;
use frap_scene::id::layers;
use frap_spec_ops_object::object;

/// `RNA_collection_local_itemf`: every local collection, identifier and name = its name.
fn collection_items(c: &Context) -> Vec<EnumItemOwned> {
    c.main
        .collections
        .iter()
        .filter(|(_, col)| col.header.library.is_none())
        .map(|(_, col)| EnumItemOwned {
            identifier: col.header.name.clone(),
            name: col.header.name.clone(),
            description: Default::default(),
        })
        .collect()
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    dyn_enums = [("collection", collection_items)],
    notes = ["collection items: the names of the local bpy.data.collections"]
)]
fn exec(cx: &mut OpCtx, p: object::CollectionLink) -> OpResult {
    // The session converts the arguments with the context; a caller converting without one
    // gets the dynamic items checked here, with Blender's conversion message.
    let items = collection_items(&cx.ctx());
    // Unset: `RNA_enum_get` gives value 0, the first collection (`BLI_findlink(…, 0)`).
    let wanted = if p.collection.is_set {
        p.collection.value.clone()
    } else {
        items
            .first()
            .map(|i| i.identifier.clone())
            .unwrap_or_default()
    };
    if p.collection.is_set && !items.iter().any(|i| i.identifier == wanted) {
        let list = items
            .iter()
            .map(|i| format!("'{}'", i.identifier))
            .collect::<Vec<_>>()
            .join(", ");
        let mut e = RnaError::new(
            RnaErrorKind::Enum,
            PyExc::TypeError,
            format!(
                "Converting py args to operator properties: enum \"{}\" not found in ({list})",
                wanted
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
        return Err(OpError::Rna(e));
    }
    let ob = cx.ctx().object();
    let col = cx.main().collections.find_in(&wanted, None);
    let (Some(ob), Some(col)) = (ob, col) else {
        cx.report(
            ReportType::Warning,
            "frap: nothing linked: collection_link needs an active object and a local collection",
        );
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    if main
        .collections
        .get(col)
        .is_some_and(|c| c.objects.contains(&ob))
    {
        return Ok(OpStatus::Finished);
    }
    if let Some(c) = main.collections.get_mut(col, log) {
        c.objects.push(ob);
    }
    layers::sync_all(main, log);
    Ok(OpStatus::Finished)
}
