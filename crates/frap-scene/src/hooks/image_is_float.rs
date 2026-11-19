// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.is_float` (`rna_Image_is_float_get`): the buffer is a float buffer (float generated images, 16-bit PNG, OpenEXR, HDR).
//!
//! Answered from `Image::info` (the stored buffer, the generator settings or the file's header):
//! no pixels are loaded or generated.

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
    let info = im.info(&crate::id::PathBase::of(h));
    Ok(Field::Value(Value::Bool(info.is_some_and(|i| i.float))))
}
