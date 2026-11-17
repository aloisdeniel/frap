// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.collection`: the collection it shows; the root layer collection shows the
//! scene's embedded master collection (verified repr `bpy.data.scenes['Scene'].collection`).

use frap_rna::{EmbeddedSlot, Field, PropDef, RnaError, RnaPtr, Root, rna_hook};

use crate::HostRead;
use crate::id::paths::{internal, layer_collection_of};

#[rna_hook(srna = "LayerCollection", prop = "collection", krate = crate)]
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
    Ok(match lc.collection {
        Some(c) => Field::Id(Some(c.any())),
        None => Field::Ptr(RnaPtr::new(
            Root::Embedded {
                owner: scene.any(),
                slot: EmbeddedSlot::MasterCollection,
            },
            &frap_spec_rna::COLLECTION,
        )),
    })
}
