// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_specular`: typed struct + validation (§17.1). WP 3.6.
//!
//! Dielectric specular strength `specularFactor` (A of `specularTexture`) and colour
//! `specularColorFactor` (RGB of `specularColorTexture`). Not required
//! (`blender/exp/material/extensions/specular.py`).

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

/// Specular parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsSpecular {
    /// Specular strength (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular_factor: Option<f64>,
    /// Strength texture (A channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular_texture: Option<TextureInfo>,
    /// Linear F0 colour (default white; may exceed 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular_color_factor: Option<[f64; 3]>,
    /// Colour texture (sRGB, RGB channels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular_color_texture: Option<TextureInfo>,
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

impl Extension for KhrMaterialsSpecular {
    const NAME: &'static str = "KHR_materials_specular";
}

impl KhrMaterialsSpecular {
    /// Effective `specularFactor`.
    pub fn specular_factor_or_default(&self) -> f64 {
        self.specular_factor.unwrap_or(1.0)
    }
    /// Effective `specularColorFactor`.
    pub fn specular_color_factor_or_default(&self) -> [f64; 3] {
        self.specular_color_factor.unwrap_or([1.0; 3])
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsSpecular>(r) else {
        return;
    };
    check_range(cx, r, "specularFactor", e.specular_factor, 0.0, 1.0);
    for (i, c) in e.specular_color_factor.iter().flatten().enumerate() {
        check_range(
            cx,
            r,
            &format!("specularColorFactor/{i}"),
            Some(*c),
            0.0,
            f64::MAX,
        );
    }
    check_texture(
        cx,
        r,
        "specularTexture",
        e.specular_texture.map(|t| t.index),
    );
    check_texture(
        cx,
        r,
        "specularColorTexture",
        e.specular_color_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsSpecular::NAME,
    required_by_default: KhrMaterialsSpecular::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsSpecular {
            specular_factor: Some(0.5),
            specular_color_factor: Some([1.5, 1.0, 0.5]),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsSpecular::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let e = KhrMaterialsSpecular::default();
        assert_eq!(e.specular_factor_or_default(), 1.0);
        assert_eq!(e.specular_color_factor_or_default(), [1.0; 3]);
        d.materials[0].extensions.set(Some(KhrMaterialsSpecular {
            specular_factor: Some(1.5),
            ..Default::default()
        }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
    }
}
