// SPDX-License-Identifier: GPL-3.0-or-later
//! Objects a final render sees (the render depsgraph of one view layer).
//!
//! Verified on Blender 5.2.4 (Workbench renders of the factory cube): an object is rendered
//! unless `Object.hide_render`, a `Collection.hide_render` on its path, or `exclude` on its
//! layer collection hides it. `Object.hide_viewport`, `hide_set(True)`,
//! `Collection.hide_viewport`, and the layer collection's `hide_viewport`, `holdout` and
//! `indirect_only` do not remove it (`holdout`/`indirect_only` change shading in Cycles/EEVEE
//! only).

use frap_rna::{CollectionId, IdKey, IdRef, ObjectId};
use frap_scene::id::{Collection, Scene};
use frap_scene::{LayerCollection, Main, ViewLayer};
use indexmap::IndexSet;

/// Every render-visible object of `scene` in `view_layer`, each once, in collection traversal
/// order (master collection first, then children depth-first).
pub fn render_objects(main: &Main, scene: &Scene, view_layer: &ViewLayer) -> Vec<IdRef<ObjectId>> {
    let mut out = IndexSet::new();
    let mut stack = Vec::new();
    visit(
        main,
        &scene.collection,
        None,
        Some(&view_layer.layer_collection),
        &mut stack,
        &mut out,
    );
    out.into_iter()
        .filter(|ob| main.objects.get(*ob).is_some_and(|o| !o.hide_render))
        .collect()
}

fn visit(
    main: &Main,
    coll: &Collection,
    key: Option<IdKey>,
    lc: Option<&LayerCollection>,
    stack: &mut Vec<IdKey>,
    out: &mut IndexSet<IdRef<ObjectId>>,
) {
    if coll.hide_render || lc.is_some_and(|lc| lc.exclude) {
        return;
    }
    out.extend(coll.objects.iter().copied());
    if let Some(k) = key {
        stack.push(k);
    }
    for &child in &coll.children {
        // Blender forbids collection cycles; guard anyway.
        if stack.contains(&child.key()) {
            continue;
        }
        let Some(c) = main.collections.get(child) else {
            continue;
        };
        visit(
            main,
            c,
            Some(child.key()),
            child_layer(lc, child),
            stack,
            out,
        );
    }
    if key.is_some() {
        stack.pop();
    }
}

fn child_layer(
    lc: Option<&LayerCollection>,
    child: IdRef<CollectionId>,
) -> Option<&LayerCollection> {
    lc?.children.iter().find(|c| c.collection == Some(child))
}
