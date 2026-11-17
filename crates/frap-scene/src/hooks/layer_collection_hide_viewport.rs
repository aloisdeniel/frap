// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.hide_viewport`: hidden in this view layer's viewport: its objects are not visible and leave `selected_objects`, but keep their selection flag (verified on 5.2.4). Set through a hook because nested layer collections are
//! reached through the `children` proxy.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::layers::set_layer_collection_flag;

#[rna_hook(srna = "LayerCollection", prop = "hide_viewport", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    set_layer_collection_flag(h, this, &v, |lc| &mut lc.hide_viewport)
}
