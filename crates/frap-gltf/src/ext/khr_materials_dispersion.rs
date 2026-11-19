// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_dispersion`: typed struct + validation (§17.1). WP 3.6.
//!
//! Chromatic dispersion of a volume (`20 / Abbe number`, default 0). Meaningful only with
//! `KHR_materials_volume`: io_scene_gltf2 drops it otherwise
//! (`blender/exp/material/extensions/dispersion.py`, `export.py::__check_dispersion`).
//! Not required.

use serde::{Deserialize, Serialize};

use super::khr_materials_clearcoat::{check_owner, check_range};
use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras};
use crate::validate::Report;

/// Dispersion.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsDispersion {
    /// Dispersion strength (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispersion: Option<f64>,
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

impl Extension for KhrMaterialsDispersion {
    const NAME: &'static str = "KHR_materials_dispersion";
}

impl KhrMaterialsDispersion {
    /// Effective `dispersion`.
    pub fn dispersion_or_default(&self) -> f64 {
        self.dispersion.unwrap_or(0.0)
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsDispersion>(r) else {
        return;
    };
    check_range(cx, r, "dispersion", e.dispersion, 0.0, f64::MAX);
    if let Some(i) = cx.owner_index_in("materials")
        && !cx.doc.materials[i]
            .extensions
            .contains("KHR_materials_volume")
    {
        r.warning(
            "EXTENSION_DEPENDENCY",
            cx.pointer(),
            "KHR_materials_dispersion has no effect without KHR_materials_volume",
        );
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsDispersion::NAME,
    required_by_default: KhrMaterialsDispersion::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsDispersion {
            dispersion: Some(0.5),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsDispersion::NAME, false);
        let r = validate(&d);
        assert!(r.is_ok(), "{r}");
        assert!(r.has("EXTENSION_DEPENDENCY"), "{r}");
        d.materials[0]
            .extensions
            .insert_raw("KHR_materials_volume", serde_json::json!({}));
        d.use_extension("KHR_materials_volume", false);
        assert!(!validate(&d).has("EXTENSION_DEPENDENCY"));
        d.materials[0].extensions.set(Some(KhrMaterialsDispersion {
            dispersion: Some(-1.0),
            ..Default::default()
        }));
        assert!(validate(&d).has("VALUE_NOT_IN_RANGE"));
    }
}
