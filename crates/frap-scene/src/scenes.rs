// SPDX-License-Identifier: GPL-3.0-or-later
//! Multiple scenes (M3, WP 3.10; created as stubs by M3 W0): `bpy.data.scenes.new(name)`
//! (`BKE_scene_add`), `scene.new(type=…)` (`BKE_scene_duplicate` for `NEW`, `EMPTY`,
//! `LINK_COPY`, `FULL_COPY`), and scene removal (`bpy.data.scenes.remove`, `scene.delete`:
//! the last scene cannot be removed, windows showing it switch to another scene first).
//! The RNA functions (`frap-ops-data/src/funcs/blend_data_scenes_*.rs`) and the operators
//! (`frap-ops-object/src/ops/scene__{new,delete}.rs`) call these.

use frap_rna::{IdRef, SceneId};

use crate::{ChangeLog, Main, WindowState};

/// `scene.new(type=…)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SceneCopy {
    /// `NEW`: a new, empty scene with default settings.
    New,
    /// `EMPTY`: a copy of the settings without objects.
    Empty,
    /// `LINK_COPY`: settings copied, collections and objects linked.
    LinkCopy,
    /// `FULL_COPY`: everything copied (objects and data duplicated).
    FullCopy,
}

/// `BKE_scene_add`: a new scene with Blender's default settings, a master collection and one
/// view layer `ViewLayer` (name uniquified by `IdMap::insert`).
pub fn new_scene(main: &mut Main, name: &str, log: &mut ChangeLog) -> IdRef<SceneId> {
    let _ = (main, name, log);
    todo!("WP 3.10: bpy.data.scenes.new")
}

/// `scene.new(type)` from the window's scene: returns the new scene (the operator makes it the
/// window's scene).
pub fn copy_scene(
    main: &mut Main,
    src: IdRef<SceneId>,
    ty: SceneCopy,
    log: &mut ChangeLog,
) -> IdRef<SceneId> {
    let _ = (main, src, ty, log);
    todo!("WP 3.10: scene.new(type=…)")
}

/// Removes `scene` (`BKE_scene_can_be_removed`, `ED_scene_delete`): `Err` with Blender's
/// message when it is the last scene; a window showing it switches to another scene.
pub fn remove_scene(
    main: &mut Main,
    win: &mut WindowState,
    scene: IdRef<SceneId>,
    log: &mut ChangeLog,
) -> Result<(), String> {
    let _ = (main, win, scene, log);
    todo!("WP 3.10: scene removal")
}
