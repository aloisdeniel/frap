// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.depth` (`rna_Image_depth_get`): bits per pixel of the source colour mode (8 grey, 16 grey + alpha, 24 RGB, 32 RGBA), times 4 for a float buffer; 0 without data. Verified on 5.2.4: `images.new` 24, `images.new(alpha=True)` and `image.new()` 32, a float image 96 or 128, a 16-bit RGB PNG 96.
//!
//! Like Blender it loads the image's pixels (packed bytes, file or generator) to answer.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::image_packed_file::image_of;
use crate::HostRead;

#[rna_hook(srna = "Image", prop = "depth", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let im = image_of(h, this)?;
    let buf = im.acquire(&crate::id::PathBase::of(h));
    Ok(Field::Value(Value::Int(buf.map_or(0, |b| i64::from(b.depth())))))
}
