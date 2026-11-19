// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.reload()`. Port of `image_reload_exec` (`editors/space_image/image_ops.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! The operator has no poll; without a context image (headless Blender and frap have no Image
//! Editor, see `image__pack.rs`) it returns `CANCELLED` (verified on 5.2.4: `{'CANCELLED'}`).
//! `bpy.data.images['X'].reload()` reloads a given image.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_data::image;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_RELOAD,
    notes = [
        "reloads the Image Editor's image, which headless Blender and frap do not have: returns CANCELLED as in Blender's background mode; use bpy.data.images['Name'].reload()",
    ]
)]
fn exec(_cx: &mut OpCtx, _p: image::Reload) -> OpResult {
    // `image_reload_exec`: `if (!ima) return OPERATOR_CANCELLED;` (`image_from_context` is
    // always empty in frap).
    Ok(OpStatus::Cancelled)
}
