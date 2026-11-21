// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.timeline_markers.clear()` (`rna_TimeLine_clear`,
//! blender/source/blender/makesrna/intern/rna_scene.cc, Blender 5.2.4, GPL-2.0-or-later):
//! removes every marker of the scene; the scene camera stays as it is (verified on 5.2.4).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::timeline_marker::scene_of;
use frap_scene::{FuncArgs, Host};

use super::_common::refuse_detached;

#[rna_func(srna = "TimelineMarkers", func = "clear")]
fn timeline_markers_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    refuse_detached(this, "clear")?;
    let scene = scene_of(this)?;
    let (main, log) = h.main_mut();
    if main
        .scenes
        .get(scene)
        .is_some_and(|s| !s.timeline_markers.is_empty())
        && let Some(sc) = main.scenes.get_mut(scene, log)
    {
        sc.timeline_markers.clear();
    }
    Ok(Value::None)
}
