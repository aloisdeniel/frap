// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.exclude`: excluded from the view layer: its objects (and its children's) have no base; re-including creates fresh, unselected bases and the active object is dropped (verified on 5.2.4 for the active object; frap does not keep the selection of excluded bases). Set through a hook because nested layer collections are
//! reached through the `children` proxy.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::layers::set_layer_collection_flag;

#[rna_hook(srna = "LayerCollection", prop = "exclude", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    set_layer_collection_flag(h, this, &v, |lc| &mut lc.exclude)
}
