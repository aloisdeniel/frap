// SPDX-License-Identifier: GPL-3.0-or-later
//! `ImagePackedFile.save()` (`rna_ImagePackedFile_save` → `BKE_packedfile_write_to_file`):
//! writes the packed bytes to the packed file's `filepath` (`//` relative to the saved file's
//! folder or the working directory), replacing an existing file. Verified on 5.2.4: an image
//! packed from memory has no path and fails with `Error: Error creating file ''` and
//! `Error: Could not save packed file to disk as ''`; success reports INFO
//! `Saved packed file to: <path>`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};

use super::image_save::write::{Reports, image, write_file};

#[rna_func(srna = "ImagePackedFile", func = "save")]
fn image_packed_file_save(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let im = image(h, this)?;
    let mut rep = Reports::default();
    if let Some(p) = &im.packed_file {
        let base = PathBase::of(h);
        let abs = base.abs(&im.packed_filepath);
        match write_file(&base, &abs, &p.bytes) {
            Ok(()) => rep.info(format!("Saved packed file to: {abs}")),
            Err(_) => {
                rep.error(format!("Error creating file '{abs}'"));
                rep.error(format!(
                    "Could not save packed file to disk as '{}'",
                    im.packed_filepath
                ));
            }
        }
    }
    rep.finish(h)?;
    Ok(Value::None)
}
