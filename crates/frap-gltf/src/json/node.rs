// SPDX-License-Identifier: GPL-3.0-or-later
//! `nodes[]`: the node hierarchy and its local transforms.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index};

/// A node: either `matrix` or TRS (never both), plus attached mesh/camera/skin.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Camera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Index>,
    /// Children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Index>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Column-major local matrix (default identity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f64; 16]>,
    /// Mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<Index>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Unit quaternion, `x, y, z, w` (default `[0,0,0,1]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f64; 4]>,
    /// Scale (default `[1,1,1]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f64; 3]>,
    /// Skin (requires `mesh`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<Index>,
    /// Translation (default `[0,0,0]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<[f64; 3]>,
    /// Morph target weights overriding `mesh.weights`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weights: Vec<f64>,
}

/// Identity matrix, column-major.
pub const IDENTITY: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

impl Node {
    /// Effective translation.
    pub fn translation_or_default(&self) -> [f64; 3] {
        self.translation.unwrap_or([0.0; 3])
    }
    /// Effective rotation.
    pub fn rotation_or_default(&self) -> [f64; 4] {
        self.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0])
    }
    /// Effective scale.
    pub fn scale_or_default(&self) -> [f64; 3] {
        self.scale.unwrap_or([1.0; 3])
    }
    /// Effective matrix.
    pub fn matrix_or_default(&self) -> [f64; 16] {
        self.matrix.unwrap_or(IDENTITY)
    }
}
