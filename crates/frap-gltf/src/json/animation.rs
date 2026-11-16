// SPDX-License-Identifier: GPL-3.0-or-later
//! `animations[]`: channels and samplers.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index, str_enum};

str_enum! {
    /// Sampler interpolation.
    Interpolation {
        /// Linear (slerp for rotations; default).
        Linear = "LINEAR",
        /// Step.
        Step = "STEP",
        /// Cubic Hermite spline (in-tangent, value, out-tangent per key).
        CubicSpline = "CUBICSPLINE",
    }
}

/// Animated property. `Other` keeps paths defined by extensions this crate does not know.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum AnimationPath {
    /// `translation`
    Translation,
    /// `rotation`
    Rotation,
    /// `scale`
    Scale,
    /// `weights`
    Weights,
    /// `pointer` (KHR_animation_pointer)
    Pointer,
    /// Any other path, verbatim.
    Other(String),
}

impl AnimationPath {
    /// The JSON spelling.
    pub fn as_str(&self) -> &str {
        match self {
            AnimationPath::Translation => "translation",
            AnimationPath::Rotation => "rotation",
            AnimationPath::Scale => "scale",
            AnimationPath::Weights => "weights",
            AnimationPath::Pointer => "pointer",
            AnimationPath::Other(s) => s,
        }
    }
}

impl From<String> for AnimationPath {
    fn from(s: String) -> Self {
        match s.as_str() {
            "translation" => AnimationPath::Translation,
            "rotation" => AnimationPath::Rotation,
            "scale" => AnimationPath::Scale,
            "weights" => AnimationPath::Weights,
            "pointer" => AnimationPath::Pointer,
            _ => AnimationPath::Other(s),
        }
    }
}

impl From<AnimationPath> for String {
    fn from(p: AnimationPath) -> String {
        match p {
            AnimationPath::Other(s) => s,
            p => p.as_str().to_owned(),
        }
    }
}

/// An animation: channels driven by samplers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    /// Channels.
    pub channels: Vec<Channel>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Samplers.
    pub samplers: Vec<AnimationSampler>,
}

/// Connects a sampler to an animated property.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Sampler (index into the animation's `samplers`).
    pub sampler: Index,
    /// Animated property.
    pub target: ChannelTarget,
}

/// The node and path a channel animates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelTarget {
    /// Extensions (KHR_animation_pointer).
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Node (absent with KHR_animation_pointer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<Index>,
    /// Property.
    pub path: AnimationPath,
}

/// Keyframe times (`input`) and values (`output`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationSampler {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// SCALAR FLOAT seconds, strictly increasing.
    pub input: Index,
    /// Interpolation (default LINEAR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpolation: Option<Interpolation>,
    /// Values.
    pub output: Index,
}

impl AnimationSampler {
    /// Effective interpolation.
    pub fn interpolation_or_default(&self) -> Interpolation {
        self.interpolation.unwrap_or(Interpolation::Linear)
    }
}
