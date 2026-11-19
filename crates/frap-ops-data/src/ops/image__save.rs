// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.save()`. Port of `image_save_poll` (`image_from_context_has_data_poll`,
//! `editors/space_image/image_ops.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! It saves the context image, which headless Blender and frap do not have: the poll fails
//! (verified on 5.2.4: `RuntimeError: Operator bpy.ops.image.save.poll() failed, context is
//! incorrect`; see `image__pack.rs`). `bpy.data.images['X'].save()` does the work.

use frap_op::prelude::*;
use frap_op::PollReq;
use frap_op::{Poll, PollFail, operator};
use frap_scene::Context;
use frap_scene::id::PathBase;
use frap_spec_ops_data::image;

use super::image__pack::{image_from_context, no_context_image};

/// `image_from_context_has_data_poll`
fn has_data_poll(c: &Context) -> Result<(), PollFail> {
    let alternative = "bpy.data.images['Name'].save(filepath='//name.png')";
    let im = image_from_context(c)
        .and_then(|r| c.main.images.get(r))
        .ok_or_else(|| no_context_image(alternative))?;
    let base = PathBase {
        blend: c.main.filepath.clone(),
        cwd: None,
    };
    if im.info(&base).is_none() {
        return Err(no_context_image(alternative));
    }
    Ok(())
}

/// `image_save_poll` / `image_save_as_poll`
pub(crate) const HAS_DATA_POLL: Poll = Poll {
    requires: &[PollReq::ContextImage],
    check: has_data_poll,
};

#[operator(
    poll = HAS_DATA_POLL,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_SAVE,
    notes = [
        "needs the Image Editor's image, which headless Blender and frap do not have: the poll fails as in Blender's background mode; use bpy.data.images['Name'].save()",
    ]
)]
fn exec(_cx: &mut OpCtx, _p: image::Save) -> OpResult {
    // `image_save_exec` without a context image.
    Ok(OpStatus::Cancelled)
}
