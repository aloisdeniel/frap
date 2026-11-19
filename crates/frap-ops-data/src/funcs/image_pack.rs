// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.pack(data=None, data_len=0)` (`rna_Image_pack` → `BKE_image_packfile_ensure`,
//! `image.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4:
//! - an image already packed and not modified is left alone (a second `pack()` changes
//!   nothing);
//! - a modified image packs its buffer from memory (`BKE_image_memorypack`): PNG for a byte
//!   buffer (in its colour mode: `images.new` images are RGB), OpenEXR for a float buffer;
//!   `file_format` becomes `PNG`/`OPEN_EXR` (also for a JPEG file), a GENERATED image becomes
//!   FILE (type `IMAGE`, `filepath` stays `''`), the packed file's `filepath` is the image's,
//!   and the image is no longer dirty;
//! - an unmodified image packs the bytes of its file (`BKE_image_packfiles`), with the
//!   image's `filepath` as the packed file's path; an image without a path (an unmodified
//!   generated image) packs nothing, silently; a missing file fails with `Error: Unable to pack
//!   file, source path 'sub/x.png' not found`.
//!
//! `data` (raw bytes) has no frap value type: passing it answers `NotImplementedError`.

use std::sync::Arc;

use frap_rna::{IdRef, ImageId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::EncodedImage;
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{internal, not_implemented};
use super::image_save::write::{Reports, image_ref, memorypack, mime_of};

/// `BKE_image_packfile_ensure` without raw data.
pub(crate) fn pack(h: &mut dyn Host, r: IdRef<ImageId>, rep: &mut Reports) -> Result<(), RnaError> {
    let base = PathBase::of(h);
    let main = h.main();
    let im = main
        .images
        .get(r)
        .ok_or_else(|| internal("the image was removed"))?;
    let is_dirty = im.is_dirty();
    if im.packed_file.is_some() && !is_dirty {
        return Ok(());
    }
    if is_dirty {
        // `BKE_image_memorypack`
        let mut buf = (**im
            .live_buffer()
            .expect("invariant: dirty images have a buffer"))
        .clone();
        let (bytes, mime, ff) = memorypack(&buf).map_err(not_implemented)?;
        let packed_filepath = im.filepath.clone();
        let (main, log) = h.main_mut();
        if let Some(im) = main.images.get_mut(r, log) {
            im.packed_file = Some(Arc::new(EncodedImage {
                bytes,
                mime: SmolStr::new_static(mime),
            }));
            im.packed_filepath = packed_filepath;
            im.file_format = SmolStr::new_static(ff);
            if im.source == "GENERATED" {
                im.source = SmolStr::new_static("FILE");
            }
            buf.dirty = false;
            buf.ftype = SmolStr::new_static(ff);
            im.buffer = Some(Arc::new(buf));
        }
        return Ok(());
    }
    // `BKE_image_packfiles`: the file's bytes (render results and images without a path are
    // skipped silently; the image was not packed, or it would be packed and clean).
    if im.filepath.is_empty() {
        return Ok(());
    }
    let abs = base.abs(&im.filepath);
    let filepath = im.filepath.clone();
    let Ok(bytes) = std::fs::read(base.file(&im.filepath)) else {
        rep.error(format!(
            "Unable to pack file, source path '{abs}' not found"
        ));
        return Ok(());
    };
    let (main, log) = h.main_mut();
    if let Some(im) = main.images.get_mut(r, log) {
        im.packed_file = Some(Arc::new(EncodedImage {
            mime: SmolStr::new_static(mime_of(&bytes)),
            bytes,
        }));
        im.packed_filepath = filepath;
    }
    Ok(())
}

#[rna_func(srna = "Image", func = "pack")]
fn image_pack(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    if args
        .get("data")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
    {
        return Err(not_implemented(
            "Image.pack(data=…): raw bytes cannot be passed to frap; write them to a file and use \
             bpy.data.images.load(filepath) then pack()",
        ));
    }
    let r = image_ref(this)?;
    let mut rep = Reports::default();
    pack(h, r, &mut rep)?;
    rep.finish(h)?;
    Ok(Value::None)
}
