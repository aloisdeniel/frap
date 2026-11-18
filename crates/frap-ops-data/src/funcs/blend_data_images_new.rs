// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.images.new(name, width, height, alpha=False, float_buffer=False, stereo3d=False,
//! is_data=False, tiled=False)`: a generated BLANK image (`Image::new`), verified on 5.2.4:
//! black with alpha 1 and `alpha_mode` `STRAIGHT` whatever `alpha` is; sizes below 1 are
//! clamped to 1 by the argument binding (the parameters' hard minimum); `float_buffer` sets
//! `use_generated_float` and the colour space `Linear Rec.709`; `is_data` sets `Non-Color`
//! (also with `float_buffer`); `tiled` makes the source `TILED` (type `IMAGE`). `alpha` is
//! the buffer's colour depth (`gen_depth` 32, else 24: `Image.depth`, and RGBA vs RGB when the
//! image is packed or saved); `stereo3d` has no stored counterpart in frap.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Image;
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{arg_bool, arg_int, arg_str};

#[rna_func(srna = "BlendDataImages", func = "new")]
fn blend_data_images_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let w = arg_int(args, "width", 1024).clamp(1, i32::MAX as i64) as i32;
    let ht = arg_int(args, "height", 1024).clamp(1, i32::MAX as i64) as i32;
    let mut img = Image::new(&name, w, ht);
    if arg_bool(args, "alpha", false) {
        img.generated_depth = 32;
    }
    if arg_bool(args, "float_buffer", false) {
        img.use_generated_float = true;
        img.colorspace_settings.name = SmolStr::new_static("Linear Rec.709");
    }
    if arg_bool(args, "is_data", false) {
        img.colorspace_settings.name = SmolStr::new_static("Non-Color");
        img.colorspace_settings.is_data = true;
    }
    if arg_bool(args, "tiled", false) {
        img.source = SmolStr::new_static("TILED");
    }
    let (main, log) = h.main_mut();
    Ok(main.images.insert(&name, img, log).into())
}
