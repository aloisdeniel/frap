// SPDX-License-Identifier: GPL-3.0-or-later
//! `materials[]`: metallic-roughness PBR (material extensions are typed in `ext/`).

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, NormalTextureInfo, OcclusionTextureInfo, TextureInfo, str_enum};

str_enum! {
    /// Alpha mode.
    AlphaMode {
        /// Alpha ignored (default).
        Opaque = "OPAQUE",
        /// Alpha tested against `alphaCutoff`.
        Mask = "MASK",
        /// Alpha blended.
        Blend = "BLEND",
    }
}

/// A material.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// MASK cutoff (default 0.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_cutoff: Option<f64>,
    /// Alpha mode (default OPAQUE).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_mode: Option<AlphaMode>,
    /// Back faces rendered (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_sided: Option<bool>,
    /// Linear emissive colour (default black).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_factor: Option<[f64; 3]>,
    /// Emissive texture (sRGB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_texture: Option<TextureInfo>,
    /// Extensions (`KHR_materials_*`).
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tangent-space normal map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_texture: Option<NormalTextureInfo>,
    /// Occlusion (R channel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occlusion_texture: Option<OcclusionTextureInfo>,
    /// Metallic-roughness parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pbr_metallic_roughness: Option<PbrMetallicRoughness>,
}

impl Material {
    /// Effective alpha mode.
    pub fn alpha_mode_or_default(&self) -> AlphaMode {
        self.alpha_mode.unwrap_or(AlphaMode::Opaque)
    }
    /// Effective alpha cutoff.
    pub fn alpha_cutoff_or_default(&self) -> f64 {
        self.alpha_cutoff.unwrap_or(0.5)
    }
    /// Effective emissive factor.
    pub fn emissive_factor_or_default(&self) -> [f64; 3] {
        self.emissive_factor.unwrap_or([0.0; 3])
    }
}

/// `pbrMetallicRoughness`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbrMetallicRoughness {
    /// Linear RGBA (default white).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_factor: Option<[f64; 4]>,
    /// Base colour texture (sRGB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<TextureInfo>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Metalness (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_factor: Option<f64>,
    /// G = roughness, B = metallic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<TextureInfo>,
    /// Roughness (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness_factor: Option<f64>,
}

impl PbrMetallicRoughness {
    /// Effective base colour factor.
    pub fn base_color_factor_or_default(&self) -> [f64; 4] {
        self.base_color_factor.unwrap_or([1.0; 4])
    }
    /// Effective metallic factor.
    pub fn metallic_factor_or_default(&self) -> f64 {
        self.metallic_factor.unwrap_or(1.0)
    }
    /// Effective roughness factor.
    pub fn roughness_factor_or_default(&self) -> f64 {
        self.roughness_factor.unwrap_or(1.0)
    }
}
