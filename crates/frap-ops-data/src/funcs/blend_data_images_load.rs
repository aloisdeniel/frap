// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.images.load(filepath, check_existing=False)` (`rna_Main_images_load` →
//! `BKE_image_load` / `BKE_image_load_exists`, `image.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4:
//! - the image is named after the file (`rgb.png`, uniquified: `rgb.png.001`), `source`
//!   `FILE`, `filepath` exactly as given (`//rgb.png` stays `//`-relative), no users;
//!   `file_format`, `size` and the colour space come from the file (8-bit and 16-bit PNG, JPEG:
//!   `sRGB`; OpenEXR/HDR: `Linear Rec.709`, alpha mode `PREMUL` by extension);
//! - a file that cannot be opened fails with `RuntimeError: Error: Cannot read 'nope.png': No
//!   such file or directory`; a file that opens but is not an image still gives an image, with
//!   size `(0, 0)` and no data;
//! - `check_existing=True` returns an image (not GENERATED/VIEWER, not linked) whose path is
//!   the same *string* after `//` expansion: `rgb.png` finds `rgb.png` but not
//!   `/abs/dir/rgb.png`.

use frap_rna::{IdRef, ImageId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::{Image, LoadError, PathBase};
use frap_scene::{ChangeLog, FuncArgs, Host, Main};

use super::_common::{arg_bool, arg_str, report_error};

/// `BKE_image_load_exists` lookup: an image already loaded from the same path.
pub(crate) fn existing(main: &Main, filepath: &str) -> Option<IdRef<ImageId>> {
    let base = PathBase {
        blend: main.filepath.clone(),
        cwd: None,
    };
    let want = base.abs(filepath);
    main.images
        .iter()
        .find(|(_, im)| {
            !matches!(im.source.as_str(), "VIEWER" | "GENERATED")
                && im.header.library.is_none()
                && base.abs(&im.filepath) == want
        })
        .map(|(r, _)| r)
}

/// `BKE_image_load` + insertion: the new image, or Blender's `Cannot read` message.
pub(crate) fn load(
    main: &mut Main,
    log: &mut ChangeLog,
    base: &PathBase,
    filepath: &str,
) -> Result<IdRef<ImageId>, String> {
    match Image::load(filepath, base) {
        Ok(im) => {
            let name = im.header.name.clone();
            Ok(main.images.insert(&name, im, log))
        }
        Err(LoadError::Io(why)) => Err(format!("Cannot read '{filepath}': {why}")),
    }
}

#[rna_func(srna = "BlendDataImages", func = "load")]
fn blend_data_images_load(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let filepath = arg_str(args, "filepath").to_owned();
    let check_existing = arg_bool(args, "check_existing", false);
    let base = PathBase::of(h);
    let (main, log) = h.main_mut();
    if check_existing && let Some(r) = existing(main, &filepath) {
        return Ok(r.into());
    }
    load(main, log, &base, &filepath)
        .map(Value::from)
        .map_err(report_error)
}
