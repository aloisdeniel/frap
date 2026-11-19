// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.packed_files`: the image's packed files (one or none in frap: no views, no UDIM
//! tiles), keyed by nothing (`packed_files[0]`).

use frap_rna::{
    CollRef, Field, Item, PropDef, RnaColl, RnaError, RnaPtr, Seg, StructDef, StructRef, rna_hook,
};
use smol_str::SmolStr;

use super::image_packed_file::{ImagePackedFileRef, image_of};
use crate::HostRead;
use crate::id::Image;

/// The collection proxy.
struct PackedFiles<'h> {
    image: &'h Image,
}

impl RnaColl for PackedFiles<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::IMAGE_PACKED_FILE
    }
    fn len(&self) -> usize {
        usize::from(self.image.packed_file.is_some())
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        match (&self.image.packed_file, i) {
            (Some(file), 0) => Ok(Item::Struct(StructRef::Owned(Box::new(
                ImagePackedFileRef {
                    image: self.image,
                    file,
                },
            )))),
            _ => Err(frap_rna::traits::index_out_of_range(i, self.len())),
        }
    }
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}

#[rna_hook(srna = "Image", prop = "packed_files", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let image = image_of(h, this)?;
    Ok(Field::Coll(CollRef::Owned(Box::new(PackedFiles { image }))))
}
