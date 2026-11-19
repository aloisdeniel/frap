// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.reload()` (`rna_Image_reload` → `BKE_image_signal(IMA_SIGNAL_RELOAD)`, `image.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! A packed image re-reads its packed file from the packed file's `filepath` when that file
//! exists (else Blender prints `ERROR: Image "…" not available. Keeping packed image` and keeps
//! the bytes); then the buffer is freed, so modified pixels are lost and the image reads its
//! source again. Verified on 5.2.4: a modified FILE image reads the file's pixels again and is
//! not dirty; a modified generated image is generated again.

use std::sync::Arc;

use frap_rna::{IdRef, ImageId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::EncodedImage;
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::internal;
use super::image_save::write::{image_ref, mime_of};

/// `IMA_SIGNAL_RELOAD`
pub(crate) fn reload(h: &mut dyn Host, r: IdRef<ImageId>) -> Result<(), RnaError> {
    let base = PathBase::of(h);
    let main = h.main();
    let im = main
        .images
        .get(r)
        .ok_or_else(|| internal("the image was removed"))?;
    let repacked = match &im.packed_file {
        Some(_) if !im.packed_filepath.is_empty() => {
            std::fs::read(base.file(&im.packed_filepath)).ok()
        }
        _ => None,
    };
    let (main, log) = h.main_mut();
    if let Some(im) = main.images.get_mut(r, log) {
        if let Some(bytes) = repacked {
            im.packed_file = Some(Arc::new(EncodedImage {
                mime: SmolStr::new_static(mime_of(&bytes)),
                bytes,
            }));
        }
        im.free_buffers();
    }
    Ok(())
}

#[rna_func(srna = "Image", func = "reload")]
fn image_reload(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    reload(h, image_ref(this)?)?;
    Ok(Value::None)
}
