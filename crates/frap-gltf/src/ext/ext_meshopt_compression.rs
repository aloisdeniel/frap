// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_meshopt_compression`: typed structs + validation (§17.1). WP 7.1.
//!
//! On a buffer view: the compressed stream (`buffer`, `byteOffset`, `byteLength`) and the
//! decoded layout (`byteStride`, `count`, `mode`, `filter`); the view's own `buffer`/
//! `byteOffset`/`byteLength` address the uncompressed fallback. On a buffer: `fallback: true`
//! marks a buffer that has no data (the add-on writes it with no `uri`, `exporter.py`
//! `finalize_buffer`). Required (the add-on lists it in `extensionsRequired`, `add_meshopt_extension`).
//!
//! Validation (the extension specification, shared with `KHR_meshopt_compression` through
//! [`validate_view`]): `mode` and `filter` are known values; `ATTRIBUTES` strides are
//! multiples of 4 in 4..=256, index strides 2 or 4, `TRIANGLES` counts multiples of 3; a
//! filter only on `ATTRIBUTES`, with the stride it needs (`OCTAHEDRAL` and `COLOR` 4 or 8,
//! `QUATERNION` 8, `EXPONENTIAL` a multiple of 4; `QUATERNION` at 16, the add-on's sampled
//! object rotations, is a warning: Blender's decoder reads the quaternions from the first
//! `count * 8` bytes and imports the file); `COLOR` only for the KHR variant; the
//! stream lies inside its buffer; a parent `byteStride` equals the extension's (warning), and so
//! does `count * byteStride` the parent's `byteLength` (warning: the add-on writes the float
//! length for its quaternion-filtered GPU instance rotations); a fallback buffer has no `uri`
//! (warning: the spec allows fallback data at a `uri`, which loaders then ignore).

use serde::{Deserialize, Serialize};

use crate::error::GltfError;
use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "EXT_meshopt_compression";
const REQUIRED: bool = true;

/// The extension on a buffer view.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtMeshoptCompression {
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
    /// `NONE` (default) | `OCTAHEDRAL` | `QUATERNION` | `EXPONENTIAL`.
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
pub struct ExtMeshoptCompressionBuffer {
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

impl Extension for ExtMeshoptCompression {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

impl Extension for ExtMeshoptCompressionBuffer {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

/// The two names of the meshopt extension, in the add-on's lookup order (`EXT_` first,
/// `io/imp/gltf2_io_binary_meshopt.py`).
pub const MESHOPT_EXTENSIONS: [&str; 2] = [NAME, super::khr_meshopt_compression::NAME];

/// The compression of a buffer view, if any: the extension's name and its value (the KHR
/// variant has the same members and is returned in the EXT struct).
pub fn view_compression(
    ext: &Extensions,
) -> Result<Option<(&'static str, ExtMeshoptCompression)>, GltfError> {
    if let Some(e) = ext.get::<ExtMeshoptCompression>()? {
        return Ok(Some((NAME, e)));
    }
    Ok(ext
        .get::<super::khr_meshopt_compression::KhrMeshoptCompression>()?
        .map(|e| (super::khr_meshopt_compression::NAME, e.into())))
}

/// Whether buffer `b` is marked `fallback: true` by either meshopt extension.
pub fn is_fallback_buffer(ext: &Extensions) -> bool {
    MESHOPT_EXTENSIONS.iter().any(|n| {
        ext.get_raw(n)
            .and_then(|v| v.get("fallback"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    })
}

/// The specification's rules for one compressed view (both variants; `COLOR` only with
/// `allow_color`).
pub fn validate_view(
    cx: &ExtensionCx,
    r: &mut Report,
    e: &ExtMeshoptCompression,
    allow_color: bool,
) {
    let stride = e.byte_stride as usize;
    let mode = e.mode.as_str();
    match mode {
        "ATTRIBUTES" => {
            if stride == 0 || !stride.is_multiple_of(4) || stride > 256 {
                r.error(
                    "VALUE_NOT_IN_RANGE",
                    cx.member("byteStride"),
                    format!(
                        "ATTRIBUTES byteStride must be a multiple of 4 in 4..=256, not {stride}"
                    ),
                );
            }
        }
        "TRIANGLES" | "INDICES" => {
            if stride != 2 && stride != 4 {
                r.error(
                    "VALUE_NOT_IN_LIST",
                    cx.member("byteStride"),
                    format!("{mode} byteStride must be 2 or 4, not {stride}"),
                );
            }
            if mode == "TRIANGLES" && !e.count.is_multiple_of(3) {
                r.error(
                    "VALUE_NOT_IN_RANGE",
                    cx.member("count"),
                    format!("TRIANGLES count must be a multiple of 3, not {}", e.count),
                );
            }
        }
        _ => r.error(
            "VALUE_NOT_IN_LIST",
            cx.member("mode"),
            format!("unknown mode {mode:?} (ATTRIBUTES, TRIANGLES, INDICES)"),
        ),
    }
    if let Some(f) = e.filter.as_deref() {
        let ok = match f {
            "NONE" => true,
            "OCTAHEDRAL" => stride == 4 || stride == 8,
            "QUATERNION" if stride == 16 => {
                // The add-on's sampled object rotations (`sampled/object/sampler.py`, the
                // default `export_force_sampling`): the quaternions fill the first
                // `count * 8` bytes, where release meshoptimizer's decoder (Blender's, and
                // frap's) reads them; Blender 5.2.4 imports these files.
                r.warning(
                    "MESHOPT_QUATERNION_STRIDE",
                    cx.member("filter"),
                    "filter QUATERNION with byteStride 16 (the specification requires 8): the \
                     quaternions are read from the first count × 8 bytes, as Blender's decoder \
                     reads them"
                        .to_owned(),
                );
                true
            }
            "QUATERNION" => stride == 8,
            "EXPONENTIAL" => stride.is_multiple_of(4),
            "COLOR" if allow_color => stride == 4 || stride == 8,
            _ => {
                r.error(
                    "VALUE_NOT_IN_LIST",
                    cx.member("filter"),
                    format!("unknown filter {f:?} for {}", cx.name),
                );
                true
            }
        };
        if !ok {
            r.error(
                "VALUE_NOT_IN_RANGE",
                cx.member("filter"),
                format!("filter {f} cannot apply to a byteStride of {stride}"),
            );
        }
        if f != "NONE" && mode != "ATTRIBUTES" {
            r.error(
                "VALUE_NOT_IN_LIST",
                cx.member("filter"),
                format!("filter {f} is only allowed in ATTRIBUTES mode"),
            );
        }
    }
    match cx.doc.buffers.get(e.buffer as usize) {
        None => r.error(
            "UNRESOLVED_REFERENCE",
            cx.member("buffer"),
            format!("buffer {} does not exist", e.buffer),
        ),
        Some(b) => {
            let end = e.byte_offset.unwrap_or(0).checked_add(e.byte_length);
            if end.is_none_or(|end| end > b.byte_length) {
                r.error(
                    "BUFFER_VIEW_TOO_LONG",
                    cx.pointer(),
                    format!(
                        "the compressed stream (offset {}, length {}) exceeds buffer {} ({} bytes)",
                        e.byte_offset.unwrap_or(0),
                        e.byte_length,
                        e.buffer,
                        b.byte_length
                    ),
                );
            }
        }
    }
    let parent = cx.owner_value;
    if let Some(s) = parent.get("byteStride").and_then(serde_json::Value::as_u64)
        && s != u64::from(e.byte_stride)
    {
        r.warning(
            "MESHOPT_STRIDE_MISMATCH",
            cx.member("byteStride"),
            format!(
                "the buffer view's byteStride is {s}, the extension's {}",
                e.byte_stride
            ),
        );
    }
    if let Some(l) = parent.get("byteLength").and_then(serde_json::Value::as_u64)
        && l != u64::from(e.count) * u64::from(e.byte_stride)
    {
        r.warning(
            "MESHOPT_LENGTH_MISMATCH",
            cx.member("count"),
            format!(
                "count {} × byteStride {} differs from the buffer view's byteLength {l}",
                e.count, e.byte_stride
            ),
        );
    }
}

/// The rule for the buffer form (both variants).
pub fn validate_buffer(cx: &ExtensionCx, r: &mut Report, fallback: Option<bool>) {
    if fallback == Some(true) && cx.owner_value.get("uri").is_some() {
        r.warning(
            "MESHOPT_FALLBACK_URI",
            cx.member("fallback"),
            "a fallback buffer's data is never read; it should have no uri",
        );
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("bufferViews").is_some() {
        if let Some(e) = cx.decode::<ExtMeshoptCompression>(r) {
            validate_view(cx, r, &e, false);
        }
    } else if cx.owner_index_in("buffers").is_some() {
        if let Some(b) = cx.decode::<ExtMeshoptCompressionBuffer>(r) {
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
    use crate::Document;

    #[test]
    fn typed_round_trip() {
        let v = serde_json::json!({
            "buffer": 1, "byteLength": 40, "byteStride": 12, "count": 3,
            "mode": "ATTRIBUTES", "filter": "NONE"
        });
        let e: ExtMeshoptCompression = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.count, 3);
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
        let b: ExtMeshoptCompressionBuffer =
            serde_json::from_value(serde_json::json!({"fallback": true})).unwrap();
        assert_eq!(b.fallback, Some(true));
    }

    fn doc(view_ext: serde_json::Value, extra: serde_json::Value) -> Document {
        let mut j = serde_json::json!({
            "asset": {"version": "2.0"},
            "extensionsUsed": [NAME], "extensionsRequired": [NAME],
            "buffers": [
                {"byteLength": 100},
                {"byteLength": 72, "extensions": {NAME: {"fallback": true}}}
            ],
            "bufferViews": [{"buffer": 1, "byteLength": 72, "extensions": {NAME: view_ext}}]
        });
        if let serde_json::Value::Object(m) = extra {
            for (k, v) in m {
                j["bufferViews"][0][k] = v;
            }
        }
        Document::from_slice(&serde_json::to_vec(&j).unwrap()).unwrap()
    }

    fn codes(d: &Document) -> Vec<&'static str> {
        crate::validate::validate(d)
            .issues
            .iter()
            .map(|i| i.code)
            .collect()
    }

    #[test]
    fn blender_shaped_views_validate() {
        let d = doc(
            serde_json::json!({"buffer": 0, "byteOffset": 0, "byteStride": 12, "byteLength": 96,
                "count": 6, "mode": "ATTRIBUTES", "filter": "EXPONENTIAL"}),
            serde_json::json!({}),
        );
        let r = crate::validate::validate(&d);
        assert!(r.is_ok(), "{r}");
        // The add-on's sampled object rotation (stride 16): a warning, not an error.
        let q = doc(
            serde_json::json!({"buffer": 0, "byteOffset": 0, "byteStride": 16, "byteLength": 60,
                "count": 4, "mode": "ATTRIBUTES", "filter": "QUATERNION"}),
            serde_json::json!({}),
        );
        let r = crate::validate::validate(&q);
        assert!(r.is_ok(), "{r}");
        assert!(r.has("MESHOPT_QUATERNION_STRIDE"), "{r}");
        let (name, e) = view_compression(&d.buffer_views[0].extensions)
            .unwrap()
            .unwrap();
        assert_eq!((name, e.count), (NAME, 6));
        assert!(is_fallback_buffer(&d.buffers[1].extensions));
        assert!(!is_fallback_buffer(&d.buffers[0].extensions));
    }

    #[test]
    fn spec_rules_are_reported() {
        let bad = |ext: serde_json::Value| codes(&doc(ext, serde_json::json!({})));
        let base = |mode: &str, stride: u32, count: u32, filter: Option<&str>| {
            let mut v = serde_json::json!({"buffer": 0, "byteLength": 10, "byteStride": stride,
                "count": count, "mode": mode});
            if let Some(f) = filter {
                v["filter"] = f.into();
            }
            v
        };
        assert!(bad(base("ATTRIBUTES", 6, 12, None)).contains(&"VALUE_NOT_IN_RANGE"));
        assert!(bad(base("TRIANGLES", 2, 4, None)).contains(&"VALUE_NOT_IN_RANGE"));
        assert!(bad(base("INDICES", 8, 9, None)).contains(&"VALUE_NOT_IN_LIST"));
        assert!(bad(base("STRIPS", 4, 18, None)).contains(&"VALUE_NOT_IN_LIST"));
        assert!(bad(base("ATTRIBUTES", 12, 6, Some("QUATERNION"))).contains(&"VALUE_NOT_IN_RANGE"));
        // COLOR is KHR-only.
        assert!(bad(base("ATTRIBUTES", 4, 18, Some("COLOR"))).contains(&"VALUE_NOT_IN_LIST"));
        assert!(bad(base("TRIANGLES", 4, 18, Some("EXPONENTIAL"))).contains(&"VALUE_NOT_IN_LIST"));
        let mut far = base("ATTRIBUTES", 4, 18, None);
        far["byteOffset"] = 96.into();
        assert!(bad(far).contains(&"BUFFER_VIEW_TOO_LONG"));
        let mut missing = base("ATTRIBUTES", 4, 18, None);
        missing["buffer"] = 7.into();
        assert!(bad(missing).contains(&"UNRESOLVED_REFERENCE"));
        let warn = codes(&doc(
            base("ATTRIBUTES", 8, 8, None),
            serde_json::json!({"byteStride": 12}),
        ));
        assert!(warn.contains(&"MESHOPT_STRIDE_MISMATCH"), "{warn:?}");
        assert!(warn.contains(&"MESHOPT_LENGTH_MISMATCH"));
    }
}
