// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_ior`: typed struct + validation (§17.1). WP 3.6.
//!
//! The index of refraction (default 1.5; 0 means infinite, as for metals). io_scene_gltf2
//! keeps it only with `KHR_materials_transmission`, `KHR_materials_volume` or
//! `KHR_materials_specular` [GLTF §4.5] (`blender/exp/material/extensions/ior.py`,
//! `export.py::__check_ior`). Not required.

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::check_owner;
use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras};
use crate::validate::Report;

/// The glTF default index of refraction (`GLTF_IOR` in `io/com/constants.py`).
pub const DEFAULT_IOR: f64 = 1.5;

/// Index of refraction.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsIor {
    /// Index of refraction (default 1.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ior: Option<f64>,
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

impl Extension for KhrMaterialsIor {
    const NAME: &'static str = "KHR_materials_ior";
}

impl KhrMaterialsIor {
    /// Effective `ior`.
    pub fn ior_or_default(&self) -> f64 {
        self.ior.unwrap_or(DEFAULT_IOR)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsIor>(r) else {
        return;
    };
    if let Some(v) = e.ior
        && !(v.is_finite() && (v == 0.0 || v >= 1.0))
    {
        r.error(
            "VALUE_NOT_IN_RANGE",
            cx.member("ior"),
            format!("ior {v} must be 0 or ≥ 1"),
        );
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsIor::NAME,
    required_by_default: KhrMaterialsIor::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsIor {
            ior: Some(1.45),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsIor::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        assert_eq!(KhrMaterialsIor::default().ior_or_default(), 1.5);
        for bad in [0.5, -1.0] {
            d.materials[0].extensions.set(Some(KhrMaterialsIor {
                ior: Some(bad),
                ..Default::default()
            }));
            assert!(validate(&d).has("VALUE_NOT_IN_RANGE"), "{bad}");
        }
        d.materials[0].extensions.set(Some(KhrMaterialsIor {
            ior: Some(0.0),
            ..Default::default()
        }));
        assert!(validate(&d).is_ok());
    }
}
