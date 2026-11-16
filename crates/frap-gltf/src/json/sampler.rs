// SPDX-License-Identifier: GPL-3.0-or-later
//! `samplers[]`: filtering and wrapping.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, gl_enum};

gl_enum! {
    /// Magnification filter.
    MagFilter {
        /// 9728
        Nearest = 9728,
        /// 9729
        Linear = 9729,
    }
}

gl_enum! {
    /// Minification filter.
    MinFilter {
        /// 9728
        Nearest = 9728,
        /// 9729
        Linear = 9729,
        /// 9984
        NearestMipmapNearest = 9984,
        /// 9985
        LinearMipmapNearest = 9985,
        /// 9986
        NearestMipmapLinear = 9986,
        /// 9987
        LinearMipmapLinear = 9987,
    }
}

gl_enum! {
    /// Wrap mode.
    WrapMode {
        /// 33071
        ClampToEdge = 33071,
        /// 33648
        MirroredRepeat = 33648,
        /// 10497 (default)
        Repeat = 10497,
    }
}

/// Texture sampler.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Magnification filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mag_filter: Option<MagFilter>,
    /// Minification filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_filter: Option<MinFilter>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// S (U) wrapping (default REPEAT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_s: Option<WrapMode>,
    /// T (V) wrapping (default REPEAT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_t: Option<WrapMode>,
}
