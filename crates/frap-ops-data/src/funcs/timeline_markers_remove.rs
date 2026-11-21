// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.timeline_markers.remove(marker)` (`rna_TimeLine_remove`,
//! blender/source/blender/makesrna/intern/rna_scene.cc, Blender 5.2.4, GPL-2.0-or-later):
//! removes the marker from this scene's list; a marker of another scene raises
//! `RuntimeError: Error: Timeline marker '<name>' not found in scene '<scene>'` (verified on
//! 5.2.4). The scene camera stays as it is.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::timeline_marker::{marker_of, removed, scene_of};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, refuse_detached, report_error};

#[rna_func(srna = "TimelineMarkers", func = "remove")]
fn timeline_markers_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    refuse_detached(this, "remove")?;
    let scene = scene_of(this)?;
    let p = arg_ptr(args, "marker")
        .ok_or_else(|| internal("timeline_markers.remove(): no marker argument"))?;
    let main = h.main();
    let (owner, i) = marker_of(main, p)?.ok_or_else(removed)?;
    if owner != scene {
        let name = main
            .scenes
            .get(owner)
            .and_then(|s| s.timeline_markers.get(i))
            .map(|m| m.name.clone())
            .unwrap_or_default();
        let scene_name = main
            .scenes
            .get(scene)
            .map(|s| s.header.name.clone())
            .unwrap_or_default();
        return Err(report_error(format!(
            "Timeline marker '{name}' not found in scene '{scene_name}'"
        )));
    }
    let (main, log) = h.main_mut();
    if let Some(sc) = main.scenes.get_mut(scene, log) {
        sc.timeline_markers.remove(i);
    }
    Ok(Value::None)
}
