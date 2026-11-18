// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.channels` (`rna_Image_channels_get`): `ibuf->channels` (4 for every image frap loads or generates), 0 without data.
//!
//! Answered from `Image::info` (the stored buffer, the generator settings or the file's header):
//! no pixels are loaded or generated.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::image_packed_file::image_of;
use crate::HostRead;

#[rna_hook(srna = "Image", prop = "channels", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let im = image_of(h, this)?;
    let info = im.info(&crate::id::PathBase::of(h));
    Ok(Field::Value(Value::Int(info.map_or(0, |_| 4))))
}
