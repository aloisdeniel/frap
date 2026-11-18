// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.children`: one layer collection per child collection, keyed by collection
//! name (`view_layer.layer_collection.children['Collection']`).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::context::rna::LayerCollectionChildren;
use crate::id::paths::{internal, layer_collection_of};

#[rna_hook(srna = "LayerCollection", prop = "children", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let (scene, i, path) = layer_collection_of(main, this)?;
    let lc = main
        .scenes
        .get(scene)
        .and_then(|s| s.view_layers[i].layer_collection.at(&path))
        .ok_or_else(|| internal("layer collection not found"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(
        LayerCollectionChildren {
            main,
            scene,
            items: &lc.children,
        },
    ))))
}
