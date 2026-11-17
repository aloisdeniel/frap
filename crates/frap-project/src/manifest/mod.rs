// SPDX-License-Identifier: GPL-3.0-or-later
//! `manifest.json` (`format`, `schema_version`, `blender_api`, `generator`, buffers, images) (WP 1.9).
//!
//! ```json
//! {
//!   "format": "frap",
//!   "schema_version": 1,
//!   "blender_api": "5.2",
//!   "generator": "frap 0.1.0",
//!   "buffers": [{"hash": "5e1c…", "size": 28}],
//!   "images": [{"hash": "77aa…", "ext": "png", "size": 5120}]
//! }
//! ```
//!
//! `buffers` and `images` list every `buffers/<hash>.bin` / `images/<hash>.<ext>` entry of the
//! archive, in entry order (hash order), with their uncompressed byte size.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use smol_str::SmolStr;

use crate::buffers::{BufferStore, ContentHash, ImageRef, ImageStore};

/// Value of `format`.
pub const FORMAT: &str = "frap";
/// Content of the `mimetype` entry.
pub const MIMETYPE: &str = "application/vnd.frap+zip";
/// The schema version this build writes; older versions are migrated on load, newer refused.
pub const SCHEMA_VERSION: u32 = 1;
/// The Blender Python API the RNA identifiers in `scene.json` follow.
pub const BLENDER_API: &str = "5.2";

/// `generator`: `"frap <version>"`.
#[must_use]
pub fn generator() -> String {
    format!("frap {}", env!("CARGO_PKG_VERSION"))
}

/// Errors of `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest.json is not a JSON object")]
    NotAnObject,
    #[error("manifest.json has no integer `schema_version`")]
    MissingSchemaVersion,
    #[error("manifest.json `format` is {0:?}, expected \"frap\"")]
    WrongFormat(String),
    #[error(
        "unknown field `{field}` in manifest.json{location} (open with --lenient to ignore it)"
    )]
    UnknownField { field: String, location: String },
    #[error("invalid manifest.json: {0}")]
    Invalid(String),
}

/// One `buffers/<hash>.bin` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BufferEntry {
    pub hash: ContentHash,
    /// Uncompressed size in bytes.
    pub size: u64,
}

/// One `images/<hash>.<ext>` entry.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ImageEntry {
    pub hash: ContentHash,
    pub ext: SmolStr,
    /// Size in bytes.
    pub size: u64,
}

impl ImageEntry {
    #[must_use]
    pub fn image_ref(&self) -> ImageRef {
        ImageRef {
            hash: self.hash,
            ext: self.ext.clone(),
        }
    }
}

/// The parsed `manifest.json`. Field order is the written key order.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: SmolStr,
    pub schema_version: u32,
    pub blender_api: SmolStr,
    pub generator: String,
    pub buffers: Vec<BufferEntry>,
    pub images: Vec<ImageEntry>,
}

const TOP_FIELDS: &[&str] = &[
    "format",
    "schema_version",
    "blender_api",
    "generator",
    "buffers",
    "images",
];
const BUFFER_FIELDS: &[&str] = &["hash", "size"];
const IMAGE_FIELDS: &[&str] = &["hash", "ext", "size"];

impl Manifest {
    /// The manifest this build writes for the given entries.
    #[must_use]
    pub fn new(buffers: &BufferStore, images: &ImageStore) -> Self {
        Self {
            format: FORMAT.into(),
            schema_version: SCHEMA_VERSION,
            blender_api: BLENDER_API.into(),
            generator: generator(),
            buffers: buffers
                .iter()
                .map(|(hash, b)| BufferEntry {
                    hash: *hash,
                    size: b.len() as u64,
                })
                .collect(),
            images: images
                .iter()
                .map(|(r, b)| ImageEntry {
                    hash: r.hash,
                    ext: r.ext.clone(),
                    size: b.len() as u64,
                })
                .collect(),
        }
    }

    /// `manifest.json` bytes (the `.frap` JSON layout, trailing newline).
    #[must_use]
    pub fn to_json(&self) -> Vec<u8> {
        crate::container::to_json_pretty(self).expect("manifest serialization cannot fail")
    }

    /// `schema_version` of an unparsed manifest (read before migrations run).
    pub fn schema_version_of(v: &Value) -> Result<u32, ManifestError> {
        let obj = v.as_object().ok_or(ManifestError::NotAnObject)?;
        obj.get("schema_version")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(ManifestError::MissingSchemaVersion)
    }

    /// Parses a (migrated) manifest. Unknown fields are an error unless `lenient`.
    pub fn from_value(v: Value, lenient: bool) -> Result<Self, ManifestError> {
        let obj = v.as_object().ok_or(ManifestError::NotAnObject)?;
        if !lenient {
            check_fields(obj, TOP_FIELDS, "")?;
            for (key, fields) in [("buffers", BUFFER_FIELDS), ("images", IMAGE_FIELDS)] {
                if let Some(Value::Array(items)) = obj.get(key) {
                    for (i, item) in items.iter().enumerate() {
                        if let Some(o) = item.as_object() {
                            check_fields(o, fields, &format!(" at {key}[{i}]"))?;
                        }
                    }
                }
            }
        }
        if let Some(format) = obj.get("format")
            && format.as_str() != Some(FORMAT)
        {
            return Err(ManifestError::WrongFormat(format.to_string()));
        }
        Self::deserialize(v).map_err(|e| ManifestError::Invalid(e.to_string()))
    }
}

fn check_fields(
    obj: &serde_json::Map<String, Value>,
    known: &[&str],
    location: &str,
) -> Result<(), ManifestError> {
    match obj.keys().find(|k| !known.contains(&k.as_str())) {
        Some(field) => Err(ManifestError::UnknownField {
            field: field.clone(),
            location: location.to_owned(),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn written_shape() {
        let mut buffers = BufferStore::new();
        buffers.put(&[0u8; 100]);
        let mut images = ImageStore::new();
        images.insert(b"png bytes".to_vec(), "png").unwrap();
        let m = Manifest::new(&buffers, &images);
        let v: Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(
            v,
            json!({
                "format": "frap", "schema_version": 1, "blender_api": "5.2",
                "generator": format!("frap {}", env!("CARGO_PKG_VERSION")),
                "buffers": [{"hash": ContentHash::of(&[0u8; 100]).to_hex(), "size": 100}],
                "images": [{"hash": ContentHash::of(b"png bytes").to_hex(), "ext": "png", "size": 9}],
            })
        );
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, TOP_FIELDS);
        assert!(m.to_json().ends_with(b"}\n"));
        assert_eq!(Manifest::from_value(v, false).unwrap(), m);
    }

    #[test]
    fn unknown_fields_need_lenient() {
        let base = Manifest::new(&BufferStore::new(), &ImageStore::new());
        let mut v = serde_json::to_value(&base).unwrap();
        v["extra"] = json!(1);
        assert_eq!(
            Manifest::from_value(v.clone(), false),
            Err(ManifestError::UnknownField {
                field: "extra".into(),
                location: String::new()
            })
        );
        assert_eq!(Manifest::from_value(v, true).unwrap(), base);

        let mut v = serde_json::to_value(&base).unwrap();
        v["buffers"] = json!([{"hash": ContentHash::of(b"").to_hex(), "size": 0, "crc": 1}]);
        let err = Manifest::from_value(v.clone(), false).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown field `crc` in manifest.json at buffers[0] (open with --lenient to ignore it)"
        );
        assert_eq!(Manifest::from_value(v, true).unwrap().buffers.len(), 1);
    }

    #[test]
    fn rejects_bad_manifests() {
        assert_eq!(
            Manifest::from_value(json!([]), false),
            Err(ManifestError::NotAnObject)
        );
        let mut v =
            serde_json::to_value(Manifest::new(&BufferStore::new(), &ImageStore::new())).unwrap();
        v["format"] = json!("gltf");
        assert!(matches!(
            Manifest::from_value(v.clone(), true),
            Err(ManifestError::WrongFormat(_))
        ));
        v["format"] = json!("frap");
        v.as_object_mut().unwrap().remove("generator");
        assert!(matches!(
            Manifest::from_value(v, false),
            Err(ManifestError::Invalid(_))
        ));
        assert_eq!(
            Manifest::schema_version_of(&json!({"schema_version": 7})),
            Ok(7)
        );
        assert_eq!(
            Manifest::schema_version_of(&json!({"schema_version": "1"})),
            Err(ManifestError::MissingSchemaVersion)
        );
        assert_eq!(
            Manifest::schema_version_of(&json!({"schema_version": -1})),
            Err(ManifestError::MissingSchemaVersion)
        );
        assert_eq!(
            Manifest::schema_version_of(&json!({})),
            Err(ManifestError::MissingSchemaVersion)
        );
    }
}
