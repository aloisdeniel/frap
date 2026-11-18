// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.size` (`rna_Image_size_get`): the buffer's width and height, `(0, 0)` when the image cannot be loaded.
//!
//! Like Blender it loads the image's pixels (packed bytes, file or generator) to answer.

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
    let size = if im.buffer.is_some() || im.source == "GENERATED" {
        im.known_size()
    } else {
        im.acquire(&crate::id::PathBase::of(h))
            .map_or([0, 0], |b| [b.x as i32, b.y as i32])
    };
    Ok(Field::Value(Value::IntArray(size.iter().map(|&v| i64::from(v)).collect())))
}
