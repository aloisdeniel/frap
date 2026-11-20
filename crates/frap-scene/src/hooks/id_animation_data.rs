// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.animation_data` (WP 6.1): every datablock with an `animation_data` property reads its
//! `Option<AnimData>` through this get hook (the ID bindings leave the property unbound), as
//! the `AnimData` struct or `None` (`BKE_animdata_from_id`). One hook on `ID` serves every ID
//! type; the embedded node trees (`material.node_tree.animation_data`) are reached when their
//! binding defers to it.
//!
//! The rest of this file is shared by the `AnimData` / `NlaTrack` / `NlaStrip` set hooks
//! (`anim_data_*.rs`): writes below `animation_data` cannot walk from the ID binding, so each
//! writable property has a set hook that finds the `AnimData` in `Main` and writes through
//! `frap_anim::rna::anim_data`.

use frap_anim::AnimData;
use frap_anim::rna::anim_data::{self as adt_rna, AdtLoc};
use frap_rna::{
    EmbeddedSlot, Field, IdAny, IdCode, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, Seg,
    SetOutcome, StructRef, Value, rna_hook,
};

use crate::id::paths::internal;
use crate::{ChangeLog, Host, HostRead, Main};

/// No ID binding declares `animation_data`: the hook declares itself.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ID",
    prop: "animation_data",
    coll: false,
};

/// The `AnimData` storage of the datablock at `root` (`None` for a type frap does not animate
/// yet, such as meshes).
pub fn adt_of(main: &Main, root: Root) -> Option<&Option<AnimData>> {
    fn typed<K: frap_rna::IdKind>(a: IdAny) -> Option<frap_rna::IdRef<K>> {
        a.typed()
    }
    match root {
        Root::Id(a) => match a.code {
            IdCode::OB => main.objects.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::MA => main.materials.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::CA => main.cameras.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::LA => main.lights.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::WO => main.worlds.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::IM => main.images.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::AR => main.armatures.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::SC => main.scenes.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::KE => main.shape_keys.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::NT => main.node_groups.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::TE => main.textures.get(typed(a)?).map(|x| &x.animation_data),
            IdCode::PT => main.pointclouds.get(typed(a)?).map(|x| &x.animation_data),
            _ => None,
        },
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => main
                .materials
                .get(typed(owner)?)
                .map(|x| &x.node_tree.animation_data),
            IdCode::WO => main
                .worlds
                .get(typed(owner)?)
                .map(|x| &x.node_tree.animation_data),
            _ => None,
        },
        _ => None,
    }
}

/// [`adt_of`], mutably (through the owner's `get_mut`: generation bump, change log).
pub fn adt_of_mut<'m>(
    main: &'m mut Main,
    log: &mut ChangeLog,
    root: Root,
) -> Option<&'m mut Option<AnimData>> {
    fn typed<K: frap_rna::IdKind>(a: IdAny) -> Option<frap_rna::IdRef<K>> {
        a.typed()
    }
    match root {
        Root::Id(a) => match a.code {
            IdCode::OB => main.objects.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::MA => main
                .materials
                .get_mut(typed(a)?, log)
                .map(|x| &mut x.animation_data),
            IdCode::CA => main.cameras.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::LA => main.lights.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::WO => main.worlds.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::IM => main.images.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::AR => main
                .armatures
                .get_mut(typed(a)?, log)
                .map(|x| &mut x.animation_data),
            IdCode::SC => main.scenes.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::KE => main
                .shape_keys
                .get_mut(typed(a)?, log)
                .map(|x| &mut x.animation_data),
            IdCode::NT => main
                .node_groups
                .get_mut(typed(a)?, log)
                .map(|x| &mut x.animation_data),
            IdCode::TE => main.textures.get_mut(typed(a)?, log).map(|x| &mut x.animation_data),
            IdCode::PT => main
                .pointclouds
                .get_mut(typed(a)?, log)
                .map(|x| &mut x.animation_data),
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

/// The animated ID's type (`ActionSlot.target_id_type` identifier) and name with its
/// two-letter code (`OBCube`, `NTShader Nodetree`), as slot auto-assignment matches them.
pub fn animated_id(main: &Main, root: Root) -> Result<(&'static str, String), RnaError> {
    let (code, name) = match root {
        Root::Id(a) => (a.code, main.header(a).map(|h| h.name.clone())),
        Root::Embedded { owner, slot } => (
            IdCode::NT,
            main.embedded_header(owner, slot).map(|h| h.name.clone()),
        ),
        _ => return Err(internal("animation data outside a datablock")),
    };
    let name = name.ok_or_else(|| internal("the datablock was removed"))?;
    Ok((
        frap_anim::action::id_type_of_prefix(code.as_str()),
        format!("{}{name}", code.as_str()),
    ))
}

/// The path of `this` below `animation_data` (`[]` for the `AnimData` itself).
pub fn below_adt(this: &RnaPtr) -> Result<&[Seg], RnaError> {
    match this.path.iter().position(|s| *s == Seg::Prop("animation_data")) {
        Some(i) => Ok(&this.path[i + 1..]),
        None => Err(internal(format!(
            "{} is not below animation_data",
            this.srna.identifier
        ))),
    }
}

/// `StructRNA of type AnimData has been removed`
pub fn removed() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "StructRNA of type AnimData has been removed",
    )
}

/// Runs `f` on the `AnimData` of `this` and the location of `this` inside it.
pub fn with_adt_mut<R>(
    h: &mut dyn Host,
    this: &RnaPtr,
    f: impl FnOnce(&mut AnimData, AdtLoc) -> Result<R, RnaError>,
) -> Result<R, RnaError> {
    let rest = below_adt(this)?;
    let (main, log) = h.main_mut();
    let adt = adt_of_mut(main, log, this.root)
        .and_then(Option::as_mut)
        .ok_or_else(removed)?;
    let (loc, coll) = adt_rna::locate(adt, rest).ok_or_else(removed)?;
    if coll.is_some() {
        return Err(internal("a collection is not a struct"));
    }
    f(adt, loc)
}

/// The set hook of every plain `AnimData` / `NlaTrack` / `NlaStrip` property.
pub fn write(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    with_adt_mut(h, this, |adt, loc| match adt_rna::set(adt, loc, prop, v)? {
        SetOutcome::Done => Ok(()),
        SetOutcome::NeedsHook => Err(internal(format!(
            "{}.{} has no set hook",
            this.srna.identifier, prop.identifier
        ))),
    })
}

#[rna_hook(srna = "ID", prop = "animation_data", krate = crate)]
fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    Ok(match adt_of(h.main(), this.root) {
        Some(Some(adt)) => Field::Struct(StructRef::Borrowed(adt)),
        _ => Field::Value(Value::None),
    })
}
