// SPDX-License-Identifier: GPL-3.0-or-later
//! The bases of the context view layer as Blender's object selection code sees them (WP
//! 8.3.4). Ported from blender/source/blender/editors/object/object_select.cc (`base_select`,
//! `base_activate`, `base_deselect_all_ex`) and the `visible_bases` / `selectable_bases`
//! context members (`editors/screen/screen_context.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! [`Bases::load`] takes one snapshot, the operator changes it in memory, [`Bases::commit`]
//! writes back the bases whose selection changed and the active object.

use frap_op::OpCtx;
use frap_rna::{IdRef, ObjectId, SceneId};
use frap_scene::BaseFlag;

/// One base of the context view layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Base {
    /// The object.
    pub ob: IdRef<ObjectId>,
    /// `BASE_SELECTED`.
    pub selected: bool,
    /// `BASE_VISIBLE` (`visible_get()`).
    pub visible: bool,
    /// `BASE_SELECTABLE` (a base hidden with `hide_set`, `hide_select` or a collection's
    /// `hide_select` / `hide_viewport` is not; a layer collection's `hide_viewport` keeps it).
    pub selectable: bool,
}

/// The bases of the context view layer, in base order, and its active object.
#[derive(Clone, Debug)]
pub(crate) struct Bases {
    scene: IdRef<SceneId>,
    layer: usize,
    /// Base order.
    pub list: Vec<Base>,
    /// `view_layer.objects.active`.
    pub active: Option<IdRef<ObjectId>>,
    loaded: Vec<Base>,
    loaded_active: Option<IdRef<ObjectId>>,
}

impl Bases {
    /// Snapshot of the context view layer.
    pub fn load(cx: &OpCtx) -> Bases {
        let c = cx.ctx();
        let vis = c.base_visibility();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let list: Vec<Base> = vl
            .bases
            .iter()
            .map(|b| {
                let v = vis.get(&b.object).copied().unwrap_or_default();
                Base {
                    ob: b.object,
                    selected: b.selected(),
                    visible: v.visible,
                    selectable: v.selectable,
                }
            })
            .collect();
        Bases {
            scene: c.win.scene,
            layer,
            loaded: list.clone(),
            list,
            active: vl.active,
            loaded_active: vl.active,
        }
    }

    /// Index of the base of `ob` (`BKE_view_layer_base_find`).
    pub fn find(&self, ob: IdRef<ObjectId>) -> Option<usize> {
        self.list.iter().position(|b| b.ob == ob)
    }

    /// `base_select(base, BA_SELECT / BA_DESELECT)`: selecting needs `BASE_SELECTABLE`.
    pub fn select(&mut self, i: usize, state: bool) {
        let b = &mut self.list[i];
        if !state {
            b.selected = false;
        } else if b.selectable {
            b.selected = true;
        }
    }

    /// `BASE_SELECTABLE(v3d, base)`: visible and selectable (`selectable_bases`).
    pub fn is_selectable(&self, i: usize) -> bool {
        self.list[i].visible && self.list[i].selectable
    }

    /// Indices of the visible bases (`visible_bases`), in base order.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.list.len())
            .filter(|&i| self.list[i].visible)
            .collect()
    }

    /// Indices of the visible and selectable bases (`selectable_bases`), in base order.
    pub fn selectable(&self) -> Vec<usize> {
        (0..self.list.len())
            .filter(|&i| self.is_selectable(i))
            .collect()
    }

    /// Indices of the selected visible bases (`selected_bases` / `selected_objects`).
    pub fn selected(&self) -> Vec<usize> {
        (0..self.list.len())
            .filter(|&i| self.list[i].visible && self.list[i].selected)
            .collect()
    }

    /// `base_deselect_all(…, SEL_DESELECT)`: deselects every visible selected base; returns
    /// whether one changed.
    pub fn deselect_all(&mut self) -> bool {
        let mut changed = false;
        for i in self.visible() {
            if self.list[i].selected {
                self.select(i, false);
                changed = true;
            }
        }
        changed
    }

    /// Whether the selection or the active object differs from the snapshot.
    pub fn changed(&self) -> bool {
        self.active != self.loaded_active
            || self
                .list
                .iter()
                .zip(&self.loaded)
                .any(|(a, b)| a.selected != b.selected)
    }

    /// Writes the changed selection flags and the active object to the view layer.
    pub fn commit(self, cx: &mut OpCtx) {
        if !self.changed() {
            return;
        }
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(self.scene, log) {
            let vl = &mut s.view_layers[self.layer];
            for (b, old) in self.list.iter().zip(&self.loaded) {
                if b.selected != old.selected
                    && let Some(base) = vl.base_mut(b.ob)
                {
                    base.flag.set(BaseFlag::SELECTED, b.selected);
                }
            }
            vl.active = self.active;
        }
        log.context_changed = true;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use frap_rna::IdKey;

    /// Bases for unit tests: (selected, visible, selectable) per object.
    pub(crate) fn bases(spec: &[(bool, bool, bool)]) -> (Bases, Vec<IdRef<ObjectId>>) {
        let obs: Vec<IdRef<ObjectId>> = spec
            .iter()
            .map(|_| IdRef::from_key(IdKey::next()))
            .collect();
        let list: Vec<Base> = spec
            .iter()
            .zip(&obs)
            .map(|(&(selected, visible, selectable), &ob)| Base {
                ob,
                selected,
                visible,
                selectable,
            })
            .collect();
        (
            Bases {
                scene: IdRef::from_key(IdKey::next()),
                layer: 0,
                loaded: list.clone(),
                list,
                active: None,
                loaded_active: None,
            },
            obs,
        )
    }

    #[test]
    fn select_needs_selectable_and_deselect_all_skips_hidden() {
        let (mut b, _) = bases(&[
            (true, true, true),
            (true, false, true),
            (false, true, false),
        ]);
        b.select(2, true);
        assert!(!b.list[2].selected);
        assert!(b.deselect_all());
        assert!(!b.list[0].selected);
        // A hidden base keeps its selection (it is not a visible base).
        assert!(b.list[1].selected);
        assert!(!b.deselect_all());
        assert_eq!(b.selectable(), vec![0]);
    }
}
