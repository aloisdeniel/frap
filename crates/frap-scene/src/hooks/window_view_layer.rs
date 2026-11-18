// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context.window.view_layer` (§8): the view layer of the window's scene shown by the
//! context. A view layer of another scene is ignored (verified on 5.2.4: no error, the context
//! view layer does not change).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::id::paths::{view_layer_of, view_layer_ptr};
use crate::{Host, HostRead};

#[rna_hook(srna = "Window", prop = "view_layer", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        _this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let c = h.ctx();
        Ok(Field::Ptr(view_layer_ptr(
            h.win().scene,
            &c.view_layer().name,
        )))
    }

    pub fn set(
        h: &mut dyn Host,
        _this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Value::Ptr(p) = v else {
            return Ok(());
        };
        let Ok((scene, i, _)) = view_layer_of(h.main(), &p) else {
            return Ok(());
        };
        if scene != h.win().scene {
            return Ok(());
        }
        let Some(name) = h
            .main()
            .scenes
            .get(scene)
            .map(|s| s.view_layers[i].name.clone())
        else {
            return Ok(());
        };
        if h.win().view_layer != name {
            h.win_mut().view_layer = name;
            h.main_mut().1.context_changed = true;
        }
        Ok(())
    }
}
