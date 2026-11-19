// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.size` (`rna_Image_size_get`): the buffer's width and height, `(0, 0)` when the image cannot be loaded.
//!
//! Answered from `Image::info` (the stored buffer, the generator settings or the file's header):
//! no pixels are loaded or generated.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::image_packed_file::image_of;
use crate::HostRead;

#[rna_hook(srna = "Image", prop = "size", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let im = image_of(h, this)?;
    let size = im
        .info(&crate::id::PathBase::of(h))
        .map_or([0, 0], |b| [b.x as i32, b.y as i32]);
    Ok(Field::Value(Value::IntArray(size.iter().map(|&v| i64::from(v)).collect())))
}
