// SPDX-License-Identifier: GPL-3.0-or-later
//! Draco primitive decoding (WP 7.2): every primitive with `KHR_draco_mesh_compression` is
//! decoded through `frap_codec_mesh::draco::codec`, its index and attribute accessors are
//! re-pointed at new buffer views over the decoded data, and the extension is removed (the
//! mesh importer then reads ordinary accessors).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/imp/
//! draco_compression_extension.py (`decode_primitive`, Apache-2.0, The glTF-Blender-IO
//! authors) and blender/intern/draco_bridge/intern/decoder.cpp (Apache-2.0, Blender Authors):
//!
//! - the primitives of the meshes the nodes use, with a POSITION attribute (`mesh.py` decodes
//!   them when it creates the mesh, after skipping primitives without POSITION);
//! - a bitstream that does not decode: ERROR `Draco Decoder: Unable to decode. Skipping
//!   primitive [unnamed].` (`prim.name` does not exist on a primitive, so the add-on always
//!   names it `[unnamed]`), and the accessors are left as they are;
//! - the indices first: a decoded count other than the accessor's is a WARNING and updates the
//!   accessor; the face indices are cast to the accessor's component type
//!   (`decodeIndices<T>`, C++ truncation);
//! - then each attribute of the extension, in its order: one the primitive lacks is an ERROR
//!   that skips the rest; a decoded point count other than the accessor's is a WARNING and
//!   updates it; the values are converted to the accessor's component type and component
//!   count with Draco's `GeometryAttribute::ConvertValue` (ported below) — a value it refuses
//!   is an ERROR that skips the rest;
//! - each decoded array becomes a buffer of its own with one view (`byteLength` only), and the
//!   accessor points at it. The accessor keeps its `byteOffset`, `normalized`, `min`/`max`.
//!
//! A primitive the add-on skips half-way keeps the accessors it re-pointed, and the others
//! read as zeros (no `bufferView`), as in Blender. A Draco primitive without `indices` makes the
//! add-on fail (`gltf.data.accessors[None]`): the import fails with the same message.
//!
//! Shared accessors: the add-on decodes a primitive and reads it at once (`mesh.py`
//! `do_primitives`), so when Draco primitives share an accessor (Blender's exporter shares the
//! view-less index accessors of equal content), each reads its own decoded data, and the
//! accessor is left with the last one's. frap decodes every primitive before reading the
//! meshes, so a primitive that writes an accessor an earlier one already wrote gets a copy of
//! it (the state that earlier decode left: count, component type, `normalized`, `min`/`max`)
//! and its reference points at the copy. Before its decode, a primitive's references point at
//! the accessor holding the latest decode of each, which is what Blender reads for a primitive
//! that is skipped or skipped half-way.
//!
//! Malformed bitstreams: `draco-core` and Google Draco 1.5.7 do not refuse exactly the same
//! corrupted streams (a mutated stream one decodes, possibly to out-of-range indices that make
//! the add-on raise, the other may refuse with `Unable to decode`); valid streams decode alike.

use frap_codec_mesh::draco::{self, DracoBackend, DracoDecoded};
use frap_codec_mesh::{AttrData, Attribute};
use frap_gltf::Document;
use frap_gltf::ext::khr_draco_mesh_compression::KhrDracoMeshCompression;
use frap_gltf::json::{BufferView, ComponentType, Index};
use frap_rna::{Report, ReportType};
use std::collections::HashMap;

use crate::import::ImportError;

const NAME: &str = "KHR_draco_mesh_compression";
/// `prim.name if hasattr(prim, 'name') else '[unnamed]'`: glTF primitives have no name.
const UNNAMED: &str = "[unnamed]";

/// Decodes the Draco primitives of `doc` (see the module docs).
pub(super) fn decode_primitives(
    doc: &mut Document,
    buffers: &mut Vec<Vec<u8>>,
    reports: &mut Vec<Report>,
) -> Result<(), ImportError> {
    let mut meshes: Vec<usize> = doc
        .nodes
        .iter()
        .filter_map(|n| n.mesh.map(|m| m as usize))
        .filter(|&m| m < doc.meshes.len())
        .collect();
    meshes.sort_unstable();
    meshes.dedup();
    let todo: Vec<(usize, usize)> = meshes
        .into_iter()
        .flat_map(|m| {
            doc.meshes[m]
                .primitives
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.extensions.contains(NAME) && p.attributes.contains_key("POSITION")
                })
                .map(move |(i, _)| (m, i))
                .collect::<Vec<_>>()
        })
        .collect();
    if todo.is_empty() {
        return Ok(());
    }
    let codec = draco::codec(DracoBackend::Auto)
        .ok_or_else(|| ImportError::NotImplemented(DracoBackend::Auto.unavailable().to_string()))?;
    let mut written = Written::default();
    for (m, p) in todo {
        decode_primitive(codec, doc, buffers, reports, &mut written, m, p)?;
        doc.meshes[m].primitives[p].extensions.remove(NAME);
    }
    Ok(())
}

/// The accessors earlier decodes wrote: the file's accessor index → the accessor holding the
/// latest decode's state of it (see the module docs, shared accessors).
#[derive(Default)]
struct Written(HashMap<Index, Index>);

impl Written {
    /// The accessor holding the latest state of the file's accessor `a`.
    fn current(&self, a: Index) -> Index {
        self.0.get(&a).copied().unwrap_or(a)
    }

    /// The accessor this primitive writes for the file's accessor `a`: `a` itself the first
    /// time, else a copy of its latest state.
    fn slot(&mut self, doc: &mut Document, a: Index) -> Result<Index, ImportError> {
        let cur = self.current(a);
        let acc = doc
            .accessors
            .get(cur as usize)
            .ok_or_else(|| ImportError::Data(format!("accessor {cur} out of range")))?
            .clone();
        let slot = if self.0.contains_key(&a) {
            doc.accessors.push(acc);
            (doc.accessors.len() - 1) as Index
        } else {
            a
        };
        self.0.insert(a, slot);
        Ok(slot)
    }
}

/// Appends `data` as a new buffer with one view over it; returns the view.
fn push_buffer(doc: &mut Document, buffers: &mut Vec<Vec<u8>>, data: Vec<u8>) -> Index {
    let buffer = buffers.len() as Index;
    doc.buffer_views.push(BufferView {
        buffer,
        byte_length: data.len() as u64,
        ..BufferView::default()
    });
    buffers.push(data);
    (doc.buffer_views.len() - 1) as Index
}

fn error(reports: &mut Vec<Report>, message: String) {
    reports.push(Report::new(ReportType::Error, message));
}

fn warning(reports: &mut Vec<Report>, message: String) {
    reports.push(Report::new(ReportType::Warning, message));
}

fn decode_primitive(
    codec: &dyn draco::DracoCodec,
    doc: &mut Document,
    buffers: &mut Vec<Vec<u8>>,
    reports: &mut Vec<Report>,
    written: &mut Written,
    m: usize,
    p: usize,
) -> Result<(), ImportError> {
    // The file's accessor indices of this primitive (the references are rewritten below).
    let prim = doc.meshes[m].primitives[p].clone();
    // What Blender reads for this primitive before (or without) its decode: the latest state.
    {
        let target = &mut doc.meshes[m].primitives[p];
        if let Some(i) = target.indices.as_mut() {
            *i = written.current(*i);
        }
        for a in target.attributes.values_mut() {
            *a = written.current(*a);
        }
    }
    let ext: KhrDracoMeshCompression = prim
        .extensions
        .get()
        .map_err(|e| ImportError::Data(e.to_string()))?
        .ok_or_else(|| ImportError::Data(format!("{NAME} missing")))?;
    let bytes = frap_gltf::accessor::buffer_view_bytes(doc, buffers, ext.buffer_view)?.to_vec();
    let ids: Vec<u32> = ext.attributes.values().copied().collect();
    // One decode for every attribute; a stream that decodes but lacks an id is decoded again
    // per attribute below, to report what the add-on reports.
    let all = codec.decode_counted(&bytes, &ids);
    let base = match &all {
        Ok(d) => DracoDecoded {
            mesh: frap_codec_mesh::MeshData {
                indices: d.mesh.indices.clone(),
                attributes: Vec::new(),
            },
            point_count: d.point_count,
        },
        Err(_) => match codec.decode_counted(&bytes, &[]) {
            Ok(d) => d,
            Err(_) => {
                error(
                    reports,
                    format!("Draco Decoder: Unable to decode. Skipping primitive {UNNAMED}."),
                );
                return Ok(());
            }
        },
    };

    // Indices.
    let Some(ia) = prim.indices else {
        return Err(ImportError::Data(
            "list indices must be integers or slices, not NoneType".into(),
        ));
    };
    let ia = written.slot(doc, ia)?;
    doc.meshes[m].primitives[p].indices = Some(ia);
    let ia = ia as usize;
    let acc = &mut doc.accessors[ia];
    let index_count = base.mesh.indices.len() as u64;
    if index_count != acc.count {
        warning(
            reports,
            "Draco Decoder: Index count of accessor and decoded index count does not match. \
             Updating accessor."
                .into(),
        );
        acc.count = index_count;
    }
    let ct = acc.component_type;
    let Some(data) = index_bytes(&base.mesh.indices, ct) else {
        error(
            reports,
            format!("Draco Decoder: Unable to decode indices. Skipping primitive {UNNAMED}."),
        );
        return Ok(());
    };
    let view = push_buffer(doc, buffers, data);
    doc.accessors[ia].buffer_view = Some(view);

    // Attributes.
    for (k, (attr, &id)) in ext.attributes.iter().enumerate() {
        let Some(&ai) = prim.attributes.get(attr) else {
            error(
                reports,
                format!(
                    "Draco Decoder: Draco attribute {attr} not in primitive attributes. \
                     Skipping primitive {UNNAMED}."
                ),
            );
            return Ok(());
        };
        let ai = written.slot(doc, ai)?;
        doc.meshes[m].primitives[p]
            .attributes
            .insert(attr.clone(), ai);
        let ai = ai as usize;
        let acc = &mut doc.accessors[ai];
        if u64::from(base.point_count) != acc.count {
            warning(
                reports,
                format!(
                    "Draco Decoder: Vertex count of accessor and decoded vertex count does not \
                     match for attribute {attr}. Updating accessor."
                ),
            );
            acc.count = u64::from(base.point_count);
        }
        let (ct, n) = (acc.component_type, acc.type_.components());
        let decoded = match &all {
            Ok(d) => d.mesh.attributes.get(k).cloned(),
            Err(_) => codec
                .decode(&bytes, &[id])
                .ok()
                .and_then(|d| d.attributes.into_iter().next()),
        };
        let converted =
            decoded.and_then(|a| convert_attribute(&a, base.point_count as usize, ct, n));
        let Some(data) = converted else {
            error(
                reports,
                format!(
                    "Draco Decoder: Could not decode attribute {attr}. Skipping primitive \
                     {UNNAMED}."
                ),
            );
            return Ok(());
        };
        let view = push_buffer(doc, buffers, data);
        doc.accessors[ai].buffer_view = Some(view);
    }
    Ok(())
}

/// `decoderReadIndices`: the face indices as `ct` (C++ `static_cast` truncation); `None` for
/// FLOAT.
fn index_bytes(indices: &[u32], ct: ComponentType) -> Option<Vec<u8>> {
    Some(match ct {
        ComponentType::Byte | ComponentType::UnsignedByte => {
            indices.iter().map(|&i| i as u8).collect()
        }
        ComponentType::Short | ComponentType::UnsignedShort => indices
            .iter()
            .flat_map(|&i| (i as u16).to_le_bytes())
            .collect(),
        ComponentType::UnsignedInt => indices.iter().flat_map(|&i| i.to_le_bytes()).collect(),
        ComponentType::Float => return None,
    })
}

/// A stored Draco value.
#[derive(Copy, Clone, Debug, PartialEq)]
enum In {
    /// An integer of a type with this many bits, signed or not.
    Int { v: i64, max: i64, signed: bool },
    /// FLOAT32.
    Float(f32),
}

fn value(data: &AttrData, i: usize) -> Option<In> {
    Some(match data {
        AttrData::F32(v) => In::Float(*v.get(i)?),
        AttrData::I8(v) => In::Int {
            v: i64::from(*v.get(i)?),
            max: i64::from(i8::MAX),
            signed: true,
        },
        AttrData::U8(v) => In::Int {
            v: i64::from(*v.get(i)?),
            max: i64::from(u8::MAX),
            signed: false,
        },
        AttrData::I16(v) => In::Int {
            v: i64::from(*v.get(i)?),
            max: i64::from(i16::MAX),
            signed: true,
        },
        AttrData::U16(v) => In::Int {
            v: i64::from(*v.get(i)?),
            max: i64::from(u16::MAX),
            signed: false,
        },
        AttrData::U32(v) => In::Int {
            v: i64::from(*v.get(i)?),
            max: i64::from(u32::MAX),
            signed: false,
        },
        #[allow(unreachable_patterns)] // `AttrData` is non-exhaustive
        _ => return None,
    })
}

/// `(min, max)` of an integral component type.
fn int_range(ct: ComponentType) -> (i64, i64) {
    match ct {
        ComponentType::Byte => (i64::from(i8::MIN), i64::from(i8::MAX)),
        ComponentType::UnsignedByte => (0, i64::from(u8::MAX)),
        ComponentType::Short => (i64::from(i16::MIN), i64::from(i16::MAX)),
        ComponentType::UnsignedShort => (0, i64::from(u16::MAX)),
        ComponentType::UnsignedInt => (0, i64::from(u32::MAX)),
        ComponentType::Float => (0, 0),
    }
}

/// The little-endian bytes of integer `v` as `ct` (C++ `static_cast`: modular).
fn int_bytes(v: i64, ct: ComponentType, out: &mut Vec<u8>) {
    match ct {
        ComponentType::Byte | ComponentType::UnsignedByte => out.push(v as u8),
        ComponentType::Short | ComponentType::UnsignedShort => {
            out.extend_from_slice(&(v as u16).to_le_bytes());
        }
        _ => out.extend_from_slice(&(v as u32).to_le_bytes()),
    }
}

/// Draco's `GeometryAttribute::ConvertComponentValue<T, OutT>` (attributes/
/// geometry_attribute.h, Draco 1.5.7, Apache-2.0, The Draco Authors): appends the converted
/// value, or answers `false` for a value the conversion refuses.
fn convert_component(x: In, normalized: bool, ct: ComponentType, out: &mut Vec<u8>) -> bool {
    match (x, ct) {
        // Integer → integer: range check, then a plain cast.
        (In::Int { v, signed, .. }, ct) if ct != ComponentType::Float => {
            let (min, max) = int_range(ct);
            // `in_value < kOutMin || in_value > max`, with C++'s usual arithmetic conversions:
            // a signed narrow value against UNSIGNED_INT compares as unsigned 32-bit.
            let v_cmp = if signed && ct == ComponentType::UnsignedInt {
                i64::from(v as u32)
            } else {
                v
            };
            let k_out_min = if signed { min } else { 0 };
            if v_cmp < k_out_min || v_cmp > max {
                return false;
            }
            int_bytes(v, ct, out);
            true
        }
        // Integer → float.
        (In::Int { v, max, .. }, _) => {
            let f = if normalized {
                v as f32 / max as f32
            } else {
                v as f32
            };
            out.extend_from_slice(&f.to_le_bytes());
            true
        }
        // Float → float.
        (In::Float(f), ComponentType::Float) => {
            out.extend_from_slice(&f.to_le_bytes());
            true
        }
        // Float → integer.
        (In::Float(f), ct) => {
            if f.is_nan() || f.is_infinite() {
                return false;
            }
            let (min, max) = int_range(ct);
            // `in_value < min() || in_value >= max()`, compared as float.
            if f < min as f32 || f >= max as f32 {
                return false;
            }
            if normalized {
                if !(0.0..=1.0).contains(&f) {
                    return false;
                }
                let v = (f64::from(f) * max as f64 + 0.5).floor() as i64;
                int_bytes(v, ct, out);
            } else {
                int_bytes(f.trunc() as i64, ct, out);
            }
            true
        }
    }
}

/// `decoderReadAttribute`: `point_count` values of `n` components of `ct`
/// (`ConvertValue`: the components both have, then zeros).
fn convert_attribute(
    a: &Attribute,
    point_count: usize,
    ct: ComponentType,
    n: usize,
) -> Option<Vec<u8>> {
    let c_in = usize::from(a.components);
    let mut out = Vec::with_capacity(point_count * n * ct.size());
    for p in 0..point_count {
        for c in 0..n.min(c_in) {
            let x = value(&a.data, p * c_in + c)?;
            if !convert_component(x, a.normalized, ct, &mut out) {
                return None;
            }
        }
        for _ in c_in..n {
            if ct == ComponentType::Float {
                out.extend_from_slice(&0f32.to_le_bytes());
            } else {
                int_bytes(0, ct, &mut out);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    fn conv(x: In, normalized: bool, ct: ComponentType) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        convert_component(x, normalized, ct, &mut out).then_some(out)
    }

    #[test]
    fn convert_value_follows_draco() {
        let u16v = |v| In::Int {
            v,
            max: 65535,
            signed: false,
        };
        let i8v = |v| In::Int {
            v,
            max: 127,
            signed: true,
        };
        // Same type: a copy.
        assert_eq!(
            conv(u16v(65535), true, ComponentType::UnsignedShort),
            Some(vec![255, 255])
        );
        // Out of the target range.
        assert_eq!(conv(u16v(256), false, ComponentType::UnsignedByte), None);
        assert_eq!(conv(i8v(-1), false, ComponentType::UnsignedByte), None);
        // C++ compares int8 against uint32 as unsigned: -1 passes and wraps.
        assert_eq!(
            conv(i8v(-1), false, ComponentType::UnsignedInt),
            Some(vec![255, 255, 255, 255])
        );
        // Normalized integer → float divides by the source type's max.
        assert_eq!(
            conv(u16v(65535), true, ComponentType::Float),
            Some(1f32.to_le_bytes().to_vec())
        );
        assert_eq!(
            conv(i8v(-127), true, ComponentType::Float),
            Some((-1f32).to_le_bytes().to_vec())
        );
        // Float → normalized integer rounds; outside [0, 1] is refused.
        assert_eq!(
            conv(In::Float(0.5), true, ComponentType::UnsignedByte),
            Some(vec![128])
        );
        assert_eq!(
            conv(In::Float(-0.5), true, ComponentType::UnsignedByte),
            None
        );
        // Float → integer truncates; NaN and the type's max are refused.
        assert_eq!(
            conv(In::Float(3.9), false, ComponentType::UnsignedByte),
            Some(vec![3])
        );
        assert_eq!(
            conv(In::Float(255.0), false, ComponentType::UnsignedByte),
            None
        );
        assert_eq!(conv(In::Float(f32::NAN), false, ComponentType::Short), None);
    }

    #[test]
    fn missing_components_are_zero_and_extra_ones_dropped() {
        let a = Attribute {
            kind: frap_codec_mesh::AttributeKind::Generic,
            components: 2,
            normalized: false,
            data: AttrData::F32(vec![1.0, 2.0, 3.0, 4.0]),
        };
        let out = convert_attribute(&a, 2, ComponentType::Float, 3).unwrap();
        let f: Vec<f32> = out
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(f, [1.0, 2.0, 0.0, 3.0, 4.0, 0.0]);
        let out = convert_attribute(&a, 2, ComponentType::Float, 1).unwrap();
        assert_eq!(out.len(), 8);
    }

    #[test]
    fn indices_are_cast_to_the_accessor_type() {
        assert_eq!(
            index_bytes(&[1, 65537], ComponentType::UnsignedShort),
            Some(vec![1, 0, 1, 0])
        );
        assert_eq!(index_bytes(&[1], ComponentType::Float), None);
    }

    fn repo(rel: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    const CASES: &str = "tests/golden/cases/gltf/draco";

    /// The inputs of the `import_blender` and `import_shared` (Blender's exports) and
    /// `import_frap` (frap's) goldens, and the dump of Blender's bridge decoding each
    /// (`bridge/<name>.json`). `blender_shared.glb`: two primitives share an index accessor
    /// that decodes differently for each, so each must read its own decode.
    const FILES: [(&str, &str); 7] = [
        ("import_blender.inputs", "blender_default"),
        ("import_blender.inputs", "blender_attrs"),
        ("import_blender.inputs", "blender_generic8"),
        ("import_blender.inputs", "blender_skin"),
        ("import_shared.inputs", "blender_shared"),
        ("import_frap.inputs", "frap_default"),
        ("import_frap.inputs", "frap_lossless"),
    ];

    fn load(dir: &str, name: &str) -> (Document, Vec<Vec<u8>>) {
        let p = repo(&format!("{CASES}/{dir}/{name}.glb"));
        let glb = frap_gltf::read(&std::fs::read(&p).unwrap()).unwrap();
        let buffers =
            frap_gltf::load_buffers(&glb.document, glb.bin.as_deref(), |u| Err(u.to_owned()))
                .unwrap();
        (glb.document, buffers)
    }

    fn decode(doc: &mut Document, buffers: &mut Vec<Vec<u8>>) -> Vec<Report> {
        let mut reports = Vec::new();
        decode_primitives(doc, buffers, &mut reports).unwrap();
        reports
    }

    fn components(doc: &Document, buffers: &[Vec<u8>], acc: Index) -> Vec<f64> {
        use frap_gltf::Components as C;
        let d = frap_gltf::read_accessor(doc, buffers, acc).unwrap();
        match d.components {
            C::F32(v) => v.into_iter().map(f64::from).collect(),
            C::U8(v) => v.into_iter().map(f64::from).collect(),
            C::I8(v) => v.into_iter().map(f64::from).collect(),
            C::U16(v) => v.into_iter().map(f64::from).collect(),
            C::I16(v) => v.into_iter().map(f64::from).collect(),
            C::U32(v) => v.into_iter().map(f64::from).collect(),
        }
    }

    /// Compares frap's decoded accessors with Blender's bridge dump: counts and integers
    /// exactly, floats within 1e-6 (Blender's arm64 Draco fuses multiply-adds: deviation D-132).
    fn check_against_bridge(
        doc: &Document,
        buffers: &[Vec<u8>],
        dump: &serde_json::Value,
        what: &str,
    ) {
        let entries = dump.as_array().unwrap();
        assert!(!entries.is_empty(), "{what}");
        for e in entries {
            let p = &doc.meshes[e["mesh"].as_u64().unwrap() as usize].primitives
                [e["primitive"].as_u64().unwrap() as usize];
            let at = format!("{what}: mesh {} primitive {}", e["mesh"], e["primitive"]);
            assert!(!p.extensions.contains(NAME), "{at}: extension left");
            let ind = components(doc, buffers, p.indices.unwrap());
            let want: Vec<f64> = e["indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            assert_eq!(ind, want, "{at}: indices");
            for (name, values) in e["attributes"].as_object().unwrap() {
                let acc = p.attributes[name];
                let float = doc.accessors[acc as usize].component_type == ComponentType::Float;
                let got = components(doc, buffers, acc);
                let want: Vec<f64> = values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap())
                    .collect();
                assert_eq!(got.len(), want.len(), "{at}: {name} count");
                assert_eq!(
                    doc.accessors[acc as usize].count,
                    e["vertices"].as_u64().unwrap(),
                    "{at}: {name}"
                );
                for (k, (a, b)) in got.iter().zip(&want).enumerate() {
                    let ok = if float { (a - b).abs() <= 1e-6 } else { a == b };
                    assert!(ok, "{at}: {name}[{k}]: frap {a} != Blender {b}");
                }
            }
        }
    }

    #[test]
    fn decodes_like_blenders_bridge() {
        for (dir, name) in FILES {
            let (mut doc, mut buffers) = load(dir, name);
            let reports = decode(&mut doc, &mut buffers);
            assert!(reports.is_empty(), "{name}: {reports:?}");
            let p = repo(&format!("{CASES}/bridge/{name}.json"));
            let dump: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            check_against_bridge(&doc, &buffers, &dump, name);
        }
    }

    #[test]
    fn a_shared_accessor_is_copied_for_each_later_decode() {
        let (doc0, _) = load("import_shared.inputs", "blender_shared");
        let shared = doc0.meshes[0].primitives[0].indices.unwrap();
        assert_eq!(doc0.meshes[1].primitives[0].indices, Some(shared));

        // Both decode: A writes the file's accessor, B a copy of the state A left.
        let (mut doc, mut buffers) = load("import_shared.inputs", "blender_shared");
        assert!(decode(&mut doc, &mut buffers).is_empty());
        let (a, b) = (
            doc.meshes[0].primitives[0].indices.unwrap(),
            doc.meshes[1].primitives[0].indices.unwrap(),
        );
        assert_eq!(a, shared);
        assert_eq!(b as usize, doc0.accessors.len());
        let (orig, copy) = (&doc0.accessors[shared as usize], &doc.accessors[b as usize]);
        assert_eq!(
            (copy.count, copy.component_type, copy.normalized, copy.type_),
            (orig.count, orig.component_type, orig.normalized, orig.type_)
        );
        assert_eq!((&copy.min, &copy.max), (&orig.min, &orig.max));
        assert_ne!(components(&doc, &buffers, a), components(&doc, &buffers, b));

        // B's bitstream does not decode: B reads what Blender's shared accessor holds then,
        // A's decode.
        let (mut doc, mut buffers) = load("import_shared.inputs", "blender_shared");
        let ext: KhrDracoMeshCompression = doc.meshes[1].primitives[0]
            .extensions
            .get()
            .unwrap()
            .unwrap();
        let v = doc.buffer_views[ext.buffer_view as usize].clone();
        let o = v.byte_offset.unwrap_or(0) as usize;
        buffers[v.buffer as usize][o..o + 8].copy_from_slice(b"NOTDRACO");
        let reports = decode(&mut doc, &mut buffers);
        assert_eq!(reports.len(), 1);
        assert_eq!(doc.meshes[1].primitives[0].indices, Some(shared));
        assert_eq!(doc.accessors.len(), doc0.accessors.len());
    }

    #[test]
    fn frap_inputs_are_the_transcripts_files() {
        for name in ["frap_default", "frap_lossless"] {
            let a = std::fs::read(repo(&format!("{CASES}/import_frap.inputs/{name}.glb"))).unwrap();
            let b = std::fs::read(repo(&format!(
                "tests/cli/m7/draco/draco_bpy.out/out/{name}.glb"
            )))
            .unwrap();
            assert!(
                a == b,
                "{name}: copy the transcript's output into import_frap.inputs"
            );
        }
    }

    /// The first Draco primitive of `blender_default.glb` and its extension.
    fn first(doc: &Document) -> (usize, usize, KhrDracoMeshCompression) {
        for (m, mesh) in doc.meshes.iter().enumerate() {
            for (p, prim) in mesh.primitives.iter().enumerate() {
                if let Some(e) = prim.extensions.get::<KhrDracoMeshCompression>().unwrap() {
                    return (m, p, e);
                }
            }
        }
        panic!("no Draco primitive")
    }

    fn messages(r: &[Report]) -> Vec<(ReportType, &str)> {
        r.iter().map(|r| (r.level, r.message.as_str())).collect()
    }

    #[test]
    fn a_bitstream_that_does_not_decode_skips_the_primitive() {
        let (mut doc, mut buffers) = load("import_blender.inputs", "blender_default");
        let (m, p, ext) = first(&doc);
        let v = doc.buffer_views[ext.buffer_view as usize].clone();
        let o = v.byte_offset.unwrap_or(0) as usize;
        buffers[v.buffer as usize][o..o + 8].copy_from_slice(b"NOTDRACO");
        let reports = decode(&mut doc, &mut buffers);
        assert_eq!(
            messages(&reports),
            [(
                ReportType::Error,
                "Draco Decoder: Unable to decode. Skipping primitive [unnamed]."
            )]
        );
        let prim = &doc.meshes[m].primitives[p];
        assert!(!prim.extensions.contains(NAME));
        // The accessors are left without a view (they read as zeros, as in Blender).
        assert!(
            doc.accessors[prim.indices.unwrap() as usize]
                .buffer_view
                .is_none()
        );
        assert!(
            doc.accessors[prim.attributes["POSITION"] as usize]
                .buffer_view
                .is_none()
        );
    }

    #[test]
    fn mismatches_follow_the_add_on() {
        // Counts that differ from the decoded ones: WARNINGs, accessors updated.
        let (mut doc, mut buffers) = load("import_blender.inputs", "blender_default");
        let (m, p, _) = first(&doc);
        let prim = doc.meshes[m].primitives[p].clone();
        doc.accessors[prim.indices.unwrap() as usize].count += 3;
        doc.accessors[prim.attributes["NORMAL"] as usize].count = 1;
        let want_index = doc.accessors[prim.indices.unwrap() as usize].count - 3;
        let reports = decode(&mut doc, &mut buffers);
        assert_eq!(
            messages(&reports),
            [
                (
                    ReportType::Warning,
                    "Draco Decoder: Index count of accessor and decoded index count does not \
                     match. Updating accessor."
                ),
                (
                    ReportType::Warning,
                    "Draco Decoder: Vertex count of accessor and decoded vertex count does not \
                     match for attribute NORMAL. Updating accessor."
                ),
            ]
        );
        assert_eq!(
            doc.accessors[prim.indices.unwrap() as usize].count,
            want_index
        );
        assert_eq!(
            doc.accessors[prim.attributes["NORMAL"] as usize].count,
            doc.accessors[prim.attributes["POSITION"] as usize].count
        );

        // An attribute of the extension the primitive lacks: ERROR, the rest skipped (the
        // indices and the attributes before it are decoded).
        let (mut doc, mut buffers) = load("import_blender.inputs", "blender_default");
        let (m, p, mut ext) = first(&doc);
        let mut attrs: IndexMap<String, u32> = IndexMap::new();
        attrs.insert("POSITION".into(), ext.attributes["POSITION"]);
        attrs.insert("TEXCOORD_7".into(), 9);
        attrs.insert("NORMAL".into(), ext.attributes["NORMAL"]);
        ext.attributes = attrs;
        doc.meshes[m].primitives[p]
            .extensions
            .insert_raw(NAME, serde_json::to_value(&ext).unwrap());
        let reports = decode(&mut doc, &mut buffers);
        assert_eq!(
            messages(&reports),
            [(
                ReportType::Error,
                "Draco Decoder: Draco attribute TEXCOORD_7 not in primitive attributes. \
                 Skipping primitive [unnamed]."
            )]
        );
        let prim = &doc.meshes[m].primitives[p];
        assert!(
            doc.accessors[prim.attributes["POSITION"] as usize]
                .buffer_view
                .is_some()
        );
        assert!(
            doc.accessors[prim.attributes["NORMAL"] as usize]
                .buffer_view
                .is_none()
        );

        // An id the bitstream lacks: ERROR `Could not decode attribute`.
        let (mut doc, mut buffers) = load("import_blender.inputs", "blender_default");
        let (m, p, mut ext) = first(&doc);
        ext.attributes.insert("NORMAL".into(), 42);
        doc.meshes[m].primitives[p]
            .extensions
            .insert_raw(NAME, serde_json::to_value(&ext).unwrap());
        let reports = decode(&mut doc, &mut buffers);
        assert_eq!(
            messages(&reports),
            [(
                ReportType::Error,
                "Draco Decoder: Could not decode attribute NORMAL. Skipping primitive [unnamed]."
            )]
        );

        // No indices: the add-on fails on `gltf.data.accessors[None]`.
        let (mut doc, mut buffers) = load("import_blender.inputs", "blender_default");
        let (m, p, _) = first(&doc);
        doc.meshes[m].primitives[p].indices = None;
        let mut reports = Vec::new();
        let e = decode_primitives(&mut doc, &mut buffers, &mut reports).unwrap_err();
        assert_eq!(
            e.to_string(),
            "Bad glTF: list indices must be integers or slices, not NoneType"
        );
    }

    /// Blender's bridge decoding every Draco primitive of a GLB into JSON (the accessors'
    /// component types, `decoderReadAttribute` / `decoderReadIndices`), as `bridge/*.json`
    /// were made.
    const BRIDGE_PY: &str = r#"
import bpy, sys, json, struct
from ctypes import CDLL, c_void_p, c_bool, c_size_t, c_uint32, c_char_p
from io_scene_gltf2.io.com.library import dll_path
src, out = sys.argv[sys.argv.index('--') + 1:]
b = open(src, 'rb').read()
n = struct.unpack('<I', b[12:16])[0]
j = json.loads(b[20:20 + n])
o = 20 + n
binc = b[o + 8:o + 8 + struct.unpack('<I', b[o:o + 4])[0]]
dll = CDLL(str(dll_path('bf_intern_draco_bridge', 'Draco').resolve()))
dll.decoderCreate.restype = c_void_p
dll.decoderRelease.argtypes = [c_void_p]
dll.decoderDecode.argtypes = [c_void_p, c_void_p, c_size_t]
dll.decoderDecode.restype = c_bool
dll.decoderGetVertexCount.argtypes = [c_void_p]
dll.decoderGetVertexCount.restype = c_uint32
dll.decoderReadAttribute.argtypes = [c_void_p, c_uint32, c_size_t, c_char_p]
dll.decoderReadAttribute.restype = c_bool
dll.decoderGetAttributeByteLength.argtypes = [c_void_p, c_uint32]
dll.decoderGetAttributeByteLength.restype = c_size_t
dll.decoderCopyAttribute.argtypes = [c_void_p, c_uint32, c_void_p]
dll.decoderReadIndices.argtypes = [c_void_p, c_size_t]
dll.decoderReadIndices.restype = c_bool
dll.decoderGetIndicesByteLength.argtypes = [c_void_p]
dll.decoderGetIndicesByteLength.restype = c_size_t
dll.decoderCopyIndices.argtypes = [c_void_p, c_void_p]
FMT = {5120: 'b', 5121: 'B', 5122: 'h', 5123: 'H', 5125: 'I', 5126: 'f'}
res = []
for mi, m in enumerate(j['meshes']):
    for pi, p in enumerate(m['primitives']):
        ext = (p.get('extensions') or {}).get('KHR_draco_mesh_compression')
        if ext is None:
            continue
        v = j['bufferViews'][ext['bufferView']]
        data = binc[v.get('byteOffset', 0):v.get('byteOffset', 0) + v['byteLength']]
        d = dll.decoderCreate()
        assert dll.decoderDecode(d, data, len(data))
        ia = j['accessors'][p['indices']]
        assert dll.decoderReadIndices(d, ia['componentType'])
        nb = dll.decoderGetIndicesByteLength(d)
        buf = bytes(nb)
        dll.decoderCopyIndices(d, buf)
        f = FMT[ia['componentType']]
        entry = {'mesh': mi, 'primitive': pi, 'vertices': dll.decoderGetVertexCount(d),
                 'indices': list(struct.unpack('<%d%s' % (nb // struct.calcsize(f), f), buf)),
                 'attributes': {}}
        for name, did in ext['attributes'].items():
            acc = j['accessors'][p['attributes'][name]]
            assert dll.decoderReadAttribute(d, did, acc['componentType'], acc['type'].encode())
            nb = dll.decoderGetAttributeByteLength(d, did)
            buf = bytes(nb)
            dll.decoderCopyAttribute(d, did, buf)
            f = FMT[acc['componentType']]
            entry['attributes'][name] = list(struct.unpack('<%d%s' % (nb // struct.calcsize(f), f), buf))
        dll.decoderRelease(d)
        res.append(entry)
json.dump(res, open(out, 'w'), separators=(',', ':'))
"#;

    fn locate_blender() -> Option<std::path::PathBuf> {
        if let Some(p) = std::env::var_os("BLENDER_BIN") {
            return Some(p.into());
        }
        let exe = if cfg!(windows) {
            "blender.exe"
        } else {
            "blender"
        };
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                if dir.join(exe).is_file() {
                    return Some(dir.join(exe));
                }
            }
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
        let p = home.join("Applications/Blender-5.2.app/Contents/MacOS/Blender");
        p.is_file().then_some(p)
    }

    /// Live (when Blender 5.2 is found; `FRAP_REQUIRE_BLENDER=1` makes a missing Blender a
    /// failure): Blender's bridge decodes the seven files (Blender's and frap's bitstreams) to
    /// the committed dumps, so `decodes_like_blenders_bridge` compares with the real decoder.
    #[test]
    fn bridge_dumps_match_live_blender() {
        let Some(blender) = locate_blender() else {
            assert!(
                std::env::var_os("FRAP_REQUIRE_BLENDER").is_none(),
                "FRAP_REQUIRE_BLENDER is set but no Blender was found"
            );
            eprintln!("SKIPPED bridge_dumps_match_live_blender: Blender 5.2 not found");
            return;
        };
        let dir = std::env::temp_dir().join(format!("frap-draco-bridge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("bridge.py");
        std::fs::write(&script, BRIDGE_PY).unwrap();
        for (sub, name) in FILES {
            let out = dir.join(format!("{name}.json"));
            let st = std::process::Command::new(&blender)
                .args(["-b", "--factory-startup", "--python"])
                .arg(&script)
                .arg("--")
                .arg(repo(&format!("{CASES}/{sub}/{name}.glb")))
                .arg(&out)
                .output()
                .unwrap();
            assert!(
                st.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&st.stderr)
            );
            let live: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
            let committed: serde_json::Value = serde_json::from_slice(
                &std::fs::read(repo(&format!("{CASES}/bridge/{name}.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(live, committed, "{name}: regenerate bridge/{name}.json");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
