// SPDX-License-Identifier: GPL-3.0-or-later
//! `ViewLayer.active_layer_collection`: where `*_add` operators link new objects
//! (`context.collection`). Reading gives the canonical layer-collection pointer; assigning
//! takes a layer collection **of this view layer** (verified on 5.2.4: one of another view
//! layer is refused like `None`: `TypeError: ViewLayer.active_layer_collection does not
//! support a 'None' assignment LayerCollection type`).

use frap_rna::{Field, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook};

use crate::id::paths::{layer_collection_of, layer_collection_ptr, view_layer, view_layer_of};
use crate::{Host, HostRead};

fn refused() -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        "ViewLayer.active_layer_collection does not support a 'None' assignment LayerCollection type",
    )
}

#[rna_hook(srna = "ViewLayer", prop = "active_layer_collection", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let (scene, _, vl) = view_layer(h.main(), this)?;
        Ok(Field::Ptr(layer_collection_ptr(
            h.main(),
            scene,
            vl,
            &vl.active_layer_collection,
        )))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let (scene, i, _) = view_layer_of(h.main(), this)?;
        let Value::Ptr(p) = v else {
            return Err(refused());
        };
        let (s2, i2, path) = layer_collection_of(h.main(), &p).map_err(|_| refused())?;
        if (s2, i2) != (scene, i) {
            return Err(refused());
        }
        let (main, log) = h.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.view_layers[i].active_layer_collection = path;
        }
        log.context_changed = true;
        Ok(())
    }
}
