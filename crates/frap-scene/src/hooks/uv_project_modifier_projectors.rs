// SPDX-License-Identifier: GPL-3.0-or-later
//! `UVProjectModifier.projectors` and `UVProjector.object` (WP 8.3.12): the projector objects
//! of a UV_PROJECT modifier, stored on the generated `UVProjectSettings.projectors` (DNA
//! `projectors[MOD_UVPROJECT_MAXPROJECTORS]`, ten slots).
//!
//! Ported from blender/source/blender/makesrna/intern/rna_modifier.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `rna_UVProject_projectors_begin` iterates the first `projector_count`
//! slots (items repr as `projectors[i]`), `rna_UVProjector_object_get/set` read and write the
//! slot (any object type; `PROP_ID_SELF_CHECK`: the modifier's own object raises
//! `TypeError: UVProjector.object ID type does not support assignment to itself`). A slot
//! whose object was removed reads `None`, as Blender clears it.

use frap_rna::traits::index_out_of_range;
use frap_rna::{
    CollRef, Field, IdAny, Item, PropDef, PyExc, Rna, RnaColl, RnaError, RnaErrorKind, RnaPtr, Seg,
    StructDef, StructRef, Value, rna_hook,
};
use smol_str::SmolStr;

use crate::id::paths::{internal, object_of};
use crate::id::{Modifier, ModifierSettings};
use crate::{Host, HostRead, Main};

/// Declares a `=> hook` binding (the registry test pairs every `RNA_HOOKS` entry with one).
macro_rules! hook_decl {
    ($name:ident, $srna:literal, $prop:literal, $coll:literal) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
        #[linkme(crate = frap_rna::linkme)]
        static $name: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
            srna: $srna,
            prop: $prop,
            coll: $coll,
        };
    };
}

hook_decl!(PROJECTORS_DECL, "UVProjectModifier", "projectors", true);
hook_decl!(PROJECTOR_OBJECT_DECL, "UVProjector", "object", false);

/// The settings of the UV_PROJECT modifier `this` points into.
fn settings<'h>(
    main: &'h Main,
    this: &RnaPtr,
) -> Result<&'h crate::generated::modifier_settings::UVProjectSettings, RnaError> {
    let ob = object_of(this)?;
    let o = main
        .objects
        .get(ob)
        .ok_or_else(|| internal("UVProjectModifier.projectors: the object was removed"))?;
    let i = Modifier::index_in(&o.modifiers, &this.path)
        .ok_or_else(|| internal("UVProjectModifier.projectors: not a modifier pointer"))?;
    match &o.modifiers[i].settings {
        ModifierSettings::UvProject(s) => Ok(s),
        _ => Err(internal(
            "UVProjectModifier.projectors: not a UV_PROJECT modifier",
        )),
    }
}

/// The slot index of a `…projectors[i]` pointer.
fn slot_of(this: &RnaPtr) -> Result<usize, RnaError> {
    let n = this.path.len();
    match this.path.get(n.wrapping_sub(2)..) {
        Some([Seg::Prop("projectors"), Seg::Index(i)]) => {
            usize::try_from(*i).map_err(|_| internal("UVProjector: negative projector index"))
        }
        _ => Err(internal("UVProjector: not a projectors[i] pointer")),
    }
}

/// An object pointer that still exists.
fn live(main: &Main, a: Option<IdAny>) -> Option<IdAny> {
    a.filter(|a| {
        a.typed::<frap_rna::ObjectId>()
            .is_some_and(|o| main.objects.get(o).is_some())
    })
}

/// The `projectors` collection (read proxy).
struct Projectors<'h> {
    main: &'h Main,
    slots: [Option<IdAny>; 10],
    count: usize,
}

impl RnaColl for Projectors<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::UV_PROJECTOR
    }
    fn len(&self) -> usize {
        self.count
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        if i >= self.count {
            return Err(index_out_of_range(i, self.count));
        }
        Ok(Item::Struct(StructRef::Owned(Box::new(ProjectorRef {
            object: live(self.main, self.slots[i]),
        }))))
    }
}

/// One `UVProjector` (read proxy; writes go through the `UVProjector.object` set hook).
struct ProjectorRef {
    object: Option<IdAny>,
}

impl Rna for ProjectorRef {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::UV_PROJECTOR
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "object" => Ok(Field::Id(self.object)),
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

#[rna_hook(srna = "UVProjectModifier", prop = "projectors", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let s = settings(main, this)?;
    Ok(Field::Coll(CollRef::Owned(Box::new(Projectors {
        main,
        slots: s.projectors,
        count: s.projector_count.clamp(1, 10) as usize,
    }))))
}

#[rna_hook(srna = "UVProjector", prop = "object", krate = crate)]
mod object_hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let main = h.main();
        let s = settings(main, this)?;
        let i = slot_of(this)?;
        Ok(Field::Id(live(
            main,
            s.projectors.get(i).copied().flatten(),
        )))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let slot = slot_of(this)?;
        let target = match v {
            Value::Id(Some(a)) => Some(a),
            _ => None,
        };
        if target == Some(ob.any()) {
            return Err(RnaError::new(
                RnaErrorKind::Type,
                PyExc::TypeError,
                "UVProjector.object ID type does not support assignment to itself",
            ));
        }
        let (main, log) = h.main_mut();
        let o = main
            .objects
            .get(ob)
            .ok_or_else(|| internal("UVProjector.object: the object was removed"))?;
        let i = Modifier::index_in(&o.modifiers, &this.path)
            .ok_or_else(|| internal("UVProjector.object: not a modifier pointer"))?;
        let Some(o) = main.objects.get_mut(ob, log) else {
            return Ok(());
        };
        if let ModifierSettings::UvProject(s) = &mut o.modifiers[i].settings
            && let Some(p) = s.projectors.get_mut(slot)
        {
            *p = target;
        }
        Ok(())
    }
}
