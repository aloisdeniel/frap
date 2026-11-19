// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.pack()`. Port of `image_pack_poll` / `image_pack_exec`
//! (`editors/space_image/image_ops.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! The image ops of the `image.*` namespace act on the *context image*
//! (`image_from_context`): the `edit_image` member that UI templates set, else the Image
//! Editor's image. A script run headless has neither, and neither has frap (no Image Editor,
//! no `temp_override`), so the poll fails exactly as Blender's does in background mode
//! (verified on 5.2.4: `RuntimeError: Operator bpy.ops.image.pack.poll() failed, context is
//! incorrect`). The same work is available on the datablock: `bpy.data.images['X'].pack()`.
//!
//! This file also hosts the context lookup shared by the other context-image operators.

use frap_op::PollReq;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, operator};
use frap_rna::{IdRef, ImageId};
use frap_scene::Context;
use frap_spec_ops_data::image;
use smallvec::SmallVec;

/// `image_from_context`: `edit_image`, else the Image Editor's image. frap has neither.
pub(crate) fn image_from_context(_c: &Context) -> Option<IdRef<ImageId>> {
    None
}

/// The poll failure of an operator without a context image, with the datablock equivalent.
pub(crate) fn no_context_image(alternative: &str) -> PollFail {
    PollFail {
        message: None,
        unmet: smallvec::smallvec![PollReq::ContextImage],
        hint: Some(format!(
            "this operator works on the Image Editor's image, which headless Blender and frap do \
             not have; use {alternative}"
        )),
    }
}

/// `image_pack_test` with Blender's poll messages.
fn pack_poll(c: &Context) -> Result<(), PollFail> {
    let Some(r) = image_from_context(c) else {
        return Err(no_context_image("bpy.data.images['Name'].pack()"));
    };
    let Some(im) = c.main.images.get(r) else {
        return Err(no_context_image("bpy.data.images['Name'].pack()"));
    };
    let message = if im.header.library.is_some() {
        Some("Image is not editable")
    } else if matches!(im.source.as_str(), "SEQUENCE" | "MOVIE") {
        Some("Movies or image sequences do not support packing")
    } else if matches!(im.source.as_str(), "VIEWER") {
        Some("Render Result and Viewer Nodes cannot be packed")
    } else {
        None
    };
    match message {
        Some(m) => Err(PollFail {
            message: Some(format!("Operator bpy.ops.image.pack.poll() {m}")),
            unmet: SmallVec::new(),
            hint: None,
        }),
        None => Ok(()),
    }
}

/// `image_pack_poll`
pub(crate) const PACK_POLL: Poll = Poll {
    requires: &[PollReq::ContextImage],
    check: pack_poll,
};

#[operator(
    poll = PACK_POLL,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_PACK,
    notes = [
        "needs the Image Editor's image, which headless Blender and frap do not have: the poll fails as in Blender's background mode; use bpy.data.images['Name'].pack()",
    ]
)]
fn exec(_cx: &mut OpCtx, _p: image::Pack) -> OpResult {
    // `image_pack_exec`: `image_pack_test(nullptr)` fails without a context image.
    Ok(OpStatus::Cancelled)
}
