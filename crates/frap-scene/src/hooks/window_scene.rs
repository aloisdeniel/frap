// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context.window.scene` (§8): switches the context scene (verified in `-b`) through
//! [`crate::window::set_scene`] (`WM_window_set_active_scene`): the window keeps its view
//! layer name when the new scene has one of that name, else it shows the new scene's first
//! view layer. `None` is refused by `coerce` (`NEVER_NULL`: `Window.scene does not support a
//! 'None' assignment Scene type`).

use frap_rna::{
    Field, IdRef, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, SceneId, Value, rna_hook,
};

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
        let before = (h.win().scene, h.win().view_layer.clone());
        let mut win = h.win().clone();
        crate::window::set_scene(h.main(), &mut win, scene)
            .map_err(|m| RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, m))?;
        if (win.scene, &win.view_layer) != (before.0, &before.1) {
            *h.win_mut() = win;
            h.main_mut().1.context_changed = true;
        }
        Ok(())
    }
}
