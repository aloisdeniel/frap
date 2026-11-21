// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_meshopt_compression`: typed structs + validation (§17.1). WP 7.1 (M7 W0 stub: the typed model;
//! the WP adds the spec's validation rules and tests).
//!
//! On a buffer view: the compressed stream (`buffer`, `byteOffset`, `byteLength`) and the
//! decoded layout (`byteStride`, `count`, `mode`, `filter`); the view's own `buffer`/
//! `byteOffset`/`byteLength` address the uncompressed fallback. On a buffer: `fallback: true`
//! marks a buffer that has no data (the add-on writes it with no `uri`, `exporter.py`
//! `finalize_buffer`). Required, as the EXT variant; vertex codec version 1 and the `COLOR` filter.

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "KHR_meshopt_compression";
const REQUIRED: bool = true;

/// The extension on a buffer view.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMeshoptCompression {
    /// Buffer holding the compressed stream.
    pub buffer: Index,
    /// Offset of the stream in `buffer` (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
    /// Length of the stream.
    pub byte_length: u64,
    /// Bytes per decoded element.
    pub byte_stride: u32,
    /// Decoded elements.
    pub count: u32,
    /// `ATTRIBUTES` | `TRIANGLES` | `INDICES`.
    pub mode: String,
    /// `NONE` (default) | `OCTAHEDRAL` | `QUATERNION` | `EXPONENTIAL` | `COLOR`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
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
}

/// The extension on a buffer: `fallback` marks a buffer without data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMeshoptCompressionBuffer {
    /// The buffer holds no data (only the compressed views do).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<bool>,
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
}

impl Extension for KhrMeshoptCompression {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

impl Extension for KhrMeshoptCompressionBuffer {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("bufferViews").is_some() {
        let _ = cx.decode::<KhrMeshoptCompression>(r);
    } else if cx.owner_index_in("buffers").is_some() {
        let _ = cx.decode::<KhrMeshoptCompressionBuffer>(r);
    } else {
        cx.wrong_location(r, "a buffer view or a buffer");
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: NAME,
    required_by_default: REQUIRED,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_round_trip() {
        let v = serde_json::json!({
            "buffer": 1, "byteLength": 40, "byteStride": 12, "count": 3,
            "mode": "ATTRIBUTES", "filter": "NONE"
        });
        let e: KhrMeshoptCompression = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.count, 3);
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
        let b: KhrMeshoptCompressionBuffer =
            serde_json::from_value(serde_json::json!({"fallback": true})).unwrap();
        assert_eq!(b.fallback, Some(true));
    }
}
