// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.active_material_index`: clamped to the slots (`rna_Object_active_material_index_range`:
//! `[0, max(len(material_slots) - 1, 0)]`; verified: 5 on a one-slot cube reads back 0).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::object_of;
use crate::id::slots::slot_count;

#[rna_hook(srna = "Object", prop = "active_material_index", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let Value::Int(i) = v else {
        return Ok(());
    };
    let max = h
        .main()
        .objects
        .get(ob)
        .map_or(0, |o| slot_count(h.main(), o).saturating_sub(1)) as i64;
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.active_material_index = i.clamp(0, max) as i32;
    }
    Ok(())
}
