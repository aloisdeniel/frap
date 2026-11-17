// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.hide_select`: making an object unselectable deselects it in every view layer at
//! once (verified on 5.2.4: `select_get()` is False right after the assignment).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::layers::sync_all;
use crate::id::paths::object_of;

#[rna_hook(srna = "Object", prop = "hide_select", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.hide_select = v.as_bool().unwrap_or(o.hide_select);
    }
    sync_all(main, log);
    Ok(())
}
