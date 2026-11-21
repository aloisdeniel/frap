// SPDX-License-Identifier: GPL-3.0-or-later
//! `TimelineMarker.camera` (WP 7.6): any object (Blender does not check the type: a mesh can
//! be bound). Assigning runs `rna_TimelineMarker_camera_update`, which switches the scene camera
//! for the current frame (`BKE_scene_camera_switch_update`; verified on 5.2.4: binding a camera
//! to a marker after the current frame, the only marker with a camera, makes it the scene
//! camera at once, as the left-most marker). Assigning `None` switches nothing back. The camera
//! is no user of the object (`IDWALK_CB_NOP`); removing the object clears it.

use frap_rna::{Field, IdRef, ObjectId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::id::timeline_marker::{camera_switch_update, marker_of, removed, with_marker_mut};
use crate::{Host, HostRead};

#[rna_hook(srna = "TimelineMarker", prop = "camera", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let main = h.main();
        let (scene, i) = marker_of(main, this)?.ok_or_else(removed)?;
        let camera = main
            .scenes
            .get(scene)
            .and_then(|s| s.timeline_markers.get(i))
            .ok_or_else(removed)?
            .camera;
        Ok(Field::Id(camera.map(IdRef::any)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let camera: Option<IdRef<ObjectId>> = match v {
            Value::Id(Some(a)) => IdRef::from_any(a),
            _ => None,
        };
        let (main, log) = h.main_mut();
        let (scene, _) = marker_of(main, this)?.ok_or_else(removed)?;
        with_marker_mut(main, log, this, |m| m.camera = camera)?;
        camera_switch_update(main, log, scene);
        Ok(())
    }
}
