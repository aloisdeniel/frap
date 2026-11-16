// SPDX-License-Identifier: GPL-3.0-or-later
//! `buffers[]` and `bufferViews[]`.

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index, gl_enum};

gl_enum! {
    /// Buffer view target (GPU binding hint).
    Target {
        /// 34962: vertex attributes.
        ArrayBuffer = 34962,
        /// 34963: indices.
        ElementArrayBuffer = 34963,
    }
}

/// A block of binary data: an external file, a data URI, or (no `uri`, buffer 0 of a GLB)
/// the BIN chunk.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    /// Length in bytes (may be up to 3 bytes shorter than a padded BIN chunk).
    pub byte_length: u64,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// A contiguous range of a buffer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    /// Buffer.
    pub buffer: Index,
    /// Length in bytes.
    pub byte_length: u64,
    /// Start in the buffer (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
    /// Distance between vertex attribute elements (4..=252, multiple of 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_stride: Option<u32>,
    /// Extensions (EXT_meshopt_compression lives here).
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Binding hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
}

impl BufferView {
    /// Effective start in the buffer.
    pub fn byte_offset_or_default(&self) -> u64 {
        self.byte_offset.unwrap_or(0)
    }
}
