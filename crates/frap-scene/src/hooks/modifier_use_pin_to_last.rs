// SPDX-License-Identifier: GPL-3.0-or-later
//! `Modifier.use_pin_to_last` (`rna_Modifier_use_pin_to_last_set`, Blender 5.2.4): sets the
//! flag and moves the modifier, so that pinned modifiers stay at the end of the stack
//! (`Modifier::set_pin_to_last`). Verified: pinning the first of five unpinned modifiers moves
//! it last; pinning another one puts it just before; unpinning an unpinned modifier changes
//! nothing.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::Modifier;
use crate::id::paths::{internal, object_of};

#[rna_hook(srna = "Modifier", prop = "use_pin_to_last", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let value = v.as_bool().unwrap_or(false);
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    let o = main
        .objects
        .get(ob)
        .ok_or_else(|| internal("Modifier.use_pin_to_last: the object was removed"))?;
    let i = Modifier::index_in(&o.modifiers, &this.path)
        .ok_or_else(|| internal("Modifier.use_pin_to_last: not a modifier pointer"))?;
    if let Some(o) = main.objects.get_mut(ob, log) {
        Modifier::set_pin_to_last(&mut o.modifiers, i, value);
    }
    Ok(())
}
