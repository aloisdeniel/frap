// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_anisotropy`: typed struct + validation (§17.1). WP 3.6.
//!
//! Anisotropic specular: `anisotropyStrength`, `anisotropyRotation` (radians, counter-clockwise
//! from the tangent) and `anisotropyTexture` (RG direction in [-1, 1], B strength). Not required
//! (`blender/exp/material/extensions/anisotropy.py`).

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range, check_texture};
use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, TextureInfo,
};
use crate::validate::Report;

/// Anisotropy parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsAnisotropy {
    /// Strength (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anisotropy_strength: Option<f64>,
    /// Rotation in radians (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anisotropy_rotation: Option<f64>,
    /// Direction (RG) and strength (B) texture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anisotropy_texture: Option<TextureInfo>,
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

impl Extension for KhrMaterialsAnisotropy {
    const NAME: &'static str = "KHR_materials_anisotropy";
}

impl KhrMaterialsAnisotropy {
    /// Effective `anisotropyStrength`.
    pub fn anisotropy_strength_or_default(&self) -> f64 {
        self.anisotropy_strength.unwrap_or(0.0)
    }
    /// Effective `anisotropyRotation`.
    pub fn anisotropy_rotation_or_default(&self) -> f64 {
        self.anisotropy_rotation.unwrap_or(0.0)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsAnisotropy>(r) else {
        return;
    };
    check_range(cx, r, "anisotropyStrength", e.anisotropy_strength, 0.0, 1.0);
    check_range(
        cx,
        r,
        "anisotropyRotation",
        e.anisotropy_rotation,
        f64::MIN,
        f64::MAX,
    );
    check_texture(
        cx,
        r,
        "anisotropyTexture",
        e.anisotropy_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsAnisotropy::NAME,
    required_by_default: KhrMaterialsAnisotropy::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsAnisotropy {
            anisotropy_strength: Some(0.5),
            anisotropy_rotation: Some(1.25),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsAnisotropy::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        assert_eq!(
            KhrMaterialsAnisotropy::default().anisotropy_rotation_or_default(),
            0.0
        );
        d.materials[0].extensions.set(Some(KhrMaterialsAnisotropy {
            anisotropy_strength: Some(1.5),
            anisotropy_texture: Some(TextureInfo::new(0)),
            ..Default::default()
        }));
        let r = validate(&d);
        assert!(
            r.has("VALUE_NOT_IN_RANGE") && r.has("UNRESOLVED_REFERENCE"),
            "{r}"
        );
    }
}
