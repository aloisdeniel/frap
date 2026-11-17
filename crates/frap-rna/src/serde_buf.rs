// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[serde(with = "frap_rna::serde_buf")]` for `Arc<Vec<T>>` fields (§15): goes through a
//! scoped [`BufferSink`]/[`BufferSource`] (thread-local during `.frap` save/load), so data
//! crates never depend on `frap-project`.
//!
//! Wire forms:
//!
//! * inline: the plain array (`[true, true, false]`, `[[0.0, 1.0], …]`); used when no sink is
//!   installed, or when the sink declines the buffer (`frap-project` inlines arrays shorter
//!   than 64 elements, §15);
//! * buffer: `{"$buf": "<id>", "dtype": "f32x3", "len": 8}`, the data stored by the sink as
//!   little-endian raw bytes (`len` items of `dtype`).
//!
//! Deserialization accepts both forms; a `$buf` reference needs a [`BufferSource`] installed
//! with [`with_source`]. `Option<Arc<Vec<T>>>` fields use [`option`].
//!
//! ```ignore
//! #[derive(Serialize, Deserialize)]
//! struct Attr { #[serde(with = "frap_rna::serde_buf")] data: Arc<Vec<[f32; 3]>> }
//! ```

use std::any::Any;
use std::cell::RefCell;
use std::sync::Arc;

use serde::de::{DeserializeOwned, Error as _};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Element types storable as a raw little-endian buffer.
pub trait BufElem: Copy + Serialize + DeserializeOwned + 'static {
    /// Size of one element in bytes.
    const SIZE: usize;
    /// `dtype` string: `f32`, `i32`, `u8`, `bool`, … and `f32x3` for `[f32; 3]`.
    fn dtype() -> String;
    /// Appends the little-endian bytes of `self`.
    fn write_le(&self, out: &mut Vec<u8>);
    /// Reads one element from exactly [`Self::SIZE`] bytes.
    fn read_le(b: &[u8]) -> Self;
}

macro_rules! buf_scalar {
    ($($t:ty => $name:literal),* $(,)?) => {$(
        impl BufElem for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn dtype() -> String {
                $name.to_owned()
            }
            fn write_le(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn read_le(b: &[u8]) -> Self {
                let mut a = [0u8; std::mem::size_of::<$t>()];
                a.copy_from_slice(b);
                <$t>::from_le_bytes(a)
            }
        }
    )*};
}

buf_scalar!(
    f32 => "f32", f64 => "f64",
    i8 => "i8", i16 => "i16", i32 => "i32", i64 => "i64",
    u8 => "u8", u16 => "u16", u32 => "u32", u64 => "u64",
);

impl BufElem for bool {
    const SIZE: usize = 1;
    fn dtype() -> String {
        "bool".to_owned()
    }
    fn write_le(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }
    fn read_le(b: &[u8]) -> Self {
        b[0] != 0
    }
}

impl<T: BufElem, const N: usize> BufElem for [T; N]
where
    [T; N]: Serialize + DeserializeOwned,
{
    const SIZE: usize = T::SIZE * N;
    fn dtype() -> String {
        format!("{}x{N}", T::dtype())
    }
    fn write_le(&self, out: &mut Vec<u8>) {
        for x in self {
            x.write_le(out);
        }
    }
    fn read_le(b: &[u8]) -> Self {
        std::array::from_fn(|i| T::read_le(&b[i * T::SIZE..(i + 1) * T::SIZE]))
    }
}

/// Receives buffers while a `.frap` is being written (implemented by `frap-project`).
pub trait BufferSink {
    /// Stores `len` items of `dtype` as little-endian `bytes` and returns the buffer id to
    /// reference, or `None` to write the array inline instead.
    fn put(&mut self, dtype: &str, len: usize, bytes: Vec<u8>) -> Option<String>;
}

/// Provides buffers while a `.frap` is being read (implemented by `frap-project`).
pub trait BufferSource {
    /// The bytes of buffer `id`, or `None` if it does not exist.
    fn get(&self, id: &str) -> Option<Vec<u8>>;
}

trait ErasedSink {
    fn put(&mut self, dtype: &str, len: usize, bytes: Vec<u8>) -> Option<String>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<S: BufferSink + 'static> ErasedSink for S {
    fn put(&mut self, dtype: &str, len: usize, bytes: Vec<u8>) -> Option<String> {
        BufferSink::put(self, dtype, len, bytes)
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

trait ErasedSource {
    fn get(&self, id: &str) -> Option<Vec<u8>>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<S: BufferSource + 'static> ErasedSource for S {
    fn get(&self, id: &str) -> Option<Vec<u8>> {
        BufferSource::get(self, id)
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

thread_local! {
    static SINK: RefCell<Option<Box<dyn ErasedSink>>> = const { RefCell::new(None) };
    static SOURCE: RefCell<Option<Box<dyn ErasedSource>>> = const { RefCell::new(None) };
}

/// Restores the previously installed sink/source on drop (also on panic).
struct Restore<F: FnMut()>(F);
impl<F: FnMut()> Drop for Restore<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Runs `f` with `sink` installed for this thread, then returns `f`'s result and the sink
/// (with every buffer it received). Nested calls restore the outer sink.
pub fn with_sink<S: BufferSink + 'static, R>(sink: S, f: impl FnOnce() -> R) -> (R, S) {
    let prev = SINK.with(|c| c.borrow_mut().replace(Box::new(sink)));
    let mut prev = Some(prev);
    let mut taken: Option<Box<dyn ErasedSink>> = None;
    let r = {
        let _guard = Restore(|| {
            let mine = SINK.with(|c| {
                let mut slot = c.borrow_mut();
                let mine = slot.take();
                *slot = prev.take().flatten();
                mine
            });
            if taken.is_none() {
                taken = mine;
            }
        });
        f()
    };
    let sink = taken
        .expect("invariant: the sink installed by with_sink is still there")
        .into_any()
        .downcast::<S>()
        .expect("invariant: with_sink gets back the sink type it installed");
    (r, *sink)
}

/// Runs `f` with `source` installed for this thread, then returns `f`'s result and the
/// source. Nested calls restore the outer source.
pub fn with_source<S: BufferSource + 'static, R>(source: S, f: impl FnOnce() -> R) -> (R, S) {
    let prev = SOURCE.with(|c| c.borrow_mut().replace(Box::new(source)));
    let mut prev = Some(prev);
    let mut taken: Option<Box<dyn ErasedSource>> = None;
    let r = {
        let _guard = Restore(|| {
            let mine = SOURCE.with(|c| {
                let mut slot = c.borrow_mut();
                let mine = slot.take();
                *slot = prev.take().flatten();
                mine
            });
            if taken.is_none() {
                taken = mine;
            }
        });
        f()
    };
    let source = taken
        .expect("invariant: the source installed by with_source is still there")
        .into_any()
        .downcast::<S>()
        .expect("invariant: with_source gets back the source type it installed");
    (r, *source)
}

/// The `{"$buf": …}` reference written in place of the array.
#[derive(Serialize, Deserialize)]
struct BufRef {
    #[serde(rename = "$buf")]
    buf: String,
    dtype: String,
    len: usize,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Repr<T> {
    Inline(Vec<T>),
    Buf(BufRef),
}

/// Serializes an `Arc<Vec<T>>` (inline or through the installed sink).
pub fn serialize<T: BufElem, S: Serializer>(v: &Arc<Vec<T>>, s: S) -> Result<S::Ok, S::Error> {
    serialize_slice(v.as_slice(), s)
}

fn serialize_slice<T: BufElem, S: Serializer>(v: &[T], s: S) -> Result<S::Ok, S::Error> {
    let dtype = T::dtype();
    let id = SINK.with(|c| {
        c.borrow_mut().as_mut().and_then(|sink| {
            let mut bytes = Vec::with_capacity(v.len() * T::SIZE);
            for x in v {
                x.write_le(&mut bytes);
            }
            sink.put(&dtype, v.len(), bytes)
        })
    });
    match id {
        Some(id) => {
            let mut m = s.serialize_map(Some(3))?;
            m.serialize_entry("$buf", &id)?;
            m.serialize_entry("dtype", &dtype)?;
            m.serialize_entry("len", &v.len())?;
            m.end()
        }
        None => v.serialize(s),
    }
}

/// Deserializes an `Arc<Vec<T>>` from either wire form.
pub fn deserialize<'de, T: BufElem, D: Deserializer<'de>>(d: D) -> Result<Arc<Vec<T>>, D::Error> {
    match Repr::<T>::deserialize(d)? {
        Repr::Inline(v) => Ok(Arc::new(v)),
        Repr::Buf(r) => decode(&r).map(Arc::new).map_err(D::Error::custom),
    }
}

fn decode<T: BufElem>(r: &BufRef) -> Result<Vec<T>, String> {
    let dtype = T::dtype();
    if r.dtype != dtype {
        return Err(format!(
            "buffer {} has dtype {}, expected {dtype}",
            r.buf, r.dtype
        ));
    }
    let bytes = SOURCE
        .with(|c| c.borrow().as_ref().map(|src| src.get(&r.buf)))
        .ok_or_else(|| {
            format!(
                "buffer {} referenced but no BufferSource is installed",
                r.buf
            )
        })?
        .ok_or_else(|| format!("buffer {} not found", r.buf))?;
    if bytes.len() != r.len * T::SIZE {
        return Err(format!(
            "buffer {} has {} bytes, expected {} ({} × {dtype})",
            r.buf,
            bytes.len(),
            r.len * T::SIZE,
            r.len
        ));
    }
    Ok(bytes.chunks_exact(T::SIZE).map(T::read_le).collect())
}

/// `#[serde(with = "frap_rna::serde_buf::option")]` for `Option<Arc<Vec<T>>>` fields
/// (`None` is `null`).
pub mod option {
    use super::*;

    /// Serializes `None` as `null`, `Some` like [`super::serialize`].
    pub fn serialize<T: BufElem, S: Serializer>(
        v: &Option<Arc<Vec<T>>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => super::serialize_slice(v.as_slice(), s),
            None => s.serialize_none(),
        }
    }

    /// Deserializes `null` as `None`, anything else like [`super::deserialize`].
    pub fn deserialize<'de, T: BufElem, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Arc<Vec<T>>>, D::Error> {
        match Option::<Repr<T>>::deserialize(d)? {
            None => Ok(None),
            Some(Repr::Inline(v)) => Ok(Some(Arc::new(v))),
            Some(Repr::Buf(r)) => decode(&r)
                .map(|v| Some(Arc::new(v)))
                .map_err(D::Error::custom),
        }
    }
}
