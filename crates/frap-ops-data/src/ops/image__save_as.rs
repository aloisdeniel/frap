// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.save_as(save_as_render=False, copy=False, filepath='', …)`. Port of
//! `image_save_as_poll` (`editors/space_image/image_ops.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! It saves the context image, which headless Blender and frap do not have: the poll fails
//! (verified on 5.2.4: `bpy.ops.image.save_as(filepath='x.png')` → `RuntimeError: Operator
//! bpy.ops.image.save_as.poll() failed, context is incorrect`; see `image__pack.rs`).
//! `bpy.data.images['X'].save(filepath=…, save_copy=…)` and `.save_render(filepath)` do the
//! work.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_data::image;

use super::image__save::HAS_DATA_POLL;

#[operator(
    poll = HAS_DATA_POLL,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_SAVE_AS,
    notes = [
        "needs the Image Editor's image, which headless Blender and frap do not have: the poll fails as in Blender's background mode; use bpy.data.images['Name'].save(filepath=…, save_copy=…) or .save_render(filepath)",
    ]
)]
fn exec(_cx: &mut OpCtx, _p: image::SaveAs) -> OpResult {
    // `image_save_as_exec` without a context image.
    Ok(OpStatus::Cancelled)
}
