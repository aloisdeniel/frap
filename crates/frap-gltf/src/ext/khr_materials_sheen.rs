// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_sheen`: typed struct + validation (§17.1). WP 3.6.
//!
//! A sheen layer (cloth): `sheenColorFactor` (RGB of `sheenColorTexture`) and
//! `sheenRoughnessFactor` (A of `sheenRoughnessTexture`). Not required
//! (`blender/exp/material/extensions/sheen.py`).

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

/// Sheen parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsSheen {
    /// Linear sheen colour (default black).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheen_color_factor: Option<[f64; 3]>,
    /// Colour texture (sRGB, RGB channels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheen_color_texture: Option<TextureInfo>,
    /// Sheen roughness (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheen_roughness_factor: Option<f64>,
    /// Roughness texture (A channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheen_roughness_texture: Option<TextureInfo>,
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

impl Extension for KhrMaterialsSheen {
    const NAME: &'static str = "KHR_materials_sheen";
}

impl KhrMaterialsSheen {
    /// Effective `sheenColorFactor`.
    pub fn sheen_color_factor_or_default(&self) -> [f64; 3] {
        self.sheen_color_factor.unwrap_or([0.0; 3])
    }
    /// Effective `sheenRoughnessFactor`.
    pub fn sheen_roughness_factor_or_default(&self) -> f64 {
        self.sheen_roughness_factor.unwrap_or(0.0)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsSheen>(r) else {
        return;
    };
    for (i, c) in e.sheen_color_factor.iter().flatten().enumerate() {
        check_range(cx, r, &format!("sheenColorFactor/{i}"), Some(*c), 0.0, 1.0);
    }
    check_range(
        cx,
        r,
        "sheenRoughnessFactor",
        e.sheen_roughness_factor,
        0.0,
        1.0,
    );
    check_texture(
        cx,
        r,
        "sheenColorTexture",
        e.sheen_color_texture.map(|t| t.index),
    );
    check_texture(
        cx,
        r,
        "sheenRoughnessTexture",
        e.sheen_roughness_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsSheen::NAME,
    required_by_default: KhrMaterialsSheen::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsSheen {
            sheen_color_factor: Some([1.0, 0.5, 0.25]),
            sheen_roughness_factor: Some(0.5),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsSheen::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let json = String::from_utf8(d.to_vec(&Default::default())).unwrap();
        assert!(
            json.contains(r#""sheenColorFactor":[1,0.5,0.25],"sheenRoughnessFactor":0.5"#),
            "{json}"
        );
        assert_eq!(
            KhrMaterialsSheen::default().sheen_color_factor_or_default(),
            [0.0; 3]
        );
        d.materials[0].extensions.set(Some(KhrMaterialsSheen {
            sheen_color_factor: Some([1.5, 0.0, 0.0]),
            ..Default::default()
        }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
    }
}
