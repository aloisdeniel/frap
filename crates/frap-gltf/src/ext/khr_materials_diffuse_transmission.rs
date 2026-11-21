// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_diffuse_transmission` (§17.1). WP 7.4 (M7 W0 stub: the typed model).
//!
//! On a material: the diffuse transmission factor (default 0) and colour (default white),
//! each with an optional texture (factor from A, colour from RGB). Written from
//! `LoweredMaterial::exts.diffuse_transmission` with `frap_diffuse_transmission`.

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::check_owner;
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

const NAME: &str = "KHR_materials_diffuse_transmission";
const REQUIRED: bool = false;

/// The extension on a material.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsDiffuseTransmission {
    /// Fraction of light transmitted diffusely (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse_transmission_factor: Option<f64>,
    /// Texture whose A channel multiplies the factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse_transmission_texture: Option<TextureInfo>,
    /// Colour of the transmitted light (default `[1, 1, 1]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse_transmission_color_factor: Option<[f64; 3]>,
    /// Texture whose RGB channels multiply the colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse_transmission_color_texture: Option<TextureInfo>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
    pub extras: Option<Extras>,
}

impl Extension for KhrMaterialsDiffuseTransmission {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let _ = cx.decode::<KhrMaterialsDiffuseTransmission>(r);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: NAME,
    required_by_default: REQUIRED,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_round_trip() {
        let v = serde_json::json!({"diffuseTransmissionFactor": 0.5,
            "diffuseTransmissionColorFactor": [1.0, 0.5, 0.25]});
        let e: KhrMaterialsDiffuseTransmission = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.diffuse_transmission_factor, Some(0.5));
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }
}
