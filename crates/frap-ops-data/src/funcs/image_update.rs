// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.update()` (`rna_Image_update`): refreshes the display buffer after `pixels` edits,
//! which frap does not keep (renders read the pixels directly), so it only checks that the
//! image has data: `Error: Image 'X' does not have any image data` otherwise (verified on
//! 5.2.4).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};

use super::image_save::write::{Reports, image};

#[rna_func(srna = "Image", func = "update")]
fn image_update(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    let im = image(h, this)?;
    let mut rep = Reports::default();
    if im.acquire(&PathBase::of(h)).is_none() {
        rep.error(format!(
            "Image '{}' does not have any image data",
            im.header.name
        ));
    }
    rep.finish(h)?;
    Ok(Value::None)
}
