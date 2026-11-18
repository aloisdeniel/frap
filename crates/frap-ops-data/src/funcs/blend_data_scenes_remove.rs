// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.scenes.remove(scene, do_unlink=True)` (`rna_Main_scenes_remove`, M3 WP 3.10).
//! Verified on 5.2.4:
//! - the last local scene cannot be removed: `RuntimeError: Error: Scene 'Scene' is the last
//!   local one, cannot be removed` (whatever `do_unlink`);
//! - otherwise `do_unlink=False` always refuses, since every scene has a user:
//!   `Error: Scene 'Scene' must have zero users to be removed, found 1 (try with
//!   do_unlink=True parameter)`;
//! - `do_unlink=True` switches a window showing the scene to its neighbour
//!   (`BKE_scene_find_replacement`: the previous scene in `bpy.data.scenes`, else the next)
//!   and deletes the scene; its objects and collections stay in `bpy.data`.

use frap_rna::{RnaError, RnaPtr, SceneId, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_bool, arg_id, internal, report_error};

#[rna_func(srna = "BlendDataScenes", func = "remove")]
fn blend_data_scenes_remove(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let scene = arg_id(args, "scene")
        .and_then(|a| a.typed::<SceneId>())
        .ok_or_else(|| internal("scenes.remove(): no scene argument"))?;
    let do_unlink = arg_bool(args, "do_unlink", true);
    let main = h.main();
    let name = main
        .scenes
        .get(scene)
        .map(|s| s.header.name.clone())
        .ok_or_else(|| internal("scenes.remove(): the scene was already removed"))?;
    if !frap_scene::scenes::can_be_removed(main, scene)
        || frap_scene::scenes::find_replacement(main, scene).is_none()
    {
        return Err(report_error(frap_scene::scenes::last_scene_message(&name)));
    }
    if !do_unlink {
        // `ID_REAL_USERS`: a scene always has its `id_us_ensure_real` user.
        let fake = main.header(scene.any()).is_some_and(|hd| hd.use_fake_user);
        let real = main
            .users(scene.any())
            .saturating_sub(u32::from(fake))
            .max(1);
        if real > 0 {
            return Err(report_error(format!(
                "Scene '{name}' must have zero users to be removed, found {real} (try with \
                 do_unlink=True parameter)"
            )));
        }
    }
    let mut win = h.win().clone();
    let (main, log) = h.main_mut();
    frap_scene::scenes::remove_scene(main, &mut win, scene, log).map_err(report_error)?;
    *h.win_mut() = win;
    Ok(Value::None)
}
