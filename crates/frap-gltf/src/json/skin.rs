// SPDX-License-Identifier: GPL-3.0-or-later
//! `skins[]`: joints and inverse bind matrices.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index};

/// Joints and inverse bind matrices of a skinned mesh.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// MAT4 FLOAT accessor, one per joint (identity when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse_bind_matrices: Option<Index>,
    /// Joint nodes.
    pub joints: Vec<Index>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Common root hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skeleton: Option<Index>,
}
