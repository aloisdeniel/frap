// SPDX-License-Identifier: GPL-3.0-or-later
//! `cameras[]`: perspective or orthographic projection.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, str_enum};

str_enum! {
    /// Projection type.
    CameraType {
        /// Perspective.
        Perspective = "perspective",
        /// Orthographic.
        Orthographic = "orthographic",
    }
}

/// A camera.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Orthographic parameters (when `type` is orthographic).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orthographic: Option<Orthographic>,
    /// Perspective parameters (when `type` is perspective).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perspective: Option<Perspective>,
    /// Projection type.
    #[serde(rename = "type")]
    pub type_: CameraType,
}

/// Perspective projection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Perspective {
    /// Width / height (absent: the viewport's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f64>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Vertical field of view in radians.
    pub yfov: f64,
    /// Far plane (absent: infinite projection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zfar: Option<f64>,
    /// Near plane.
    pub znear: f64,
}

/// Orthographic projection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Orthographic {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Horizontal half extent.
    pub xmag: f64,
    /// Vertical half extent.
    pub ymag: f64,
    /// Far plane.
    pub zfar: f64,
    /// Near plane.
    pub znear: f64,
}
