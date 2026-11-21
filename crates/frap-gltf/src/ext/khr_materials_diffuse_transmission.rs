// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_diffuse_transmission` (§17.1, WP 7.4).
//!
//! On a material: the diffuse transmission factor (default 0) and colour (default white),
//! each with an optional texture (factor from A, colour from RGB). Written from
//! `LoweredMaterial::exts.diffuse_transmission` with `frap_diffuse_transmission`.
//!
//! Validation (the extension's JSON schema): on a material only; `diffuseTransmissionFactor`
//! and each `diffuseTransmissionColorFactor` component in `[0, 1]` (VALUE_NOT_IN_RANGE), the
//! colour factor exactly 3 numbers (decoding), texture references resolved.

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
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
    let Some(e) = cx.decode::<KhrMaterialsDiffuseTransmission>(r) else {
        return;
    };
    check_range(
        cx,
        r,
        "diffuseTransmissionFactor",
        e.diffuse_transmission_factor,
        0.0,
        1.0,
    );
    for (i, c) in e
        .diffuse_transmission_color_factor
        .iter()
        .flatten()
        .enumerate()
    {
        check_range(
            cx,
            r,
            &format!("diffuseTransmissionColorFactor/{i}"),
            Some(*c),
            0.0,
            1.0,
        );
    }
    check_texture(
        cx,
        r,
        "diffuseTransmissionTexture",
        e.diffuse_transmission_texture.map(|t| t.index),
    );
    check_texture(
        cx,
        r,
        "diffuseTransmissionColorTexture",
        e.diffuse_transmission_color_texture.map(|t| t.index),
    );
}

impl KhrMaterialsDiffuseTransmission {
    /// Effective `diffuseTransmissionFactor`.
    pub fn diffuse_transmission_factor_or_default(&self) -> f64 {
        self.diffuse_transmission_factor.unwrap_or(0.0)
    }

    /// Effective `diffuseTransmissionColorFactor`.
    pub fn diffuse_transmission_color_factor_or_default(&self) -> [f64; 3] {
        self.diffuse_transmission_color_factor.unwrap_or([1.0; 3])
    }
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
        let d = KhrMaterialsDiffuseTransmission::default();
        assert_eq!(d.diffuse_transmission_factor_or_default(), 0.0);
        assert_eq!(d.diffuse_transmission_color_factor_or_default(), [1.0; 3]);
    }

    #[test]
    fn validation_rules() {
        use crate::json::{Document, Material, Node};
        use crate::validate::validate;
        let mut d = Document::default();
        let mut m = Material::default();
        m.extensions.set(Some(KhrMaterialsDiffuseTransmission {
            diffuse_transmission_factor: Some(0.25),
            diffuse_transmission_color_factor: Some([1.0, 0.5, 0.0]),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        d.materials[0]
            .extensions
            .set(Some(KhrMaterialsDiffuseTransmission {
                diffuse_transmission_factor: Some(1.5),
                diffuse_transmission_color_factor: Some([1.0, -0.5, 0.0]),
                diffuse_transmission_color_texture: Some(TextureInfo::new(2)),
                ..Default::default()
            }));
        let r = validate(&d);
        let at = |p: &str| r.issues.iter().any(|i| i.pointer.ends_with(p));
        assert!(at("diffuseTransmissionFactor"), "{r}");
        assert!(at("diffuseTransmissionColorFactor/1"), "{r}");
        assert!(at("diffuseTransmissionColorTexture/index"), "{r}");
        // A malformed colour (2 numbers) and the wrong owner.
        d.materials[0].extensions.insert_raw(
            NAME,
            serde_json::json!({"diffuseTransmissionColorFactor": [1, 1]}),
        );
        assert!(validate(&d).has("EXTENSION_INVALID"));
        let mut d = Document::default();
        let mut n = Node::default();
        n.extensions
            .set(Some(KhrMaterialsDiffuseTransmission::default()));
        d.nodes.push(n);
        d.use_extension(NAME, false);
        assert!(validate(&d).has("EXTENSION_INVALID_LOCATION"));
    }
}
