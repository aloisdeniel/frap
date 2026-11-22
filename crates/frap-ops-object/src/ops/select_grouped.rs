// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_grouped(extend=False, type='CHILDREN_RECURSIVE')`, tier T0,
//! *headless-extended* (deviations D-150; `tests/golden/cases/object/select_m8/grouped_*.py`).
//! Ported from blender/source/blender/editors/object/object_select.cc
//! (`object_select_grouped_exec` and the `select_grouped_*` helpers, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Blender's poll refuses background mode (`G.background`: the `COLLECTION` type may open a
//! menu), so `blender -b` never runs it; frap runs the exec with the poll of the other object
//! selection operators (`objects_selectable_poll`). Every type behaves as the C code:
//!
//! - `extend=False` first deselects every visible base, which counts as a change;
//! - no active object → `RuntimeError: Error: No active object`;
//! - `CHILDREN_RECURSIVE` / `CHILDREN`: the selectable (visible) bases whose parent is the
//!   active object, recursively for the former; `PARENT`: the parent's base, when visible and
//!   selectable, is selected and becomes active; `SIBLINGS`: the same parent (none included);
//!   `TYPE`: the same object type; `PASS`: the same `pass_index`; `COLOR`: every channel of
//!   the RGB `color` within 0.005; `LIGHT_TYPE`: lights of the same light type (an active
//!   object that is no light is `RuntimeError: Error: Active object must be a light`); `HOOK`:
//!   the objects of the active object's Hook modifiers;
//! - `COLLECTION`: the objects of the one collection of `bpy.data.collections` holding the
//!   active object directly (every collection is walked, in name order; the search stops at
//!   the 24th holding one, `COLLECTION_MENU_MAX`); with several, Blender opens a menu of
//!   `select_same_collection` entries: frap only deselects (as Blender) and adds a WARNING that
//!   names them (at most 24); the master collection does not count;
//! - `KEYINGSET`: frap scenes have no keying sets (`Scene.keying_sets` is not bound; the
//!   "Keying, frames, NLA" row of deviations §2, WP 6.4), so there is never an active one:
//!   `RuntimeError: Error: No active Keying Set to use`;
//! - `FINISHED` when anything changed (the deselect included), else `CANCELLED`. A statement
//!   that ends in an error or `CANCELLED` is rolled back (Blender keeps the deselect, D-150).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, ObjectId};
use frap_scene::{Context, Main, ObjectMode};
use frap_spec_ops_object::object::{self, select_grouped::Type};

pub(crate) mod bases;

use self::bases::Bases;

/// `objects_selectable_poll`: no object in Edit Mode and the active object (if any) in Object
/// Mode (a linked scene is accepted: "selection is still allowed for inspection").
pub(crate) fn objects_selectable(c: &Context) -> Result<(), PollFail> {
    match poll::active(c) {
        Some((_, ob)) if ob.mode != ObjectMode::Object => Err(poll::fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        )),
        _ => Ok(()),
    }
}

/// `objects_selectable_poll`
pub(crate) const SELECTABLE: Poll = Poll {
    requires: &[PollReq::ModeObject],
    check: objects_selectable,
};

/// `COLLECTION_MENU_MAX`
const COLLECTION_MENU_MAX: usize = 24;

/// `select_grouped_children`
fn children(main: &Main, b: &mut Bases, ob: IdRef<ObjectId>, recursive: bool) -> bool {
    let mut changed = false;
    for i in b.selectable() {
        let child = b.list[i].ob;
        if main.objects.get(child).and_then(|o| o.parent) != Some(ob) {
            continue;
        }
        if !b.list[i].selected {
            b.select(i, true);
            changed = true;
        }
        if recursive {
            changed |= children(main, b, child, true);
        }
    }
    changed
}

/// `select_grouped_parent`: the parent becomes selected and active.
fn parent(main: &Main, b: &mut Bases, ob: IdRef<ObjectId>) -> bool {
    if b.find(ob).is_none() {
        return false;
    }
    let Some(par) = main.objects.get(ob).and_then(|o| o.parent) else {
        return false;
    };
    match b.find(par) {
        Some(i) if b.is_selectable(i) => {
            b.select(i, true);
            b.active = Some(par);
            true
        }
        _ => false,
    }
}

/// The selectable, unselected bases whose object passes `f` get selected
/// (`select_grouped_siblings`, `_type`, `_index_object`, `_color`, `_lighttype`).
fn select_where(b: &mut Bases, f: impl Fn(IdRef<ObjectId>) -> bool) -> bool {
    let mut changed = false;
    for i in b.selectable() {
        if !b.list[i].selected && f(b.list[i].ob) {
            b.select(i, true);
            changed = true;
        }
    }
    changed
}

/// `select_grouped_collection`: `Ok(changed)`, or the names of the collections Blender would
/// offer in its menu. All of `bpy.data.collections` is walked; `COLLECTION_MENU_MAX` caps the
/// number of *holding* collections collected (`collection_count < COLLECTION_MENU_MAX`).
fn collection(main: &Main, b: &mut Bases, ob: IdRef<ObjectId>) -> Result<bool, Vec<String>> {
    let holding: Vec<_> = main
        .collections
        .iter()
        .filter(|(_, c)| c.objects.contains(&ob))
        .take(COLLECTION_MENU_MAX)
        .collect();
    match holding.as_slice() {
        [] => Ok(false),
        [(_, col)] => {
            let mut changed = false;
            for i in b.visible() {
                let base = b.list[i];
                if !base.selected && base.selectable && col.objects.contains(&base.ob) {
                    b.select(i, true);
                    changed = true;
                }
            }
            Ok(changed)
        }
        many => Err(many
            .iter()
            .map(|(_, c)| c.header.name.to_string())
            .collect()),
    }
}

/// `select_grouped_object_hooks`: the objects of the Hook modifiers.
fn hooks(main: &Main, b: &mut Bases, ob: IdRef<ObjectId>) -> bool {
    let Some(o) = main.objects.get(ob) else {
        return false;
    };
    let mut changed = false;
    for md in o
        .modifiers
        .iter()
        .filter(|m| m.settings.type_identifier() == "HOOK")
    {
        let target = md
            .settings
            .id_refs()
            .into_iter()
            .find(|(k, _)| *k == "object")
            .and_then(|(_, r)| r)
            .and_then(IdRef::<ObjectId>::from_any);
        if let Some(t) = target
            && let Some(i) = b.find(t)
            && !b.list[i].selected
            && b.is_selectable(i)
        {
            b.select(i, true);
            changed = true;
        }
    }
    changed
}

/// The light type of a light object.
fn light_type(main: &Main, ob: IdRef<ObjectId>) -> Option<&str> {
    let o = main.objects.get(ob)?;
    let l = main.lights.get(o.data?.typed()?)?;
    Some(l.r#type.as_str())
}

/// `compare_v3v3(a, b, 0.005)` on the RGB part of `Object.color`.
fn same_color(a: &[f32; 4], b: &[f32; 4]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= 0.005)
}

#[operator(
    poll = SELECTABLE,
    tier = T0,
    notes = [
        "headless-extended (D-150): Blender's poll fails in background mode; frap runs the exec with objects_selectable_poll",
        "COLLECTION with several collections holding the active object only deselects and warns (Blender opens a menu)",
        "KEYINGSET always fails: keying sets are not bound (No active Keying Set to use)",
    ]
)]
fn exec(cx: &mut OpCtx, p: object::SelectGrouped) -> OpResult {
    let mut b = Bases::load(cx);
    let mut changed = false;
    if !p.extend.value {
        changed = b.deselect_all();
    }
    let Some(ob) = b.active else {
        return Err(OpError::Runtime("No active object".into()));
    };
    let mut warning = None;
    {
        let main = cx.main();
        let Some(act) = main.objects.get(ob) else {
            return Err(OpError::Runtime("No active object".into()));
        };
        changed |= match p.r#type.value {
            Type::ChildrenRecursive => children(main, &mut b, ob, true),
            Type::Children => children(main, &mut b, ob, false),
            Type::Parent => parent(main, &mut b, ob),
            Type::Siblings => {
                let par = act.parent;
                select_where(&mut b, |o| {
                    main.objects.get(o).is_some_and(|x| x.parent == par)
                })
            }
            Type::Type => {
                let ty = act.object_type();
                select_where(&mut b, |o| {
                    main.objects.get(o).is_some_and(|x| x.object_type() == ty)
                })
            }
            Type::Collection => match collection(main, &mut b, ob) {
                Ok(c) => c,
                Err(names) => {
                    warning = Some(names);
                    false
                }
            },
            Type::Hook => hooks(main, &mut b, ob),
            Type::Pass => {
                let pass = act.pass_index;
                select_where(&mut b, |o| {
                    main.objects.get(o).is_some_and(|x| x.pass_index == pass)
                })
            }
            Type::Color => {
                let color = act.color;
                select_where(&mut b, |o| {
                    main.objects
                        .get(o)
                        .is_some_and(|x| same_color(&x.color, &color))
                })
            }
            Type::Keyingset => {
                return Err(OpError::Runtime("No active Keying Set to use".into()));
            }
            Type::LightType => {
                let Some(lt) = (act.object_type() == "LIGHT")
                    .then(|| light_type(main, ob))
                    .flatten()
                else {
                    return Err(OpError::Runtime("Active object must be a light".into()));
                };
                select_where(&mut b, |o| light_type(main, o) == Some(lt))
            }
        };
    }
    if let Some(names) = &warning {
        let list = names
            .iter()
            .map(|n| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let first = poll::py_name(&names[0]);
        cx.report(
            ReportType::Warning,
            format!(
                "frap: the active object is in several collections ({list}): Blender opens a menu here; select one with bpy.ops.object.select_same_collection(collection={first})"
            ),
        );
    }
    if !changed {
        if warning.is_none() {
            cx.report(
                ReportType::Warning,
                format!(
                    "frap: nothing to select: select_grouped(type='{}') changes nothing",
                    p.r#type.value.identifier()
                ),
            );
        }
        return Ok(OpStatus::Cancelled);
    }
    b.commit(cx);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::ChangeLog;
    use frap_scene::id::Collection;

    #[test]
    fn color_tolerance_is_per_channel() {
        assert!(same_color(
            &[1.0, 0.5, 0.0, 1.0],
            &[1.004, 0.496, 0.005, 0.0]
        ));
        assert!(!same_color(&[1.0, 0.5, 0.0, 1.0], &[1.0, 0.5, 0.0051, 1.0]));
    }

    /// `COLLECTION_MENU_MAX` caps the holding collections, not the collections walked: with 30
    /// collections `C00`..`C29`, the one holding the active object (`C27`) is still found
    /// (checked in GUI Blender 5.2.4: Cube and Light in `C27` are selected, `FINISHED`).
    #[test]
    fn collection_walks_every_collection() {
        let mut main = Main::default();
        let mut log = ChangeLog::default();
        let (mut b, obs) = bases::tests::bases(&[(false, true, true); 3]);
        for i in 0..30 {
            let name = format!("C{i:02}");
            let mut col = Collection::new(&name);
            if i == 27 {
                col.objects = vec![obs[0], obs[1]];
            }
            main.collections.insert(&name, col, &mut log);
        }
        assert_eq!(collection(&main, &mut b, obs[0]), Ok(true));
        let sel: Vec<bool> = b.list.iter().map(|x| x.selected).collect();
        assert_eq!(sel, [true, true, false]);
    }

    /// With more than `COLLECTION_MENU_MAX` holding collections, the menu lists the first 24.
    #[test]
    fn collection_menu_lists_at_most_24_holders() {
        let mut main = Main::default();
        let mut log = ChangeLog::default();
        let (mut b, obs) = bases::tests::bases(&[(false, true, true)]);
        for i in 0..30 {
            let name = format!("C{i:02}");
            let mut col = Collection::new(&name);
            if i >= 3 {
                col.objects = vec![obs[0]];
            }
            main.collections.insert(&name, col, &mut log);
        }
        let menu = collection(&main, &mut b, obs[0]).unwrap_err();
        assert_eq!(menu.len(), COLLECTION_MENU_MAX);
        assert_eq!(menu.first().map(String::as_str), Some("C03"));
        assert_eq!(menu.last().map(String::as_str), Some("C26"));
    }
}
