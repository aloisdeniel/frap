// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.hide_viewport` (the global "monitor" toggle): hiding also deselects the object in
//! every view layer (`BKE_base_eval_flags`: an object hidden in viewports is not selectable;
//! verified on 5.2.4).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::layers::sync_all;
use crate::id::paths::object_of;

#[rna_hook(srna = "Object", prop = "hide_viewport", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.hide_viewport = v.as_bool().unwrap_or(o.hide_viewport);
    }
    sync_all(main, log);
    Ok(())
}
