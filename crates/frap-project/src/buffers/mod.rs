// SPDX-License-Identifier: GPL-3.0-or-later
//! Content-addressed buffers (sha256, first 32 hex chars) and the inline threshold (< 64 elements) (WP 1.9).
//!
//! An array in `scene.json` is either written inline (a JSON array of elements, so small
//! edits stay visible in diffs) or as a reference to a `buffers/<hash>.bin` entry holding the
//! little-endian raw bytes:
//!
//! ```json
//! {"$buf": "9a0f…(32 hex)", "dtype": "f32x3", "len": 8}
//! ```
//!
//! `len` counts elements (8 positions), not scalars. Equal bytes are stored once whatever
//! their dtype; the dtype lives in the reference. An array is inline when it has fewer than
//! [`INLINE_THRESHOLD`] elements and every scalar has a JSON spelling (non-finite floats always
//! go to a buffer, since JSON has no NaN or infinity).
//!
//! `frap_rna::serde_buf` (WP 1.2) reaches a [`BufferStore`] through its thread-local
//! `BufferSink`/`BufferSource`; the glue lives in `scene_map` (WP 1.23).

mod dtype;
mod hash;
mod images;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

pub use dtype::{Dtype, Element, ParseDtypeError, Scalar, ScalarType};
pub use hash::{ContentHash, ParseHashError};
pub use images::{ImageRef, ImageStore, normalize_ext};

/// Arrays with fewer elements than this are written inline in `scene.json` (§15).
pub const INLINE_THRESHOLD: usize = 64;

/// Key of a buffer reference in `scene.json`.
pub const BUF_KEY: &str = "$buf";

/// Errors of buffer and image encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BufferError {
    #[error("buffer {0} is referenced but not present in the file")]
    Missing(ContentHash),
    #[error("buffer {hash} holds {actual} bytes, but {dtype} × {len} needs {expected}")]
    SizeMismatch {
        hash: ContentHash,
        dtype: Dtype,
        len: u64,
        expected: u64,
        actual: u64,
    },
    #[error("expected an array of {expected}, found dtype {found}")]
    DtypeMismatch { expected: Dtype, found: Dtype },
    #[error("buffer {hash}: {reason}")]
    InvalidData { hash: ContentHash, reason: String },
    #[error("content of {path} does not match its hash (computed {computed})")]
    HashMismatch { path: String, computed: ContentHash },
    #[error("invalid inline array: {0}")]
    InvalidInline(String),
    #[error("invalid image extension {0:?} (expected 1-8 ASCII letters or digits)")]
    InvalidImageExt(String),
}

/// `{"$buf": hash, "dtype": "f32x3", "len": n}`: an array stored in `buffers/<hash>.bin`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BufRef {
    #[serde(rename = "$buf")]
    pub hash: ContentHash,
    pub dtype: Dtype,
    /// Element count (not scalar count).
    pub len: u64,
}

impl BufRef {
    /// The archive entry holding the bytes.
    #[must_use]
    pub fn path(&self) -> String {
        buffer_path(&self.hash)
    }

    /// Byte size implied by `dtype` × `len` (`None` on overflow).
    #[must_use]
    pub fn byte_len(&self) -> Option<u64> {
        self.len.checked_mul(self.dtype.item_size() as u64)
    }
}

/// `buffers/<hash>.bin`.
#[must_use]
pub fn buffer_path(hash: &ContentHash) -> String {
    format!("buffers/{hash}.bin")
}

/// Whether `data` is written inline (fewer than [`INLINE_THRESHOLD`] elements, all JSON-representable).
#[must_use]
pub fn should_inline<T: Element>(data: &[T]) -> bool {
    data.len() < INLINE_THRESHOLD
        && data
            .iter()
            .all(|e| e.scalars().iter().all(|s| s.json_representable()))
}

/// Little-endian bytes of `data`.
#[must_use]
pub fn to_le_bytes<T: Element>(data: &[T]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * T::dtype().item_size());
    for e in data {
        for s in e.scalars() {
            s.write_le(&mut out);
        }
    }
    out
}

/// An array as written in `scene.json`: inline elements or a [`BufRef`]. Serialize it directly
/// into the output (not through `serde_json::Value`) so `f32` keeps its shortest spelling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoded<'a, T: Element> {
    Inline(&'a [T]),
    Buffer(BufRef),
}

impl<T: Element> Serialize for Encoded<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct One<'a, T>(&'a T);
        impl<T: Element> Serialize for One<'_, T> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.0.serialize_json(s)
            }
        }
        match self {
            Self::Inline(data) => {
                let mut seq = s.serialize_seq(Some(data.len()))?;
                for e in *data {
                    seq.serialize_element(&One(e))?;
                }
                seq.end()
            }
            Self::Buffer(r) => r.serialize(s),
        }
    }
}

/// The `buffers/` entries of one `.frap`, keyed (and written) in hash order.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct BufferStore {
    bufs: BTreeMap<ContentHash, Arc<[u8]>>,
}

impl BufferStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `bytes` (once per distinct content) and returns their hash.
    pub fn insert(&mut self, bytes: Vec<u8>) -> ContentHash {
        let hash = ContentHash::of(&bytes);
        self.bufs.entry(hash).or_insert_with(|| bytes.into());
        hash
    }

    /// Stores bytes read from `path`, checking that they hash to `hash`.
    pub fn insert_verified(
        &mut self,
        hash: ContentHash,
        bytes: Vec<u8>,
        path: &str,
    ) -> Result<(), BufferError> {
        let computed = ContentHash::of(&bytes);
        if computed != hash {
            return Err(BufferError::HashMismatch {
                path: path.to_owned(),
                computed,
            });
        }
        self.bufs.insert(hash, bytes.into());
        Ok(())
    }

    /// Bytes of a stored buffer.
    #[must_use]
    pub fn get(&self, hash: &ContentHash) -> Option<&[u8]> {
        self.bufs.get(hash).map(|b| &**b)
    }

    #[must_use]
    pub fn contains(&self, hash: &ContentHash) -> bool {
        self.bufs.contains_key(hash)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bufs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bufs.is_empty()
    }

    /// Buffers in hash order (the order of the archive entries and of `manifest.buffers`).
    pub fn iter(&self) -> impl Iterator<Item = (&ContentHash, &[u8])> {
        self.bufs.iter().map(|(h, b)| (h, &**b))
    }

    /// Keeps only the buffers for which `keep` returns true.
    pub fn retain(&mut self, mut keep: impl FnMut(&ContentHash) -> bool) {
        self.bufs.retain(|h, _| keep(h));
    }

    /// Stores raw little-endian bytes of `len` elements of `dtype`.
    pub fn put_raw(
        &mut self,
        dtype: Dtype,
        len: u64,
        bytes: Vec<u8>,
    ) -> Result<BufRef, BufferError> {
        let expected = len.checked_mul(dtype.item_size() as u64);
        if expected != Some(bytes.len() as u64) {
            let hash = ContentHash::of(&bytes);
            return Err(BufferError::SizeMismatch {
                hash,
                dtype,
                len,
                expected: expected.unwrap_or(u64::MAX),
                actual: bytes.len() as u64,
            });
        }
        Ok(BufRef {
            hash: self.insert(bytes),
            dtype,
            len,
        })
    }

    /// Stores `data` in a buffer, whatever its length.
    pub fn put<T: Element>(&mut self, data: &[T]) -> BufRef {
        let hash = self.insert(to_le_bytes(data));
        BufRef {
            hash,
            dtype: T::dtype(),
            len: data.len() as u64,
        }
    }

    /// Inline if [`should_inline`], else stored in a buffer.
    pub fn encode<'a, T: Element>(&mut self, data: &'a [T]) -> Encoded<'a, T> {
        if should_inline(data) {
            Encoded::Inline(data)
        } else {
            Encoded::Buffer(self.put(data))
        }
    }

    /// The raw bytes a reference points to, checked against `dtype` × `len`.
    pub fn get_raw(&self, r: &BufRef) -> Result<&[u8], BufferError> {
        let bytes = self.get(&r.hash).ok_or(BufferError::Missing(r.hash))?;
        let expected = r.byte_len();
        if expected != Some(bytes.len() as u64) {
            return Err(BufferError::SizeMismatch {
                hash: r.hash,
                dtype: r.dtype,
                len: r.len,
                expected: expected.unwrap_or(u64::MAX),
                actual: bytes.len() as u64,
            });
        }
        Ok(bytes)
    }

    /// Decodes a buffer reference into elements of `T` (the dtype must match exactly).
    pub fn load<T: Element>(&self, r: &BufRef) -> Result<Vec<T>, BufferError> {
        let expected = T::dtype();
        if r.dtype != expected {
            return Err(BufferError::DtypeMismatch {
                expected,
                found: r.dtype,
            });
        }
        let bytes = self.get_raw(r)?;
        let ssize = expected.scalar.size();
        let lanes = usize::from(expected.lanes);
        let mut scalars = Vec::with_capacity(lanes);
        let mut out = Vec::with_capacity(bytes.len() / expected.item_size().max(1));
        for item in bytes.chunks_exact(expected.item_size()) {
            scalars.clear();
            for (i, chunk) in item.chunks_exact(ssize).enumerate() {
                let s = T::Scalar::read_le(chunk).ok_or_else(|| BufferError::InvalidData {
                    hash: r.hash,
                    reason: format!(
                        "invalid {} at element {}, lane {i}",
                        expected.scalar.name(),
                        out.len()
                    ),
                })?;
                scalars.push(s);
            }
            out.push(T::from_scalars(&scalars));
        }
        Ok(out)
    }

    /// Decodes a `scene.json` array: inline elements or a `{"$buf", …}` reference.
    pub fn decode<T: Element>(&self, v: &Value) -> Result<Vec<T>, BufferError> {
        match v {
            Value::Array(items) => decode_inline(items),
            Value::Object(_) => {
                let r = BufRef::deserialize(v)
                    .map_err(|e| BufferError::InvalidInline(e.to_string()))?;
                self.load(&r)
            }
            other => Err(BufferError::InvalidInline(format!(
                "expected an array or a buffer reference, found {other}"
            ))),
        }
    }
}

/// Every buffer and image a `scene.json` tree references (`{"$buf", …}` / `{"$img", …}` objects
/// anywhere in it): what a save must write and a load must find.
#[must_use]
pub fn referenced(
    scene: &Value,
) -> (
    std::collections::BTreeSet<ContentHash>,
    std::collections::BTreeSet<ImageRef>,
) {
    fn walk(
        v: &Value,
        bufs: &mut std::collections::BTreeSet<ContentHash>,
        imgs: &mut std::collections::BTreeSet<ImageRef>,
    ) {
        match v {
            Value::Object(o) => {
                if o.contains_key(BUF_KEY)
                    && let Ok(r) = BufRef::deserialize(v)
                {
                    bufs.insert(r.hash);
                    return;
                }
                if o.contains_key("$img")
                    && let Ok(r) = ImageRef::deserialize(v)
                {
                    imgs.insert(r);
                    return;
                }
                o.values().for_each(|x| walk(x, bufs, imgs));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, bufs, imgs)),
            _ => {}
        }
    }
    let (mut bufs, mut imgs) = Default::default();
    walk(scene, &mut bufs, &mut imgs);
    (bufs, imgs)
}

fn decode_inline<T: Element>(items: &[Value]) -> Result<Vec<T>, BufferError> {
    let dtype = T::dtype();
    let bad = |i: usize, v: &Value| {
        BufferError::InvalidInline(format!("element {i} ({v}) is not a {dtype}"))
    };
    let mut scalars = Vec::with_capacity(usize::from(T::LANES));
    items
        .iter()
        .enumerate()
        .map(|(i, v)| {
            scalars.clear();
            if T::LANES == 1 {
                scalars.push(T::Scalar::from_json(v).ok_or_else(|| bad(i, v))?);
            } else {
                let lanes = v
                    .as_array()
                    .filter(|a| a.len() == usize::from(T::LANES))
                    .ok_or_else(|| bad(i, v))?;
                for s in lanes {
                    scalars.push(T::Scalar::from_json(s).ok_or_else(|| bad(i, v))?);
                }
            }
            Ok(T::from_scalars(&scalars))
        })
        .collect()
}

#[cfg(test)]
mod tests;
