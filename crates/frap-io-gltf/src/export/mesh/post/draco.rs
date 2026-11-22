// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_draco_mesh_compression` (WP 7.2): `export_draco_mesh_compression_enable`,
//! `export_draco_mesh_compression_level`, `export_draco_*_quantization`, `frap_draco_backend`.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/io/exp/draco.py
//! (`encode_scene_primitives`, `__encode_primitive`, `__cleanup_node`) and
//! `blender/exp/exporter.py` (`add_draco_extension`), Apache-2.0, The glTF-Blender-IO authors:
//!
//! - only TRIANGLES primitives are encoded (LINES and POINTS keep their buffer views);
//! - every attribute of the primitive (`POSITION`, `NORMAL`, `TEXCOORD_n`, then the others, in
//!   the add-on's order) goes into the bitstream with the accessor's component type and
//!   `normalized`, its Draco id being its place in that order; the indices follow;
//! - `compression_level` and the five quantization props go to the bridge as they are; a
//!   primitive with morph targets is encoded with `preserve_triangle_order`
//!   (`MESH_SEQUENTIAL_ENCODING`), and its target accessors keep their buffer views;
//! - the bitstream becomes a buffer view of its own (no `target`), the extension
//!   `{bufferView, attributes}` goes on the primitive, `mode` is written as 4, and the
//!   attribute and index accessors lose their `bufferView` but keep their component type,
//!   `normalized` and POSITION's `min`/`max`; their `count` becomes the encoded counts;
//! - the extension is used and required.
//!
//! A primitive shared by several nodes is encoded once (`encoded_primitives_cache`): the mesh
//! gatherer writes each mesh's primitives once and reuses them.
//!
//! The encoder is `frap_codec_mesh::draco::codec(frap_draco_backend)`; draco-core writes the
//! bytes of Blender's Google Draco 1.5.7 (spike gate, §22.9; the tests below re-encode the
//! uncompressed twins of Blender's `gltf/draco` fixtures and compare every bitstream byte for
//! byte). A backend this build lacks fails the export with the missing cargo feature
//! (Blender hides the props when its bridge library is missing). An encoder error fails the
//! export too (`RuntimeError` naming Draco's message), where the add-on logs `Could not encode
//! primitive. Skipping primitive.`, returns FINISHED and still writes the failed encoder's
//! partial bitstream with accessor counts of 0, which no loader decodes (Blender's own import
//! gives an empty mesh; deviations D-144). Two user-reachable inputs do this:
//! `export_draco_normal_quantization=1` (inside the prop's range 0..=30, but Draco's
//! octahedral transform needs 2..=30) and a non-finite vertex coordinate or attribute value
//! (no finite quantization range); the others draco-core refuses (indices out of range, a
//! partial triangle) the gatherer never produces.

use frap_codec_mesh::draco::{self, DracoBackend, DracoEncoded, DracoQuantization};
use frap_codec_mesh::{AttrData, Attribute, AttributeKind, MeshData};
use frap_gltf::BufferBuilder;
use frap_gltf::ext::khr_draco_mesh_compression::KhrDracoMeshCompression;
use indexmap::IndexMap;

use crate::export::ExportError;
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_POST_PROCESS, PrimitiveDraft,
    PrimitivePostProcess,
};

/// The extension name.
pub const NAME: &str = "KHR_draco_mesh_compression";

/// The props this file honours.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_draco_mesh_compression_enable",
    "export_draco_mesh_compression_level",
    "export_draco_position_quantization",
    "export_draco_normal_quantization",
    "export_draco_texcoord_quantization",
    "export_draco_color_quantization",
    "export_draco_generic_quantization",
    "frap_draco_backend",
];

/// `glTF primitive mode` TRIANGLES.
const TRIANGLES: u32 = 4;

/// The Draco settings of an export (`export_settings['gltf_draco_*']`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DracoSettings {
    /// `export_draco_mesh_compression_level`.
    pub level: u8,
    /// The five `export_draco_*_quantization` props.
    pub quantization: DracoQuantization,
    /// `frap_draco_backend`.
    pub backend: DracoBackend,
}

/// The Draco settings, `None` when `export_draco_mesh_compression_enable` is off.
pub fn settings(cx: &MeshExportCx) -> Option<DracoSettings> {
    let p = &cx.settings.props;
    if !p.export_draco_mesh_compression_enable.value {
        return None;
    }
    // The RNA ranges (0–10, 0–30) hold for every value the operator accepts.
    let bits = |v: i32| v.clamp(0, 30) as u8;
    Some(DracoSettings {
        level: p.export_draco_mesh_compression_level.value.clamp(0, 10) as u8,
        quantization: DracoQuantization {
            position: bits(p.export_draco_position_quantization.value),
            normal: bits(p.export_draco_normal_quantization.value),
            texcoord: bits(p.export_draco_texcoord_quantization.value),
            color: bits(p.export_draco_color_quantization.value),
            generic: bits(p.export_draco_generic_quantization.value),
        },
        backend: DracoBackend::from_identifier(p.frap_draco_backend.value.identifier())
            .unwrap_or_default(),
    })
}

/// `getAttributeSemantics` of the bridge.
pub fn attribute_kind(semantic: &str) -> AttributeKind {
    if semantic == "POSITION" {
        AttributeKind::Position
    } else if semantic == "NORMAL" {
        AttributeKind::Normal
    } else if semantic.starts_with("TEXCOORD") {
        AttributeKind::TexCoord
    } else if semantic.starts_with("COLOR") {
        AttributeKind::Color
    } else {
        AttributeKind::Generic
    }
}

fn flat<const N: usize, T: Copy>(v: &[[T; N]]) -> Vec<T> {
    v.iter().flat_map(|e| e.iter().copied()).collect()
}

/// A column's values and component count as codec data.
fn attr_data(values: &AttrValues) -> (u8, AttrData) {
    match values {
        AttrValues::F32x2(v) => (2, AttrData::F32(flat(v))),
        AttrValues::F32x3(v) => (3, AttrData::F32(flat(v))),
        AttrValues::F32x4(v) => (4, AttrData::F32(flat(v))),
        AttrValues::U8x4(v) => (4, AttrData::U8(flat(v))),
        AttrValues::U16x4(v) => (4, AttrData::U16(flat(v))),
        AttrValues::I8x2(v) => (2, AttrData::I8(flat(v))),
        AttrValues::I8x3(v) => (3, AttrData::I8(flat(v))),
        AttrValues::I8x4(v) => (4, AttrData::I8(flat(v))),
        AttrValues::U8x2(v) => (2, AttrData::U8(flat(v))),
        AttrValues::U8x3(v) => (3, AttrData::U8(flat(v))),
        AttrValues::I16x2(v) => (2, AttrData::I16(flat(v))),
        AttrValues::I16x3(v) => (3, AttrData::I16(flat(v))),
        AttrValues::I16x4(v) => (4, AttrData::I16(flat(v))),
        AttrValues::U16x2(v) => (2, AttrData::U16(flat(v))),
        AttrValues::U16x3(v) => (3, AttrData::U16(flat(v))),
        AttrValues::F32(v) => (1, AttrData::F32(v.clone())),
        AttrValues::F32x16(v) => (16, AttrData::F32(flat(v))),
    }
}

/// Whether the accessor of `values` is `normalized` (as `gather::column_data` writes it: only
/// integer component types can be).
fn is_normalized(values: &AttrValues, normalized: bool) -> bool {
    normalized
        && !matches!(
            values,
            AttrValues::F32x2(_) | AttrValues::F32x3(_) | AttrValues::F32x4(_)
        )
}

/// The primitive's attributes in the accessors' order, each as the gatherer will write it
/// (a column `encoding.attributes` replaces is encoded replaced).
pub fn columns(d: &PrimitiveDraft) -> Vec<(String, AttrColumn)> {
    let mut out: Vec<(String, AttrColumn)> = Vec::new();
    let own = |values: AttrValues| AttrColumn {
        values,
        normalized: false,
    };
    out.push((
        "POSITION".into(),
        own(AttrValues::F32x3(d.positions.clone())),
    ));
    if !d.normals.is_empty() {
        out.push(("NORMAL".into(), own(AttrValues::F32x3(d.normals.clone()))));
    }
    for (k, tc) in d.texcoords.iter().enumerate() {
        out.push((format!("TEXCOORD_{k}"), own(AttrValues::F32x2(tc.clone()))));
    }
    for (name, col) in &d.attributes {
        out.push((name.to_string(), col.clone()));
    }
    for (name, col) in &mut out {
        if let Some(r) = d.encoding.attributes.get(name.as_str()) {
            *col = r.clone();
        }
    }
    out
}

/// The codec input of a primitive: its attributes (see [`columns`]) and indices.
pub fn mesh_data(d: &PrimitiveDraft) -> (Vec<String>, MeshData) {
    let mut names = Vec::new();
    let mut attributes = Vec::new();
    for (name, col) in columns(d) {
        let (components, data) = attr_data(&col.values);
        attributes.push(Attribute {
            kind: attribute_kind(&name),
            components,
            normalized: is_normalized(&col.values, col.normalized),
            data,
        });
        names.push(name);
    }
    (
        names,
        MeshData {
            indices: d.indices.clone(),
            attributes,
        },
    )
}

/// Encodes `d` with `s` (the bridge's `encoderEncode`).
pub fn encode(
    d: &PrimitiveDraft,
    s: &DracoSettings,
) -> Result<(Vec<String>, DracoEncoded), ExportError> {
    let codec = draco::codec(s.backend)
        .ok_or_else(|| ExportError::NotImplemented(s.backend.unavailable().to_string()))?;
    let (names, mesh) = mesh_data(d);
    let encoded = if d.targets.is_empty() {
        codec.encode(&mesh, &s.quantization, s.level)
    } else {
        codec.encode_preserving_order(&mesh, &s.quantization, s.level)
    };
    encoded
        .map(|e| (names, e))
        .map_err(|e| ExportError::Invalid(format!("Draco encoder: {e}")))
}

/// `count` elements standing for `v` in an accessor without a buffer view: the same first
/// element (or the default), and, for POSITION, the per-component `min` and `max` of `v` as
/// the first two elements, so the accessor keeps the add-on's bounds (gathered before the
/// encode) while its `count` is the encoded one.
fn stand_in<const N: usize, T: Copy + Default + PartialOrd>(
    v: &[[T; N]],
    count: usize,
    bounds: bool,
) -> Vec<[T; N]> {
    let fill = v.first().copied().unwrap_or([T::default(); N]);
    let mut out = vec![fill; count];
    if bounds && !v.is_empty() {
        let mut lo = v[0];
        let mut hi = v[0];
        for e in v {
            for c in 0..N {
                if e[c] < lo[c] {
                    lo[c] = e[c];
                }
                if e[c] > hi[c] {
                    hi[c] = e[c];
                }
            }
        }
        if let Some(x) = out.get_mut(0) {
            *x = lo;
        }
        if let Some(x) = out.get_mut(1) {
            *x = hi;
        }
    }
    out
}

fn stand_in_values(v: &AttrValues, count: usize) -> AttrValues {
    match v {
        AttrValues::F32x2(v) => AttrValues::F32x2(stand_in(v, count, false)),
        AttrValues::F32x3(v) => AttrValues::F32x3(stand_in(v, count, false)),
        AttrValues::F32x4(v) => AttrValues::F32x4(stand_in(v, count, false)),
        AttrValues::U8x4(v) => AttrValues::U8x4(stand_in(v, count, false)),
        AttrValues::U16x4(v) => AttrValues::U16x4(stand_in(v, count, false)),
        AttrValues::I8x2(v) => AttrValues::I8x2(stand_in(v, count, false)),
        AttrValues::I8x3(v) => AttrValues::I8x3(stand_in(v, count, false)),
        AttrValues::I8x4(v) => AttrValues::I8x4(stand_in(v, count, false)),
        AttrValues::U8x2(v) => AttrValues::U8x2(stand_in(v, count, false)),
        AttrValues::U8x3(v) => AttrValues::U8x3(stand_in(v, count, false)),
        AttrValues::I16x2(v) => AttrValues::I16x2(stand_in(v, count, false)),
        AttrValues::I16x3(v) => AttrValues::I16x3(stand_in(v, count, false)),
        AttrValues::I16x4(v) => AttrValues::I16x4(stand_in(v, count, false)),
        AttrValues::U16x2(v) => AttrValues::U16x2(stand_in(v, count, false)),
        AttrValues::U16x3(v) => AttrValues::U16x3(stand_in(v, count, false)),
        AttrValues::F32(v) => AttrValues::F32(vec![v.first().copied().unwrap_or_default(); count]),
        AttrValues::F32x16(v) => AttrValues::F32x16(stand_in(v, count, false)),
    }
}

/// The add-on's `attributes[attr_name].count = encoded_vertices` and `indices.count =
/// encoded index count`: when the encode changed a count (EdgeBreaker splits non-manifold
/// vertices and drops degenerate faces), the draft's columns become stand-ins of the encoded
/// counts. Only counts, component types and POSITION's bounds reach the file, since the
/// accessors have no buffer view. The index stand-in keeps the largest index the count
/// allows, so the index component type stays the one of the original indices.
fn apply_counts(d: &mut PrimitiveDraft, points: usize, indices: usize) {
    if points != d.positions.len() {
        d.positions = stand_in(&d.positions, points, true);
        d.normals = if d.normals.is_empty() {
            Vec::new()
        } else {
            stand_in(&d.normals, points, false)
        };
        for tc in &mut d.texcoords {
            *tc = stand_in(tc, points, false);
        }
        for col in d.attributes.values_mut() {
            col.values = stand_in_values(&col.values, points);
        }
        for col in d.encoding.attributes.values_mut() {
            col.values = stand_in_values(&col.values, points);
        }
        // Per-vertex bookkeeping must keep the vertex count of the columns.
        d.corners = vec![u32::MAX; points];
        d.vertices = vec![0; points];
    }
    if indices != d.indices.len() || points != d.positions.len() {
        let max = d.indices.iter().copied().max().unwrap_or(0);
        let top = max.min(points.saturating_sub(1) as u32);
        let mut v = vec![0; indices];
        if let Some(x) = v.first_mut() {
            *x = top;
        }
        d.indices = v;
    }
}

fn run(
    cx: &MeshExportCx,
    draft: &mut PrimitiveDraft,
    buffer: &mut BufferBuilder,
) -> Result<(), ExportError> {
    let Some(s) = settings(cx) else {
        return Ok(());
    };
    // "Only do TRIANGLES primitives".
    if draft.mode != TRIANGLES || draft.positions.is_empty() {
        return Ok(());
    }
    let (names, encoded) = encode(draft, &s)?;
    let view = buffer.push_view(&encoded.bytes, 4, None, None);
    let attributes: IndexMap<String, u32> = names
        .into_iter()
        .zip(encoded.attribute_ids.iter().copied())
        .collect();
    let ext = KhrDracoMeshCompression {
        buffer_view: view,
        attributes,
        ..KhrDracoMeshCompression::default()
    };
    let value = serde_json::to_value(&ext)
        .map_err(|e| ExportError::Invalid(format!("KHR_draco_mesh_compression: {e}")))?;
    let points = draft.positions.len();
    if encoded.point_count as usize != points || encoded.index_count as usize != draft.indices.len()
    {
        apply_counts(
            draft,
            encoded.point_count as usize,
            encoded.index_count as usize,
        );
    }
    draft.encoding.without_buffer_views = true;
    draft.encoding.extensions.insert_raw(NAME, value);
    if !draft
        .encoding
        .extensions_used
        .iter()
        .any(|(n, _)| *n == NAME)
    {
        draft.encoding.extensions_used.push((NAME, true));
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static DRACO: PrimitivePostProcess = PrimitivePostProcess {
    name: "draco",
    order: 400,
    run,
};

#[cfg(test)]
mod tests {
    use super::*;
    use frap_gltf::json::Document;
    use frap_gltf::{Components, read_accessor};

    /// A blessed Blender 5.2.4 export of the `gltf/draco` goldens.
    fn fixture(name: &str) -> (Document, Vec<u8>) {
        let p = format!(
            "{}/../../tests/golden/fixtures/gltf/draco/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let glb = frap_gltf::read(&bytes).unwrap();
        (glb.document, glb.bin.unwrap_or_default())
    }

    fn view<'a>(doc: &Document, bin: &'a [u8], v: u32) -> &'a [u8] {
        let v = &doc.buffer_views[v as usize];
        let o = v.byte_offset.unwrap_or(0) as usize;
        &bin[o..o + v.byte_length as usize]
    }

    /// The bridge's input for a primitive of an uncompressed file: each attribute's accessor
    /// data, the indices.
    fn bridge_input(
        doc: &Document,
        bin: &[u8],
        mesh: usize,
        prim: usize,
    ) -> (Vec<String>, MeshData) {
        let p = &doc.meshes[mesh].primitives[prim];
        let bufs = [bin.to_vec()];
        let mut names = Vec::new();
        let mut attributes = Vec::new();
        for (name, &a) in &p.attributes {
            let d = read_accessor(doc, &bufs, a).unwrap();
            let data = match d.components {
                Components::F32(v) => AttrData::F32(v),
                Components::U8(v) => AttrData::U8(v),
                Components::U16(v) => AttrData::U16(v),
                Components::I8(v) => AttrData::I8(v),
                Components::I16(v) => AttrData::I16(v),
                Components::U32(v) => AttrData::U32(v),
            };
            attributes.push(Attribute {
                kind: attribute_kind(name),
                components: d.type_.components() as u8,
                normalized: d.normalized,
                data,
            });
            names.push(name.clone());
        }
        let ind = read_accessor(doc, &bufs, p.indices.unwrap()).unwrap();
        let indices = match ind.components {
            Components::U16(v) => v.into_iter().map(u32::from).collect(),
            Components::U32(v) => v,
            Components::U8(v) => v.into_iter().map(u32::from).collect(),
            other => panic!("indices {other:?}"),
        };
        (
            names,
            MeshData {
                indices,
                attributes,
            },
        )
    }

    fn settings(level: u8, bits: [u8; 5]) -> DracoSettings {
        DracoSettings {
            level,
            quantization: DracoQuantization {
                position: bits[0],
                normal: bits[1],
                texcoord: bits[2],
                color: bits[3],
                generic: bits[4],
            },
            backend: DracoBackend::Auto,
        }
    }

    /// Encodes every Draco primitive of `draco_file` from its uncompressed twin in `plain_file`
    /// (meshes matched by name) and checks the bitstream, the attribute ids and the accessor
    /// counts against Blender's. Returns the number of primitives checked.
    fn check_file(draco_file: &str, plain_file: &str, s: DracoSettings) -> usize {
        let (dd, dbin) = fixture(draco_file);
        let (pd, pbin) = fixture(plain_file);
        let codec = draco::codec(s.backend).unwrap();
        let mut checked = 0;
        for m in &dd.meshes {
            let pm = pd
                .meshes
                .iter()
                .position(|x| x.name == m.name)
                .unwrap_or_else(|| panic!("{plain_file}: no mesh {:?}", m.name));
            for (pi, prim) in m.primitives.iter().enumerate() {
                let Some(ext) = prim.extensions.get::<KhrDracoMeshCompression>().unwrap() else {
                    continue;
                };
                let what = format!("{draco_file}: {:?} primitive {pi}", m.name);
                let (names, mesh) = bridge_input(&pd, &pbin, pm, pi);
                assert_eq!(
                    names,
                    prim.attributes.keys().cloned().collect::<Vec<_>>(),
                    "{what}"
                );
                let sequential = !prim.targets.is_empty();
                let e = if sequential {
                    codec.encode_preserving_order(&mesh, &s.quantization, s.level)
                } else {
                    codec.encode(&mesh, &s.quantization, s.level)
                }
                .unwrap();
                assert!(
                    e.bytes == view(&dd, &dbin, ext.buffer_view),
                    "{what}: the bitstream differs from Blender's ({} vs {} bytes)",
                    e.bytes.len(),
                    view(&dd, &dbin, ext.buffer_view).len()
                );
                let ids: IndexMap<String, u32> = names
                    .iter()
                    .cloned()
                    .zip(e.attribute_ids.iter().copied())
                    .collect();
                assert_eq!(ids, ext.attributes, "{what}");
                for a in prim.attributes.values() {
                    assert_eq!(
                        dd.accessors[*a as usize].count,
                        u64::from(e.point_count),
                        "{what}"
                    );
                }
                let ia = prim.indices.unwrap() as usize;
                assert_eq!(dd.accessors[ia].count, u64::from(e.index_count), "{what}");
                checked += 1;
            }
        }
        checked
    }

    #[test]
    fn bitstreams_equal_blenders_for_every_setting() {
        // export_basic.py: Bow, Cube (shared by Twin), Sphere, Two (two primitives).
        let d = [14, 10, 12, 10, 12];
        let plain = "export_basic.out_plain.glb";
        assert_eq!(
            check_file("export_basic.out_default.glb", plain, settings(6, d)),
            5
        );
        assert_eq!(
            check_file("export_basic.out_core.glb", plain, settings(6, d)),
            5
        );
        assert_eq!(
            check_file("export_basic.out_level0.glb", plain, settings(0, d)),
            5
        );
        assert_eq!(
            check_file("export_basic.out_level10.glb", plain, settings(10, d)),
            5
        );
        assert_eq!(
            check_file(
                "export_basic.out_lossless.glb",
                plain,
                settings(6, [0, 0, 0, 10, 12])
            ),
            5
        );
        assert_eq!(
            check_file(
                "export_basic.out_coarse.glb",
                plain,
                settings(3, [8, 6, 7, 10, 12])
            ),
            5
        );
    }

    #[test]
    fn bitstreams_equal_blenders_for_every_attribute_kind() {
        // COLOR_0/COLOR_1 (normalized integers), TANGENT, a morph target (sequential), the
        // TRIANGLES primitive of a mesh with LINES and POINTS.
        let d = [14, 10, 12, 10, 12];
        assert_eq!(
            check_file(
                "export_attributes.out_attrs.glb",
                "export_attributes.out_attrs_plain.glb",
                settings(6, d)
            ),
            3
        );
        assert_eq!(
            check_file(
                "export_attributes.out_generic8.glb",
                "export_attributes.out_attrs_plain.glb",
                settings(10, [14, 10, 12, 4, 8])
            ),
            3
        );
        // JOINTS_0 (UNSIGNED_BYTE) and WEIGHTS_0 (FLOAT, generic bits), next to the meshes the
        // case imported back (their morph targets, colours, tangents).
        assert_eq!(
            check_file(
                "export_attributes.out_skin.glb",
                "export_attributes.out_skin_plain.glb",
                settings(6, d)
            ),
            10
        );
    }

    #[test]
    fn lines_points_and_targets_stay_uncompressed() {
        let (d, _) = fixture("export_attributes.out_attrs.glb");
        let mut modes = Vec::new();
        for m in &d.meshes {
            for p in &m.primitives {
                let draco = p.extensions.contains(NAME);
                modes.push((
                    m.name.clone().unwrap_or_default(),
                    p.mode.map(|m| m as u32),
                    draco,
                ));
                for t in &p.targets {
                    for a in t.values() {
                        let acc = &d.accessors[*a as usize];
                        assert!(acc.buffer_view.is_some() || acc.sparse.is_some());
                    }
                }
                for a in p.attributes.values() {
                    assert_eq!(d.accessors[*a as usize].buffer_view.is_none(), draco);
                }
            }
        }
        assert!(
            modes.contains(&("Loose".into(), Some(1), false)),
            "{modes:?}"
        );
        assert!(
            modes.contains(&("Loose".into(), Some(0), false)),
            "{modes:?}"
        );
        assert!(
            modes.contains(&("Loose".into(), Some(4), true)),
            "{modes:?}"
        );
        assert_eq!(d.extensions_used[0], NAME, "declared before the traversal");
    }

    /// A draft of `n` vertices on a bent strip, with a normal, a UV and a colour column.
    fn strip(n: usize) -> PrimitiveDraft {
        let mut d = PrimitiveDraft {
            mode: TRIANGLES,
            ..PrimitiveDraft::default()
        };
        for i in 0..n {
            let x = (i / 2) as f32 * 0.37;
            let y = (i % 2) as f32;
            d.positions.push([x, y, (x * 1.3).sin()]);
            d.normals.push([0.0, 0.0, 1.0]);
        }
        d.texcoords = vec![d.positions.iter().map(|p| [p[0] / 4.0, p[1]]).collect()];
        d.attributes.insert(
            "COLOR_0".into(),
            AttrColumn {
                values: AttrValues::U16x4(
                    (0..n).map(|i| [i as u16 * 100, 0, 65535, 65535]).collect(),
                ),
                normalized: true,
            },
        );
        for i in 0..(n as u32 / 2 - 1) {
            let a = 2 * i;
            d.indices.extend([a, a + 1, a + 2, a + 2, a + 1, a + 3]);
        }
        d
    }

    #[test]
    fn decoded_values_are_within_quantization_bounds() {
        let d = strip(40);
        let s = settings(6, [14, 10, 12, 10, 12]);
        let (_, e) = encode(&d, &s).unwrap();
        let codec = draco::codec(DracoBackend::Auto).unwrap();
        let out = codec.decode_counted(&e.bytes, &e.attribute_ids).unwrap();
        assert_eq!(out.point_count as usize, d.positions.len());
        let AttrData::F32(pos) = &out.mesh.attributes[0].data else {
            panic!()
        };
        // Draco quantizes positions over the largest extent: error ≤ range / (2^bits - 1) / 2.
        let range = 19.0 * 0.37;
        let bound = range / f32::from((1u16 << 14) - 1) / 2.0 + 1e-6;
        for p in pos.chunks_exact(3) {
            let best = d
                .positions
                .iter()
                .map(|q| (0..3).map(|c| (q[c] - p[c]).abs()).fold(0f32, f32::max))
                .fold(f32::MAX, f32::min);
            assert!(best <= bound, "{p:?}: {best} > {bound}");
        }
        // Integer colours are lossless.
        let AttrData::U16(col) = &out.mesh.attributes[3].data else {
            panic!()
        };
        let mut got: Vec<_> = col.chunks_exact(4).map(|c| c.to_vec()).collect();
        let mut want: Vec<_> = (0..40)
            .map(|i| vec![i as u16 * 100, 0, 65535, 65535])
            .collect();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn changed_counts_keep_the_bounds_and_the_index_type() {
        let mut d = strip(8);
        let before: Vec<[f32; 3]> = d.positions.clone();
        apply_counts(&mut d, 11, 15);
        assert_eq!(d.positions.len(), 11);
        assert_eq!(d.normals.len(), 11);
        assert_eq!(d.texcoords[0].len(), 11);
        assert_eq!(d.indices.len(), 15);
        assert_eq!(d.indices[0], 7, "the largest index the count allows");
        assert!(d.indices.iter().all(|&i| (i as usize) < 11));
        for c in 0..3 {
            let lo = before.iter().map(|p| p[c]).fold(f32::MAX, f32::min);
            let hi = before.iter().map(|p| p[c]).fold(f32::MIN, f32::max);
            assert_eq!(d.positions[0][c], lo);
            assert_eq!(d.positions[1][c], hi);
            assert!(d.positions.iter().all(|p| p[c] >= lo && p[c] <= hi));
        }
        let AttrValues::U16x4(c) = &d.attributes["COLOR_0"].values else {
            panic!()
        };
        assert_eq!(c.len(), 11);
    }

    #[test]
    fn semantics_map_to_draco_types() {
        assert_eq!(attribute_kind("POSITION"), AttributeKind::Position);
        assert_eq!(attribute_kind("NORMAL"), AttributeKind::Normal);
        assert_eq!(attribute_kind("TEXCOORD_3"), AttributeKind::TexCoord);
        assert_eq!(attribute_kind("COLOR_1"), AttributeKind::Color);
        for g in ["TANGENT", "JOINTS_0", "WEIGHTS_0", "_CUSTOM"] {
            assert_eq!(attribute_kind(g), AttributeKind::Generic);
        }
    }

    #[test]
    fn every_draco_prop_is_supported() {
        for p in PROPS {
            assert!(crate::export::settings::is_supported(p), "{p}");
        }
    }
}
