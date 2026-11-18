// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.channels` (`rna_Image_channels_get`): `ibuf->channels` (4 for every image frap loads or generates), 0 without data.
//!
//! Like Blender it loads the image's pixels (packed bytes, file or generator) to answer.

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
    let buf = im.acquire(&crate::id::PathBase::of(h));
    Ok(Field::Value(Value::Int(buf.map_or(0, |_| 4))))
}
