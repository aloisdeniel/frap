// SPDX-License-Identifier: GPL-3.0-or-later
//! Buffer layout of the spec-complete writer (WP 7.4): `frap_interleave` and the
//! UNSIGNED_BYTE indices of `frap_compat='SPEC'`.
//!
//! The add-on writes one tightly packed view per accessor (`io/exp/buffer.py`) and indices as
//! UNSIGNED_SHORT or UNSIGNED_INT (`gltf2_blender_gather_primitives`: `max_index < 65535`).
//! Both options re-lay out the gathered buffer once, after every accessor exists:
//!
//! - **Interleaving.** For each primitive in document order, its vertex attributes (then each
//!   morph target's) whose accessor is the only one in its ARRAY_BUFFER view, with no sparse
//!   storage and no other user, are written into one view with a `byteStride` (each element
//!   padded to 4 bytes; a group whose stride would pass the spec's 252 bytes is split, in
//!   attribute order). An accessor that two primitives share (objects sharing a mesh with
//!   different materials) is interleaved once, with the first primitive's attributes.
//!   Draco-compressed attributes (no view) are left alone; meshopt (WP 7.1) compresses the
//!   interleaved views afterwards.
//! - **UNSIGNED_BYTE indices** (`frap_compat='SPEC'`): an UNSIGNED_SHORT index accessor whose
//!   every value is below 255 (the type's primitive-restart value) is rewritten as
//!   UNSIGNED_BYTE in its own view.
//!
//! Every other view is copied with its alignment phase (as `frap_gltf::repack` does), and
//! every reference to a view is renumbered: accessors, sparse indices and values, images
//! (GLB / embedded textures) and `KHR_draco_mesh_compression`.

use std::collections::{HashMap, HashSet};

use frap_gltf::accessor::{AccessorData, read_accessor};
use frap_gltf::builder::smallest_index_data;
use frap_gltf::json::{Buffer, ComponentType, Document, Index, Target};
use frap_gltf::{BufferBuilder, GltfError};

use crate::export::ExportError;
use crate::export::gather::Gather;

const DRACO: &str = "KHR_draco_mesh_compression";

fn gltf_err(e: GltfError) -> ExportError {
    ExportError::Invalid(format!("spec-complete layout: {e}"))
}

/// The gathered buffer as a document `read_accessor` can read (views and accessors still
/// live in the builder, numbered from 0).
fn buffer_doc(g: &Gather) -> Document {
    Document {
        buffer_views: g.buf.views.clone(),
        accessors: g.buf.accessors.clone(),
        buffers: vec![Buffer {
            byte_length: g.buf.bytes.len() as u64,
            ..Buffer::default()
        }],
        ..Document::default()
    }
}

/// Decodes gathered accessor `a`.
pub(crate) fn read(g: &Gather, a: Index) -> Result<AccessorData, ExportError> {
    read_accessor(&buffer_doc(g), &[&g.buf.bytes], a).map_err(gltf_err)
}

/// Overwrites the bytes of gathered accessor `a` with `data` (same type and count; the view
/// must be tightly packed), and its `min`/`max` when `min_max` and it has them.
pub(crate) fn write_in_place(
    g: &mut Gather,
    a: Index,
    data: &AccessorData,
    min_max: bool,
) -> Result<(), ExportError> {
    let acc = g
        .buf
        .accessor(a)
        .ok_or_else(|| ExportError::Invalid(format!("accessor {a} does not exist")))?
        .clone();
    let Some(v) = acc.buffer_view else {
        return Ok(());
    };
    let view = g
        .buf
        .view(v)
        .ok_or_else(|| ExportError::Invalid(format!("buffer view {v} does not exist")))?;
    let bytes = data.encode();
    if view
        .byte_stride
        .is_some_and(|s| s as usize != data.element_size())
        || acc.component_type != data.component_type()
        || acc.count as usize != data.count()
    {
        return Err(ExportError::Invalid(format!(
            "accessor {a} cannot be rewritten in place"
        )));
    }
    let start = (view.byte_offset_or_default() + acc.byte_offset_or_default()) as usize;
    g.buf.bytes[start..start + bytes.len()].copy_from_slice(&bytes);
    if min_max
        && acc.min.is_some()
        && let Some((min, max)) = data.min_max()
        && let Some(acc) = g.buf.accessor_mut(a)
    {
        acc.min = Some(min);
        acc.max = Some(max);
    }
    Ok(())
}

/// How many objects reference each gathered view (accessors, sparse parts, images, Draco).
fn view_users(g: &Gather) -> HashMap<Index, usize> {
    let mut users: HashMap<Index, usize> = HashMap::new();
    let mut add = |v: Index| *users.entry(v).or_default() += 1;
    for acc in &g.buf.accessors {
        if let Some(v) = acc.buffer_view {
            add(v);
        }
        if let Some(sp) = &acc.sparse {
            add(sp.indices.buffer_view);
            add(sp.values.buffer_view);
        }
    }
    for img in &g.doc.images {
        if let Some(v) = img.buffer_view {
            add(v);
        }
    }
    for p in g.doc.meshes.iter().flat_map(|m| &m.primitives) {
        if let Some(v) = draco_view(&p.extensions) {
            add(v);
        }
    }
    users
}

fn draco_view(e: &frap_gltf::Extensions) -> Option<Index> {
    e.get_raw(DRACO)?
        .get("bufferView")?
        .as_u64()
        .and_then(|v| Index::try_from(v).ok())
}

/// The interleave groups: accessor lists in attribute order, each sharing one new view.
fn interleave_groups(g: &Gather, users: &HashMap<Index, usize>) -> Vec<Vec<Index>> {
    let mut grouped: HashSet<Index> = HashSet::new();
    let mut groups = Vec::new();
    let eligible = |a: Index, grouped: &HashSet<Index>| {
        let Some(acc) = g.buf.accessor(a) else {
            return false;
        };
        let Some(v) = acc.buffer_view else {
            return false;
        };
        !grouped.contains(&a)
            && acc.sparse.is_none()
            && acc.byte_offset_or_default() == 0
            && users.get(&v) == Some(&1)
            && g.buf.view(v).is_some_and(|view| {
                view.target == Some(Target::ArrayBuffer) && view.extensions.is_empty()
            })
    };
    for p in g.doc.meshes.iter().flat_map(|m| &m.primitives) {
        if p.extensions.contains(DRACO) {
            continue;
        }
        let sets = std::iter::once(&p.attributes).chain(&p.targets);
        for set in sets {
            let mut cols: Vec<Index> = Vec::new();
            for &a in set.values() {
                if eligible(a, &grouped) && !cols.contains(&a) {
                    cols.push(a);
                }
            }
            // Same count only (the spec requires it of a primitive's attributes anyway).
            let Some(count) = cols
                .first()
                .and_then(|&a| g.buf.accessor(a))
                .map(|a| a.count)
            else {
                continue;
            };
            cols.retain(|&a| g.buf.accessor(a).is_some_and(|acc| acc.count == count));
            // Split at the 252-byte stride limit.
            let mut chunks: Vec<Vec<Index>> = vec![Vec::new()];
            let mut stride = 0usize;
            for a in cols {
                let size = g.buf.accessor(a).map_or(0, |acc| {
                    frap_gltf::accessor::element_size(acc.component_type, acc.type_).div_ceil(4) * 4
                });
                if stride + size > 252 && chunks.last().is_some_and(|c| !c.is_empty()) {
                    chunks.push(Vec::new());
                    stride = 0;
                }
                stride += size;
                chunks.last_mut().expect("never empty").push(a);
            }
            for c in chunks.into_iter().filter(|c| c.len() > 1) {
                grouped.extend(c.iter().copied());
                groups.push(c);
            }
        }
    }
    groups
}

/// The UNSIGNED_SHORT index accessors that fit UNSIGNED_BYTE, with their new data.
fn byte_indices(
    g: &Gather,
    users: &HashMap<Index, usize>,
) -> Result<HashMap<Index, AccessorData>, ExportError> {
    let mut out = HashMap::new();
    for p in g.doc.meshes.iter().flat_map(|m| &m.primitives) {
        let Some(a) = p.indices else { continue };
        if out.contains_key(&a) {
            continue;
        }
        let Some(acc) = g.buf.accessor(a) else {
            continue;
        };
        let Some(v) = acc.buffer_view else { continue };
        if acc.component_type != ComponentType::UnsignedShort
            || acc.sparse.is_some()
            || acc.byte_offset_or_default() != 0
            || users.get(&v) != Some(&1)
        {
            continue;
        }
        let ix = read(g, a)?.to_u32().map_err(gltf_err)?;
        if let Some(d) = smallest_index_data(&ix) {
            out.insert(a, d);
        }
    }
    Ok(out)
}

/// Re-lays out the gathered buffer for `frap_interleave` (`interleave`) and the UNSIGNED_BYTE
/// indices of SPEC (`bytes`). Does nothing when neither changes anything.
pub fn apply(g: &mut Gather, interleave: bool, bytes: bool) -> Result<(), ExportError> {
    if !(interleave || bytes) || g.buf.view_base != 0 || g.buf.accessor_base != 0 {
        return Ok(());
    }
    let users = view_users(g);
    let groups = if interleave {
        interleave_groups(g, &users)
    } else {
        Vec::new()
    };
    let replaced = if bytes {
        byte_indices(g, &users)?
    } else {
        HashMap::new()
    };
    if groups.is_empty() && replaced.is_empty() {
        return Ok(());
    }
    // Decode the grouped columns before the buffer changes.
    let mut columns: Vec<Vec<AccessorData>> = Vec::with_capacity(groups.len());
    for grp in &groups {
        columns.push(grp.iter().map(|&a| read(g, a)).collect::<Result<_, _>>()?);
    }
    // Old view → (group, anchor?) for grouped views, old view → accessor for replaced ones.
    let mut view_group: HashMap<Index, usize> = HashMap::new();
    for (gi, grp) in groups.iter().enumerate() {
        for &a in grp {
            if let Some(v) = g.buf.accessor(a).and_then(|acc| acc.buffer_view) {
                view_group.insert(v, gi);
            }
        }
    }
    let mut view_replaced: HashMap<Index, Index> = HashMap::new();
    for &a in replaced.keys() {
        if let Some(v) = g.buf.accessor(a).and_then(|acc| acc.buffer_view) {
            view_replaced.insert(v, a);
        }
    }

    let old = std::mem::take(&mut g.buf);
    let mut nb = BufferBuilder::new();
    let mut view_map: HashMap<Index, Index> = HashMap::new();
    let mut group_view: Vec<Option<(Index, Vec<u64>)>> = vec![None; groups.len()];
    for (i, view) in old.views.iter().enumerate() {
        let i = Index::try_from(i).expect("view count fits u32");
        if let Some(&gi) = view_group.get(&i) {
            if group_view[gi].is_none() {
                let cols: Vec<&AccessorData> = columns[gi].iter().collect();
                group_view[gi] = Some(nb.push_interleaved_view(&cols).map_err(gltf_err)?);
            }
            continue;
        }
        if let Some(a) = view_replaced.get(&i) {
            let data = &replaced[a];
            let nv = nb.push_view(&data.encode(), 1, None, Some(Target::ElementArrayBuffer));
            let new = nb.views.last_mut().expect("just pushed");
            new.name.clone_from(&view.name);
            view_map.insert(i, nv);
            continue;
        }
        let start = view.byte_offset_or_default() as usize;
        let end = start + view.byte_length as usize;
        let data = old.bytes.get(start..end).ok_or_else(|| {
            ExportError::Invalid(format!("buffer view {i} is outside the buffer"))
        })?;
        view_map.insert(i, nb.push_view_like(view, data));
    }

    // Renumber every view reference.
    let mut accessors = old.accessors;
    for (gi, grp) in groups.iter().enumerate() {
        let (nv, offsets) = group_view[gi].clone().expect("every group was written");
        for (&a, &off) in grp.iter().zip(&offsets) {
            let acc = &mut accessors[a as usize];
            acc.buffer_view = Some(nv);
            acc.byte_offset = (off != 0).then_some(off);
        }
    }
    let grouped: HashSet<Index> = groups.iter().flatten().copied().collect();
    for (a, acc) in accessors.iter_mut().enumerate() {
        let a = Index::try_from(a).expect("accessor count fits u32");
        if !grouped.contains(&a)
            && let Some(v) = acc.buffer_view
        {
            acc.buffer_view = view_map.get(&v).copied();
        }
        if let Some(d) = replaced.get(&a) {
            acc.component_type = d.component_type();
        }
        if let Some(sp) = &mut acc.sparse {
            if let Some(&v) = view_map.get(&sp.indices.buffer_view) {
                sp.indices.buffer_view = v;
            }
            if let Some(&v) = view_map.get(&sp.values.buffer_view) {
                sp.values.buffer_view = v;
            }
        }
    }
    nb.accessors = accessors;
    for img in &mut g.doc.images {
        if let Some(v) = img.buffer_view {
            img.buffer_view = view_map.get(&v).copied();
        }
    }
    for p in g.doc.meshes.iter_mut().flat_map(|m| &mut m.primitives) {
        if let Some(v) = draco_view(&p.extensions)
            && let Some(&nv) = view_map.get(&v)
            && let Some(serde_json::Value::Object(o)) = p.extensions.0.get_mut(DRACO)
        {
            o.insert("bufferView".into(), nv.into());
        }
    }
    g.buf = nb;
    Ok(())
}
