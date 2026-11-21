// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_meshopt_compression`: typed structs + validation (§17.1). WP 7.1.
//!
//! On a buffer view: the compressed stream (`buffer`, `byteOffset`, `byteLength`) and the
//! decoded layout (`byteStride`, `count`, `mode`, `filter`); the view's own `buffer`/
//! `byteOffset`/`byteLength` address the uncompressed fallback. On a buffer: `fallback: true`
//! marks a buffer that has no data (the add-on writes it with no `uri`, `exporter.py`
//! `finalize_buffer`). Required, as the EXT variant; vertex codec version 1 and the `COLOR` filter.
//! The validation rules are the EXT variant's ([`super::ext_meshopt_compression::validate_view`]),
//! with `COLOR` allowed.

use serde::{Deserialize, Serialize};

use super::ext_meshopt_compression::{ExtMeshoptCompression, validate_buffer, validate_view};
use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

/// The extension name.
pub const NAME: &str = "KHR_meshopt_compression";
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

impl From<KhrMeshoptCompression> for ExtMeshoptCompression {
    fn from(e: KhrMeshoptCompression) -> Self {
        ExtMeshoptCompression {
            buffer: e.buffer,
            byte_offset: e.byte_offset,
            byte_length: e.byte_length,
            byte_stride: e.byte_stride,
            count: e.count,
            mode: e.mode,
            filter: e.filter,
            extensions: e.extensions,
            extras: e.extras,
        }
    }
}

impl From<ExtMeshoptCompression> for KhrMeshoptCompression {
    fn from(e: ExtMeshoptCompression) -> Self {
        KhrMeshoptCompression {
            buffer: e.buffer,
            byte_offset: e.byte_offset,
            byte_length: e.byte_length,
            byte_stride: e.byte_stride,
            count: e.count,
            mode: e.mode,
            filter: e.filter,
            extensions: e.extensions,
            extras: e.extras,
        }
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("bufferViews").is_some() {
        if let Some(e) = cx.decode::<KhrMeshoptCompression>(r) {
            validate_view(cx, r, &e.into(), true);
        }
    } else if cx.owner_index_in("buffers").is_some() {
        if let Some(b) = cx.decode::<KhrMeshoptCompressionBuffer>(r) {
            validate_buffer(cx, r, b.fallback);
        }
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
        let back: KhrMeshoptCompression = ExtMeshoptCompression::from(e.clone()).into();
        assert_eq!(back, e);
    }

    #[test]
    fn colour_filter_is_allowed_here() {
        let j = serde_json::json!({
            "asset": {"version": "2.0"},
            "extensionsUsed": [NAME], "extensionsRequired": [NAME],
            "buffers": [{"byteLength": 64}],
            "bufferViews": [{"buffer": 0, "byteLength": 16, "extensions": {NAME: {
                "buffer": 0, "byteLength": 20, "byteStride": 4, "count": 4,
                "mode": "ATTRIBUTES", "filter": "COLOR"}}}]
        });
        let d = crate::Document::from_slice(&serde_json::to_vec(&j).unwrap()).unwrap();
        let r = crate::validate::validate(&d);
        assert!(r.is_ok(), "{r}");
        let (name, e) =
            super::super::ext_meshopt_compression::view_compression(&d.buffer_views[0].extensions)
                .unwrap()
                .unwrap();
        assert_eq!((name, e.filter.as_deref()), (NAME, Some("COLOR")));
    }
}
