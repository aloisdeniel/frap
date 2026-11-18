// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.buffers_free()` (`rna_Image_buffers_free` → `BKE_image_free_buffers_ex`): frees the
//! image buffer; modified pixels are lost and the image reads its source again (verified on
//! 5.2.4: a modified generated image is generated again and is not dirty).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::image_save::write::image_ref;

#[rna_func(srna = "Image", func = "buffers_free")]
fn image_buffers_free(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let r = image_ref(this)?;
    let (main, log) = h.main_mut();
    if let Some(im) = main.images.get_mut(r, log) {
        im.free_buffers();
    }
    Ok(Value::None)
}
