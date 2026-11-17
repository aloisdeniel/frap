// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.active_material`: the material of the active slot. Assigning writes that slot
//! like `material_slots[active_material_index].material = m` (the data's material list for a
//! `DATA` slot, the object override for an `OBJECT` slot); with no slot it does nothing
//! (verified on 5.2.4 for a camera and an empty).

use frap_rna::{Field, IdRef, MaterialId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::id::paths::{internal, object_of};
use crate::id::slots::{set_slot_material, slot, slot_count};
use crate::{Host, HostRead};

#[rna_hook(srna = "Object", prop = "active_material", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let main = h.main();
        let ob = main
            .objects
            .get(object_of(this)?)
            .ok_or_else(|| internal("Object.active_material: the object was removed"))?;
        let i = ob.active_material_index.max(0) as usize;
        let m = if i < slot_count(main, ob) {
            slot(main, ob, i).1
        } else {
            None
        };
        Ok(Field::Id(m.map(IdRef::any)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let m: Option<IdRef<MaterialId>> = match v {
            Value::Id(Some(a)) => IdRef::from_any(a),
            _ => None,
        };
        let i = h
            .main()
            .objects
            .get(ob)
            .map_or(0, |o| o.active_material_index.max(0) as usize);
        let (main, log) = h.main_mut();
        set_slot_material(main, ob, i, m, log);
        Ok(())
    }
}
