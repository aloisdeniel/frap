// SPDX-License-Identifier: GPL-3.0-or-later
//! The window's scene (M3, WP 3.10; created as a stub by M3 W0): `bpy.context.window.scene =
//! …` (`WM_window_set_active_scene`) switches the context scene and picks the view layer
//! (`BKE_view_layer_default_view`: the window's view layer name if the new scene has it, else
//! its first), and `context.view_layer` follows. The `Window.scene` / `Window.view_layer` set
//! hooks (`hooks/window_*.rs`) call these.

use frap_rna::{IdRef, SceneId};

use crate::{Main, WindowState};

/// `WM_window_set_active_scene`: makes `scene` the window's scene (`Err` with Blender's message
/// when it does not exist).
pub fn set_scene(main: &Main, win: &mut WindowState, scene: IdRef<SceneId>) -> Result<(), String> {
    let _ = (main, win, scene);
    todo!("WP 3.10: window.scene = …")
}
