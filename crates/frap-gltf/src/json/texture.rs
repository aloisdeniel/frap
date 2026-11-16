// SPDX-License-Identifier: GPL-3.0-or-later
//! `textures[]` and the texture references used by materials.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index};

/// A texture: an image plus a sampler.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    /// Extensions (EXT_texture_webp, KHR_texture_basisu).
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Sampler (absent: repeat + auto filtering).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampler: Option<Index>,
    /// Image (may be absent when an extension supplies one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Index>,
}

/// Reference to a texture (`baseColorTexture`, `emissiveTexture`, extension textures).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    /// Extensions (KHR_texture_transform).
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Texture.
    pub index: Index,
    /// TEXCOORD set (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<u32>,
}

impl TextureInfo {
    /// Reference to `index` with defaults.
    pub fn new(index: Index) -> Self {
        TextureInfo {
            index,
            ..TextureInfo::default()
        }
    }
    /// Effective TEXCOORD set.
    pub fn tex_coord_or_default(&self) -> u32 {
        self.tex_coord.unwrap_or(0)
    }
}

/// `normalTexture`: a texture reference with a normal `scale`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalTextureInfo {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Texture.
    pub index: Index,
    /// XY scale of the sampled normal (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// TEXCOORD set (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<u32>,
}

/// `occlusionTexture`: a texture reference with an occlusion `strength`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcclusionTextureInfo {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Texture.
    pub index: Index,
    /// Occlusion strength (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
    /// TEXCOORD set (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<u32>,
}
