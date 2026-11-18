// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.is_float` (`rna_Image_is_float_get`): the buffer is a float buffer (float generated images, 16-bit PNG, OpenEXR, HDR).
//!
//! Like Blender it loads the image's pixels (packed bytes, file or generator) to answer.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::image_packed_file::image_of;
use crate::HostRead;

#[rna_hook(srna = "Image", prop = "is_float", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let im = image_of(h, this)?;
    let buf = im.acquire(&crate::id::PathBase::of(h));
    Ok(Field::Value(Value::Bool(buf.is_some_and(|b| b.is_float()))))
}
