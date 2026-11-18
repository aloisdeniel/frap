// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.new(name='Untitled', width=1024, height=1024, color=(0, 0, 0, 1),
//! alpha=True, generated_type='BLANK', float=False, use_stereo_3d=False, tiled=False)`.
//! Port of `image_new_exec` (`editors/space_image/image_ops.cc`) and
//! `BKE_image_add_generated` (`blenkernel/intern/image.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (`-b --factory-startup`):
//! - adds a GENERATED image (`Untitled`, uniquified) with no users: from a script there is no
//!   UI template or Image Editor to show it in (`id_us_min`);
//! - `color` is stored as `generated_color`; without `alpha` its alpha is 1 and the buffer is
//!   RGB (`depth` 24, else 32; float 96/128); `image.new(color=(0.5,)*4, width=2, height=1)`
//!   reads pixels 0.50196 (bytes), a float image the sRGB → linear fill;
//! - `float` images use the `Linear Rec.709` colour space; `tiled` makes a `TILED` image
//!   (type `IMAGE`);
//! - `UV_GRID` and `COLOR_GRID` fill the buffer with Blender's test patterns (`COLOR_GRID`
//!   without its letter labels, docs/deviations.md).

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::{Image, imbuf};
use frap_spec_ops_data::image;
use smol_str::SmolStr;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_NEW,
    notes = [
        "the new image has no users and is shown nowhere (no Image Editor headless, as Blender from scripts); assign it to an Image Texture node with node.image = bpy.data.images['Untitled']",
        "COLOR_GRID has no letter/number labels (frap has no font rasteriser); every other pixel is Blender's",
    ]
)]
fn exec(cx: &mut OpCtx, p: image::ImageNew) -> OpResult {
    let name: SmolStr = p.name.value.clone();
    let width = p.width.value.max(1);
    let height = p.height.value.max(1);
    let mut color = p.color.value;
    if !p.alpha.value {
        color[3] = 1.0;
    }
    // Blender allocates the buffer here: an image over the budget fails at once.
    frap_math::budget::reserve_buffer(
        imbuf::BUFFER,
        imbuf::buffer_bytes(width as u32, height as u32, p.float.value),
    )
    .map_err(|e| OpError::Limit(e.to_string()))?;
    let mut im = Image::new(&name, width, height);
    im.generated_type = SmolStr::new_static(p.generated_type.value.identifier());
    im.generated_color = color;
    im.generated_depth = if p.alpha.value { 32 } else { 24 };
    if p.float.value {
        im.use_generated_float = true;
        im.colorspace_settings.name = SmolStr::new_static("Linear Rec.709");
    }
    if p.tiled.value {
        im.source = SmolStr::new_static("TILED");
    }
    let (main, log) = cx.main_mut();
    let r = main.images.insert(&name, im, log);
    cx.result("image", r.any());
    Ok(OpStatus::Finished)
}
