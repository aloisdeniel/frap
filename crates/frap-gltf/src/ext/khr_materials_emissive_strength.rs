// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_emissive_strength`: typed struct + validation (§17.1). WP 1.5.
//!
//! Scales `emissiveFactor` beyond 1. io_scene_gltf2 writes it as not required
//! (`blender/exp/material/extensions/emission.py`: `Extension(..., False)`).

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras};
use crate::validate::Report;

/// Emissive strength multiplier.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsEmissiveStrength {
    /// Multiplier of `emissiveFactor` (default 1, ≥ 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_strength: Option<f64>,
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

impl Extension for KhrMaterialsEmissiveStrength {
    const NAME: &'static str = "KHR_materials_emissive_strength";
}

impl KhrMaterialsEmissiveStrength {
    /// Effective strength.
    pub fn emissive_strength_or_default(&self) -> f64 {
        self.emissive_strength.unwrap_or(1.0)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("materials").is_none() {
        cx.wrong_location(r, "a material");
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsEmissiveStrength>(r) else {
        return;
    };
    if let Some(s) = e.emissive_strength
        && (s < 0.0 || !s.is_finite())
    {
        r.error(
            "VALUE_NOT_IN_RANGE",
            cx.member("emissiveStrength"),
            format!("emissiveStrength {s} must be ≥ 0"),
        );
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsEmissiveStrength::NAME,
    required_by_default: KhrMaterialsEmissiveStrength::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsEmissiveStrength {
            emissive_strength: Some(5.0),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(
            KhrMaterialsEmissiveStrength::NAME,
            KhrMaterialsEmissiveStrength::REQUIRED_BY_DEFAULT,
        );
        assert!(d.extensions_required.is_empty());
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let json = String::from_utf8(d.to_vec(&Default::default())).unwrap();
        assert!(
            json.contains(r#"{"KHR_materials_emissive_strength":{"emissiveStrength":5}}"#),
            "{json}"
        );
        let e: KhrMaterialsEmissiveStrength = d.materials[0].extensions.get().unwrap().unwrap();
        assert_eq!(e.emissive_strength_or_default(), 5.0);
        d.materials[0]
            .extensions
            .set(Some(KhrMaterialsEmissiveStrength {
                emissive_strength: Some(-1.0),
                ..Default::default()
            }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
        d.materials[0]
            .extensions
            .set::<KhrMaterialsEmissiveStrength>(None);
        assert!(d.materials[0].extensions.is_empty());
        assert!(validate(&d).has("UNUSED_EXTENSION"));
    }
}
