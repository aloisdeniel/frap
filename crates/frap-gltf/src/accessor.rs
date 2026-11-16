// SPDX-License-Identifier: GPL-3.0-or-later
//! Accessor encode/decode: normalized ints, sparse, byteStride, min/max, quantized types (§17.1). WP 1.5.
//!
//! Decoding follows the glTF 2.0 spec (§3.6.2 accessors, §3.6.2.4 sparse) and matches
//! io_scene_gltf2's `BinaryData.decode_accessor_obj` (`io/imp/gltf2_io_binary.py`, Apache-2.0,
//! The glTF-Blender-IO authors): base data from the buffer view (honouring `byteStride`) or
//! zeros, sparse substitution of raw values, then normalization computed in f64 and rounded to
//! f32. Unlike the add-on, MAT2/MAT3 column padding is handled.

use base64::Engine as _;

use crate::error::GltfError;
use crate::json::{Accessor, AccessorType, ComponentType, Document};

/// Most components an accessor without a buffer view may expand to (it allocates zeros).
const MAX_ZERO_FILL: usize = 1 << 28;

/// Raw component values of an accessor, element-major, without padding.
#[derive(Clone, Debug, PartialEq)]
pub enum Components {
    /// BYTE
    I8(Vec<i8>),
    /// UNSIGNED_BYTE
    U8(Vec<u8>),
    /// SHORT
    I16(Vec<i16>),
    /// UNSIGNED_SHORT
    U16(Vec<u16>),
    /// UNSIGNED_INT
    U32(Vec<u32>),
    /// FLOAT
    F32(Vec<f32>),
}

macro_rules! each_components {
    ($self:expr, $v:ident => $e:expr) => {
        match $self {
            Components::I8($v) => $e,
            Components::U8($v) => $e,
            Components::I16($v) => $e,
            Components::U16($v) => $e,
            Components::U32($v) => $e,
            Components::F32($v) => $e,
        }
    };
}

impl Components {
    /// `n` zero components of type `ct`.
    pub fn zeros(ct: ComponentType, n: usize) -> Components {
        match ct {
            ComponentType::Byte => Components::I8(vec![0; n]),
            ComponentType::UnsignedByte => Components::U8(vec![0; n]),
            ComponentType::Short => Components::I16(vec![0; n]),
            ComponentType::UnsignedShort => Components::U16(vec![0; n]),
            ComponentType::UnsignedInt => Components::U32(vec![0; n]),
            ComponentType::Float => Components::F32(vec![0.0; n]),
        }
    }

    /// The component type.
    pub fn component_type(&self) -> ComponentType {
        match self {
            Components::I8(_) => ComponentType::Byte,
            Components::U8(_) => ComponentType::UnsignedByte,
            Components::I16(_) => ComponentType::Short,
            Components::U16(_) => ComponentType::UnsignedShort,
            Components::U32(_) => ComponentType::UnsignedInt,
            Components::F32(_) => ComponentType::Float,
        }
    }

    /// Number of components.
    pub fn len(&self) -> usize {
        each_components!(self, v => v.len())
    }

    /// No components.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Component `i` widened to f64 (raw, not normalized).
    pub fn get_f64(&self, i: usize) -> f64 {
        each_components!(self, v => f64::from(v[i]))
    }

    /// Reads component `i` from little-endian `bytes` at `at` into slot `i`.
    fn read_le(&mut self, i: usize, bytes: &[u8], at: usize) {
        match self {
            Components::I8(v) => v[i] = i8::from_le_bytes([bytes[at]]),
            Components::U8(v) => v[i] = bytes[at],
            Components::I16(v) => v[i] = i16::from_le_bytes([bytes[at], bytes[at + 1]]),
            Components::U16(v) => v[i] = u16::from_le_bytes([bytes[at], bytes[at + 1]]),
            Components::U32(v) => {
                v[i] = u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
            }
            Components::F32(v) => {
                v[i] = f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
            }
        }
    }

    /// Appends component `i` as little-endian bytes.
    fn write_le(&self, i: usize, out: &mut Vec<u8>) {
        each_components!(self, v => out.extend_from_slice(&v[i].to_le_bytes()))
    }

    /// Copies component `j` of `src` (same type) into slot `i`.
    pub(crate) fn copy_from(&mut self, i: usize, src: &Components, j: usize) {
        match (self, src) {
            (Components::I8(d), Components::I8(s)) => d[i] = s[j],
            (Components::U8(d), Components::U8(s)) => d[i] = s[j],
            (Components::I16(d), Components::I16(s)) => d[i] = s[j],
            (Components::U16(d), Components::U16(s)) => d[i] = s[j],
            (Components::U32(d), Components::U32(s)) => d[i] = s[j],
            (Components::F32(d), Components::F32(s)) => d[i] = s[j],
            _ => unreachable!("sparse values share the accessor's component type"),
        }
    }
}

/// A component type that can appear in an accessor.
pub trait Component: Copy + 'static {
    /// The glTF component type.
    const COMPONENT_TYPE: ComponentType;
    /// Wraps a vector of this type.
    fn wrap(v: Vec<Self>) -> Components;
}

macro_rules! component {
    ($t:ty, $ct:ident, $var:ident) => {
        impl Component for $t {
            const COMPONENT_TYPE: ComponentType = ComponentType::$ct;
            fn wrap(v: Vec<Self>) -> Components {
                Components::$var(v)
            }
        }
    };
}
component!(i8, Byte, I8);
component!(u8, UnsignedByte, U8);
component!(i16, Short, I16);
component!(u16, UnsignedShort, U16);
component!(u32, UnsignedInt, U32);
component!(f32, Float, F32);

/// A Rust value that maps to one accessor element (`f32` → FLOAT SCALAR, `[f32; 3]` → FLOAT
/// VEC3, `[u16; 4]` → UNSIGNED_SHORT VEC4, …). Matrices are built with [`AccessorData::new`].
pub trait Element: Copy {
    /// Component type.
    type C: Component;
    /// Element shape.
    const TYPE: AccessorType;
    /// Appends the components.
    fn push(&self, out: &mut Vec<Self::C>);
}

impl<T: Component> Element for T {
    type C = T;
    const TYPE: AccessorType = AccessorType::Scalar;
    fn push(&self, out: &mut Vec<T>) {
        out.push(*self);
    }
}

macro_rules! array_element {
    ($n:literal, $ty:ident) => {
        impl<T: Component> Element for [T; $n] {
            type C = T;
            const TYPE: AccessorType = AccessorType::$ty;
            fn push(&self, out: &mut Vec<T>) {
                out.extend_from_slice(self);
            }
        }
    };
}
array_element!(2, Vec2);
array_element!(3, Vec3);
array_element!(4, Vec4);

/// Decoded (or to-be-encoded) accessor contents.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessorData {
    /// Element shape.
    pub type_: AccessorType,
    /// Integers are normalized (`[0,1]` / `[-1,1]`).
    pub normalized: bool,
    /// Raw components, `count × type_.components()`.
    pub components: Components,
}

impl AccessorData {
    /// Checked constructor: `components.len()` must be a multiple of the shape's size, and
    /// `normalized` is only allowed on BYTE/UNSIGNED_BYTE/SHORT/UNSIGNED_SHORT.
    pub fn new(
        type_: AccessorType,
        normalized: bool,
        components: Components,
    ) -> Result<AccessorData, GltfError> {
        if !components.len().is_multiple_of(type_.components()) {
            return Err(GltfError::Data(format!(
                "{} components do not form whole {} elements",
                components.len(),
                type_.as_str()
            )));
        }
        if normalized && !components.component_type().can_normalize() {
            return Err(GltfError::Data(format!(
                "{:?} components cannot be normalized",
                components.component_type()
            )));
        }
        Ok(AccessorData {
            type_,
            normalized,
            components,
        })
    }

    /// Not-normalized data from typed elements.
    pub fn from_elements<E: Element>(elements: &[E]) -> AccessorData {
        let mut v = Vec::with_capacity(elements.len() * E::TYPE.components());
        for e in elements {
            e.push(&mut v);
        }
        AccessorData {
            type_: E::TYPE,
            normalized: false,
            components: E::C::wrap(v),
        }
    }

    /// Normalized data from typed integer elements (COLOR_0 as UNSIGNED_SHORT, …).
    ///
    /// # Panics
    /// For FLOAT or UNSIGNED_INT elements.
    pub fn normalized_from_elements<E: Element>(elements: &[E]) -> AccessorData {
        assert!(
            E::C::COMPONENT_TYPE.can_normalize(),
            "{:?} cannot be normalized",
            E::C::COMPONENT_TYPE
        );
        AccessorData {
            normalized: true,
            ..AccessorData::from_elements(elements)
        }
    }

    /// Component type.
    pub fn component_type(&self) -> ComponentType {
        self.components.component_type()
    }

    /// Number of elements.
    pub fn count(&self) -> usize {
        self.components.len() / self.type_.components()
    }

    /// Bytes per element, matrix column padding included.
    pub fn element_size(&self) -> usize {
        element_size(self.component_type(), self.type_)
    }

    /// Components as f32: normalized integers are dequantized (spec formulas, computed in f64
    /// as io_scene_gltf2 does), other integers are converted.
    pub fn to_f32(&self) -> Vec<f32> {
        let n = self.normalized;
        #[allow(clippy::cast_possible_truncation)] // f64 → f32 rounding is intended
        match &self.components {
            Components::F32(v) => v.clone(),
            Components::I8(v) if n => v
                .iter()
                .map(|&c| (f64::from(c) / 127.0).max(-1.0) as f32)
                .collect(),
            Components::U8(v) if n => v.iter().map(|&c| (f64::from(c) / 255.0) as f32).collect(),
            Components::I16(v) if n => v
                .iter()
                .map(|&c| (f64::from(c) / 32767.0).max(-1.0) as f32)
                .collect(),
            Components::U16(v) if n => v.iter().map(|&c| (f64::from(c) / 65535.0) as f32).collect(),
            Components::I8(v) => v.iter().map(|&c| f32::from(c)).collect(),
            Components::U8(v) => v.iter().map(|&c| f32::from(c)).collect(),
            Components::I16(v) => v.iter().map(|&c| f32::from(c)).collect(),
            Components::U16(v) => v.iter().map(|&c| f32::from(c)).collect(),
            Components::U32(v) => v.iter().map(|&c| c as f32).collect(),
        }
    }

    /// Elements as `[f32; N]` (see [`AccessorData::to_f32`]); `N` must equal the shape's
    /// component count.
    pub fn to_f32_elements<const N: usize>(&self) -> Result<Vec<[f32; N]>, GltfError> {
        if N != self.type_.components() {
            return Err(GltfError::Data(format!(
                "{} has {} components, not {N}",
                self.type_.as_str(),
                self.type_.components()
            )));
        }
        Ok(self
            .to_f32()
            .chunks_exact(N)
            .map(|c| c.try_into().expect("chunk of N"))
            .collect())
    }

    /// Components as u32, for non-normalized unsigned integer data (indices, joints).
    pub fn to_u32(&self) -> Result<Vec<u32>, GltfError> {
        if self.normalized {
            return Err(GltfError::Data("normalized data is not integral".into()));
        }
        match &self.components {
            Components::U8(v) => Ok(v.iter().map(|&c| u32::from(c)).collect()),
            Components::U16(v) => Ok(v.iter().map(|&c| u32::from(c)).collect()),
            Components::U32(v) => Ok(v.clone()),
            c => Err(GltfError::Data(format!(
                "{:?} data is not unsigned integer data",
                c.component_type()
            ))),
        }
    }

    /// Per-component `(min, max)` of the raw values, as glTF writes them; `None` when empty.
    /// NaNs are ignored.
    pub fn min_max(&self) -> Option<(Vec<f64>, Vec<f64>)> {
        let n = self.type_.components();
        if self.count() == 0 {
            return None;
        }
        let mut min = vec![f64::INFINITY; n];
        let mut max = vec![f64::NEG_INFINITY; n];
        for i in 0..self.components.len() {
            let v = self.components.get_f64(i);
            let j = i % n;
            if v < min[j] {
                min[j] = v;
            }
            if v > max[j] {
                max[j] = v;
            }
        }
        Some((min, max))
    }

    /// Little-endian bytes, element-major, each element [`element_size`] bytes (matrix
    /// columns padded to 4 bytes with zeros).
    pub fn encode(&self) -> Vec<u8> {
        let ct = self.component_type();
        let (cols, rows) = self.type_.columns_rows();
        let col_bytes = rows * ct.size();
        let col_stride = column_stride(ct, self.type_);
        let mut out = Vec::with_capacity(self.count() * self.element_size());
        let mut i = 0;
        for _ in 0..self.count() {
            for _ in 0..cols {
                for _ in 0..rows {
                    self.components.write_le(i, &mut out);
                    i += 1;
                }
                out.resize(out.len() + col_stride - col_bytes, 0);
            }
        }
        out
    }
}

/// Bytes between matrix columns (columns start on 4-byte boundaries); for vectors and
/// scalars, the element size.
pub fn column_stride(ct: ComponentType, ty: AccessorType) -> usize {
    let (_, rows) = ty.columns_rows();
    let bytes = rows * ct.size();
    match ty {
        AccessorType::Mat2 | AccessorType::Mat3 | AccessorType::Mat4 => bytes.div_ceil(4) * 4,
        _ => bytes,
    }
}

/// Bytes per element (MAT2/MAT3 of 1- and 2-byte types include column padding).
pub fn element_size(ct: ComponentType, ty: AccessorType) -> usize {
    ty.columns_rows().0 * column_stride(ct, ty)
}

/// Normalized float → raw integer, the inverse of the spec's dequantization: clamp to the
/// type's range, scale, round to nearest (halves away from zero, which for non-negative
/// values equals io_scene_gltf2's `+ 0.5` then truncate, `blender/exp/primitive_attributes.py`).
/// Returns `None` for FLOAT and UNSIGNED_INT, which have no normalized form.
pub fn quantize(ct: ComponentType, f: f32) -> Option<i64> {
    let (lo, scale) = match ct {
        ComponentType::Byte => (-1.0, 127.0),
        ComponentType::UnsignedByte => (0.0, 255.0),
        ComponentType::Short => (-1.0, 32767.0),
        ComponentType::UnsignedShort => (0.0, 65535.0),
        _ => return None,
    };
    let f = if f.is_nan() { 0.0 } else { f.clamp(lo, 1.0) };
    #[allow(clippy::cast_possible_truncation)] // in range by the clamp
    Some((f * scale).round() as i64)
}

/// Index component type for indices up to `max_index`: UNSIGNED_SHORT below 65535 (the
/// restart value is never used, UNSIGNED_BYTE is avoided), otherwise UNSIGNED_INT, as
/// io_scene_gltf2 chooses (`blender/exp/primitives.py`).
pub fn index_component_type(max_index: u32) -> ComponentType {
    if max_index < 65535 {
        ComponentType::UnsignedShort
    } else {
        ComponentType::UnsignedInt
    }
}

fn usize_of(v: u64, what: &str) -> Result<usize, GltfError> {
    usize::try_from(v).map_err(|_| GltfError::Data(format!("{what} {v} does not fit in memory")))
}

/// The bytes of buffer view `view`.
///
/// Fails for out-of-range indices, views extending past their buffer, and views compressed
/// with EXT_/KHR_meshopt_compression (decompression lives in frap-codec-mesh).
pub fn buffer_view_bytes<'a, B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &'a [B],
    view: u32,
) -> Result<&'a [u8], GltfError> {
    let bv = doc
        .buffer_views
        .get(view as usize)
        .ok_or(GltfError::Index {
            what: "bufferView",
            index: u64::from(view),
            len: doc.buffer_views.len(),
        })?;
    for ext in ["EXT_meshopt_compression", "KHR_meshopt_compression"] {
        if bv.extensions.contains(ext) {
            return Err(GltfError::Compressed {
                view,
                extension: ext.to_owned(),
            });
        }
    }
    let buf = buffers
        .get(bv.buffer as usize)
        .ok_or(GltfError::Index {
            what: "buffer",
            index: u64::from(bv.buffer),
            len: buffers.len(),
        })?
        .as_ref();
    let start = usize_of(bv.byte_offset_or_default(), "byteOffset")?;
    let len = usize_of(bv.byte_length, "byteLength")?;
    start
        .checked_add(len)
        .and_then(|end| buf.get(start..end))
        .ok_or_else(|| {
            GltfError::OutOfBounds(format!(
                "bufferView {view} (offset {start}, length {len}) exceeds buffer {} ({} bytes)",
                bv.buffer,
                buf.len()
            ))
        })
}

/// Decodes accessor `index` of `doc`; `buffers[i]` holds the bytes of `doc.buffers[i]`
/// (see [`load_buffers`]).
pub fn read_accessor<B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &[B],
    index: u32,
) -> Result<AccessorData, GltfError> {
    let acc = doc.accessors.get(index as usize).ok_or(GltfError::Index {
        what: "accessor",
        index: u64::from(index),
        len: doc.accessors.len(),
    })?;
    read_accessor_obj(doc, buffers, acc)
}

/// Decodes `count` tightly or `stride`-spaced elements from `bytes` starting at `offset`.
fn decode_dense(
    bytes: &[u8],
    offset: usize,
    stride: Option<usize>,
    count: usize,
    ct: ComponentType,
    ty: AccessorType,
    what: &dyn Fn() -> String,
) -> Result<Components, GltfError> {
    let elem = element_size(ct, ty);
    let stride = stride.unwrap_or(elem);
    let (cols, rows) = ty.columns_rows();
    let col_stride = column_stride(ct, ty);
    let csize = ct.size();
    if count > 0 {
        let need = (count - 1)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(offset))
            .and_then(|n| n.checked_add(elem));
        if need.is_none_or(|n| n > bytes.len()) {
            return Err(GltfError::OutOfBounds(format!(
                "{} needs {} bytes from offset {offset} (stride {stride}, {count} × {elem}-byte elements) but its buffer view has {}",
                what(),
                need.map_or_else(|| "too many".to_owned(), |n| (n - offset).to_string()),
                bytes.len()
            )));
        }
    }
    let mut out = Components::zeros(ct, count * cols * rows);
    let mut i = 0;
    for e in 0..count {
        let base = offset + e * stride;
        for c in 0..cols {
            for r in 0..rows {
                out.read_le(i, bytes, base + c * col_stride + r * csize);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Decodes an accessor object (which need not be in `doc.accessors`).
pub fn read_accessor_obj<B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &[B],
    acc: &Accessor,
) -> Result<AccessorData, GltfError> {
    let ct = acc.component_type;
    let ty = acc.type_;
    let n = ty.components();
    let count = usize_of(acc.count, "accessor count")?;
    let mut comps = match acc.buffer_view {
        Some(view) => {
            let bytes = buffer_view_bytes(doc, buffers, view)?;
            let stride = doc.buffer_views[view as usize]
                .byte_stride
                .map(|s| s as usize);
            if let Some(s) = stride
                && s < element_size(ct, ty)
            {
                return Err(GltfError::Data(format!(
                    "byteStride {s} is smaller than the {}-byte element",
                    element_size(ct, ty)
                )));
            }
            let offset = usize_of(acc.byte_offset_or_default(), "byteOffset")?;
            decode_dense(bytes, offset, stride, count, ct, ty, &|| {
                "accessor".to_owned()
            })?
        }
        None => {
            if count.saturating_mul(n) > MAX_ZERO_FILL {
                return Err(GltfError::Data(format!(
                    "accessor without bufferView has {count} elements, too many to zero-fill"
                )));
            }
            Components::zeros(ct, count * n)
        }
    };
    if let Some(sp) = &acc.sparse {
        let scount = usize_of(sp.count, "sparse count")?;
        if scount > count {
            return Err(GltfError::Data(format!(
                "sparse count {scount} exceeds accessor count {count}"
            )));
        }
        let ict = sp.indices.component_type;
        if !ict.is_unsigned_int() {
            return Err(GltfError::Data(format!(
                "sparse indices have component type {ict:?}"
            )));
        }
        let ibytes = buffer_view_bytes(doc, buffers, sp.indices.buffer_view)?;
        let ioff = usize_of(sp.indices.byte_offset.unwrap_or(0), "byteOffset")?;
        let indices = AccessorData {
            type_: AccessorType::Scalar,
            normalized: false,
            components: decode_dense(
                ibytes,
                ioff,
                None,
                scount,
                ict,
                AccessorType::Scalar,
                &|| "sparse indices".to_owned(),
            )?,
        }
        .to_u32()?;
        let vbytes = buffer_view_bytes(doc, buffers, sp.values.buffer_view)?;
        let voff = usize_of(sp.values.byte_offset.unwrap_or(0), "byteOffset")?;
        let values = decode_dense(vbytes, voff, None, scount, ct, ty, &|| {
            "sparse values".to_owned()
        })?;
        for (k, &idx) in indices.iter().enumerate() {
            let idx = idx as usize;
            if idx >= count {
                return Err(GltfError::OutOfBounds(format!(
                    "sparse index {idx} out of range (accessor count {count})"
                )));
            }
            for c in 0..n {
                comps.copy_from(idx * n + c, &values, k * n + c);
            }
        }
    }
    Ok(AccessorData {
        type_: ty,
        normalized: acc.normalized_or_default(),
        components: comps,
    })
}

/// Decodes a `data:` URI; `None` when `uri` is not a data URI. Only base64 payloads are
/// supported (the only form glTF exporters write). Returns `(media type, bytes)`.
pub fn decode_data_uri(uri: &str) -> Option<Result<(String, Vec<u8>), GltfError>> {
    let rest = uri.strip_prefix("data:")?;
    Some((|| {
        let (head, payload) = rest
            .split_once(',')
            .ok_or_else(|| GltfError::Data("data URI without ','".into()))?;
        let Some(mime) = head.strip_suffix(";base64") else {
            return Err(GltfError::Data(format!(
                "data URI {head:?} is not base64-encoded"
            )));
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|e| GltfError::Data(format!("bad base64 in data URI: {e}")))?;
        Ok((mime.to_owned(), bytes))
    })())
}

/// Encodes bytes as a base64 `data:` URI (`.gltf` with embedded buffers).
pub fn encode_data_uri(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// Percent-decodes a relative URI to a path (`"my%20file.bin"` → `"my file.bin"`). Invalid
/// escapes are kept literally.
pub fn uri_to_path(uri: &str) -> String {
    let b = uri.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(h) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
        {
            out.push(h);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The bytes of every buffer of `doc`, in order: data URIs are decoded, a buffer without
/// `uri` is the GLB `bin` chunk when it is buffer 0 (empty otherwise, e.g. a meshopt fallback
/// buffer), and other URIs go through `load` (which receives the URI as written). Each buffer
/// must hold at least `byteLength` bytes; longer data (GLB padding) is truncated to it.
pub fn load_buffers(
    doc: &Document,
    bin: Option<&[u8]>,
    mut load: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<Vec<u8>>, GltfError> {
    let mut out = Vec::with_capacity(doc.buffers.len());
    for (i, b) in doc.buffers.iter().enumerate() {
        let len = usize_of(b.byte_length, "byteLength")?;
        let mut data = match &b.uri {
            None if i == 0 => bin
                .ok_or_else(|| {
                    GltfError::Data("buffer 0 has no uri and there is no GLB BIN chunk".into())
                })?
                .to_vec(),
            None => {
                out.push(Vec::new());
                continue;
            }
            Some(uri) => match decode_data_uri(uri) {
                Some(r) => r?.1,
                None => load(uri).map_err(|message| GltfError::Load {
                    uri: uri.clone(),
                    message,
                })?,
            },
        };
        if data.len() < len {
            return Err(GltfError::OutOfBounds(format!(
                "buffer {i} declares {len} bytes but only {} are available",
                data.len()
            )));
        }
        data.truncate(len);
        out.push(data);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{Buffer, BufferView, Sparse, SparseIndices, SparseValues};

    fn doc_with(bytes: &[u8], stride: Option<u32>) -> Document {
        Document {
            buffers: vec![Buffer {
                byte_length: bytes.len() as u64,
                ..Buffer::default()
            }],
            buffer_views: vec![BufferView {
                buffer: 0,
                byte_length: bytes.len() as u64,
                byte_stride: stride,
                ..BufferView::default()
            }],
            ..Document::default()
        }
    }

    #[test]
    fn normalized_decode_matches_spec() {
        let d = AccessorData {
            type_: AccessorType::Scalar,
            normalized: true,
            components: Components::I8(vec![-128, -127, 0, 127]),
        };
        assert_eq!(d.to_f32(), [-1.0, -1.0, 0.0, 1.0]);
        let d = AccessorData {
            type_: AccessorType::Scalar,
            normalized: true,
            components: Components::U16(vec![0, 65535, 32768]),
        };
        assert_eq!(d.to_f32(), [0.0, 1.0, (32768.0f64 / 65535.0) as f32]);
        let d = AccessorData {
            type_: AccessorType::Scalar,
            normalized: true,
            components: Components::I16(vec![-32768, 16384]),
        };
        assert_eq!(d.to_f32(), [-1.0, (16384.0f64 / 32767.0) as f32]);
        let d = AccessorData {
            type_: AccessorType::Scalar,
            normalized: true,
            components: Components::U8(vec![0, 128, 255]),
        };
        assert_eq!(d.to_f32(), [0.0, (128.0f64 / 255.0) as f32, 1.0]);
    }

    #[test]
    fn quantize_round_trips() {
        for ct in [
            ComponentType::Byte,
            ComponentType::UnsignedByte,
            ComponentType::Short,
            ComponentType::UnsignedShort,
        ] {
            let max = match ct {
                ComponentType::Byte => 127,
                ComponentType::UnsignedByte => 255,
                ComponentType::Short => 32767,
                _ => 65535,
            };
            for raw in [0i64, 1, max / 2, max - 1, max] {
                let f = raw as f32 / max as f32;
                assert_eq!(quantize(ct, f), Some(raw), "{ct:?} {raw}");
            }
            assert_eq!(quantize(ct, 2.0), Some(max));
        }
        assert_eq!(quantize(ComponentType::Byte, -5.0), Some(-127));
        assert_eq!(quantize(ComponentType::UnsignedByte, -5.0), Some(0));
        assert_eq!(quantize(ComponentType::Float, 0.5), None);
        assert_eq!(quantize(ComponentType::UnsignedInt, 0.5), None);
    }

    #[test]
    fn matrix_columns_are_padded() {
        assert_eq!(
            element_size(ComponentType::UnsignedByte, AccessorType::Mat2),
            8
        );
        assert_eq!(
            element_size(ComponentType::UnsignedByte, AccessorType::Mat3),
            12
        );
        assert_eq!(element_size(ComponentType::Short, AccessorType::Mat3), 24);
        assert_eq!(element_size(ComponentType::Short, AccessorType::Mat2), 8);
        assert_eq!(element_size(ComponentType::Float, AccessorType::Mat4), 64);
        assert_eq!(
            element_size(ComponentType::UnsignedByte, AccessorType::Vec3),
            3
        );
        let d = AccessorData::new(
            AccessorType::Mat3,
            false,
            Components::U8((1..=18).collect()),
        )
        .unwrap();
        let bytes = d.encode();
        assert_eq!(
            bytes,
            [
                1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 0, 10, 11, 12, 0, 13, 14, 15, 0, 16, 17, 18, 0
            ]
        );
        let doc = doc_with(&bytes, None);
        let mut acc = Accessor::new(Some(0), ComponentType::UnsignedByte, AccessorType::Mat3, 2);
        acc.byte_offset = Some(0);
        assert_eq!(read_accessor_obj(&doc, &[&bytes], &acc).unwrap(), d);
    }

    #[test]
    fn strided_decode() {
        // Two VEC2 u16 elements, stride 8: [1,2,pad,pad] [3,4,pad,pad]
        let bytes: Vec<u8> = [1u16, 2, 0xAAAA, 0xAAAA, 3, 4, 0xBBBB, 0xBBBB]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let doc = doc_with(&bytes, Some(8));
        let acc = Accessor::new(Some(0), ComponentType::UnsignedShort, AccessorType::Vec2, 2);
        let d = read_accessor_obj(&doc, &[&bytes], &acc).unwrap();
        assert_eq!(d.components, Components::U16(vec![1, 2, 3, 4]));
        // Offset into the stride.
        let mut acc2 = acc.clone();
        acc2.byte_offset = Some(4);
        acc2.type_ = AccessorType::Scalar;
        let d = read_accessor_obj(&doc, &[&bytes], &acc2).unwrap();
        assert_eq!(d.components, Components::U16(vec![0xAAAA, 0xBBBB]));
        // Stride smaller than the element is rejected.
        let doc_bad = doc_with(&bytes, Some(2));
        assert!(read_accessor_obj(&doc_bad, &[&bytes], &acc).is_err());
        // Out of range.
        let mut acc3 = acc.clone();
        acc3.count = 3;
        assert!(matches!(
            read_accessor_obj(&doc, &[&bytes], &acc3),
            Err(GltfError::OutOfBounds(_))
        ));
    }

    #[test]
    fn sparse_without_buffer_view() {
        // indices u8 [1, 3], values VEC2 f32 [(1,2), (3,4)]
        let mut bytes = vec![1u8, 3, 0, 0];
        for v in [1.0f32, 2.0, 3.0, 4.0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let mut doc = doc_with(&bytes, None);
        doc.buffer_views = vec![
            BufferView {
                buffer: 0,
                byte_length: 2,
                ..BufferView::default()
            },
            BufferView {
                buffer: 0,
                byte_offset: Some(4),
                byte_length: 16,
                ..BufferView::default()
            },
        ];
        let mut acc = Accessor::new(None, ComponentType::Float, AccessorType::Vec2, 4);
        acc.sparse = Some(Sparse {
            count: 2,
            indices: SparseIndices {
                buffer_view: 0,
                byte_offset: None,
                component_type: ComponentType::UnsignedByte,
                extensions: Default::default(),
                extras: None,
            },
            values: SparseValues {
                buffer_view: 1,
                byte_offset: None,
                extensions: Default::default(),
                extras: None,
            },
            extensions: Default::default(),
            extras: None,
        });
        let d = read_accessor_obj(&doc, &[&bytes], &acc).unwrap();
        assert_eq!(
            d.to_f32_elements::<2>().unwrap(),
            [[0.0, 0.0], [1.0, 2.0], [0.0, 0.0], [3.0, 4.0]]
        );
        // Index past count.
        acc.count = 2;
        assert!(read_accessor_obj(&doc, &[&bytes], &acc).is_err());
    }

    #[test]
    fn min_max_and_elements() {
        let d = AccessorData::from_elements(&[[1.0f32, -2.0, 3.0], [-1.0, 5.0, 0.5]]);
        assert_eq!(d.type_, AccessorType::Vec3);
        assert_eq!(d.count(), 2);
        assert_eq!(
            d.min_max(),
            Some((vec![-1.0, -2.0, 0.5], vec![1.0, 5.0, 3.0]))
        );
        let d = AccessorData::from_elements::<u16>(&[]);
        assert_eq!(d.min_max(), None);
        let d = AccessorData::normalized_from_elements(&[[65535u16, 0, 0, 65535]]);
        assert!(d.normalized);
        assert_eq!(d.to_f32(), [1.0, 0.0, 0.0, 1.0]);
        assert!(d.to_u32().is_err());
        assert!(
            AccessorData::new(AccessorType::Vec3, false, Components::F32(vec![0.0; 4])).is_err()
        );
        assert!(AccessorData::new(AccessorType::Scalar, true, Components::U32(vec![0])).is_err());
    }

    #[test]
    fn data_uris() {
        let uri = encode_data_uri("application/octet-stream", &[1, 2, 3, 250]);
        assert_eq!(uri, "data:application/octet-stream;base64,AQID+g==");
        let (mime, bytes) = decode_data_uri(&uri).unwrap().unwrap();
        assert_eq!(mime, "application/octet-stream");
        assert_eq!(bytes, [1, 2, 3, 250]);
        assert!(decode_data_uri("file.bin").is_none());
        assert!(decode_data_uri("data:text/plain,hello").unwrap().is_err());
        assert_eq!(uri_to_path("a%20b%2Fc.bin"), "a b/c.bin");
        assert_eq!(uri_to_path("50%"), "50%");
        assert_eq!(uri_to_path("%zz"), "%zz");
    }

    #[test]
    fn index_types() {
        assert_eq!(index_component_type(0), ComponentType::UnsignedShort);
        assert_eq!(index_component_type(65534), ComponentType::UnsignedShort);
        assert_eq!(index_component_type(65535), ComponentType::UnsignedInt);
    }

    #[test]
    fn load_buffers_rules() {
        let mut doc = Document {
            buffers: vec![
                Buffer {
                    byte_length: 3,
                    ..Buffer::default()
                },
                Buffer {
                    byte_length: 2,
                    uri: Some(encode_data_uri("application/octet-stream", &[7, 8])),
                    ..Buffer::default()
                },
                Buffer {
                    byte_length: 1,
                    uri: Some("ext.bin".into()),
                    ..Buffer::default()
                },
            ],
            ..Document::default()
        };
        let bufs = load_buffers(&doc, Some(&[1, 2, 3, 0]), |uri| {
            assert_eq!(uri, "ext.bin");
            Ok(vec![9])
        })
        .unwrap();
        assert_eq!(bufs, [vec![1, 2, 3], vec![7, 8], vec![9]]);
        assert!(load_buffers(&doc, None, |_| Ok(vec![9])).is_err());
        let e = load_buffers(&doc, Some(&[1, 2, 3]), |_| Err("nope".into())).unwrap_err();
        assert!(e.to_string().contains("ext.bin"));
        doc.buffers[0].byte_length = 8;
        assert!(load_buffers(&doc, Some(&[1, 2, 3, 0]), |_| Ok(vec![9])).is_err());
    }
}
