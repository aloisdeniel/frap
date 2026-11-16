// SPDX-License-Identifier: GPL-3.0-or-later
//! `MaterialSlot.link`: `'OBJECT'` makes the slot use the object's own material (padding the
//! overrides lazily, §5.6), `'DATA'` the data's (verified: switching to `OBJECT` shows the
//! override's material, `None` until one is assigned).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::SlotLink;
use crate::id::paths::object_of;
use crate::id::slots::{set_slot_link, slot_index};

#[rna_hook(srna = "MaterialSlot", prop = "link", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let i = slot_index(this)?;
    let link = match v.as_str() {
        Some("OBJECT") => SlotLink::Object,
        _ => SlotLink::Data,
    };
    let (main, log) = h.main_mut();
    set_slot_link(main, ob, i, link, log);
    Ok(())
}
