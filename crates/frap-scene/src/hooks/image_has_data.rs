// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.has_data` (`rna_Image_has_data_get`): pixels are available.
//!
//! Deviation (docs/deviations.md): Blender answers whether the buffer *has been loaded yet*
//! (`BKE_image_has_loaded_ibuf`), so a freshly loaded file reads `False` until something reads
//! its pixels or size; frap answers whether it can be loaded.
//!
//! Like Blender it loads the image's pixels (packed bytes, file or generator) to answer.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::image_packed_file::image_of;
use crate::HostRead;

#[rna_hook(srna = "Image", prop = "has_data", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let im = image_of(h, this)?;
    let buf = im.acquire(&crate::id::PathBase::of(h));
    Ok(Field::Value(Value::Bool(buf.is_some())))
}
