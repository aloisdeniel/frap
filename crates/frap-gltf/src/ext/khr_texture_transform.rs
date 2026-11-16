// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_texture_transform`: typed struct + validation (§17.1). WP 1.5.
//!
//! Lives in a texture reference's `extensions`: `UV' = T · R · S · UV` with `R` a
//! counter-clockwise rotation in UV space. `texCoord`, when present, overrides the reference's
//! `texCoord`. io_scene_gltf2 lists it as required (`blender/exp/material/texture_info.py`,
//! `Extension("KHR_texture_transform", ...)` default `required=True`).

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras};
use crate::validate::Report;

/// A UV transform.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrTextureTransform {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Translation (default `[0, 0]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f64; 2]>,
    /// Counter-clockwise rotation in radians (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// Scale (default `[1, 1]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f64; 2]>,
    /// TEXCOORD set overriding the texture reference's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<u32>,
}

impl Extension for KhrTextureTransform {
    const NAME: &'static str = "KHR_texture_transform";
    const REQUIRED_BY_DEFAULT: bool = true;
}

impl KhrTextureTransform {
    /// Effective offset.
    pub fn offset_or_default(&self) -> [f64; 2] {
        self.offset.unwrap_or([0.0; 2])
    }
    /// Effective rotation.
    pub fn rotation_or_default(&self) -> f64 {
        self.rotation.unwrap_or(0.0)
    }
    /// Effective scale.
    pub fn scale_or_default(&self) -> [f64; 2] {
        self.scale.unwrap_or([1.0; 2])
    }

    /// The transform as a row-major 3×3 matrix `T · R · S` (spec reference implementation).
    pub fn matrix(&self) -> [[f64; 3]; 3] {
        let [tx, ty] = self.offset_or_default();
        let [sx, sy] = self.scale_or_default();
        let r = self.rotation_or_default();
        let (s, c) = (libm::sin(r), libm::cos(r));
        [
            [c * sx, s * sy, tx],
            [-s * sx, c * sy, ty],
            [0.0, 0.0, 1.0],
        ]
    }

    /// Applies the transform to a UV coordinate.
    pub fn apply(&self, uv: [f64; 2]) -> [f64; 2] {
        let m = self.matrix();
        [
            m[0][0] * uv[0] + m[0][1] * uv[1] + m[0][2],
            m[1][0] * uv[0] + m[1][1] * uv[1] + m[1][2],
        ]
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !cx.owner_is_texture_info() {
        cx.wrong_location(r, "a texture reference");
        return;
    }
    let Some(t) = cx.decode::<KhrTextureTransform>(r) else {
        return;
    };
    let finite = t
        .offset
        .iter()
        .flatten()
        .chain(t.scale.iter().flatten())
        .chain(t.rotation.iter())
        .all(|v| v.is_finite());
    if !finite {
        r.error("VALUE_NOT_FINITE", cx.pointer(), "transform has a non-finite value");
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrTextureTransform::NAME,
    required_by_default: KhrTextureTransform::REQUIRED_BY_DEFAULT,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{Document, Material, PbrMetallicRoughness, Texture, TextureInfo};
    use crate::validate::validate;

    #[test]
    fn matrix_follows_the_spec() {
        let t = KhrTextureTransform {
            offset: Some([0.5, 0.0]),
            rotation: Some(std::f64::consts::FRAC_PI_2),
            scale: Some([2.0, 2.0]),
            ..Default::default()
        };
        // Spec example shape: scale, then rotate CCW in UV space, then translate.
        let uv = t.apply([1.0, 0.0]);
        assert!((uv[0] - 0.5).abs() < 1e-12 && (uv[1] + 2.0).abs() < 1e-12, "{uv:?}");
        assert_eq!(KhrTextureTransform::default().apply([0.25, 0.75]), [0.25, 0.75]);
    }

    #[test]
    fn location_and_round_trip() {
        let mut d = Document::default();
        d.textures.push(Texture::default());
        let mut ti = TextureInfo::new(0);
        ti.extensions.set(Some(KhrTextureTransform {
            scale: Some([2.0, 3.0]),
            tex_coord: Some(1),
            ..Default::default()
        }));
        d.materials.push(Material {
            pbr_metallic_roughness: Some(PbrMetallicRoughness {
                base_color_texture: Some(ti),
                ..Default::default()
            }),
            ..Default::default()
        });
        d.use_extension(KhrTextureTransform::NAME, true);
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let back = Document::from_slice(&d.to_vec(&Default::default())).unwrap();
        assert_eq!(back, d);
        let t: KhrTextureTransform = back.materials[0]
            .pbr_metallic_roughness
            .as_ref()
            .unwrap()
            .base_color_texture
            .as_ref()
            .unwrap()
            .extensions
            .get()
            .unwrap()
            .unwrap();
        assert_eq!(t.scale, Some([2.0, 3.0]));
        // On a material it is misplaced.
        d.materials[0]
            .extensions
            .insert_raw(KhrTextureTransform::NAME, serde_json::json!({}));
        assert!(validate(&d).has("EXTENSION_INVALID_LOCATION"));
    }
}
