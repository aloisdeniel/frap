// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.scale(width, height, frame=0, tile_index=0)` (`rna_Image_scale` →
//! `BKE_image_scale`: `IMB_scale(…, IMBScaleFilter::Box, false)` then the image is marked
//! dirty, `image.cc`, Blender 5.2.4, GPL-2.0-or-later). The box filter is ported in
//! `frap-scene` (`ImBuf::scale`, tested against Blender's buffers).
//!
//! Verified on 5.2.4: `size` becomes the new size while `generated_width/height` keep theirs
//! (so `reload()` of a generated image gives the old size back); the image is dirty even when
//! the size does not change; an image without data fails with `Error: Image 'X' failed to load
//! image buffer`. `frame` and `tile_index` have no effect in frap (no sequences, no tiles).

use std::sync::Arc;

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_int;
use super::image_save::write::{Reports, image, image_ref};

#[rna_func(srna = "Image", func = "scale")]
fn image_scale(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let r = image_ref(this)?;
    let width = arg_int(args, "width", 1).clamp(1, i64::from(i32::MAX)) as u32;
    let height = arg_int(args, "height", 1).clamp(1, i64::from(i32::MAX)) as u32;
    let im = image(h, this)?;
    let name = im.header.name.clone();
    let Some(mut buf) = im.acquire(&PathBase::of(h)) else {
        let mut rep = Reports::default();
        rep.error(format!("Image '{name}' failed to load image buffer"));
        return rep.finish(h).map(|()| Value::None);
    };
    buf.scale(width, height);
    buf.dirty = true;
    let (main, log) = h.main_mut();
    if let Some(im) = main.images.get_mut(r, log) {
        im.buffer = Some(Arc::new(buf));
    }
    Ok(Value::None)
}
