// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context.window.scene` (§8): switches the context scene (verified in `-b`) through
//! [`crate::window::set_scene`] (`WM_window_set_active_scene`): the window keeps its view
//! layer name when the new scene has one of that name, else it shows the new scene's first
//! view layer. `None` is refused by `coerce` (`NEVER_NULL`: `Window.scene does not support a
//! 'None' assignment Scene type`).
//!
//! A change of scene evaluates the new scene's depsgraph at once (`ED_scene_change_update`):
//! its animation at its own `frame_current` is written into the originals (verified on 5.2.4:
//! a manual `location` of an animated object of the new scene reads its animated value right
//! after the assignment, also when switching back to a scene shown before; assigning the
//! current scene, or switching only `window.view_layer`, applies nothing).

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
            let switched = win.scene != before.0;
            let old = h.win().clone();
            h.main_mut().0.shown_depsgraphs.remember(&old);
            *h.win_mut() = win;
            h.main_mut().1.context_changed = true;
            if switched {
                h.mark_time_dirty();
                h.apply_pending_animation()?;
            }
        }
        Ok(())
    }
}
