// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the Action data-API functions of this directory (WP 6.1). Not a concept:
//! the leading underscore keeps it out of the §10.5 file-to-registration check.
//!
//! `this` of a function on an action's struct is rooted at the action
//! (`bpy.data.actions['A'].layers["Layer"].strips[0]…`); a function of a collection API
//! (`….fcurves.new`) ends with the collection's name. [`target`] locates both through
//! `frap_anim::rna::action::locate`; [`loc_ptr`] builds the canonical pointer a function
//! returns.

use frap_anim::{Action, AnimData, FCurve};
use frap_anim::rna::action::{Loc, bag, locate};
use frap_rna::{
    ActionId, EmbeddedSlot, IdCode, IdRef, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, Seg, Value,
};
use frap_scene::{ChangeLog, FuncArgs, Host, Main};
use smol_str::SmolStr;

/// `RuntimeError` with Blender's report wording (`"Error: …\n"`, `BKE_report(RPT_ERROR)`).
pub(crate) fn report_error(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("Error: {msg}\n"),
    )
}

/// An inconsistency between the call target and the data.
pub(crate) fn internal(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("frap internal: {msg}"),
    )
}

/// `ReferenceError`-like failure: the struct the pointer named is gone.
pub(crate) fn removed(what: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("StructRNA of type {what} has been removed"),
    )
}

/// A string or enum argument (`""` when absent).
pub(crate) fn arg_str<'a>(args: &'a FuncArgs, name: &str) -> &'a str {
    args.get(name).and_then(Value::as_str).unwrap_or_default()
}

/// A boolean argument.
pub(crate) fn arg_bool(args: &FuncArgs, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

/// An integer argument.
pub(crate) fn arg_int(args: &FuncArgs, name: &str, default: i64) -> i64 {
    match args.get(name) {
        Some(Value::Int(i)) => *i,
        _ => default,
    }
}

/// A float argument.
pub(crate) fn arg_f32(args: &FuncArgs, name: &str, default: f32) -> f32 {
    args.get(name).and_then(Value::as_f32).unwrap_or(default)
}

/// An enum-flag argument (`options={'REPLACE', 'FAST'}`).
pub(crate) fn arg_flags(args: &FuncArgs, name: &str) -> Vec<SmolStr> {
    match args.get(name) {
        Some(Value::EnumSet(s)) => s.iter().cloned().collect(),
        Some(Value::Enum(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// A struct-pointer argument.
pub(crate) fn arg_ptr<'a>(args: &'a FuncArgs, name: &str) -> Option<&'a RnaPtr> {
    match args.get(name) {
        Some(Value::Ptr(p)) => Some(p),
        _ => None,
    }
}

/// The action `this` is rooted at.
pub(crate) fn action_of(this: &RnaPtr) -> Result<IdRef<ActionId>, RnaError> {
    match this.root {
        Root::Id(a) => a.typed().ok_or_else(|| internal("not rooted at an action")),
        _ => Err(internal("not rooted at an action")),
    }
}

/// The struct inside the action `this` names, and the collection the call is on (if any).
pub(crate) fn target(main: &Main, this: &RnaPtr) -> Result<(IdRef<ActionId>, Loc), RnaError> {
    let a = action_of(this)?;
    let act = main.actions.get(a).ok_or_else(|| removed("Action"))?;
    let (loc, _) = locate(act, &this.path).ok_or_else(|| removed(this.srna.identifier))?;
    Ok((a, loc))
}

/// The action of `this`, mutably, with the location of `this`.
pub(crate) fn with_action<R>(
    h: &mut dyn Host,
    this: &RnaPtr,
    f: impl FnOnce(&mut Action, Loc) -> Result<R, RnaError>,
) -> Result<R, RnaError> {
    let (a, loc) = target(h.main(), this)?;
    let (main, log) = h.main_mut();
    let act = main
        .actions
        .get_mut(a, log)
        .ok_or_else(|| removed("Action"))?;
    f(act, loc)
}

/// The F-curve of `this` (an `FCurve`, or its `keyframe_points`), mutably; `f` also gets the
/// F-curve's location.
pub(crate) fn with_fcurve<R>(
    h: &mut dyn Host,
    this: &RnaPtr,
    f: impl FnOnce(&mut FCurve, Loc) -> Result<R, RnaError>,
) -> Result<R, RnaError> {
    with_action(h, this, |act, loc| {
        let Loc::FCurve(l, s, b, i) = loc else {
            return Err(internal(format!(
                "{} is not an F-curve of an action",
                this.srna.identifier
            )));
        };
        let fc = frap_anim::rna::action::bag_mut(act, l, s, b)
            .and_then(|x| x.fcurves.get_mut(i))
            .ok_or_else(|| removed("FCurve"))?;
        f(fc, loc)
    })
}

/// The `AnimData` storage of the datablock at `root`, mutably (`None` for a type frap does not
/// animate yet, such as meshes). Mirrors frap-scene's `ID.animation_data` hook.
pub(crate) fn adt_slot_mut<'m>(
    main: &'m mut Main,
    log: &mut ChangeLog,
    root: Root,
) -> Option<&'m mut Option<AnimData>> {
    fn typed<K: frap_rna::IdKind>(a: frap_rna::IdAny) -> Option<IdRef<K>> {
        a.typed()
    }
    match root {
        Root::Id(a) => match a.code {
            IdCode::OB => main.objects.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::MA => main.materials.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::CA => main.cameras.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::LA => main.lights.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::WO => main.worlds.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::IM => main.images.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::AR => main.armatures.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::SC => main.scenes.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::KE => main.shape_keys.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::NT => main.node_groups.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::TE => main.textures.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::PT => main.pointclouds.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            _ => None,
        },
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => main
                .materials
                .get_mut(typed(owner)?, log)
                .map(|x| &mut x.node_tree.animation_data),
            IdCode::WO => main
                .worlds
                .get_mut(typed(owner)?, log)
                .map(|x| &mut x.node_tree.animation_data),
            _ => None,
        },
        _ => None,
    }
}

/// The IDs whose `AnimData` uses slot `handle` of action `a`, directly or through an NLA strip
/// (`foreach_action_slot_use`), in `bpy.data` order.
pub(crate) fn slot_users(main: &Main, a: IdRef<ActionId>, handle: i32) -> Vec<frap_rna::IdAny> {
    fn uses(adt: &AnimData, a: IdRef<ActionId>, handle: i32) -> bool {
        (adt.action == Some(a) && adt.action_slot_handle == handle)
            || adt
                .nla_tracks
                .iter()
                .flat_map(|t| &t.strips)
                .any(|s| s.action == Some(a) && s.action_slot_handle == handle)
    }
    fn of(main: &Main, id: frap_rna::IdAny) -> Option<&AnimData> {
        fn t<K: frap_rna::IdKind>(a: frap_rna::IdAny) -> Option<IdRef<K>> {
            a.typed()
        }
        let adt = match id.code {
            IdCode::OB => &main.objects.get(t(id)?)?.animation_data,
            IdCode::MA => &main.materials.get(t(id)?)?.animation_data,
            IdCode::CA => &main.cameras.get(t(id)?)?.animation_data,
            IdCode::LA => &main.lights.get(t(id)?)?.animation_data,
            IdCode::WO => &main.worlds.get(t(id)?)?.animation_data,
            IdCode::IM => &main.images.get(t(id)?)?.animation_data,
            IdCode::AR => &main.armatures.get(t(id)?)?.animation_data,
            IdCode::SC => &main.scenes.get(t(id)?)?.animation_data,
            IdCode::KE => &main.shape_keys.get(t(id)?)?.animation_data,
            IdCode::NT => &main.node_groups.get(t(id)?)?.animation_data,
            IdCode::TE => &main.textures.get(t(id)?)?.animation_data,
            IdCode::PT => &main.pointclouds.get(t(id)?)?.animation_data,
            _ => return None,
        };
        adt.as_ref()
    }
    main.all_ids()
        .into_iter()
        .filter(|id| of(main, *id).is_some_and(|adt| uses(adt, a, handle)))
        .collect()
}

/// The `(action, slot handle)` of an `ActionSlot` pointer argument (from any action).
pub(crate) fn slot_arg(main: &Main, p: &RnaPtr) -> Option<(IdRef<ActionId>, i32)> {
    let a = action_of(p).ok()?;
    let act = main.actions.get(a)?;
    let (Loc::Slot(i), None) = locate(act, &p.path)? else {
        return None;
    };
    Some((a, act.slots.get(i)?.handle))
}

/// The location of an argument pointing into action `a` (a group, an F-curve, a keyframe).
pub(crate) fn loc_arg(main: &Main, a: IdRef<ActionId>, p: &RnaPtr) -> Option<Loc> {
    if action_of(p).ok()? != a {
        return None;
    }
    let (loc, None) = locate(main.actions.get(a)?, &p.path)? else {
        return None;
    };
    Some(loc)
}

/// The canonical pointer of the struct at `loc` (`layers["Layer"].strips[0].channelbags[0]
/// .fcurves[3]`, `slots["OBCube"]`, `….groups["G"]`).
pub(crate) fn loc_ptr(a: IdRef<ActionId>, act: &Action, loc: Loc) -> Value {
    use frap_spec_rna as s;
    let root = RnaPtr::id(a.any(), &s::ACTION);
    let layer = |l: usize| {
        root.child(Seg::Prop("layers"), &s::ACTION_LAYER).child(
            Seg::Key(act.layers.get(l).map(|x| x.name.clone()).unwrap_or_default()),
            &s::ACTION_LAYER,
        )
    };
    let strip = |l: usize, st: usize| {
        layer(l)
            .child(Seg::Prop("strips"), &s::ACTION_KEYFRAME_STRIP)
            .child(Seg::Index(st as i64), &s::ACTION_KEYFRAME_STRIP)
    };
    let bag_ptr = |l: usize, st: usize, b: usize| {
        strip(l, st)
            .child(Seg::Prop("channelbags"), &s::ACTION_CHANNELBAG)
            .child(Seg::Index(b as i64), &s::ACTION_CHANNELBAG)
    };
    let fcurve = |l: usize, st: usize, b: usize, f: usize| {
        bag_ptr(l, st, b)
            .child(Seg::Prop("fcurves"), &s::F_CURVE)
            .child(Seg::Index(f as i64), &s::F_CURVE)
    };
    let ptr = match loc {
        Loc::Action => root,
        Loc::Slot(i) => root.child(Seg::Prop("slots"), &s::ACTION_SLOT).child(
            Seg::Key(act.slots.get(i).map(|x| x.identifier.clone()).unwrap_or_default()),
            &s::ACTION_SLOT,
        ),
        Loc::Layer(l) => layer(l),
        Loc::Strip(l, st) => strip(l, st),
        Loc::Bag(l, st, b) => bag_ptr(l, st, b),
        Loc::Group(l, st, b, g) => bag_ptr(l, st, b)
            .child(Seg::Prop("groups"), &s::ACTION_GROUP)
            .child(
                Seg::Key(
                    bag(act, l, st, b)
                        .and_then(|x| x.groups.get(g))
                        .map(|x| x.name.clone())
                        .unwrap_or_default(),
                ),
                &s::ACTION_GROUP,
            ),
        Loc::FCurve(l, st, b, f) => fcurve(l, st, b, f),
        Loc::Key(l, st, b, f, k) => fcurve(l, st, b, f)
            .child(Seg::Prop("keyframe_points"), &s::KEYFRAME)
            .child(Seg::Index(k as i64), &s::KEYFRAME),
    };
    Value::Ptr(Box::new(ptr))
}
