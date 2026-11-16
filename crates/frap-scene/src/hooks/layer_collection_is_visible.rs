// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.is_visible`: not excluded and not hidden along the chain (fresh, D-04;
//! Blender reports the value of the last depsgraph sync).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::HostRead;
use crate::id::layers::layer_collection_visible;
use crate::id::paths::{internal, layer_collection_of};

#[rna_hook(srna = "LayerCollection", prop = "is_visible", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let (scene, i, path) = layer_collection_of(main, this)?;
    let s = main
        .scenes
        .get(scene)
        .ok_or_else(|| internal("the scene was removed"))?;
    Ok(Field::Value(Value::Bool(layer_collection_visible(
        s,
        &s.view_layers[i],
        &main.collections,
        &path,
    ))))
}
