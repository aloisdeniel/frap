// SPDX-License-Identifier: GPL-3.0-or-later
//! The window's scene (M3, WP 3.10): `bpy.context.window.scene = …`
//! (`WM_window_set_active_scene` → `ED_screen_scene_change`) switches the context scene and
//! keeps the window's view layer name when the new scene has a view layer of that name, else
//! shows the new scene's first one (`WM_window_ensure_active_view_layer`,
//! `BKE_view_layer_default_view`); `context.view_layer` follows. Nothing else changes: objects
//! keep their modes (an object left in Edit Mode in the old scene stays in Edit Mode, and
//! `context.mode` follows the new scene's active object), and `scene.camera` is untouched
//! (`screen_set_3dview_camera` only changes the 3D View's own camera). The `Window.scene` /
//! `Window.view_layer` set hooks (`hooks/window_*.rs`), `scene.new` and scene removal call this.
//!
//! Ported from `windowmanager/intern/wm_window.cc` and `editors/screen/screen_edit.cc`,
//! Blender 5.2.4, GPL-2.0-or-later; verified on 5.2.4 in `-b`.

use frap_rna::{IdRef, SceneId};
use smol_str::SmolStr;

use crate::{Main, WindowState};

/// Runtime memory of the depsgraphs the window has shown (`Main::shown_depsgraphs`): Blender
/// keeps a depsgraph active once the window showed it (`DEG_make_active`), and
/// `Scene.frame_set` flushes the animation of every active depsgraph of its scene into the
/// originals, also when the window shows another scene (verified on 5.2.4: after switching
/// away from `Scene`, `Scene.frame_set(3)` animates its objects; for a scene the window never
/// showed it animates nothing). `Debug` leaves the entries out, so a saved and reopened state
/// prints the same.
#[derive(Clone, Default)]
pub struct ShownDepsgraphs(pub Vec<(IdRef<SceneId>, SmolStr)>);

impl std::fmt::Debug for ShownDepsgraphs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ShownDepsgraphs(runtime)")
    }
}

impl ShownDepsgraphs {
    /// Remembers the depsgraph `win` shows (called before the window switches away from it).
    pub fn remember(&mut self, win: &WindowState) {
        let key = (win.scene, win.view_layer.clone());
        if !self.0.contains(&key) {
            self.0.push(key);
        }
    }
    /// Whether the window showed `scene`'s view layer `view_layer` before.
    pub fn contains(&self, scene: IdRef<SceneId>, view_layer: &str) -> bool {
        self.0.iter().any(|(s, v)| *s == scene && v == view_layer)
    }
}

/// The view layer the window shows in `scene`: its current view layer name when `scene` has
/// one of that name, else the scene's first view layer.
pub fn view_layer_for(main: &Main, win: &WindowState, scene: IdRef<SceneId>) -> Option<SmolStr> {
    let s = main.scenes.get(scene)?;
    if s.view_layers.iter().any(|v| v.name == win.view_layer) {
        return Some(win.view_layer.clone());
    }
    s.view_layers.first().map(|v| v.name.clone())
}

/// `WM_window_set_active_scene`: makes `scene` the window's scene (`Err` with Python's message
/// when it no longer exists).
pub fn set_scene(main: &Main, win: &mut WindowState, scene: IdRef<SceneId>) -> Result<(), String> {
    let Some(vl) = view_layer_for(main, win, scene) else {
        return Err("StructRNA of type Scene has been removed".into());
    };
    win.scene = scene;
    win.view_layer = vl;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChangeLog;

    #[test]
    fn switching_keeps_or_resets_the_view_layer_name() {
        let (mut main, mut win) = crate::startup::factory();
        let mut log = ChangeLog::default();
        let s2 = crate::scenes::new_scene(&mut main, "S2", &mut log);
        // Same name: kept.
        set_scene(&main, &mut win, s2).unwrap();
        assert_eq!(win.scene, s2);
        assert_eq!(win.view_layer, "ViewLayer");
        // Another name: the first view layer of the new scene.
        win.view_layer = "Other".into();
        let first = main.scenes.iter().next().map(|(r, _)| r).unwrap();
        set_scene(&main, &mut win, first).unwrap();
        assert_eq!(win.view_layer, "ViewLayer");
        // A removed scene is refused.
        main.remove(s2.any(), &mut log);
        assert!(set_scene(&main, &mut win, s2).is_err());
    }
}
