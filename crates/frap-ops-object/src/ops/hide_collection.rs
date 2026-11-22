// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.hide_collection(collection_index=-1, toggle=False, extend=False)`, tier T0
//! (`tests/golden/cases/object/select_m8/hide_collection*.py`). Ported from
//! blender/source/blender/editors/object/object_edit.cc (`object_hide_collection_exec`) and
//! blenkernel/intern/layer.cc (`BKE_layer_collection_from_index`,
//! `BKE_layer_collection_isolate_global`, `BKE_layer_collection_activate`,
//! `BKE_layer_collection_activate_parent`, the active-collection check of
//! `BKE_layer_collection_sync`), Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - no poll; `collection_index` counts the layer collections of the context view layer
//!   breadth first per level (0 the scene collection, then its children, then the
//!   grandchildren below the first child, …); no such index (the default -1 included) →
//!   `CANCELLED`;
//! - without `extend`, every layer collection below the scene collection is hidden
//!   (`hide_viewport`), then the chosen one, its ancestors and its whole subtree are shown and
//!   it becomes the active layer collection (unless excluded); with `extend` a visible
//!   collection is hidden instead, a hidden one shown the same way;
//! - `toggle` only matters with local collections in a 3D View, which frap's virtual view does
//!   not use;
//! - after the view-layer resync, an active layer collection that is now hidden (excluded,
//!   hidden, its collection or an ancestor hidden) moves to its closest visible ancestor.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::LayerCollectionPath;
use frap_scene::id::layers;
use frap_spec_ops_object::object;

use self::layer::{hidden, layer_index_path};

/// Layer-collection helpers shared with `collection_instance_add`.
pub(crate) mod layer {
    use frap_rna::{CollectionId, IdRef};
    use frap_scene::id::Collection;
    use frap_scene::{IdMap, LayerCollection, LayerCollectionPath, ViewLayer};
    use smallvec::SmallVec;

    /// `collection_from_index`: the path of layer collection number `index`.
    pub(crate) fn layer_index_path(vl: &ViewLayer, index: i32) -> Option<LayerCollectionPath> {
        fn go(
            list: &[(&LayerCollection, SmallVec<[u16; 4]>)],
            number: i32,
            i: &mut i32,
        ) -> Option<LayerCollectionPath> {
            for (_, p) in list {
                if *i == number {
                    return Some(LayerCollectionPath(p.clone()));
                }
                *i += 1;
            }
            for (lc, p) in list {
                let children: Vec<(&LayerCollection, SmallVec<[u16; 4]>)> = lc
                    .children
                    .iter()
                    .enumerate()
                    .map(|(k, c)| {
                        let mut q = p.clone();
                        q.push(k as u16);
                        (c, q)
                    })
                    .collect();
                if let Some(r) = go(&children, number, i) {
                    return Some(r);
                }
            }
            None
        }
        if index < 0 {
            return None;
        }
        let mut i = 0;
        go(&[(&vl.layer_collection, SmallVec::new())], index, &mut i)
    }

    /// The layer collections from the root to `path`.
    fn chain<'a>(vl: &'a ViewLayer, path: &LayerCollectionPath) -> Vec<&'a LayerCollection> {
        let mut lc = &vl.layer_collection;
        let mut out = vec![lc];
        for &k in &path.0 {
            match lc.children.get(k as usize) {
                Some(c) => {
                    lc = c;
                    out.push(c);
                }
                None => break,
            }
        }
        out
    }

    /// `layer_collection_hidden`: excluded, hidden, or showing a hidden collection, itself or
    /// along its ancestors.
    pub(crate) fn hidden(
        vl: &ViewLayer,
        collections: &IdMap<Collection>,
        path: &LayerCollectionPath,
    ) -> bool {
        chain(vl, path).iter().any(|lc| {
            lc.exclude
                || lc.hide_viewport
                || lc
                    .collection
                    .and_then(|c: IdRef<CollectionId>| collections.get(c))
                    .is_some_and(|c| c.hide_viewport)
        })
    }

    /// `BKE_layer_collection_activate_parent`: the closest ancestor that is not hidden (the
    /// scene collection at the top).
    pub(crate) fn activate_parent(
        vl: &ViewLayer,
        collections: &IdMap<Collection>,
        path: &LayerCollectionPath,
    ) -> LayerCollectionPath {
        let mut p = path.clone();
        while !p.0.is_empty() {
            p.0.pop();
            if !hidden(vl, collections, &p) {
                return p;
            }
        }
        p
    }
}

/// `BKE_layer_collection_has_layer_collection(lc_parent, lc)`: `lc` is `parent` or below it.
fn contains(parent: &LayerCollectionPath, lc: &LayerCollectionPath) -> bool {
    lc.0.len() >= parent.0.len() && lc.0[..parent.0.len()] == parent.0[..]
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    notes = ["toggle only affects local collections of a 3D View (not used by frap's virtual view)"]
)]
fn exec(cx: &mut OpCtx, p: object::HideCollection) -> OpResult {
    let (scene, layer, target, visible) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let Some(target) = layer_index_path(vl, p.collection_index.value) else {
            cx.report(
                ReportType::Warning,
                format!(
                    "frap: no layer collection has index {} in view layer '{}'",
                    p.collection_index.value, vl.name
                ),
            );
            return Ok(OpStatus::Cancelled);
        };
        let visible = layers::layer_collection_visible(c.scene(), vl, &c.main.collections, &target);
        (c.win.scene, layer, target, visible)
    };
    let extend = p.extend.value;
    let hide_it = extend && visible;
    let (main, log) = cx.main_mut();
    layers::end_lazy_resync(main);
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let vl = &mut s.view_layers[layer];
        if !extend {
            fn set_all(lc: &mut frap_scene::LayerCollection) {
                lc.hide_viewport = true;
                lc.children.iter_mut().for_each(set_all);
            }
            vl.layer_collection.children.iter_mut().for_each(set_all);
        }
        if hide_it {
            if let Some(lc) = vl.layer_collection.at_mut(&target) {
                lc.hide_viewport = true;
            }
        } else {
            // The direct parents become visible (the scene collection's flag is left alone).
            let mut path = LayerCollectionPath::default();
            for &k in target.0.iter().take(target.0.len().saturating_sub(1)) {
                path.0.push(k);
                if contains(&path, &target)
                    && let Some(lc) = vl.layer_collection.at_mut(&path)
                {
                    lc.hide_viewport = false;
                }
            }
            fn unset_all(lc: &mut frap_scene::LayerCollection) {
                lc.hide_viewport = false;
                lc.children.iter_mut().for_each(unset_all);
            }
            if let Some(lc) = vl.layer_collection.at_mut(&target) {
                unset_all(lc);
            }
            // `BKE_layer_collection_activate`: not an excluded one.
            if vl
                .layer_collection
                .at(&target)
                .is_some_and(|lc| !lc.exclude)
            {
                vl.active_layer_collection = target.clone();
            }
        }
    }
    layers::sync_scene(main, scene, log);
    // `BKE_layer_collection_sync`: "Always set a valid active collection".
    let fixed = main.scenes.get(scene).and_then(|s| {
        let vl = &s.view_layers[layer];
        hidden(vl, &main.collections, &vl.active_layer_collection)
            .then(|| layer::activate_parent(vl, &main.collections, &vl.active_layer_collection))
    });
    if let Some(path) = fixed
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        s.view_layers[layer].active_layer_collection = path;
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::{LayerCollection, ViewLayer};
    use smallvec::smallvec;

    fn lc(children: Vec<LayerCollection>) -> LayerCollection {
        let mut l = LayerCollection::new(None);
        l.children = children;
        l
    }

    #[test]
    fn index_counts_each_level_before_descending() {
        // Scene Collection → [Collection, A → [A1, A2], B]
        let vl = ViewLayer {
            name: "ViewLayer".into(),
            bases: Vec::new(),
            active: None,
            layer_collection: lc(vec![
                lc(vec![]),
                lc(vec![lc(vec![]), lc(vec![])]),
                lc(vec![]),
            ]),
            active_layer_collection: LayerCollectionPath::default(),
        };
        let path = |i| layer_index_path(&vl, i).map(|p| p.0.to_vec());
        assert_eq!(path(0), Some(vec![]));
        assert_eq!(path(1), Some(vec![0]));
        assert_eq!(path(2), Some(vec![1]));
        assert_eq!(path(3), Some(vec![2]));
        assert_eq!(path(4), Some(vec![1, 0]));
        assert_eq!(path(5), Some(vec![1, 1]));
        assert_eq!(path(6), None);
        assert_eq!(path(-1), None);
        assert!(contains(
            &LayerCollectionPath(smallvec![1]),
            &LayerCollectionPath(smallvec![1, 0])
        ));
    }
}
