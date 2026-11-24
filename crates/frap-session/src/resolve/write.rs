// SPDX-License-Identifier: GPL-3.0-or-later
//! Write access (ARCHITECTURE §9.6 step 5): the transaction walks the canonical pointer again,
//! **mutably**, from its owning ID (`Main::id_mut`: copy-on-write, generation bump, logged as
//! modified), then writes through `RnaMut::set`, or through the registered set hook when the
//! binding answers `NeedsHook` (§9.5).

use frap_rna::{
    Detached, FrapError, IdNames, PropDef, PyExc, RnaCollMut, RnaError, RnaErrorKind, RnaMut,
    RnaPtr, Root, Seg, SetOutcome, StructDef, Value,
};
use frap_scene::{ChangeLog, Host, Main, find_hook};

use super::access::{not_bound, rna_err, stale};
use super::is_api_ptr;
use crate::errors::*;

/// A binding error on a property the resolver found in the `StructDef`: `NoAttribute` means
/// the binding does not serve it yet (`RNA_NOT_IMPLEMENTED`, as for reads).
fn binding_err(e: RnaError, srna: &'static StructDef, prop: &PropDef) -> FrapError {
    if e.kind == RnaErrorKind::NoAttribute {
        not_bound(srna, prop)
    } else {
        rna_err(e)
    }
}

/// Runs `f` on the struct at `ptr`, reached mutably from its owning ID.
pub fn with_struct_mut<R>(
    main: &mut Main,
    log: &mut ChangeLog,
    ptr: &RnaPtr,
    f: &mut dyn FnMut(&mut dyn RnaMut) -> Result<R, FrapError>,
) -> Result<R, FrapError> {
    refuse_detached(main, ptr)?;
    match ptr.root {
        Root::Id(id) => {
            let r = main
                .id_mut(id, log)
                .ok_or_else(|| stale(format!("{:?} no longer exists", id.code)))?;
            walk_mut(r, &ptr.path, f)
        }
        Root::Embedded { owner, slot } => {
            let o = main
                .id_mut(owner, log)
                .ok_or_else(|| stale(format!("{:?} no longer exists", owner.code)))?;
            let prop = o
                .srna()
                .find_prop(slot.prop_identifier())
                .ok_or_else(|| stale("embedded slot without a property"))?;
            let srna = o.srna();
            let mut g = o.struct_mut(prop).map_err(|e| binding_err(e, srna, prop))?;
            walk_mut(&mut *g, &ptr.path, f)
        }
        _ => Err(stale("bpy.data / bpy.context are not RNA structs")),
    }
}

fn walk_mut<R>(
    cur: &mut dyn RnaMut,
    rest: &[Seg],
    f: &mut dyn FnMut(&mut dyn RnaMut) -> Result<R, FrapError>,
) -> Result<R, FrapError> {
    let Some((seg, rest)) = rest.split_first() else {
        return f(cur);
    };
    let Seg::Prop(name) = seg else {
        return Err(stale(format!("{seg:?} without a collection")));
    };
    let prop = cur
        .srna()
        .find_prop(name)
        .ok_or_else(|| stale(format!("{}.{name} vanished", cur.srna().identifier)))?;
    let srna = cur.srna();
    match rest.first() {
        Some(Seg::Index(_) | Seg::Key(_)) => {
            let mut c = cur.coll_mut(prop).map_err(|e| binding_err(e, srna, prop))?;
            let i = match &rest[0] {
                Seg::Index(i) => usize::try_from(*i).map_err(|_| stale("negative index"))?,
                Seg::Key(k) => c
                    .find(k)
                    .ok_or_else(|| stale(format!("key {k:?} vanished")))?,
                Seg::Prop(_) => unreachable!("matched above"),
            };
            let mut item = c.item_mut(i).map_err(rna_err)?;
            walk_mut(&mut *item, &rest[1..], f)
        }
        _ => {
            let mut g = cur.struct_mut(prop).map_err(|e| {
                // A struct served by a get hook (`Object.pose`) has no mutable binding: its
                // properties are written through their own set hooks, and a bulk write
                // (`pose.bones.foreach_set`) falls back to them item by item.
                if find_hook(srna, prop.identifier).is_some_and(|h| h.get.is_some()) {
                    FrapError::new(
                        PyExc::NotImplementedError,
                        frap_rna::codes::RNA_NOT_IMPLEMENTED,
                        format!(
                            "frap: {}.{} is served by an RNA hook and cannot be written \
                             through as a struct",
                            srna.identifier, prop.identifier
                        ),
                    )
                } else {
                    binding_err(e, srna, prop)
                }
            })?;
            walk_mut(&mut *g, rest, f)
        }
    }
}

/// Runs `f` on the collection `prop` of the struct at `owner`, reached mutably.
pub fn with_coll_mut<R>(
    host: &mut dyn Host,
    owner: &RnaPtr,
    prop: &'static PropDef,
    f: &mut dyn FnMut(&mut dyn RnaCollMut) -> Result<R, FrapError>,
) -> Result<R, FrapError> {
    crate::edit::check_write(&*host, owner, prop)?;
    let (main, log) = host.main_mut();
    with_struct_mut(main, log, owner, &mut |s| {
        let srna = s.srna();
        let mut c = s.coll_mut(prop).map_err(|e| binding_err(e, srna, prop))?;
        f(&mut *c)
    })
}

/// Writes an already-coerced value: the set hook when one is registered (side effects:
/// `name` uniqueness, `rotation_mode` conversion, `parent`, …), the binding otherwise.
pub fn set_prop(
    host: &mut dyn Host,
    ptr: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), FrapError> {
    refuse_detached(host.main(), ptr)?;
    crate::edit::check_write(&*host, ptr, prop)?;
    if let Some(set) = find_hook(ptr.srna, prop.identifier).and_then(|h| h.set) {
        return set(host, ptr, prop, v).map_err(rna_err);
    }
    if ptr.root == Root::Context && !ptr.path.is_empty() {
        // `bpy.context.space_data.shading.*`, glTF add-on preferences (WP 2.3).
        return super::context::ui::set_prop(host, ptr, prop, v);
    }
    if is_api_ptr(ptr) || matches!(ptr.root, Root::Data | Root::Context) {
        return Err(needs_hook(ptr, prop));
    }
    if !modifier_object_pointer_accepts(host.main(), ptr, prop, &v)? {
        return Ok(());
    }
    let (main, log) = host.main_mut();
    let outcome = match with_struct_mut(main, log, ptr, &mut |s| {
        let srna = s.srna();
        s.set(prop, v.clone())
            .map_err(|e| binding_err(e, srna, prop))
    }) {
        Ok(o) => o,
        // A struct served by a get hook (`Object.pose`) whose proxy does not know the property
        // (`PoseBone.ik_stretch`): say it is unbound, as a read does (M8S9-1).
        Err(e) if e.code == frap_rna::codes::RNA_NOT_IMPLEMENTED => {
            let h: &dyn frap_scene::HostRead = &*host;
            let read = super::with_struct(h, ptr, &mut |s| {
                super::get_field(h, s, ptr, prop).map(|_| ())
            });
            return Err(match read {
                Err(r) if r.code == frap_rna::codes::RNA_NOT_IMPLEMENTED => r,
                _ => e,
            });
        }
        Err(e) => return Err(e),
    };
    match outcome {
        SetOutcome::Done => Ok(()),
        SetOutcome::NeedsHook => Err(needs_hook(ptr, prop)),
    }
}

/// A modifier's `Object` pointer (`frap_scene::id::object_pointer_rule`, D-97):
/// the modifier's own object raises Blender's `PROP_ID_SELF_CHECK` `TypeError`; an object
/// of another type than the setter takes (`ArrayModifier.start_cap` = a camera) is ignored,
/// the pointer keeps its value (`Ok(false)`), as `modifier_object_set` does.
fn modifier_object_pointer_accepts(
    main: &frap_scene::Main,
    ptr: &RnaPtr,
    prop: &PropDef,
    v: &Value,
) -> Result<bool, FrapError> {
    let Root::Id(owner) = ptr.root else {
        return Ok(true);
    };
    let Some(rule) = frap_scene::id::object_pointer_rule(ptr.srna.identifier, prop.identifier)
    else {
        return Ok(true);
    };
    let Value::Id(Some(target)) = v else {
        return Ok(true);
    };
    if *target == owner {
        return Err(FrapError::new(
            PyExc::TypeError,
            RNA_TYPE,
            format!(
                "{}.{} ID type does not support assignment to itself",
                ptr.srna.identifier, prop.identifier
            ),
        ));
    }
    let Some(required) = rule else {
        return Ok(true);
    };
    let ty = target
        .typed::<frap_rna::ObjectId>()
        .and_then(|o| main.objects.get(o))
        .map(|o| o.object_type());
    Ok(ty.is_none_or(|t| t == required))
}

/// Writes through a [`frap_rna::Detached`] pointer would change the original data, which
/// Blender's evaluated / temporary copy keeps untouched: frap refuses them (WP 1.22,
/// deviations.md) until it has copies of its own (M4).
pub fn refuse_detached(names: &dyn IdNames, ptr: &RnaPtr) -> Result<(), FrapError> {
    let Some(d) = ptr.detached else {
        return Ok(());
    };
    let what = match d {
        Detached::Evaluated => "an evaluated datablock (evaluated_get())",
        Detached::Temporary => {
            "part of a temporary or evaluated copy (to_mesh(), or reached from evaluated_get())"
        }
    };
    let root = RnaPtr::new(ptr.root, ptr.srna).repr(names);
    Err(FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!(
            "frap: {} is {what}; frap has no separate copies yet and reads the original data \
             through it, so writing would change the original: write to {root} instead",
            ptr.repr(names)
        ),
    )
    .with_hint("use the original datablock (bpy.data.<collection>['Name'], or .original) for edits; evaluated copies arrive with M4"))
}

fn needs_hook(ptr: &RnaPtr, prop: &PropDef) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!(
            "frap: assigning {}.{} needs an RNA set hook that is not implemented yet",
            ptr.srna.identifier, prop.identifier
        ),
    )
}
