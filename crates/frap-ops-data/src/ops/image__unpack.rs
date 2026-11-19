// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.unpack(method='USE_LOCAL', id='')`. Port of `image_unpack_exec`
//! (`editors/space_image/image_ops.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! The poll is `image_pack_poll`: it needs the context image even when `id` names one, so it
//! fails headless as in Blender (verified on 5.2.4: `bpy.ops.image.unpack(id='A')` →
//! `RuntimeError: Operator bpy.ops.image.unpack.poll() failed, context is incorrect`; see
//! `image__pack.rs`). The datablock function does the work:
//! `bpy.data.images['A'].unpack(method='USE_LOCAL')`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_data::image;

use super::image__pack::PACK_POLL;

#[operator(
    poll = PACK_POLL,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_UNPACK,
    notes = [
        "needs the Image Editor's image (also with id=…), which headless Blender and frap do not have: the poll fails as in Blender's background mode; use bpy.data.images['Name'].unpack(method='USE_LOCAL')",
    ]
)]
fn exec(_cx: &mut OpCtx, _p: image::Unpack) -> OpResult {
    // `image_unpack_exec`: no context image (and the poll rejected the call anyway).
    Ok(OpStatus::Cancelled)
}
