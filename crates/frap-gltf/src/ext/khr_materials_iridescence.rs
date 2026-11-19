// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_iridescence`: typed struct + validation (§17.1). WP 3.6.
//!
//! A thin-film layer: `iridescenceFactor` (R of `iridescenceTexture`), `iridescenceIor`
//! (default 1.3) and a thickness in nanometres between `iridescenceThicknessMinimum` (100) and
//! `iridescenceThicknessMaximum` (400), interpolated by G of `iridescenceThicknessTexture`.
//! Not required (`blender/exp/material/extensions/iridescence.py`).

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

/// The glTF default thin-film IOR (`GLTF_IRIDESCENCE_IOR`).
pub const DEFAULT_IRIDESCENCE_IOR: f64 = 1.3;
/// The glTF default minimum thickness (nm).
pub const DEFAULT_THICKNESS_MINIMUM: f64 = 100.0;
/// The glTF default maximum thickness (nm).
pub const DEFAULT_THICKNESS_MAXIMUM: f64 = 400.0;

/// Iridescence parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsIridescence {
    /// Iridescence intensity (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iridescence_factor: Option<f64>,
    /// Intensity texture (R channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iridescence_texture: Option<TextureInfo>,
    /// Thin-film index of refraction (default 1.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iridescence_ior: Option<f64>,
    /// Minimum thickness in nm (default 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iridescence_thickness_minimum: Option<f64>,
    /// Maximum thickness in nm (default 400).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iridescence_thickness_maximum: Option<f64>,
    /// Thickness texture (G channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iridescence_thickness_texture: Option<TextureInfo>,
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

impl Extension for KhrMaterialsIridescence {
    const NAME: &'static str = "KHR_materials_iridescence";
}

impl KhrMaterialsIridescence {
    /// Effective `iridescenceFactor`.
    pub fn iridescence_factor_or_default(&self) -> f64 {
        self.iridescence_factor.unwrap_or(0.0)
    }
    /// Effective `iridescenceIor`.
    pub fn iridescence_ior_or_default(&self) -> f64 {
        self.iridescence_ior.unwrap_or(DEFAULT_IRIDESCENCE_IOR)
    }
    /// Effective `iridescenceThicknessMinimum`.
    pub fn iridescence_thickness_minimum_or_default(&self) -> f64 {
        self.iridescence_thickness_minimum
            .unwrap_or(DEFAULT_THICKNESS_MINIMUM)
    }
    /// Effective `iridescenceThicknessMaximum`.
    pub fn iridescence_thickness_maximum_or_default(&self) -> f64 {
        self.iridescence_thickness_maximum
            .unwrap_or(DEFAULT_THICKNESS_MAXIMUM)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsIridescence>(r) else {
        return;
    };
    check_range(cx, r, "iridescenceFactor", e.iridescence_factor, 0.0, 1.0);
    check_range(cx, r, "iridescenceIor", e.iridescence_ior, 1.0, f64::MAX);
    check_range(
        cx,
        r,
        "iridescenceThicknessMinimum",
        e.iridescence_thickness_minimum,
        0.0,
        f64::MAX,
    );
    check_range(
        cx,
        r,
        "iridescenceThicknessMaximum",
        e.iridescence_thickness_maximum,
        0.0,
        f64::MAX,
    );
    check_texture(
        cx,
        r,
        "iridescenceTexture",
        e.iridescence_texture.map(|t| t.index),
    );
    check_texture(
        cx,
        r,
        "iridescenceThicknessTexture",
        e.iridescence_thickness_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsIridescence::NAME,
    required_by_default: KhrMaterialsIridescence::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsIridescence {
            iridescence_factor: Some(1.0),
            iridescence_ior: Some(1.33),
            iridescence_thickness_maximum: Some(500.0),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsIridescence::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let e = KhrMaterialsIridescence::default();
        assert_eq!(e.iridescence_ior_or_default(), 1.3);
        assert_eq!(e.iridescence_thickness_minimum_or_default(), 100.0);
        assert_eq!(e.iridescence_thickness_maximum_or_default(), 400.0);
        d.materials[0].extensions.set(Some(KhrMaterialsIridescence {
            iridescence_ior: Some(0.9),
            ..Default::default()
        }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
    }
}
