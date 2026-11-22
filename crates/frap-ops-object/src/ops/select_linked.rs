// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_linked(extend=False, type='OBDATA')`, tier T0
//! (`tests/golden/cases/object/select_m8/linked*.py`). Ported from
//! blender/source/blender/editors/object/object_select.cc (`object_select_linked_exec`,
//! `object_select_all_by_*`, Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//!
//! - poll `objects_selectable_poll`; `extend=False` first deselects every visible base, which
//!   does **not** count as a change; no active object → `RuntimeError: Error: No active
//!   object`;
//! - the visible, unselected, selectable bases that share with the active object its data
//!   (`OBDATA`), the material of its active slot (`MATERIAL`, any slot of the other object),
//!   its instanced collection (`DUPGROUP`: `instance_collection` while `instance_type` is
//!   `COLLECTION`, else none, compared the same way on the others), its library (`LIBRARY`:
//!   local objects match a local active object) or its data's library (`LIBRARY_OBDATA`) are
//!   selected;
//! - nothing to compare (no data, no material in the active slot, no instanced collection; and
//!   `PARTICLE` always: frap has no particle systems) or nothing newly selected → `CANCELLED`,
//!   which frap rolls back (Blender keeps the deselect, D-155).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CollectionId, IdAny, IdRef, LibraryId};
use frap_scene::Main;
use frap_scene::id::{Object, slots};
use frap_spec_ops_object::object::{self, select_linked::Type};

use super::select_grouped::SELECTABLE;
use super::select_grouped::bases::Bases;

/// The visible, unselected bases with `BASE_SELECTABLE` whose object passes `f` get selected
/// (the loop of every `object_select_all_by_*`).
fn select_all_by(main: &Main, b: &mut Bases, f: impl Fn(&Object) -> bool) -> bool {
    let mut changed = false;
    for i in b.visible() {
        let base = b.list[i];
        if base.selected || !base.selectable {
            continue;
        }
        if main.objects.get(base.ob).is_some_and(&f) {
            b.select(i, true);
            changed = true;
        }
    }
    changed
}

/// `(ob->transflag & OB_DUPLICOLLECTION) ? ob->instance_collection : nullptr`.
fn instanced(o: &Object) -> Option<IdRef<CollectionId>> {
    (o.instance_type == "COLLECTION")
        .then_some(o.instance_collection)
        .flatten()
}

/// The library of an ID (`id->lib`).
fn library_of(main: &Main, a: IdAny) -> Option<IdRef<LibraryId>> {
    main.header(a).and_then(|h| h.library)
}

#[operator(poll = SELECTABLE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectLinked) -> OpResult {
    let mut b = Bases::load(cx);
    if !p.extend.value {
        b.deselect_all();
    }
    let Some(ob) = b.active else {
        return Err(OpError::Runtime("No active object".into()));
    };
    let changed = {
        let main = cx.main();
        let Some(act) = main.objects.get(ob) else {
            return Err(OpError::Runtime("No active object".into()));
        };
        match p.r#type.value {
            Type::Obdata => act
                .data
                .map(|d| select_all_by(main, &mut b, |o| o.data == Some(d))),
            Type::Material => {
                let i = usize::try_from(act.active_material_index).unwrap_or(0);
                slots::slot(main, act, i).1.map(|mat| {
                    select_all_by(main, &mut b, |o| {
                        slots::slot_materials(main, o).contains(&Some(mat))
                    })
                })
            }
            Type::Dupgroup => act.instance_collection.map(|_| {
                let ic = instanced(act);
                select_all_by(main, &mut b, |o| instanced(o) == ic)
            }),
            // `psys_get_current`: frap objects have no particle systems.
            Type::Particle => None,
            Type::Library => {
                let lib = act.header.library;
                Some(select_all_by(main, &mut b, |o| o.header.library == lib))
            }
            Type::LibraryObdata => act.data.map(|d| {
                let lib = library_of(main, d);
                select_all_by(main, &mut b, |o| {
                    o.data.is_some_and(|od| library_of(main, od) == lib)
                })
            }),
        }
    };
    if changed != Some(true) {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: nothing to select: select_linked(type='{}') finds no other visible object linked to the active one",
                p.r#type.value.identifier()
            ),
        );
        return Ok(OpStatus::Cancelled);
    }
    b.commit(cx);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instanced_needs_the_collection_type() {
        let mut o = Object::new("E", None);
        let c: IdRef<CollectionId> = IdRef::from_key(frap_rna::IdKey::next());
        o.instance_collection = Some(c);
        assert_eq!(instanced(&o), None);
        o.instance_type = "COLLECTION".into();
        assert_eq!(instanced(&o), Some(c));
    }
}
