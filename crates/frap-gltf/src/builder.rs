// SPDX-License-Identifier: GPL-3.0-or-later
//! `BufferBuilder`: aligned buffer views, interleave option (§17.1). Named by the frozen
//! `PRIMITIVE_POST_PROCESS` contract (frap-io-gltf, §17.2). WP 1.5.
//!
//! One builder lays out one buffer (the GLB BIN chunk, or one `.bin`/data URI). It owns the
//! buffer views and accessors it creates and hands out their final indices, so post-processors
//! that only see the builder (Draco, meshopt) can still reference their views. Defaults follow
//! io_scene_gltf2 (`io/exp/buffer.py`, `blender/exp/accessors.py`): one view per accessor,
//! views 4-byte aligned, no interleaving.

use crate::accessor::{AccessorData, Components, buffer_view_bytes, index_component_type};
use crate::error::GltfError;
use crate::json::{Accessor, Buffer, BufferView, Document, Index, Sparse};
use crate::json::{ComponentType, SparseIndices, SparseValues, Target};

/// Per-accessor options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessorOpts {
    /// Buffer view target (ARRAY_BUFFER for vertex attributes, ELEMENT_ARRAY_BUFFER for
    /// indices, none for animation data, inverse bind matrices and sparse data).
    pub target: Option<Target>,
    /// Write `min`/`max` (required for POSITION and animation inputs).
    pub min_max: bool,
    /// Accessor name.
    pub name: Option<String>,
}

impl AccessorOpts {
    /// A vertex attribute (ARRAY_BUFFER), with or without min/max.
    pub fn attribute(min_max: bool) -> Self {
        AccessorOpts {
            target: Some(Target::ArrayBuffer),
            min_max,
            name: None,
        }
    }
}

/// Accumulates the bytes of one buffer with its buffer views and accessors.
#[derive(Clone, Debug, Default)]
pub struct BufferBuilder {
    /// Buffer bytes so far.
    pub bytes: Vec<u8>,
    /// Index of the buffer the views point at.
    pub buffer: Index,
    /// Views created so far; view `i` gets index `view_base + i`.
    pub views: Vec<BufferView>,
    /// Accessors created so far; accessor `i` gets index `accessor_base + i`.
    pub accessors: Vec<Accessor>,
    /// Index of the first view in the final document.
    pub view_base: Index,
    /// Index of the first accessor in the final document.
    pub accessor_base: Index,
}

fn idx(n: usize) -> Index {
    Index::try_from(n).expect("more than 2^32 glTF objects")
}

fn round_up(n: usize, to: usize) -> usize {
    n.div_ceil(to) * to
}

impl BufferBuilder {
    /// Builder for buffer 0 of an empty document.
    pub fn new() -> Self {
        BufferBuilder::default()
    }

    /// Builder appending to `doc`: a new buffer after its existing ones, views and accessors
    /// numbered after its existing ones.
    pub fn for_document(doc: &Document) -> Self {
        BufferBuilder {
            buffer: idx(doc.buffers.len()),
            view_base: idx(doc.buffer_views.len()),
            accessor_base: idx(doc.accessors.len()),
            ..BufferBuilder::default()
        }
    }

    /// The view with final index `index`.
    pub fn view(&self, index: Index) -> Option<&BufferView> {
        self.views.get(index.checked_sub(self.view_base)? as usize)
    }

    /// The accessor with final index `index`.
    pub fn accessor(&self, index: Index) -> Option<&Accessor> {
        self.accessors
            .get(index.checked_sub(self.accessor_base)? as usize)
    }

    /// Mutable access to the accessor with final index `index`.
    pub fn accessor_mut(&mut self, index: Index) -> Option<&mut Accessor> {
        self.accessors
            .get_mut(index.checked_sub(self.accessor_base)? as usize)
    }

    /// Pads the buffer with zeros so that its length is `≡ phase (mod align)`.
    fn align_to(&mut self, align: usize, phase: usize) {
        while self.bytes.len() % align != phase % align {
            self.bytes.push(0);
        }
    }

    /// Appends `data` as a new buffer view starting on a multiple of `max(align, 4)`, and
    /// returns its index.
    pub fn push_view(
        &mut self,
        data: &[u8],
        align: usize,
        byte_stride: Option<u32>,
        target: Option<Target>,
    ) -> Index {
        self.align_to(align.max(4), 0);
        self.push_view_at_phase(data, byte_stride, target)
    }

    fn push_view_at_phase(
        &mut self,
        data: &[u8],
        byte_stride: Option<u32>,
        target: Option<Target>,
    ) -> Index {
        let offset = self.bytes.len() as u64;
        self.bytes.extend_from_slice(data);
        self.views.push(BufferView {
            buffer: self.buffer,
            byte_offset: Some(offset),
            byte_length: data.len() as u64,
            byte_stride,
            target,
            ..BufferView::default()
        });
        self.view_base + idx(self.views.len() - 1)
    }

    /// Appends an accessor object as is (e.g. a Draco-compressed attribute without a buffer
    /// view) and returns its index.
    pub fn push_accessor_obj(&mut self, acc: Accessor) -> Index {
        self.accessors.push(acc);
        self.accessor_base + idx(self.accessors.len() - 1)
    }

    fn accessor_for(
        data: &AccessorData,
        view: Option<Index>,
        byte_offset: Option<u64>,
        opts: &AccessorOpts,
    ) -> Accessor {
        let mut acc = Accessor::new(view, data.component_type(), data.type_, data.count() as u64);
        acc.byte_offset = byte_offset;
        acc.normalized = data.normalized.then_some(true);
        acc.name = opts.name.clone();
        if opts.min_max
            && let Some((min, max)) = data.min_max()
        {
            acc.min = Some(min);
            acc.max = Some(max);
        }
        acc
    }

    /// Writes `data` into its own buffer view and adds an accessor for it.
    ///
    /// Vertex attributes (`target` ARRAY_BUFFER) whose element size is not a multiple of 4
    /// (VEC3 of bytes or shorts under KHR_mesh_quantization) get padded elements and a
    /// `byteStride`, as the spec requires.
    pub fn push_accessor(&mut self, data: &AccessorData, opts: &AccessorOpts) -> Index {
        let elem = data.element_size();
        let mut bytes = data.encode();
        let mut stride = None;
        if opts.target == Some(Target::ArrayBuffer) && !elem.is_multiple_of(4) {
            let padded = round_up(elem, 4);
            let mut out = Vec::with_capacity(padded * data.count());
            for e in bytes.chunks_exact(elem) {
                out.extend_from_slice(e);
                out.resize(out.len() + padded - elem, 0);
            }
            bytes = out;
            stride = Some(padded as u32);
        }
        let view = self.push_view(&bytes, data.component_type().size(), stride, opts.target);
        let acc = Self::accessor_for(data, Some(view), None, opts);
        self.push_accessor_obj(acc)
    }

    /// Adds an accessor for `data` with no buffer view (`count`, types, `normalized` and, with
    /// `opts.min_max`, `min`/`max` only): the attributes and indices of a Draco-compressed
    /// primitive, whose data lives in the Draco bitstream (M7 W0, `KHR_draco_mesh_compression`).
    pub fn push_accessor_without_view(
        &mut self,
        data: &AccessorData,
        opts: &AccessorOpts,
    ) -> Index {
        let acc = Self::accessor_for(data, None, None, opts);
        self.push_accessor_obj(acc)
    }

    /// [`BufferBuilder::push_indices`]'s accessor (same component type rule) with no buffer
    /// view: the indices of a Draco-compressed primitive (M7 W0).
    pub fn push_indices_without_view(&mut self, indices: &[u32]) -> Index {
        let max = indices.iter().copied().max().unwrap_or(0);
        let data = match index_component_type(max) {
            ComponentType::UnsignedShort => AccessorData::from_elements(
                &indices
                    .iter()
                    .map(|&i| u16::try_from(i).expect("checked by max"))
                    .collect::<Vec<_>>(),
            ),
            _ => AccessorData::from_elements(indices),
        };
        self.push_accessor_without_view(&data, &AccessorOpts::default())
    }

    /// Writes primitive indices as io_scene_gltf2 does: UNSIGNED_SHORT when every index is
    /// below 65535, else UNSIGNED_INT, ELEMENT_ARRAY_BUFFER, no min/max.
    pub fn push_indices(&mut self, indices: &[u32]) -> Index {
        let max = indices.iter().copied().max().unwrap_or(0);
        let data = match index_component_type(max) {
            ComponentType::UnsignedShort => AccessorData::from_elements(
                &indices
                    .iter()
                    .map(|&i| u16::try_from(i).expect("checked by max"))
                    .collect::<Vec<_>>(),
            ),
            _ => AccessorData::from_elements(indices),
        };
        self.push_accessor(
            &data,
            &AccessorOpts {
                target: Some(Target::ElementArrayBuffer),
                ..AccessorOpts::default()
            },
        )
    }

    /// Writes several same-count vertex attributes interleaved in one ARRAY_BUFFER view
    /// (each element padded to 4 bytes; `byteStride` = sum of padded sizes, at most 252).
    /// Returns the accessor indices in column order.
    pub fn push_interleaved(
        &mut self,
        columns: &[(&AccessorData, &AccessorOpts)],
    ) -> Result<Vec<Index>, GltfError> {
        if columns.is_empty() {
            return Ok(Vec::new());
        }
        let data: Vec<&AccessorData> = columns.iter().map(|(d, _)| *d).collect();
        let (view, offsets) = self.push_interleaved_view(&data)?;
        Ok(columns
            .iter()
            .zip(&offsets)
            .map(|((d, o), &off)| {
                let acc = Self::accessor_for(d, Some(view), Some(off), o);
                self.push_accessor_obj(acc)
            })
            .collect())
    }

    /// The byte stride [`BufferBuilder::push_interleaved_view`] gives `columns` (each element
    /// padded to 4 bytes).
    pub fn interleaved_stride(columns: &[&AccessorData]) -> usize {
        columns.iter().map(|d| round_up(d.element_size(), 4)).sum()
    }

    /// The view of [`BufferBuilder::push_interleaved`] without the accessors: the columns'
    /// elements interleaved in one ARRAY_BUFFER view with a `byteStride`. Returns the view
    /// index and each column's byte offset in it (WP 7.4: `frap_interleave` re-lays out
    /// accessors that already exist).
    pub fn push_interleaved_view(
        &mut self,
        columns: &[&AccessorData],
    ) -> Result<(Index, Vec<u64>), GltfError> {
        let Some(count) = columns.first().map(|d| d.count()) else {
            return Err(GltfError::Data("no interleaved columns".into()));
        };
        if let Some(d) = columns.iter().find(|d| d.count() != count) {
            return Err(GltfError::Data(format!(
                "interleaved columns differ in count ({count} vs {})",
                d.count()
            )));
        }
        let sizes: Vec<usize> = columns.iter().map(|d| d.element_size()).collect();
        let offsets: Vec<usize> = sizes
            .iter()
            .scan(0, |acc, &s| {
                let o = *acc;
                *acc += round_up(s, 4);
                Some(o)
            })
            .collect();
        let stride: usize = sizes.iter().map(|&s| round_up(s, 4)).sum();
        if stride > 252 {
            return Err(GltfError::Data(format!(
                "interleaved stride {stride} exceeds 252 bytes"
            )));
        }
        let encoded: Vec<Vec<u8>> = columns.iter().map(|d| d.encode()).collect();
        let mut bytes = vec![0u8; stride * count];
        for (c, enc) in encoded.iter().enumerate() {
            for (e, chunk) in enc.chunks_exact(sizes[c]).enumerate() {
                let at = e * stride + offsets[c];
                bytes[at..at + sizes[c]].copy_from_slice(chunk);
            }
        }
        let view = self.push_view(&bytes, 4, Some(stride as u32), Some(Target::ArrayBuffer));
        Ok((view, offsets.into_iter().map(|o| o as u64).collect()))
    }

    /// Appends a copy of `view` holding `data`: every member kept but `buffer`, `byteOffset`
    /// and `byteLength`, and the start offset keeps `view`'s phase modulo 4 (as [`repack`]
    /// does), so the accessors in it stay aligned. Returns the new view's index.
    pub fn push_view_like(&mut self, view: &BufferView, data: &[u8]) -> Index {
        let phase = (view.byte_offset_or_default() % 4) as usize;
        self.align_to(4, phase);
        let index = self.push_view_at_phase(data, view.byte_stride, view.target);
        let new = self.views.last_mut().expect("just pushed");
        *new = BufferView {
            buffer: new.buffer,
            byte_offset: new.byte_offset,
            byte_length: new.byte_length,
            ..view.clone()
        };
        index
    }

    /// Writes primitive indices as UNSIGNED_BYTE when every index is below 255 (the
    /// primitive-restart value of the type), else as [`BufferBuilder::push_indices`] does:
    /// the smallest index type glTF allows (WP 7.4, `frap_compat='SPEC'`; io_scene_gltf2
    /// never writes UNSIGNED_BYTE indices).
    pub fn push_indices_smallest(&mut self, indices: &[u32]) -> Index {
        match smallest_index_data(indices) {
            Some(data) => self.push_accessor(
                &data,
                &AccessorOpts {
                    target: Some(Target::ElementArrayBuffer),
                    ..AccessorOpts::default()
                },
            ),
            None => self.push_indices(indices),
        }
    }

    /// Writes `dense` as a sparse accessor: the elements at `indices` (strictly increasing)
    /// are stored as sparse values over `base` (zeros when `None`, as morph targets use).
    /// The index component type is the smallest that fits, as io_scene_gltf2 chooses
    /// (`blender/exp/accessors.py`). min/max describe `dense`.
    pub fn push_sparse(
        &mut self,
        dense: &AccessorData,
        indices: &[u32],
        base: Option<&AccessorData>,
        opts: &AccessorOpts,
    ) -> Result<Index, GltfError> {
        let n = dense.type_.components();
        if indices.windows(2).any(|w| w[0] >= w[1]) {
            return Err(GltfError::Data(
                "sparse indices must be strictly increasing".into(),
            ));
        }
        if let Some(&last) = indices.last()
            && last as usize >= dense.count()
        {
            return Err(GltfError::Data(format!(
                "sparse index {last} out of range ({} elements)",
                dense.count()
            )));
        }
        if let Some(b) = base
            && (b.count() != dense.count()
                || b.type_ != dense.type_
                || b.component_type() != dense.component_type())
        {
            return Err(GltfError::Data(
                "sparse base differs from the dense data in shape".into(),
            ));
        }
        let base_view =
            base.map(|b| self.push_view(&b.encode(), b.component_type().size(), None, opts.target));
        let max = indices.last().copied().unwrap_or(0);
        let index_data = if max <= 255 {
            AccessorData::from_elements(&indices.iter().map(|&i| i as u8).collect::<Vec<_>>())
        } else if max <= 65535 {
            AccessorData::from_elements(&indices.iter().map(|&i| i as u16).collect::<Vec<_>>())
        } else {
            AccessorData::from_elements(indices)
        };
        let index_ct = index_data.component_type();
        let index_view = self.push_view(&index_data.encode(), index_ct.size(), None, None);
        let mut values = Components::zeros(dense.component_type(), indices.len() * n);
        for (k, &i) in indices.iter().enumerate() {
            for c in 0..n {
                values.copy_from(k * n + c, &dense.components, i as usize * n + c);
            }
        }
        let values = AccessorData {
            type_: dense.type_,
            normalized: dense.normalized,
            components: values,
        };
        let values_view =
            self.push_view(&values.encode(), dense.component_type().size(), None, None);
        let mut acc = Self::accessor_for(dense, base_view, None, opts);
        acc.sparse = Some(Sparse {
            count: indices.len() as u64,
            indices: SparseIndices {
                buffer_view: index_view,
                byte_offset: None,
                component_type: index_ct,
                extensions: Default::default(),
                extras: None,
            },
            values: SparseValues {
                buffer_view: values_view,
                byte_offset: None,
                extensions: Default::default(),
                extras: None,
            },
            extensions: Default::default(),
            extras: None,
        });
        Ok(self.push_accessor_obj(acc))
    }

    /// Pads the bytes to a multiple of 4 and moves views and accessors into `doc`, creating
    /// (or updating the length of) buffer `self.buffer` with `uri` (`None` for a GLB BIN
    /// chunk). Returns the buffer bytes.
    ///
    /// # Panics
    /// When `doc` gained views, accessors or buffers since the builder was created (their
    /// indices would be wrong).
    pub fn finish(mut self, doc: &mut Document, uri: Option<String>) -> Vec<u8> {
        assert_eq!(
            doc.buffer_views.len(),
            self.view_base as usize,
            "document buffer views changed while a BufferBuilder was open"
        );
        assert_eq!(
            doc.accessors.len(),
            self.accessor_base as usize,
            "document accessors changed while a BufferBuilder was open"
        );
        self.align_to(4, 0);
        doc.buffer_views.append(&mut self.views);
        doc.accessors.append(&mut self.accessors);
        let b = self.buffer as usize;
        assert!(b <= doc.buffers.len(), "buffer index {b} skips buffers");
        if b == doc.buffers.len() {
            doc.buffers.push(Buffer::default());
        }
        doc.buffers[b].byte_length = self.bytes.len() as u64;
        doc.buffers[b].uri = uri;
        self.bytes
    }
}

/// `indices` as UNSIGNED_BYTE data when every index is below 255, else `None`.
pub fn smallest_index_data(indices: &[u32]) -> Option<AccessorData> {
    indices
        .iter()
        .map(|&i| u8::try_from(i).ok().filter(|&b| b != u8::MAX))
        .collect::<Option<Vec<u8>>>()
        .map(|b| AccessorData::from_elements(&b))
}

/// Copies every buffer view of `doc` into one new buffer 0 (a GLB BIN chunk) and returns the
/// rewritten document and the bytes. View and accessor indices are unchanged; only
/// `buffers` and each view's `buffer`/`byteOffset` change. Each view keeps its start offset
/// modulo 4, so accessor alignment is preserved. Images with a `uri` stay external.
///
/// Fails for views compressed with EXT_/KHR_meshopt_compression (their extension addresses
/// buffers directly).
pub fn repack<B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &[B],
) -> Result<(Document, Vec<u8>), GltfError> {
    let mut out = doc.clone();
    let mut b = BufferBuilder::new();
    for (i, bv) in doc.buffer_views.iter().enumerate() {
        let bytes = buffer_view_bytes(doc, buffers, idx(i))?;
        let phase = (bv.byte_offset_or_default() % 4) as usize;
        b.align_to(4, phase);
        b.push_view_at_phase(bytes, bv.byte_stride, bv.target);
        let new = b.views.last().expect("just pushed");
        let v = &mut out.buffer_views[i];
        v.buffer = 0;
        v.byte_offset = new.byte_offset;
    }
    b.align_to(4, 0);
    out.buffers = if b.bytes.is_empty() {
        Vec::new()
    } else {
        vec![Buffer {
            byte_length: b.bytes.len() as u64,
            ..Buffer::default()
        }]
    };
    Ok((out, b.bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessor::read_accessor;
    use crate::json::AccessorType;

    fn finish(b: BufferBuilder) -> (Document, Vec<u8>) {
        let mut doc = Document::default();
        let bytes = b.finish(&mut doc, None);
        (doc, bytes)
    }

    #[test]
    fn views_are_aligned_and_indices_typed() {
        let mut b = BufferBuilder::new();
        let a = b.push_accessor(
            &AccessorData::from_elements(&[1u8, 2, 3]),
            &AccessorOpts::default(),
        );
        let p = b.push_accessor(
            &AccessorData::from_elements(&[[0.0f32, 1.0, 2.0], [3.0, -4.0, 5.0]]),
            &AccessorOpts::attribute(true),
        );
        let i = b.push_indices(&[0, 1, 1]);
        let j = b.push_indices(&[0, 70000]);
        assert_eq!((a, p, i, j), (0, 1, 2, 3));
        let (doc, bytes) = finish(b);
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(doc.buffers[0].byte_length, bytes.len() as u64);
        for v in &doc.buffer_views {
            assert_eq!(v.byte_offset_or_default() % 4, 0);
        }
        assert_eq!(doc.accessors[1].min, Some(vec![0.0, -4.0, 2.0]));
        assert_eq!(doc.accessors[1].max, Some(vec![3.0, 1.0, 5.0]));
        assert_eq!(
            doc.accessors[2].component_type,
            ComponentType::UnsignedShort
        );
        assert_eq!(doc.accessors[3].component_type, ComponentType::UnsignedInt);
        assert_eq!(doc.buffer_views[2].target, Some(Target::ElementArrayBuffer));
        assert_eq!(
            read_accessor(&doc, &[&bytes], 1)
                .unwrap()
                .to_f32_elements::<3>()
                .unwrap(),
            [[0.0, 1.0, 2.0], [3.0, -4.0, 5.0]]
        );
        assert_eq!(
            read_accessor(&doc, &[&bytes], 3).unwrap().to_u32().unwrap(),
            [0, 70000]
        );
        assert!(crate::validate::validate_data(&doc, &[&bytes]).is_ok());
    }

    #[test]
    fn quantized_attributes_get_a_stride() {
        let mut b = BufferBuilder::new();
        let data = AccessorData::normalized_from_elements(&[[1i16, 2, 3], [4, 5, 6]]);
        b.push_accessor(&data, &AccessorOpts::attribute(false));
        let (doc, bytes) = finish(b);
        assert_eq!(doc.buffer_views[0].byte_stride, Some(8));
        assert_eq!(doc.buffer_views[0].byte_length, 16);
        assert_eq!(read_accessor(&doc, &[&bytes], 0).unwrap(), data);
    }

    #[test]
    fn interleaving() {
        let pos = AccessorData::from_elements(&[[1.0f32, 2.0, 3.0], [4.0, 5.0, 6.0]]);
        let uv = AccessorData::from_elements(&[[0.5f32, 0.25], [0.75, 1.0]]);
        let col = AccessorData::normalized_from_elements(&[[255u8, 0, 0], [0, 255, 0]]);
        let mut b = BufferBuilder::new();
        let ids = b
            .push_interleaved(&[
                (&pos, &AccessorOpts::attribute(true)),
                (&uv, &AccessorOpts::attribute(false)),
                (&col, &AccessorOpts::attribute(false)),
            ])
            .unwrap();
        assert_eq!(ids, [0, 1, 2]);
        let (doc, bytes) = finish(b);
        assert_eq!(doc.buffer_views.len(), 1);
        assert_eq!(doc.buffer_views[0].byte_stride, Some(12 + 8 + 4));
        assert_eq!(doc.accessors[1].byte_offset, Some(12));
        assert_eq!(doc.accessors[2].byte_offset, Some(20));
        assert_eq!(read_accessor(&doc, &[&bytes], 0).unwrap(), pos);
        assert_eq!(read_accessor(&doc, &[&bytes], 1).unwrap(), uv);
        assert_eq!(read_accessor(&doc, &[&bytes], 2).unwrap(), col);
        assert!(crate::validate::validate_data(&doc, &[&bytes]).is_ok());
        // The view alone (re-layout of existing accessors).
        let mut b = BufferBuilder::new();
        assert_eq!(BufferBuilder::interleaved_stride(&[&pos, &uv, &col]), 24);
        let (v, offsets) = b.push_interleaved_view(&[&pos, &uv, &col]).unwrap();
        assert_eq!((v, offsets), (0, vec![0, 12, 20]));
        assert!(b.accessors.is_empty());
        assert!(b.push_interleaved_view(&[]).is_err());
        // Mismatched counts.
        let short = AccessorData::from_elements(&[[0.0f32, 0.0]]);
        assert!(
            BufferBuilder::new()
                .push_interleaved(&[
                    (&pos, &AccessorOpts::default()),
                    (&short, &AccessorOpts::default())
                ])
                .is_err()
        );
    }

    #[test]
    fn byte_indices_and_view_copies() {
        let mut b = BufferBuilder::new();
        let small = b.push_indices_smallest(&[0, 1, 254]);
        let restart = b.push_indices_smallest(&[0, 255]);
        let wide = b.push_indices_smallest(&[0, 70000]);
        assert_eq!(
            b.accessor(small).unwrap().component_type,
            ComponentType::UnsignedByte
        );
        assert_eq!(
            b.accessor(restart).unwrap().component_type,
            ComponentType::UnsignedShort
        );
        assert_eq!(
            b.accessor(wide).unwrap().component_type,
            ComponentType::UnsignedInt
        );
        assert!(smallest_index_data(&[]).is_some());
        // A copy keeps the phase and the members.
        let src = BufferView {
            buffer: 3,
            byte_offset: Some(6),
            byte_length: 2,
            name: Some("v".into()),
            target: Some(Target::ElementArrayBuffer),
            ..BufferView::default()
        };
        let mut b = BufferBuilder::new();
        b.push_view(&[1], 1, None, None);
        let i = b.push_view_like(&src, &[7, 8]);
        let v = b.view(i).unwrap();
        assert_eq!(v.byte_offset_or_default() % 4, 2);
        assert_eq!(
            (v.buffer, v.byte_length, v.name.as_deref()),
            (0, 2, Some("v"))
        );
        assert_eq!(v.target, Some(Target::ElementArrayBuffer));
        let (doc, bytes) = finish(b);
        assert_eq!(&bytes[2..4], &[7, 8]);
        assert_eq!(doc.buffer_views[1].byte_offset, Some(2));
    }

    #[test]
    fn sparse_round_trip() {
        let dense = AccessorData::from_elements(&[
            [0.0f32, 0.0, 0.0],
            [1.0, 2.0, 3.0],
            [0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ]);
        let mut b = BufferBuilder::new();
        let s = b
            .push_sparse(&dense, &[1, 3], None, &AccessorOpts::attribute(true))
            .unwrap();
        let (doc, bytes) = finish(b);
        let acc = &doc.accessors[s as usize];
        assert_eq!(acc.buffer_view, None);
        let sp = acc.sparse.as_ref().unwrap();
        assert_eq!(sp.count, 2);
        assert_eq!(sp.indices.component_type, ComponentType::UnsignedByte);
        assert_eq!(acc.min, Some(vec![0.0, -1.0, 0.0]));
        assert_eq!(read_accessor(&doc, &[&bytes], s).unwrap(), dense);
        assert!(crate::validate::validate_data(&doc, &[&bytes]).is_ok());
        // With a base.
        let base = AccessorData::from_elements(&[[9.0f32, 9.0, 9.0]; 4]);
        let mut b = BufferBuilder::new();
        b.push_sparse(&dense, &[1], Some(&base), &AccessorOpts::default())
            .unwrap();
        let (doc, bytes) = finish(b);
        assert_eq!(
            read_accessor(&doc, &[&bytes], 0)
                .unwrap()
                .to_f32_elements::<3>()
                .unwrap(),
            [[9.0; 3], [1.0, 2.0, 3.0], [9.0; 3], [9.0; 3]]
        );
        assert!(
            BufferBuilder::new()
                .push_sparse(&dense, &[3, 1], None, &AccessorOpts::default())
                .is_err()
        );
        assert!(
            BufferBuilder::new()
                .push_sparse(&dense, &[4], None, &AccessorOpts::default())
                .is_err()
        );
    }

    #[test]
    fn for_document_appends() {
        let mut doc = Document::default();
        let mut b = BufferBuilder::new();
        b.push_accessor(
            &AccessorData::from_elements(&[1.0f32]),
            &AccessorOpts::default(),
        );
        let first = b.finish(&mut doc, None);
        let mut b = BufferBuilder::for_document(&doc);
        assert_eq!((b.buffer, b.view_base, b.accessor_base), (1, 1, 1));
        let a = b.push_accessor(
            &AccessorData::from_elements(&[2.0f32]),
            &AccessorOpts::default(),
        );
        assert_eq!(a, 1);
        assert_eq!(b.accessor(1).unwrap().count, 1);
        assert_eq!(b.view(1).unwrap().buffer, 1);
        assert!(b.view(0).is_none());
        let second = b.finish(&mut doc, Some("b.bin".into()));
        assert_eq!(doc.buffers.len(), 2);
        assert_eq!(doc.buffers[1].uri.as_deref(), Some("b.bin"));
        let bufs = [first, second];
        assert_eq!(read_accessor(&doc, &bufs, 1).unwrap().to_f32(), [2.0]);
    }

    #[test]
    fn repack_preserves_alignment_phase() {
        // View 0 at offset 2 holding one u16 accessor at offset 0, and f32 data at offset 4
        // of buffer 1.
        let b0 = vec![0xFF, 0xFF, 7, 0];
        let b1 = 1.5f32.to_le_bytes().to_vec();
        let mut doc = Document {
            buffers: vec![
                Buffer {
                    byte_length: 4,
                    ..Buffer::default()
                },
                Buffer {
                    byte_length: 4,
                    uri: Some("x.bin".into()),
                    ..Buffer::default()
                },
            ],
            ..Document::default()
        };
        doc.buffer_views = vec![
            BufferView {
                buffer: 0,
                byte_offset: Some(2),
                byte_length: 2,
                ..BufferView::default()
            },
            BufferView {
                buffer: 1,
                byte_length: 4,
                ..BufferView::default()
            },
        ];
        doc.accessors = vec![
            Accessor::new(
                Some(0),
                ComponentType::UnsignedShort,
                AccessorType::Scalar,
                1,
            ),
            Accessor::new(Some(1), ComponentType::Float, AccessorType::Scalar, 1),
        ];
        let (out, bytes) = repack(&doc, &[b0, b1]).unwrap();
        assert_eq!(out.buffers.len(), 1);
        assert_eq!(out.buffers[0].uri, None);
        assert_eq!(out.buffer_views[0].byte_offset_or_default() % 4, 2);
        assert_eq!(out.buffer_views[1].byte_offset_or_default() % 4, 0);
        assert_eq!(
            read_accessor(&out, &[&bytes], 0).unwrap().to_u32().unwrap(),
            [7]
        );
        assert_eq!(read_accessor(&out, &[&bytes], 1).unwrap().to_f32(), [1.5]);
    }
}
