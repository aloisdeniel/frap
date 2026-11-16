// SPDX-License-Identifier: GPL-3.0-or-later
//! `asset`: glTF version and provenance.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras};

/// Metadata about the glTF asset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    /// Copyright message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Tool that generated the asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    /// Minimum glTF version a loader must support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    /// glTF version (`"2.0"`).
    pub version: String,
}

impl Default for Asset {
    fn default() -> Self {
        Asset {
            copyright: None,
            generator: None,
            version: "2.0".to_owned(),
            min_version: None,
            extensions: Extensions::default(),
            extras: None,
        }
    }
}
