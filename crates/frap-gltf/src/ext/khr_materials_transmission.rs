// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_transmission`: typed struct + validation (§17.1). WP 3.6.
//!
//! Optical transmission: `transmissionFactor` (R of `transmissionTexture`). Not required
//! (`blender/exp/material/extensions/transmission.py`).

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

/// Transmission parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsTransmission {
    /// Fraction of light transmitted (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transmission_factor: Option<f64>,
    /// Transmission texture (R channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transmission_texture: Option<TextureInfo>,
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

impl Extension for KhrMaterialsTransmission {
    const NAME: &'static str = "KHR_materials_transmission";
}

impl KhrMaterialsTransmission {
    /// Effective `transmissionFactor`.
    pub fn transmission_factor_or_default(&self) -> f64 {
        self.transmission_factor.unwrap_or(0.0)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsTransmission>(r) else {
        return;
    };
    check_range(cx, r, "transmissionFactor", e.transmission_factor, 0.0, 1.0);
    check_texture(
        cx,
        r,
        "transmissionTexture",
        e.transmission_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsTransmission::NAME,
    required_by_default: KhrMaterialsTransmission::REQUIRED_BY_DEFAULT,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{Document, Material};
    use crate::validate::validate;

    #[test]
    fn typed_access_and_validation() {
        let mut d = Document::default();
        let mut m = Material::default();
        m.extensions.set(Some(KhrMaterialsTransmission {
            transmission_factor: Some(0.75),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsTransmission::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let e: KhrMaterialsTransmission = d.materials[0].extensions.get().unwrap().unwrap();
        assert_eq!(e.transmission_factor_or_default(), 0.75);
        d.materials[0]
            .extensions
            .set(Some(KhrMaterialsTransmission {
                transmission_factor: Some(-0.5),
                ..Default::default()
            }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
    }
}
