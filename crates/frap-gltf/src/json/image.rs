// SPDX-License-Identifier: GPL-3.0-or-later
//! `images[]`: a URI or a buffer view plus MIME type.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index};

/// Image data referenced by textures.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    /// Embedded data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_view: Option<Index>,
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
    /// MIME type (required with `bufferView`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// External file or data URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}
