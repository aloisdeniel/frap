// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_volume`: typed struct + validation (§17.1). WP 3.6.
//!
//! A volume below a transmissive surface: `thicknessFactor` (G of `thicknessTexture`),
//! `attenuationDistance` (default +∞, so absent) and `attenuationColor`. Not required
//! (`blender/exp/material/extensions/volume.py`).

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

/// Volume parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsVolume {
    /// Thickness in mesh space (default 0: thin-walled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness_factor: Option<f64>,
    /// Thickness texture (G channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness_texture: Option<TextureInfo>,
    /// Distance light travels before reaching `attenuationColor` (default +∞).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attenuation_distance: Option<f64>,
    /// Colour white light turns into at `attenuationDistance` (default white).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attenuation_color: Option<[f64; 3]>,
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

impl Extension for KhrMaterialsVolume {
    const NAME: &'static str = "KHR_materials_volume";
}

impl KhrMaterialsVolume {
    /// Effective `thicknessFactor`.
    pub fn thickness_factor_or_default(&self) -> f64 {
        self.thickness_factor.unwrap_or(0.0)
    }
    /// Effective `attenuationDistance` (+∞ when absent).
    pub fn attenuation_distance_or_default(&self) -> f64 {
        self.attenuation_distance.unwrap_or(f64::INFINITY)
    }
    /// Effective `attenuationColor`.
    pub fn attenuation_color_or_default(&self) -> [f64; 3] {
        self.attenuation_color.unwrap_or([1.0; 3])
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsVolume>(r) else {
        return;
    };
    check_range(cx, r, "thicknessFactor", e.thickness_factor, 0.0, f64::MAX);
    if let Some(d) = e.attenuation_distance
        && !(d.is_finite() && d > 0.0)
    {
        r.error(
            "VALUE_NOT_IN_RANGE",
            cx.member("attenuationDistance"),
            format!("attenuationDistance {d} must be > 0"),
        );
    }
    for (i, c) in e.attenuation_color.iter().flatten().enumerate() {
        check_range(cx, r, &format!("attenuationColor/{i}"), Some(*c), 0.0, 1.0);
    }
    check_texture(
        cx,
        r,
        "thicknessTexture",
        e.thickness_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsVolume::NAME,
    required_by_default: KhrMaterialsVolume::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsVolume {
            thickness_factor: Some(0.5),
            attenuation_distance: Some(2.0),
            attenuation_color: Some([1.0, 0.5, 0.25]),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsVolume::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let e = KhrMaterialsVolume::default();
        assert_eq!(e.attenuation_distance_or_default(), f64::INFINITY);
        assert_eq!(e.attenuation_color_or_default(), [1.0; 3]);
        d.materials[0].extensions.set(Some(KhrMaterialsVolume {
            attenuation_distance: Some(0.0),
            ..Default::default()
        }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
    }
}
