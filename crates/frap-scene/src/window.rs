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

/// Runtime memory of the datablocks a depsgraph builds **invisible** that have an animation
/// update pending (`Main::invisible_pending`): Blender keeps an object reached only through a
/// viewport-disabled modifier (an operand, its parent, the objects it references) in the
/// depsgraph as an invisible node; a time change or an `ID_RECALC_ANIMATION` tag while it is
/// invisible leaves its animation operation tagged, and the boundary that makes it visible
/// again (the modifier's `show_viewport` back on, the object linked into the scene) applies
/// its animation at the current frame (verified on 5.2.4: a BOOLEAN operand keyed
/// (0,0,0)@1 -> (2,0,0)@11 of a disabled modifier reads (1,0,0) at frame 6 after
/// `show_viewport = True` and `view_layer.update()`, also over a manual value; one that was
/// visible at the frame change keeps a manual value; one that leaves the depsgraph at a
/// boundary loses its pending update). Entries are (scene, view layer name, datablock).
/// `Debug` leaves the entries out, like [`ShownDepsgraphs`].
#[derive(Clone, Default)]
pub struct InvisiblePending(pub Vec<(IdRef<SceneId>, SmolStr, frap_rna::Root)>);

impl std::fmt::Debug for InvisiblePending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InvisiblePending(runtime)")
    }
}

impl InvisiblePending {
    /// The pending datablocks of `scene`'s view layer `view_layer`.
    pub fn of(&self, scene: IdRef<SceneId>, view_layer: &str) -> Vec<frap_rna::Root> {
        self.0
            .iter()
            .filter(|(s, v, _)| *s == scene && v == view_layer)
            .map(|(_, _, r)| *r)
            .collect()
    }
    /// Replaces the pending datablocks of `scene`'s view layer `view_layer` with `roots`.
    pub fn set(
        &mut self,
        scene: IdRef<SceneId>,
        view_layer: &str,
        roots: impl IntoIterator<Item = frap_rna::Root>,
    ) {
        self.0.retain(|(s, v, _)| !(*s == scene && v == view_layer));
        let name = SmolStr::new(view_layer);
        for r in roots {
            if !self
                .0
                .iter()
                .any(|(s, v, x)| *s == scene && *v == name && *x == r)
            {
                self.0.push((scene, name.clone(), r));
            }
        }
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
