// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Texture` (M4: what the DISPLACE modifier reads, OPERATORS.md §D "Texture,
//! ImageTexture"). The field set is frozen at M4 W0 (decision log M34W0): the `type` and the
//! generated union of the `Texture` and `ImageTexture` fields (`TextureFields`, as `Light`
//! stores `LightFields`), so `.frap` round-trips every value. WP 4.2 owns this file after W0 and
//! writes the refined RNA binding (`ImageTexture` for `type == 'IMAGE'`); until then only
//! `ID.name` is bound.

use frap_anim::AnimData;
use frap_rna::*;
use smol_str::SmolStr;

use crate::generated::plain::TextureFields;

/// Texture datablock (`bpy.data.textures`).
#[derive(Clone, Debug)]
pub struct Texture {
    /// ID header.
    pub header: IdHeader,
    /// `Texture.type` (`'IMAGE'`, `'NONE'`, `'CLOUDS'`, …).
    pub r#type: SmolStr,
    /// The plain RNA fields of `Texture` and `ImageTexture` (generated); `fields.image` is the
    /// `ImageTexture.image` pointer (a user of the image).
    pub fields: TextureFields,
    /// `Texture.animation_data`
    pub animation_data: Option<AnimData>,
}

impl Texture {
    /// A texture with the fresh values of `bpy.data.textures.new(name, type='IMAGE')`.
    pub fn new(name: impl Into<SmolStr>) -> Texture {
        Texture::with_type(name, "IMAGE")
    }

    /// A texture of `type` (`bpy.data.textures.new(name, type)`).
    pub fn with_type(name: impl Into<SmolStr>, r#type: &str) -> Texture {
        Texture {
            header: IdHeader::new(name),
            r#type: r#type.into(),
            fields: TextureFields::default(),
            animation_data: None,
        }
    }
}

crate::id::name_only_rna!(Texture, frap_spec_rna::TEXTURE, TextureId);

impl IdType for Texture {
    type Kind = TextureId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(im) = self.fields.image {
            f(im);
        }
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        if self.fields.image == Some(from) {
            self.fields.image = to;
        }
        crate::id::remap_action(&mut self.animation_data, from, to);
    }
}
