// SPDX-License-Identifier: GPL-3.0-or-later
//! `view_layer.objects.active` (`LayerObjects.active`, `rna_LayerObjects_active_object_set`,
//! verified on 5.2.4): any object with a base in the view layer may become active, selected or
//! not; `None` clears it; an object without a base raises
//! `RuntimeError: Error: ViewLayer 'ViewLayer' does not contain object 'Orphan'`.

use frap_rna::{
    Field, IdRef, ObjectId, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook,
};

use crate::id::paths::{view_layer, view_layer_of};
use crate::{Host, HostRead};

/// `LayerObjects` is the API struct of `ViewLayer.objects` (no `rna_struct!` binding of its
/// own), so its hook bindings are declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "LayerObjects",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "LayerObjects", prop = "active", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let (_, _, vl) = view_layer(h.main(), this)?;
        Ok(Field::Id(vl.active.map(IdRef::any)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let (scene, i, _) = view_layer_of(h.main(), this)?;
        let ob: Option<IdRef<ObjectId>> = match v {
            Value::Id(Some(a)) => IdRef::from_any(a),
            _ => None,
        };
        let (main, log) = h.main_mut();
        let Some(s) = main.scenes.get(scene) else {
            return Ok(());
        };
        let vl = &s.view_layers[i];
        if let Some(o) = ob
            && vl.base(o).is_none()
        {
            return Err(RnaError::new(
                RnaErrorKind::Other,
                PyExc::RuntimeError,
                format!(
                    "Error: ViewLayer '{}' does not contain object '{}'\n",
                    vl.name,
                    main.objects.name_of(o).unwrap_or_default()
                ),
            ));
        }
        if vl.active == ob {
            return Ok(());
        }
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.view_layers[i].active = ob;
        }
        log.context_changed = true;
        Ok(())
    }
}
