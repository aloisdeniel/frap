// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.filepath_from_user(image_user=None)` (`BKE_image_user_file_path`): the file path for
//! an image user; frap images have no sequences, views or UDIM tiles, so it is `filepath`
//! (verified on 5.2.4: `'sub/two.png'` for an image loaded from `'sub/two.png'`).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::image_save::write::image;

#[rna_func(srna = "Image", func = "filepath_from_user")]
fn image_filepath_from_user(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let im = image(h, this)?;
    Ok(Value::Str(im.filepath.as_str().into()))
}
