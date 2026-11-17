// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.indirect_only`: Cycles indirect-only (stored). Set through a hook because nested layer collections are
//! reached through the `children` proxy.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::layers::set_layer_collection_flag;

#[rna_hook(srna = "LayerCollection", prop = "indirect_only", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    set_layer_collection_flag(h, this, &v, |lc| &mut lc.indirect_only)
}
