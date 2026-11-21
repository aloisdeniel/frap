// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.timeline_markers.new(name, frame=1) -> TimelineMarker` (`rna_TimeLine_add`,
//! blender/source/blender/makesrna/intern/rna_scene.cc, Blender 5.2.4, GPL-2.0-or-later): a
//! selected marker appended to the scene's list, named `name` as given (not unique; cut to 63
//! bytes), at `frame` (±1048574, the parameter's range); other markers keep their selection and
//! the scene camera does not switch (verified on 5.2.4).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::TimelineMarker;
use frap_scene::id::timeline_marker::{marker_ptr, scene_of};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal, refuse_detached};

#[rna_func(srna = "TimelineMarkers", func = "new")]
fn timeline_markers_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    refuse_detached(this, "new")?;
    let scene = scene_of(this)?;
    let name = arg_str(args, "name").to_owned();
    let frame = match args.get("frame") {
        Some(Value::Int(f)) => *f as i32,
        _ => 1,
    };
    let (main, log) = h.main_mut();
    let sc = main
        .scenes
        .get_mut(scene, log)
        .ok_or_else(|| internal("timeline_markers.new(): the scene was removed"))?;
    sc.timeline_markers.push(TimelineMarker::new(&name, frame));
    let i = sc.timeline_markers.len() - 1;
    Ok(Value::Ptr(Box::new(marker_ptr(scene, i))))
}
