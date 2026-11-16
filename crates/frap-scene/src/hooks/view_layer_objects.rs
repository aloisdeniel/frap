// SPDX-License-Identifier: GPL-3.0-or-later
//! `ViewLayer.objects`: the objects of the view layer's bases, in base order (layer-collection
//! sync order, §8; verified `['Cube', 'Light', 'Camera']` on the factory scene). The
//! collection's API struct is `LayerObjects` (`active`, `selected`).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::id::idlist::IdList;
use crate::id::paths::view_layer;

#[rna_hook(srna = "ViewLayer", prop = "objects", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (_, _, vl) = view_layer(h.main(), this)?;
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(
        vl.bases.iter().map(|b| b.object).collect(),
    )))))
}
