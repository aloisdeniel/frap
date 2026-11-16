// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context.window.scene` (§8): switches the context scene (verified in `-b`). The window
//! keeps its view layer name when the new scene has one of that name, else it shows the new
//! scene's first view layer (`WM_window_set_active_scene`). `None` is refused by `coerce`
//! (`NEVER_NULL`: `Window.scene does not support a 'None' assignment Scene type`).

use frap_rna::{Field, IdRef, PropDef, RnaError, RnaPtr, SceneId, Value, rna_hook};

use crate::{Host, HostRead};

#[rna_hook(srna = "Window", prop = "scene", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        _this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        Ok(Field::Id(Some(h.win().scene.any())))
    }

    pub fn set(
        h: &mut dyn Host,
        _this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Some(scene) = (match v {
            Value::Id(Some(a)) => IdRef::<SceneId>::from_any(a),
            _ => None,
        }) else {
            return Ok(());
        };
        let Some(s) = h.main().scenes.get(scene) else {
            return Ok(());
        };
        let current = h.win().view_layer.clone();
        let vl = if s.view_layers.iter().any(|v| v.name == current) {
            current
        } else {
            s.view_layers
                .first()
                .map(|v| v.name.clone())
                .unwrap_or_default()
        };
        let changed = h.win().scene != scene;
        let win = h.win_mut();
        win.scene = scene;
        win.view_layer = vl;
        if changed {
            h.main_mut().1.context_changed = true;
        }
        Ok(())
    }
}
