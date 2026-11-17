// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_by_type(extend=False, type='MESH')`, tier T0
//! (`tests/golden/cases/object/select/by_type*.py`). Port of `object_select_by_type_exec`
//! (`editors/object/object_select.cc`, Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//! - poll `ED_operator_objectmode`;
//! - `extend=False` first deselects every **visible** base (`ED_object_base_deselect_all`), then
//!   every visible base whose object has the type is selected (`ED_object_base_select`, which
//!   leaves an unselectable base, `hide_select`, unselected);
//! - always `FINISHED`, also when nothing matches; the active object never changes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_spec_ops_object::object;

use self::common::{set_selection, visible_bases};

/// Selection helpers shared by `select_by_type` and `select_pattern` (WP 1.20).
pub(crate) mod common {
    use frap_op::OpCtx;
    use frap_rna::{IdRef, ObjectId};
    use frap_scene::BaseFlag;

    /// One visible base of the context view layer: (object, selected, selectable), base order.
    pub(crate) type VisibleBase = (IdRef<ObjectId>, bool, bool);

    /// The visible bases of the context view layer (`FOREACH_VISIBLE_BASE`), in base order.
    pub(crate) fn visible_bases(cx: &OpCtx) -> Vec<VisibleBase> {
        let c = cx.ctx();
        let vis = c.base_visibility();
        c.view_layer()
            .bases
            .iter()
            .filter_map(|b| {
                let v = vis.get(&b.object).copied().unwrap_or_default();
                v.visible.then_some((b.object, b.selected(), v.selectable))
            })
            .collect()
    }

    /// Writes the selection of bases of the context view layer (only the bases that change).
    pub(crate) fn set_selection(cx: &mut OpCtx, changes: &[(IdRef<ObjectId>, bool)]) {
        if changes.is_empty() {
            return;
        }
        let (scene, layer) = {
            let c = cx.ctx();
            let vl = c.view_layer();
            let layer = c
                .scene()
                .view_layers
                .iter()
                .position(|v| v.name == vl.name)
                .unwrap_or(0);
            (c.win.scene, layer)
        };
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            for (ob, state) in changes {
                if let Some(b) = s.view_layers[layer].base_mut(*ob) {
                    b.flag.set(BaseFlag::SELECTED, *state);
                }
            }
        }
        log.context_changed = true;
    }

    /// The new selection of the visible bases: deselect all first unless `extend`, then select
    /// the matching selectable bases (`ED_object_base_select(base, BA_SELECT)`). Returns only
    /// the bases whose state changes.
    pub(crate) fn plan(
        bases: &[VisibleBase],
        extend: bool,
        matches: impl Fn(IdRef<ObjectId>) -> bool,
    ) -> Vec<(IdRef<ObjectId>, bool)> {
        bases
            .iter()
            .filter_map(|&(ob, selected, selectable)| {
                let after_deselect = selected && extend;
                let new = after_deselect || (selectable && matches(ob));
                (new != selected).then_some((ob, new))
            })
            .collect()
    }
}

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectByType) -> OpResult {
    let ty = p.r#type.value.identifier();
    let bases = visible_bases(cx);
    let changes = {
        let main = cx.main();
        let is_type = |ob: IdRef<ObjectId>| -> bool {
            main.objects.get(ob).is_some_and(|o| o.object_type() == ty)
        };
        common::plan(&bases, p.extend.value, is_type)
    };
    set_selection(cx, &changes);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::common::plan;
    use frap_rna::{IdKey, IdRef, ObjectId};

    fn ob() -> IdRef<ObjectId> {
        IdRef::from_key(IdKey::next())
    }

    #[test]
    fn deselects_unless_extend_and_skips_unselectable() {
        let (a, b, c) = (ob(), ob(), ob());
        let bases = [(a, true, true), (b, false, true), (c, false, false)];
        // Matches b and c; c is not selectable.
        let m = |o| o == b || o == c;
        assert_eq!(plan(&bases, false, m), vec![(a, false), (b, true)]);
        assert_eq!(plan(&bases, true, m), vec![(b, true)]);
        // Nothing matches: extend changes nothing, otherwise everything is deselected.
        assert!(plan(&bases, true, |_| false).is_empty());
        assert_eq!(plan(&bases, false, |_| false), vec![(a, false)]);
    }
}
