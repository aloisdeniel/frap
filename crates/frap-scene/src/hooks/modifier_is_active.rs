// SPDX-License-Identifier: GPL-3.0-or-later
//! `Modifier.is_active` (`rna_Modifier_is_active_set`, Blender 5.2.4): `True` makes the
//! modifier the object's only active one; `False` changes nothing (verified: the flag stays
//! set).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::Modifier;
use crate::id::paths::{internal, object_of};

#[rna_hook(srna = "Modifier", prop = "is_active", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    if v.as_bool() != Some(true) {
        return Ok(());
    }
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    let o = main
        .objects
        .get(ob)
        .ok_or_else(|| internal("Modifier.is_active: the object was removed"))?;
    let i = Modifier::index_in(&o.modifiers, &this.path)
        .ok_or_else(|| internal("Modifier.is_active: not a modifier pointer"))?;
    if o.modifiers[i].is_active && o.modifiers.iter().filter(|m| m.is_active).count() == 1 {
        return Ok(());
    }
    if let Some(o) = main.objects.get_mut(ob, log) {
        for (j, m) in o.modifiers.iter_mut().enumerate() {
            m.is_active = j == i;
        }
    }
    Ok(())
}
