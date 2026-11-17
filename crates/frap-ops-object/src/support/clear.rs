// SPDX-License-Identifier: GPL-3.0-or-later
//! The loop shared by `object.location_clear/rotation_clear/scale_clear`. Port of
//! `object_clear_transform_generic_exec` (`editors/object/object_transform.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4: every selected editable (visible, local) object is cleared, a parent and
//! its child alike; the delta channels only with `clear_delta=True`; nothing selected →
//! `CANCELLED`. frap objects have no transform locks (`lock_location`, …), so every channel
//! clears.

use frap_op::prelude::*;
use frap_scene::id::Object;

/// Runs `clear` on every selected editable object.
pub fn run(cx: &mut OpCtx, what: &str, clear: impl Fn(&mut Object)) -> OpResult {
    let (objects, data_origin, skip_children) = {
        let c = cx.ctx();
        let ts = c.tool_settings();
        (
            c.selected_editable_objects(),
            ts.use_transform_data_origin,
            ts.use_transform_skip_children,
        )
    };
    if objects.is_empty() {
        cx.report(
            ReportType::Warning,
            format!("frap: nothing to clear: no visible, editable object is selected ({what})"),
        );
        return Ok(OpStatus::Cancelled);
    }
    if data_origin || skip_children {
        return Err(OpError::NotImplemented(
            "frap: clearing transforms with tool_settings.use_transform_data_origin or \
             use_transform_skip_children is not implemented yet"
                .into(),
        ));
    }
    for ob in objects {
        clear(cx.object_mut(ob)?);
    }
    Ok(OpStatus::Finished)
}
