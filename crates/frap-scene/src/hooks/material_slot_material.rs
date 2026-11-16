// SPDX-License-Identifier: GPL-3.0-or-later
//! `MaterialSlot.material`: writes `mesh.materials[i]` for a `DATA` slot (another ID: every
//! object using the mesh sees it) and the object override for an `OBJECT` slot (verified on
//! 5.2.4, RFC 0001).

use frap_rna::{IdRef, MaterialId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::object_of;
use crate::id::slots::{set_slot_material, slot_index};

#[rna_hook(srna = "MaterialSlot", prop = "material", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let i = slot_index(this)?;
    let m: Option<IdRef<MaterialId>> = match v {
        Value::Id(Some(a)) => IdRef::from_any(a),
        _ => None,
    };
    let (main, log) = h.main_mut();
    set_slot_material(main, ob, i, m, log);
    Ok(())
}
