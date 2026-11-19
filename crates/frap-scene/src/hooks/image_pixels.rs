// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.pixels` (`rna_Image_pixels_get/set`): the buffer's samples, four per pixel, bottom
//! row first. A byte buffer reads `byte / 255` and stores `unit_float_to_uchar_clamp(v)`; a
//! float buffer stores the values as given. Writing marks the image dirty (`is_dirty`) and
//! keeps the buffer in the datablock (frap's model, see `id/image.rs`).
//!
//! Verified on 5.2.4: a 4 × 2 `images.new` image has 32 values; writing
//! `pixels[0:4] = (0.5, 0.25, 1.2, -0.1)` reads back `(0.50196, 0.25098, 1.0, 0.0)`;
//! assigning 3 values raises `ValueError: bpy_struct: item.attr = val: Image.pixels: array
//! length cannot be changed to 3 (expected 32)`; an image without data has no pixels (`[]`)
//! and ignores writes.
//!
//! Limits: reading checks the array against the statement's budget first (`LIMIT_EXCEEDED`);
//! responses cap long arrays at `--max-array` values (`$array` with `range`/`truncated`, read
//! further with slices such as `pixels[65536:131072]`), also the echo of an assignment.

use std::sync::Arc;

use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{
    Field, IdRef, ImageId, MathTag, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value,
    rna_hook,
};

use super::image_packed_file::image_of;
use crate::id::paths::{internal, root_id};
use crate::{Host, HostRead};

#[rna_hook(srna = "Image", prop = "pixels", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let im = image_of(h, this)?;
        let base = crate::id::PathBase::of(h);
        // The float array is checked against the statement's budget before the pixels are
        // loaded or generated (a byte buffer reads as four times its size).
        if let Some(i) = im.info(&base) {
            frap_math::budget::check_buffer(
                crate::id::imbuf::BUFFER,
                crate::id::imbuf::buffer_bytes(i.x, i.y, true),
            );
        }
        let px = im
            .acquire(&base)
            .map(|b| b.pixels())
            .unwrap_or_default();
        Ok(Field::Value(px.to_rna_value(prop, MathTag::None)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let values = Vec::<f32>::from_rna_value(prop, v)?;
        let r: IdRef<ImageId> = IdRef::from_any(root_id(this)?)
            .ok_or_else(|| internal("the pointer is not rooted at an Image"))?;
        let base = crate::id::PathBase::of(h);
        let im = h
            .main()
            .images
            .get(r)
            .ok_or_else(|| internal("the image was removed"))?;
        let Some(mut buf) = im.acquire(&base) else {
            if values.is_empty() {
                return Ok(());
            }
            return Err(length_error(values.len(), 0));
        };
        if values.len() != buf.len() {
            return Err(length_error(values.len(), buf.len()));
        }
        buf.set_pixels(&values);
        let (main, log) = h.main_mut();
        if let Some(im) = main.images.get_mut(r, log) {
            im.buffer = Some(Arc::new(buf));
        }
        Ok(())
    }
}

/// Python's message for a dynamic array written with another length.
fn length_error(got: usize, expected: usize) -> RnaError {
    RnaError::new(
        RnaErrorKind::Length,
        PyExc::ValueError,
        format!(
            "bpy_struct: item.attr = val: Image.pixels: array length cannot be changed to {got} \
             (expected {expected})"
        ),
    )
}
