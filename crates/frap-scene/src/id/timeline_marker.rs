// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.TimelineMarker` (`Scene.timeline_markers`, `TimelineMarkers.new/remove/clear`).
//!
//! M7 W0 (decision log M7W0-8) froze the data; WP 7.6 owns this file afterwards: the RNA
//! binding (`name`, `frame`, `select`, `camera`), the `TimelineMarkers` functions, the `marker.*`
//! operators and marker camera switching (`BKE_scene_camera_switch_update`).

use frap_rna::{IdRef, ObjectId};
use smol_str::SmolStr;

/// A timeline marker (DNA `TimeMarker`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineMarker {
    /// `name`
    pub name: SmolStr,
    /// `frame`
    pub frame: i32,
    /// `select`
    pub select: bool,
    /// `camera`: the camera the scene switches to from this marker on.
    pub camera: Option<IdRef<ObjectId>>,
}
