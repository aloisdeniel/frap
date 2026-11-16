// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Light` (§5.6: generated plain fields per light type; assigning `type` re-types
//! the RNA struct to `PointLight`/`SunLight`/`SpotLight`/`AreaLight`). The field set is frozen
//! at M1 W0 (decision log W0-21); WP 1.10 owns the bindings.

use frap_anim::AnimData;
use frap_rna::*;
use smol_str::SmolStr;

use crate::generated::plain::LightFields;

/// Light data.
#[derive(Clone, Debug)]
pub struct Light {
    /// ID header.
    pub header: IdHeader,
    /// `Light.type` ("POINT", "SUN", "SPOT", "AREA")
    pub r#type: SmolStr,
    /// The plain RNA fields of every light type (union, as in Blender's DNA; generated,
    /// WP 1.1).
    pub fields: LightFields,
    /// `Light.animation_data`
    pub animation_data: Option<AnimData>,
}

impl Light {
    /// Light data with the fresh values of `bpy.data.lights.new(name, type)` (10 W,
    /// `shadow_soft_size` 0, verified; the factory Light is 1000 W).
    pub fn new(name: &str, r#type: &str) -> Light {
        Light {
            header: IdHeader::new(name),
            r#type: r#type.into(),
            fields: LightFields::default(),
            animation_data: None,
        }
    }
}

crate::id::name_only_rna!(Light, frap_spec_rna::LIGHT, LightId);

impl IdType for Light {
    type Kind = LightId;
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
