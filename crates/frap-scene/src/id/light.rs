// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Light` (§5.6: generated plain fields per light type; assigning `type` re-types
//! the RNA struct to `PointLight`/`SunLight`/`SpotLight`/`AreaLight`). The field set is frozen
//! at M1 W0 (decision log W0-21); WP 1.10 owns the bindings.
//!
//! `LightFields` is the union of every light type's fields, as in Blender's DNA, so changing
//! `light.type` keeps every value (verified: `energy` survives a POINT → SPOT → POINT round
//! trip). The binding is hand-written because the struct is refined by `type`: a property is
//! visible only when the refined struct (or `Light`/`ID`) defines it, exactly as in Blender
//! (`AttributeError: 'PointLight' object has no attribute 'spot_size'`).

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
    /// The RNA struct of a light type (`PointLight`, …; `Light` for an unknown type).
    pub fn srna_of(ty: &str) -> &'static StructDef {
        match ty {
            "POINT" => &frap_spec_rna::POINT_LIGHT,
            "SUN" => &frap_spec_rna::SUN_LIGHT,
            "SPOT" => &frap_spec_rna::SPOT_LIGHT,
            "AREA" => &frap_spec_rna::AREA_LIGHT,
            _ => &frap_spec_rna::LIGHT,
        }
    }
    fn defines(&self, prop: &PropDef) -> bool {
        Light::srna_of(&self.r#type)
            .find_prop(prop.identifier)
            .is_some()
    }
}

impl Rna for Light {
    fn srna(&self) -> &'static StructDef {
        Light::srna_of(&self.r#type)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        if let Some(r) = super::id_common_get(&self.header, "LIGHT", prop) {
            return r;
        }
        match prop.identifier {
            "type" => return Ok(Field::Value(Value::Enum(self.r#type.clone()))),
            // Light node trees (Cycles light shaders) are not modelled.
            "node_tree" => return Ok(Field::Id(None)),
            _ => {}
        }
        if self.defines(prop)
            && let Some(f) = self.fields.rna_get(prop)
        {
            return Ok(f);
        }
        Err(super::no_attr(self.srna(), prop))
    }
}

impl RnaMut for Light {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        if let Some(r) = super::id_common_set(&mut self.header, srna, prop, &v) {
            return r;
        }
        match prop.identifier {
            "type" => {
                self.r#type = v.as_str().map(SmolStr::new).unwrap_or(self.r#type.clone());
                return Ok(SetOutcome::Done);
            }
            "node_tree" | "temperature_color" => {
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
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        super::remap_action(&mut self.animation_data, from, to);
    }
}
