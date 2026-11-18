// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Texture` (M4: what the DISPLACE modifier reads, OPERATORS.md §D "Texture,
//! ImageTexture"). The field set is frozen at M4 W0 (decision log M34W0): the `type` and the
//! generated union of the `Texture` and `ImageTexture` fields (`TextureFields`, as `Light`
//! stores `LightFields`), so `.frap` round-trips every value.
//!
//! The binding (WP 4.2) is refined by `type` like `Light`'s: `'IMAGE'` is an `ImageTexture`
//! (its `image`, `extension`, crop, repeat and alpha options), every other type the `Texture`
//! base (verified on 5.2.4: `'NONE'` is `Texture`; `bpy.data.textures['N'].image` raises
//! `AttributeError: 'Texture' object has no attribute 'image'`). Changing `type` re-types the
//! struct and keeps every stored value (the union). The procedural types (`'CLOUDS'`, …) exist
//! as data with the `Texture` base properties only: frap does not model their own settings
//! (`noise_scale`, …) and DISPLACE does not evaluate them. Read-only pointers Blender leaves
//! unset on a new texture read `None` (`color_ramp` without `use_color_ramp`, `node_tree`,
//! `animation_data`); `image_user` is not modelled (`RNA_NOT_IMPLEMENTED`).

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

    /// The RNA struct of a texture type: `ImageTexture` for `'IMAGE'`, `Texture` for `'NONE'`,
    /// and for a procedural type its refined struct without its own properties (frap does not
    /// model them; errors still name `CloudsTexture`, … as Blender does).
    pub fn srna_of(ty: &str) -> &'static StructDef {
        match ty {
            "IMAGE" => &frap_spec_rna::IMAGE_TEXTURE,
            "BLEND" => &procedural::BLEND,
            "CLOUDS" => &procedural::CLOUDS,
            "DISTORTED_NOISE" => &procedural::DISTORTED_NOISE,
            "MAGIC" => &procedural::MAGIC,
            "MARBLE" => &procedural::MARBLE,
            "MUSGRAVE" => &procedural::MUSGRAVE,
            "NOISE" => &procedural::NOISE,
            "STUCCI" => &procedural::STUCCI,
            "VORONOI" => &procedural::VORONOI,
            "WOOD" => &procedural::WOOD,
            _ => &frap_spec_rna::TEXTURE,
        }
    }

    fn defines(&self, prop: &PropDef) -> bool {
        Texture::srna_of(&self.r#type)
            .find_prop(prop.identifier)
            .is_some()
    }
}

/// The procedural texture structs (identifiers verified on 5.2.4), on the `Texture` base.
mod procedural {
    use frap_rna::StructDef;

    macro_rules! procedural {
        ($($c:ident = $id:literal),* $(,)?) => {
            $(
                pub(super) static $c: StructDef = StructDef {
                    identifier: $id,
                    base: Some(&frap_spec_rna::TEXTURE),
                    name: $id,
                    description: "",
                    props: &[],
                    funcs: &[],
                    name_property: Some("name"),
                    is_id: true,
                };
            )*
        };
    }

    procedural!(
        BLEND = "BlendTexture",
        CLOUDS = "CloudsTexture",
        DISTORTED_NOISE = "DistortedNoiseTexture",
        MAGIC = "MagicTexture",
        MARBLE = "MarbleTexture",
        MUSGRAVE = "MusgraveTexture",
        NOISE = "NoiseTexture",
        STUCCI = "StucciTexture",
        VORONOI = "VoronoiTexture",
        WOOD = "WoodTexture",
    );
}

impl Rna for Texture {
    fn srna(&self) -> &'static StructDef {
        Texture::srna_of(&self.r#type)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        if let Some(r) = super::id_common_get(&self.header, "TEXTURE", prop) {
            return r;
        }
        let srna = self.srna();
        match prop.identifier {
            "type" => return Ok(Field::Value(Value::Enum(self.r#type.clone()))),
            // Node-based textures and colour ramps are not modelled: Blender's values for a
            // texture that uses neither.
            "node_tree" => return Ok(Field::Id(None)),
            "color_ramp" | "animation_data" => return Ok(Field::Value(Value::None)),
            "image_user" if self.defines(prop) => {
                return Err(RnaError::new(
                    RnaErrorKind::NotImplemented,
                    PyExc::NotImplementedError,
                    format!("frap: {}.image_user is not modelled", srna.identifier),
                ));
            }
            _ => {}
        }
        if self.defines(prop)
            && let Some(f) = self.fields.rna_get(prop)
        {
            return Ok(f);
        }
        Err(super::no_attr(srna, prop))
    }
}

impl RnaMut for Texture {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        if let Some(r) = super::id_common_set(&mut self.header, srna, prop, &v) {
            return r;
        }
        match prop.identifier {
            "type" => {
                // `rna_Texture_type_set` → `BKE_texture_type_set`: the refined struct follows.
                self.r#type = v.as_str().map(SmolStr::new).unwrap_or(self.r#type.clone());
                return Ok(SetOutcome::Done);
            }
            "node_tree" | "color_ramp" | "animation_data" | "image_user"
                if self.defines(prop) =>
            {
                return Err(frap_rna::traits::read_only(srna, prop.identifier));
            }
            _ => {}
        }
        if self.defines(prop)
            && let Some(r) = self.fields.rna_set(prop, v)
        {
            return r;
        }
        Err(super::no_attr(srna, prop))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        self.fields
            .rna_struct_mut(prop)
            .ok_or_else(|| frap_rna::traits::wrong_access(srna, prop.identifier, "a struct"))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(srna: &'static StructDef, name: &str) -> &'static PropDef {
        srna.find_prop(name).expect("property exists")
    }

    #[test]
    fn type_refines_the_struct() {
        let mut t = Texture::with_type("T", "IMAGE");
        assert_eq!(t.srna().identifier, "ImageTexture");
        let ext = prop(&frap_spec_rna::IMAGE_TEXTURE, "extension");
        assert!(matches!(t.get(ext), Ok(Field::Value(Value::Enum(e))) if e == "REPEAT"));
        t.set(ext, Value::Enum("CLIP".into())).unwrap();
        t.set(prop(&frap_spec_rna::TEXTURE, "type"), Value::Enum("NONE".into()))
            .unwrap();
        assert_eq!(t.srna().identifier, "Texture");
        let Err(e) = t.get(ext) else {
            panic!("a NONE texture has no extension")
        };
        assert_eq!(e.message, "'Texture' object has no attribute 'extension'");
        // Re-typing keeps the stored value (the union).
        t.set(prop(&frap_spec_rna::TEXTURE, "type"), Value::Enum("IMAGE".into()))
            .unwrap();
        assert!(matches!(t.get(ext), Ok(Field::Value(Value::Enum(e))) if e == "CLIP"));
    }

    #[test]
    fn unmodelled_pointers() {
        let mut t = Texture::new("T");
        assert!(matches!(
            t.get(prop(&frap_spec_rna::TEXTURE, "color_ramp")),
            Ok(Field::Value(Value::None))
        ));
        assert!(matches!(
            t.get(prop(&frap_spec_rna::TEXTURE, "node_tree")),
            Ok(Field::Id(None))
        ));
        let e = t
            .set(prop(&frap_spec_rna::TEXTURE, "node_tree"), Value::None)
            .unwrap_err();
        assert_eq!(e.kind, RnaErrorKind::ReadOnly);
        let Err(e) = t.get(prop(&frap_spec_rna::IMAGE_TEXTURE, "image_user")) else {
            panic!("image_user is not modelled")
        };
        assert_eq!(e.kind, RnaErrorKind::NotImplemented);
    }
}
