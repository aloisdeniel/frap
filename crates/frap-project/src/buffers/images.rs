// SPDX-License-Identifier: GPL-3.0-or-later
//! Packed image files: `images/<hash>.<ext>`, original encoded bytes (PNG/JPEG/WebP/EXR/HDR/KTX2),
//! content-addressed like buffers and Stored (never re-compressed) in the zip (§15).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use super::{BufferError, ContentHash};

/// `{"$img": hash, "ext": "png"}`: a packed image file stored as `images/<hash>.<ext>`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    #[serde(rename = "$img")]
    pub hash: ContentHash,
    /// Lowercase file extension without the dot.
    pub ext: SmolStr,
}

impl ImageRef {
    /// The archive entry holding the bytes.
    #[must_use]
    pub fn path(&self) -> String {
        format!("images/{}.{}", self.hash, self.ext)
    }
}

/// Normalises an extension (`".PNG"` → `"png"`); 1-8 ASCII alphanumerics.
pub fn normalize_ext(ext: &str) -> Result<SmolStr, BufferError> {
    let e = ext.strip_prefix('.').unwrap_or(ext);
    if e.is_empty() || e.len() > 8 || !e.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(BufferError::InvalidImageExt(ext.to_owned()));
    }
    Ok(SmolStr::new(e.to_ascii_lowercase()))
}

/// The `images/` entries of one `.frap`, in (hash, ext) order.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ImageStore {
    images: BTreeMap<ImageRef, Arc<[u8]>>,
}

impl ImageStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores the encoded file bytes (once per distinct content and extension).
    pub fn insert(&mut self, bytes: Vec<u8>, ext: &str) -> Result<ImageRef, BufferError> {
        let r = ImageRef {
            hash: ContentHash::of(&bytes),
            ext: normalize_ext(ext)?,
        };
        self.images.entry(r.clone()).or_insert_with(|| bytes.into());
        Ok(r)
    }

    /// Stores bytes read from `path`, checking that they hash to `r.hash`.
    pub fn insert_verified(
        &mut self,
        r: ImageRef,
        bytes: Vec<u8>,
        path: &str,
    ) -> Result<(), BufferError> {
        let computed = ContentHash::of(&bytes);
        if computed != r.hash {
            return Err(BufferError::HashMismatch {
                path: path.to_owned(),
                computed,
            });
        }
        if normalize_ext(&r.ext)? != r.ext {
            return Err(BufferError::InvalidImageExt(r.ext.to_string()));
        }
        self.images.insert(r, bytes.into());
        Ok(())
    }

    #[must_use]
    pub fn get(&self, r: &ImageRef) -> Option<&[u8]> {
        self.images.get(r).map(|b| &**b)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.images.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Images in (hash, ext) order (the order of the archive entries and of `manifest.images`).
    pub fn iter(&self) -> impl Iterator<Item = (&ImageRef, &[u8])> {
        self.images.iter().map(|(r, b)| (r, &**b))
    }

    /// Keeps only the images for which `keep` returns true.
    pub fn retain(&mut self, mut keep: impl FnMut(&ImageRef) -> bool) {
        self.images.retain(|r, _| keep(r));
    }
}
