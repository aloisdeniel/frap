// SPDX-License-Identifier: GPL-3.0-or-later
//! `LayerCollection.name`: the shown collection's name (`Scene Collection` for the root);
//! read-only (verified: `AttributeError: bpy_struct: attribute "name" from "LayerCollection"
//! is read-only`).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::id::paths::{internal, layer_collection_name, layer_collection_of};
use crate::{Host, HostRead};

#[rna_hook(srna = "LayerCollection", prop = "name", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
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
        Ok(Field::Value(Value::Str(
            layer_collection_name(main, scene, lc.collection).into(),
        )))
    }

    pub fn set(
        _h: &mut dyn Host,
        _this: &RnaPtr,
        prop: &'static PropDef,
        _v: Value,
    ) -> Result<(), RnaError> {
        Err(frap_rna::traits::read_only(
            &frap_spec_rna::LAYER_COLLECTION,
            prop.identifier,
        ))
    }
}
