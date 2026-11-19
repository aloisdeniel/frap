// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_clearcoat`: typed struct + validation (§17.1). WP 3.6.
//!
//! A clear coat layer over the base material: `clearcoatFactor` (R of `clearcoatTexture`),
//! `clearcoatRoughnessFactor` (G of `clearcoatRoughnessTexture`) and its own
//! `clearcoatNormalTexture`. io_scene_gltf2 writes it as not required
//! (`blender/exp/material/extensions/clearcoat.py`: `Extension(..., False)`).
//!
//! The material-extension validation helpers shared by the `khr_materials_*` files live here
//! (`check_owner`, `check_texture`, `check_range`).

use serde::{Deserialize, Serialize};

use crate::json::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, NormalTextureInfo,
    TextureInfo,
};
use crate::validate::Report;

/// Clear coat parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsClearcoat {
    /// Clear coat intensity (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearcoat_factor: Option<f64>,
    /// Intensity texture (R channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearcoat_texture: Option<TextureInfo>,
    /// Clear coat roughness (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearcoat_roughness_factor: Option<f64>,
    /// Roughness texture (G channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearcoat_roughness_texture: Option<TextureInfo>,
    /// Clear coat normal map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearcoat_normal_texture: Option<NormalTextureInfo>,
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

impl Extension for KhrMaterialsClearcoat {
    const NAME: &'static str = "KHR_materials_clearcoat";
}

impl KhrMaterialsClearcoat {
    /// Effective `clearcoatFactor`.
    pub fn clearcoat_factor_or_default(&self) -> f64 {
        self.clearcoat_factor.unwrap_or(0.0)
    }
    /// Effective `clearcoatRoughnessFactor`.
    pub fn clearcoat_roughness_factor_or_default(&self) -> f64 {
        self.clearcoat_roughness_factor.unwrap_or(0.0)
    }
}

/// Reports `EXTENSION_INVALID_LOCATION` unless the occurrence is on a material; returns
/// whether it is.
pub(crate) fn check_owner(cx: &ExtensionCx, r: &mut Report) -> bool {
    if cx.owner_index_in("materials").is_none() {
        cx.wrong_location(r, "a material");
        return false;
    }
    true
}

/// Reports `UNRESOLVED_REFERENCE` when texture reference `member` points past `textures`.
pub(crate) fn check_texture(cx: &ExtensionCx, r: &mut Report, member: &str, index: Option<u32>) {
    if let Some(i) = index
        && i as usize >= cx.doc.textures.len()
    {
        r.error(
            "UNRESOLVED_REFERENCE",
            format!("{}/index", cx.member(member)),
            format!(
                "texture {i} does not exist ({} defined)",
                cx.doc.textures.len()
            ),
        );
    }
}

/// Reports `VALUE_NOT_IN_RANGE` when `member` is outside `[min, max]` or not finite.
pub(crate) fn check_range(
    cx: &ExtensionCx,
    r: &mut Report,
    member: &str,
    v: Option<f64>,
    min: f64,
    max: f64,
) {
    if let Some(v) = v
        && !(v.is_finite() && (min..=max).contains(&v))
    {
        r.error(
            "VALUE_NOT_IN_RANGE",
            cx.member(member),
            format!("{member} {v} is outside [{min}, {max}]"),
        );
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !check_owner(cx, r) {
        return;
    }
    let Some(e) = cx.decode::<KhrMaterialsClearcoat>(r) else {
        return;
    };
    check_range(cx, r, "clearcoatFactor", e.clearcoat_factor, 0.0, 1.0);
    check_range(
        cx,
        r,
        "clearcoatRoughnessFactor",
        e.clearcoat_roughness_factor,
        0.0,
        1.0,
    );
    check_texture(
        cx,
        r,
        "clearcoatTexture",
        e.clearcoat_texture.map(|t| t.index),
    );
    check_texture(
        cx,
        r,
        "clearcoatRoughnessTexture",
        e.clearcoat_roughness_texture.map(|t| t.index),
    );
    check_texture(
        cx,
        r,
        "clearcoatNormalTexture",
        e.clearcoat_normal_texture.map(|t| t.index),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsClearcoat::NAME,
    required_by_default: KhrMaterialsClearcoat::REQUIRED_BY_DEFAULT,
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
        m.extensions.set(Some(KhrMaterialsClearcoat {
            clearcoat_factor: Some(0.5),
            clearcoat_roughness_factor: Some(0.25),
            ..Default::default()
        }));
        d.materials.push(m);
        d.use_extension(KhrMaterialsClearcoat::NAME, false);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let json = String::from_utf8(d.to_vec(&Default::default())).unwrap();
        assert!(
            json.contains(
                r#"{"KHR_materials_clearcoat":{"clearcoatFactor":0.5,"clearcoatRoughnessFactor":0.25}}"#
            ),
            "{json}"
        );
        let e: KhrMaterialsClearcoat = d.materials[0].extensions.get().unwrap().unwrap();
        assert_eq!(e.clearcoat_factor_or_default(), 0.5);
        assert_eq!(
            KhrMaterialsClearcoat::default().clearcoat_factor_or_default(),
            0.0
        );

        d.materials[0].extensions.set(Some(KhrMaterialsClearcoat {
            clearcoat_factor: Some(2.0),
            clearcoat_texture: Some(TextureInfo::new(3)),
            ..Default::default()
        }));
        let r = validate(&d);
        assert!(r.has("VALUE_NOT_IN_RANGE"), "{r}");
        assert!(r.has("UNRESOLVED_REFERENCE"), "{r}");
    }

    #[test]
    fn only_on_materials() {
        let mut d = Document::default();
        d.extensions.set(Some(KhrMaterialsClearcoat::default()));
        d.use_extension(KhrMaterialsClearcoat::NAME, false);
        assert!(validate(&d).has("EXTENSION_INVALID_LOCATION"));
    }
}
