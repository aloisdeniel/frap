// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.packed_file`: the first packed file (`None` when the image is not packed), a
//! `PackedFile` with its `size` (bytes). Also the read proxies of `PackedFile` and
//! `ImagePackedFile` used by `Image.packed_files`.
//!
//! `PackedFile.data` (Python `bytes`) has no frap value type: reading it answers
//! `NotImplementedError` with a hint (`Image.unpack()` or `ImagePackedFile.save()` write the
//! bytes to a file).

use frap_rna::{
    Field, IdRef, ImageId, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, StructDef, StructRef,
    Value, rna_hook,
};

use crate::HostRead;
use crate::id::paths::{internal, root_id};
use crate::id::{EncodedImage, Image};

/// The image a pointer is rooted at.
pub(crate) fn image_of<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<&'h Image, RnaError> {
    let r: IdRef<ImageId> = IdRef::from_any(root_id(this)?)
        .ok_or_else(|| internal("the pointer is not rooted at an Image"))?;
    h.main()
        .images
        .get(r)
        .ok_or_else(|| internal("the image was removed"))
}

/// `PackedFile` (read proxy).
pub struct PackedFileRef<'h> {
    /// The packed bytes.
    pub file: &'h EncodedImage,
}

impl frap_rna::Rna for PackedFileRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::PACKED_FILE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "size" => Ok(Field::Value(Value::Int(self.file.bytes.len() as i64))),
            "data" => Err(RnaError::new(
                RnaErrorKind::NotImplemented,
                PyExc::NotImplementedError,
                "frap: PackedFile.data (Python bytes) is not readable in frap",
            )
            .with_hint(
                "write the packed bytes to a file with bpy.data.images['…'].unpack(method='WRITE_LOCAL') \
                 or bpy.data.images['…'].packed_files[0].save()",
            )),
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

/// `ImagePackedFile` (read proxy): frap images have at most one packed file (no views, no
/// tiles), view 0, tile 1001.
pub struct ImagePackedFileRef<'h> {
    /// The image.
    pub image: &'h Image,
    /// Its packed bytes.
    pub file: &'h EncodedImage,
}

impl frap_rna::Rna for ImagePackedFileRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::IMAGE_PACKED_FILE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(match prop.identifier {
            "packed_file" => Field::Struct(StructRef::Owned(Box::new(PackedFileRef {
                file: self.file,
            }))),
            "filepath" => Field::Value(Value::Str(self.image.packed_filepath.as_str().into())),
            "view" => Field::Value(Value::Int(0)),
            "tile_number" => Field::Value(Value::Int(1001)),
            _ => return Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        })
    }
}

#[rna_hook(srna = "Image", prop = "packed_file", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let im = image_of(h, this)?;
    Ok(match &im.packed_file {
        Some(f) => Field::Struct(StructRef::Owned(Box::new(PackedFileRef { file: f }))),
        None => Field::Value(Value::None),
    })
}
