// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_all(action=…)`, tier T0 (`tests/golden/cases/object/delete/
//! select_all_*.py`). Port of `object_select_all_exec`, `objects_selectable_poll` and
//! `ED_object_base_deselect_all_ex` (`editors/object/object_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//! - only **visible** bases of the context view layer take part (`hide_set`, `hide_viewport`
//!   and a layer collection's `hide_viewport` hide a base; a hidden base keeps its selection);
//! - `TOGGLE` deselects when any visible base is selected, else selects;
//! - selecting a visible but unselectable base (`hide_select`, or a collection's
//!   `hide_select`) does nothing but still counts as a change, so the operator finishes;
//! - nothing changed → `CANCELLED` (`SELECT` with everything selected, `DESELECT` with nothing
//!   selected); no visible base at all → `PASS_THROUGH`, which frap answers as `FINISHED`
//!   (nothing changes either way);
//! - the active object never changes (it may end up unselected).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::{BaseFlag, Context, ObjectMode};
use frap_spec_ops_object::object::{self, select_all::Action};

use frap_op::{Poll, PollFail, PollReq};

/// `objects_selectable_poll`: no object in Edit Mode and the active object (if any) in Object
/// Mode. Unlike `ED_operator_objectmode` it accepts a linked scene ("selection is still allowed
/// for inspection").
fn selectable(c: &Context) -> Result<(), PollFail> {
    match poll::active(c) {
        Some((_, ob)) if ob.mode != ObjectMode::Object => Err(poll::fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        )),
        _ => Ok(()),
    }
}

/// `objects_selectable_poll`
const SELECTABLE: Poll = Poll {
    requires: &[PollReq::ModeObject],
    check: selectable,
};

/// What `ED_object_base_deselect_all_ex` does to the visible bases: the new selection of each
/// base it changes, whether Blender counts a change, and whether any base was visible.
#[derive(Debug, Default, PartialEq)]
struct Plan {
    set: Vec<(IdRef<ObjectId>, bool)>,
    changed: bool,
    any_visible: bool,
}

/// One visible base: (object, selected, selectable).
type VisibleBase = (IdRef<ObjectId>, bool, bool);

fn plan(action: Action, bases: &[VisibleBase]) -> Plan {
    let action = match action {
        Action::Toggle if bases.iter().any(|b| b.1) => Action::Deselect,
        Action::Toggle => Action::Select,
        a => a,
    };
    let mut p = Plan::default();
    for &(ob, selected, selectable) in bases {
        p.any_visible = true;
        // `base_select(base, BA_SELECT)` sets the flag only on a selectable base, but the
        // operator counts the call as a change either way.
        let select = match action {
            Action::Select if !selected => true,
            Action::Deselect if selected => false,
            Action::Invert => !selected,
            _ => continue,
        };
        p.changed = true;
        if select != selected && (!select || selectable) {
            p.set.push((ob, select));
        }
    }
    p
}

#[operator(poll = SELECTABLE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectAll) -> OpResult {
    let (scene, layer, bases) = {
        let c = cx.ctx();
        let vis = c.base_visibility();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let bases: Vec<VisibleBase> = vl
            .bases
            .iter()
            .filter_map(|b| {
                let v = vis.get(&b.object).copied().unwrap_or_default();
                v.visible.then_some((b.object, b.selected(), v.selectable))
            })
            .collect();
        (c.win.scene, layer, bases)
    };
    let plan = plan(p.action.value, &bases);
    if !plan.set.is_empty() {
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            for (ob, state) in &plan.set {
                if let Some(b) = s.view_layers[layer].base_mut(*ob) {
                    b.flag.set(BaseFlag::SELECTED, *state);
                }
            }
        }
        log.context_changed = true;
    }
    if plan.changed || !plan.any_visible {
        return Ok(OpStatus::Finished);
    }
    cx.report(
        ReportType::Warning,
        format!(
            "frap: nothing to change: select_all(action='{}') leaves the selection as it is",
            p.action.value.identifier()
        ),
    );
    Ok(OpStatus::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs() -> (IdRef<ObjectId>, IdRef<ObjectId>) {
        (
            IdRef::from_key(frap_rna::IdKey::next()),
            IdRef::from_key(frap_rna::IdKey::next()),
        )
    }

    #[test]
    fn toggle_deselects_when_anything_is_selected() {
        let (a, b) = obs();
        let ob = |i| if i == 1 { a } else { b };
        let bases = [(ob(1), true, true), (ob(2), false, true)];
        let p = plan(Action::Toggle, &bases);
        assert_eq!(p.set, vec![(ob(1), false)]);
        assert!(p.changed);
        let p = plan(
            Action::Toggle,
            &[(ob(1), false, true), (ob(2), false, true)],
        );
        assert_eq!(p.set, vec![(ob(1), true), (ob(2), true)]);
    }

    #[test]
    fn select_all_selected_changes_nothing() {
        let (a, _) = obs();
        let p = plan(Action::Select, &[(a, true, true)]);
        assert!(!p.changed && p.any_visible && p.set.is_empty());
        let p = plan(Action::Deselect, &[(a, false, true)]);
        assert!(!p.changed);
    }

    #[test]
    fn unselectable_base_counts_as_changed_but_stays_unselected() {
        let (a, b) = obs();
        let ob = |i| if i == 1 { a } else { b };
        let p = plan(Action::Select, &[(ob(1), false, false)]);
        assert!(p.changed);
        assert!(p.set.is_empty());
        let p = plan(
            Action::Invert,
            &[(ob(1), false, false), (ob(2), true, true)],
        );
        assert_eq!(p.set, vec![(ob(2), false)]);
    }

    #[test]
    fn no_visible_base_is_pass_through() {
        let p = plan(Action::Select, &[]);
        assert_eq!(p, Plan::default());
    }
}
