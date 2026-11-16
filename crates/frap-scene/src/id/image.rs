// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Image` (§5.6): metadata only; decoded pixels are cached outside the snapshot by
//! `Services::images` (keyed by `IdRef` and generation). The field set is frozen at M1 W0
//! (decision log W0-21); WP 1.10 owns the bindings.

use std::sync::Arc;

use frap_anim::AnimData;
use frap_rna::*;
use smol_str::SmolStr;

use crate::generated::plain::ColorManagedInputColorspaceSettings;

/// Encoded (file) bytes of a packed image (`image.packed_file`), as read or embedded in a GLB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedImage {
    /// The encoded file (PNG, JPEG, WebP, …).
    pub bytes: Vec<u8>,
    /// MIME type ("image/png").
    pub mime: SmolStr,
}

/// Image.
#[derive(Clone, Debug)]
pub struct Image {
    /// ID header.
    pub header: IdHeader,
    /// `Image.source` ("FILE", "GENERATED", "VIEWER", …)
    pub source: SmolStr,
    /// `Image.filepath` (as given; `//` is relative to `bpy.data.filepath`)
    pub filepath: String,
    /// `Image.packed_file` (shared bytes, never copied by a COW clone)
    pub packed_file: Option<Arc<EncodedImage>>,
    /// `Image.file_format` ("PNG")
    pub file_format: SmolStr,
    /// `Image.generated_type` ("BLANK", "UV_GRID", "COLOR_GRID")
    pub generated_type: SmolStr,
    /// `Image.generated_width`
    pub generated_width: i32,
    /// `Image.generated_height`
    pub generated_height: i32,
    /// `Image.generated_color` (linear RGBA)
    pub generated_color: [f32; 4],
    /// `Image.use_generated_float`
    pub use_generated_float: bool,
    /// `Image.colorspace_settings`
    pub colorspace_settings: ColorManagedInputColorspaceSettings,
    /// `Image.alpha_mode` ("STRAIGHT", "PREMUL", "CHANNEL_PACKED", "NONE")
    pub alpha_mode: SmolStr,
    /// `Image.size` (pixels; known from the header or the generator, `[0, 0]` if unknown)
    pub size: [i32; 2],
    /// `Image.animation_data`
    pub animation_data: Option<AnimData>,
}

impl Image {
    /// A generated BLANK image with the values of `bpy.data.images.new(name, w, h)` (verified
    /// on 5.2.4: black, alpha 1, sRGB, STRAIGHT).
    pub fn new(name: &str, width: i32, height: i32) -> Image {
        Image {
            header: IdHeader::new(name),
            source: "GENERATED".into(),
            filepath: String::new(),
            packed_file: None,
            file_format: "PNG".into(),
            generated_type: "BLANK".into(),
            generated_width: width,
            generated_height: height,
            generated_color: [0.0, 0.0, 0.0, 1.0],
            use_generated_float: false,
            colorspace_settings: ColorManagedInputColorspaceSettings::default(),
            alpha_mode: "STRAIGHT".into(),
            size: [width, height],
            animation_data: None,
        }
    }
}

crate::id::name_only_rna!(Image, frap_spec_rna::IMAGE, ImageId);

impl IdType for Image {
    type Kind = ImageId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, _from: IdAny, _to: Option<IdAny>) {}
}
