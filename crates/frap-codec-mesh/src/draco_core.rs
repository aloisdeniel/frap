// SPDX-License-Identifier: GPL-3.0-or-later
//! The draco-core backend (feature `draco`, WP 7.2): [`DracoCodec`] over the pure-Rust
//! `draco-core` crate, which writes the bytes of Google Draco 1.5.7 (the version Blender 5.2
//! bundles, `build_files/build_environment/cmake/versions.cmake`).
//!
//! Ported from blender/intern/draco_bridge/intern/encoder.cpp and decoder.cpp (Apache-2.0,
//! Blender Authors, Jim Eckerlein): the add-on's `bf_intern_draco_bridge`. The encoder builds a
//! `draco::Mesh` with `num_points` = the vertex count, one attribute per glTF attribute (added
//! with an identity mapping, so its unique id is its position in the input), the faces from the
//! indices, and encodes it as `draco::Encoder` does: `SetSpeedOptions(10 - level, 10 - level)`,
//! the quantization bits of each attribute's semantic (`draco::Encoder::SetAttributeQuantization`
//! is by type; `CreateExpertEncoderOptions` copies the type's options to every attribute of that
//! type), and `MESH_SEQUENTIAL_ENCODING` when the triangle order must be kept (morph targets).
//! The encoded counts are `num_encoded_points()` / `num_encoded_faces() * 3`
//! (`SetTrackEncodedProperties(true)`).
//!
//! Spike gate (§22.9): draco-core is byte-identical to Blender's encoder on the add-on's inputs
//! (tests below and in `frap-io-gltf`), so `DracoBackend::Auto` picks it.

use crate::draco::{DracoCodec, DracoQuantization};
use crate::error::CodecError;
use crate::mesh::{AttributeKind, MeshData};

/// The draco-core codec, or `None` without the `draco` feature.
pub fn codec() -> Option<&'static dyn DracoCodec> {
    #[cfg(feature = "draco")]
    {
        Some(&imp::DracoCore)
    }
    #[cfg(not(feature = "draco"))]
    {
        None
    }
}

/// `draco::MESH_SEQUENTIAL_ENCODING` (`compression/config/compression_shared.h`).
pub const MESH_SEQUENTIAL_ENCODING: i32 = 0;

/// `draco::PREDICTION_DIFFERENCE` (`compression/config/compression_shared.h`).
pub const PREDICTION_DIFFERENCE: i32 = 0;

/// The quantization bits `draco::Encoder` gives an attribute of `kind` (the type-keyed
/// options of `encoderEncode`).
pub fn quantization_bits(q: &DracoQuantization, kind: AttributeKind) -> u8 {
    match kind {
        AttributeKind::Position => q.position,
        AttributeKind::Normal => q.normal,
        AttributeKind::TexCoord => q.texcoord,
        AttributeKind::Color => q.color,
        _ => q.generic,
    }
}

/// Checks the input as the bridge's callers guarantee it (every attribute covers the vertex
/// count, indices in range, whole triangles).
pub fn check_input(mesh: &MeshData) -> Result<usize, CodecError> {
    let n = mesh.vertex_count();
    for (i, a) in mesh.attributes.iter().enumerate() {
        if !(1..=4).contains(&a.components) {
            return Err(CodecError::Encode(format!(
                "Draco attribute {i}: {} components",
                a.components
            )));
        }
        if a.data.len() != n * usize::from(a.components) {
            return Err(CodecError::Encode(format!(
                "Draco attribute {i}: {} values for {n} vertices of {} components",
                a.data.len(),
                a.components
            )));
        }
    }
    if !mesh.indices.len().is_multiple_of(3) {
        return Err(CodecError::Encode(format!(
            "Draco encoder: {} indices is not a triangle list",
            mesh.indices.len()
        )));
    }
    if let Some(bad) = mesh.indices.iter().find(|&&i| i as usize >= n) {
        return Err(CodecError::Encode(format!(
            "Draco encoder: index {bad} out of range ({n} vertices)"
        )));
    }
    Ok(n)
}

#[cfg(feature = "draco")]
mod imp {
    use draco_core::{
        DataType, DecoderBuffer, EncoderBuffer, EncoderOptions, FaceIndex, GeometryAttributeType,
        Mesh, MeshDecoder, MeshEncoder, PointAttribute, PointIndex,
    };

    use super::*;
    use crate::draco::{DracoDecoded, DracoEncoded};
    use crate::mesh::{AttrData, Attribute};

    /// The draco-core codec.
    pub struct DracoCore;

    fn geometry_type(kind: AttributeKind) -> GeometryAttributeType {
        match kind {
            AttributeKind::Position => GeometryAttributeType::Position,
            AttributeKind::Normal => GeometryAttributeType::Normal,
            AttributeKind::TexCoord => GeometryAttributeType::TexCoord,
            AttributeKind::Color => GeometryAttributeType::Color,
            _ => GeometryAttributeType::Generic,
        }
    }

    fn kind_of(t: GeometryAttributeType) -> AttributeKind {
        match t {
            GeometryAttributeType::Position => AttributeKind::Position,
            GeometryAttributeType::Normal => AttributeKind::Normal,
            GeometryAttributeType::TexCoord => AttributeKind::TexCoord,
            GeometryAttributeType::Color => AttributeKind::Color,
            _ => AttributeKind::Generic,
        }
    }

    /// `getDataType` and the attribute's little-endian bytes.
    fn data_type_and_bytes(data: &AttrData) -> Result<(DataType, Vec<u8>), CodecError> {
        Ok(match data {
            AttrData::F32(v) => (
                DataType::Float32,
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            AttrData::I8(v) => (DataType::Int8, v.iter().map(|&x| x as u8).collect()),
            AttrData::U8(v) => (DataType::Uint8, v.clone()),
            AttrData::I16(v) => (
                DataType::Int16,
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            AttrData::U16(v) => (
                DataType::Uint16,
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            AttrData::U32(v) => (
                DataType::Uint32,
                v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            ),
            #[allow(unreachable_patterns)] // `AttrData` is non-exhaustive
            _ => {
                return Err(CodecError::Encode(
                    "Draco encoder: unsupported attribute data type".into(),
                ));
            }
        })
    }

    fn draco_err(what: &str, e: draco_core::DracoError) -> String {
        format!("{what}: {}", e.message())
    }

    /// `encoderEncode`'s options.
    fn options(
        mesh: &Mesh,
        q: &DracoQuantization,
        level: u8,
        preserve_triangle_order: bool,
    ) -> EncoderOptions {
        let mut o = EncoderOptions::new();
        let speed = 10 - i32::from(level);
        o.set_global_int("encoding_speed", speed);
        o.set_global_int("decoding_speed", speed);
        for att_id in 0..mesh.num_attributes() {
            let kind = kind_of(mesh.attribute(att_id).attribute_type());
            o.set_attribute_int(
                att_id,
                "quantization_bits",
                i32::from(quantization_bits(q, kind)),
            );
            if preserve_triangle_order {
                // Google Draco's `MeshSequentialEncoder` has no corner table, so every mesh
                // prediction `SelectPredictionMethod` picks (parallelogram, the constrained
                // multi-parallelogram below speed 2, geometric normals and portable tex coords
                // below speed 4) falls back to `PREDICTION_DIFFERENCE` in
                // `CreatePredictionSchemeForEncoder`. draco-core 2.3.3 writes another stream
                // below speed 2; asking for the difference explicitly gives Draco's bytes at
                // every speed (verified against Blender's bridge at levels 0–10).
                o.set_attribute_int(att_id, "prediction_scheme", PREDICTION_DIFFERENCE);
            }
        }
        if preserve_triangle_order {
            o.set_encoding_method(MESH_SEQUENTIAL_ENCODING);
        }
        o
    }

    impl DracoCore {
        /// [`DracoCodec::encode`] with the bridge's `preserveTriangleOrder`.
        pub fn encode_with(
            &self,
            mesh: &MeshData,
            quantization: &DracoQuantization,
            level: u8,
            preserve_triangle_order: bool,
        ) -> Result<DracoEncoded, CodecError> {
            let n = check_input(mesh)?;
            let mut dm = Mesh::new();
            dm.set_num_points(n);
            let mut attribute_ids = Vec::with_capacity(mesh.attributes.len());
            for a in &mesh.attributes {
                let (dt, bytes) = data_type_and_bytes(&a.data)?;
                let mut pa = PointAttribute::new();
                pa.try_init(geometry_type(a.kind), a.components, dt, a.normalized, n)
                    .map_err(|e| CodecError::Encode(draco_err("Draco encoder", e)))?;
                pa.buffer_mut().write(0, &bytes);
                let id = dm.add_attribute(pa);
                attribute_ids.push(id as u32);
            }
            let faces = mesh.indices.len() / 3;
            dm.set_num_faces(faces);
            for (f, t) in mesh.indices.chunks_exact(3).enumerate() {
                dm.set_face(
                    FaceIndex(f as u32),
                    [PointIndex(t[0]), PointIndex(t[1]), PointIndex(t[2])],
                );
            }
            let opts = options(&dm, quantization, level, preserve_triangle_order);
            let mut enc = MeshEncoder::new();
            enc.set_mesh(dm);
            let mut out = EncoderBuffer::new();
            let info = enc
                .encode_with_info(&opts, &mut out)
                .map_err(|e| CodecError::Encode(draco_err("Error during Draco encoding", e)))?;
            Ok(DracoEncoded {
                bytes: out.data().to_vec(),
                attribute_ids,
                point_count: info.num_encoded_points as u32,
                index_count: (info.num_encoded_faces * 3) as u32,
            })
        }
    }

    /// The values of `pa` for every point, in its stored type (`mapped_index` per point).
    fn read_attribute(pa: &PointAttribute, num_points: usize) -> Result<AttrData, CodecError> {
        let comps = usize::from(pa.num_components());
        let dt = pa.data_type();
        let size = dt.byte_length();
        let stride = usize::try_from(pa.byte_stride())
            .unwrap_or(0)
            .max(comps * size);
        let offset = 0usize;
        let data = pa.buffer().data();
        let mut raw: Vec<u8> = Vec::with_capacity(num_points * comps * size);
        for p in 0..num_points {
            let avi = pa.mapped_index(PointIndex(p as u32));
            let start = (avi.0 as usize)
                .checked_mul(stride)
                .and_then(|s| s.checked_add(offset))
                .ok_or_else(|| CodecError::Decode("Draco attribute out of range".into()))?;
            let bytes = data.get(start..start + comps * size).ok_or_else(|| {
                CodecError::Decode(format!("Draco attribute value {} out of range", avi.0))
            })?;
            raw.extend_from_slice(bytes);
        }
        fn le<const N: usize, T>(raw: &[u8], f: fn([u8; N]) -> T) -> Vec<T> {
            raw.chunks_exact(N)
                .map(|c| f(c.try_into().expect("chunk size")))
                .collect()
        }
        Ok(match dt {
            DataType::Float32 => AttrData::F32(le(&raw, f32::from_le_bytes)),
            DataType::Int8 => AttrData::I8(raw.iter().map(|&b| b as i8).collect()),
            DataType::Uint8 | DataType::Bool => AttrData::U8(raw),
            DataType::Int16 => AttrData::I16(le(&raw, i16::from_le_bytes)),
            DataType::Uint16 => AttrData::U16(le(&raw, u16::from_le_bytes)),
            DataType::Uint32 => AttrData::U32(le(&raw, u32::from_le_bytes)),
            other => {
                return Err(CodecError::Decode(format!(
                    "Draco attribute data type {other:?} has no glTF accessor type"
                )));
            }
        })
    }

    impl DracoCodec for DracoCore {
        fn encode(
            &self,
            mesh: &MeshData,
            quantization: &DracoQuantization,
            level: u8,
        ) -> Result<DracoEncoded, CodecError> {
            self.encode_with(mesh, quantization, level, false)
        }

        fn encode_preserving_order(
            &self,
            mesh: &MeshData,
            quantization: &DracoQuantization,
            level: u8,
        ) -> Result<DracoEncoded, CodecError> {
            self.encode_with(mesh, quantization, level, true)
        }

        fn decode(&self, bytes: &[u8], attribute_ids: &[u32]) -> Result<MeshData, CodecError> {
            self.decode_counted(bytes, attribute_ids).map(|d| d.mesh)
        }

        fn decode_counted(
            &self,
            bytes: &[u8],
            attribute_ids: &[u32],
        ) -> Result<DracoDecoded, CodecError> {
            let mut buf = DecoderBuffer::new(bytes);
            let mut mesh = Mesh::new();
            MeshDecoder::new()
                .decode(&mut buf, &mut mesh)
                .map_err(|e| CodecError::Decode(draco_err("Error during Draco decoding", e)))?;
            let num_points = mesh.num_points();
            let mut indices = Vec::with_capacity(mesh.num_faces() * 3);
            for f in mesh.faces() {
                indices.extend(f.iter().map(|p| p.0));
            }
            let mut attributes = Vec::with_capacity(attribute_ids.len());
            for &id in attribute_ids {
                let pa = mesh.attribute_by_unique_id(id).ok_or_else(|| {
                    CodecError::Decode(format!(
                        "Attribute with id={id} does not exist in Draco data"
                    ))
                })?;
                attributes.push(Attribute {
                    kind: kind_of(pa.attribute_type()),
                    components: pa.num_components(),
                    normalized: pa.normalized(),
                    data: read_attribute(pa, num_points)?,
                });
            }
            Ok(DracoDecoded {
                mesh: MeshData {
                    indices,
                    attributes,
                },
                point_count: num_points as u32,
            })
        }
    }
}

#[cfg(all(test, feature = "draco"))]
mod tests {
    use super::*;
    use crate::draco::{DracoBackend, codec as select};
    use crate::mesh::{AttrData, Attribute};

    fn grid(n: usize) -> MeshData {
        // An n×n grid of quads on a bumpy surface (positions, normals, UVs, an RGBA16 colour,
        // JOINTS as bytes), two triangles per quad.
        let mut pos = Vec::new();
        let mut nrm = Vec::new();
        let mut uv = Vec::new();
        let mut col = Vec::new();
        let mut joints = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                let (x, y) = (i as f32 / n as f32, j as f32 / n as f32);
                pos.extend([x * 2.0 - 1.0, y * 3.0, (x * 5.0).sin() * 0.2]);
                let l = (1.0f32 + 0.04).sqrt();
                nrm.extend([0.2 / l, 0.0, 1.0 / l]);
                uv.extend([x, 1.0 - y]);
                col.extend([(i * 1000) as u16, (j * 700) as u16, 65535, 30000]);
                joints.extend([(i % 3) as u8, (j % 4) as u8, 0, 0]);
            }
        }
        let w = n as u32 + 1;
        let mut indices = Vec::new();
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let a = j * w + i;
                indices.extend([a, a + 1, a + w, a + w, a + 1, a + w + 1]);
            }
        }
        let attr = |kind, components, normalized, data| Attribute {
            kind,
            components,
            normalized,
            data,
        };
        MeshData {
            indices,
            attributes: vec![
                attr(AttributeKind::Position, 3, false, AttrData::F32(pos)),
                attr(AttributeKind::Normal, 3, false, AttrData::F32(nrm)),
                attr(AttributeKind::TexCoord, 2, false, AttrData::F32(uv)),
                attr(AttributeKind::Color, 4, true, AttrData::U16(col)),
                attr(AttributeKind::Generic, 4, false, AttrData::U8(joints)),
            ],
        }
    }

    fn codec() -> &'static dyn DracoCodec {
        select(DracoBackend::Auto).expect("draco feature on")
    }

    fn f32s(d: &AttrData) -> &[f32] {
        match d {
            AttrData::F32(v) => v,
            other => panic!("{other:?}"),
        }
    }

    /// The decoded triangles as tuples of the given float attribute, rotated so the smallest
    /// corner comes first (EdgeBreaker reorders points and faces, keeps windings).
    fn triangles(m: &MeshData, att: usize) -> Vec<[Vec<i64>; 3]> {
        let c = usize::from(m.attributes[att].components);
        let v = f32s(&m.attributes[att].data);
        let key = |i: u32| -> Vec<i64> {
            v[i as usize * c..(i as usize + 1) * c]
                .iter()
                .map(|x| (x * 1000.0).round() as i64)
                .collect()
        };
        let mut out: Vec<[Vec<i64>; 3]> = m
            .indices
            .chunks_exact(3)
            .map(|t| {
                let k = [key(t[0]), key(t[1]), key(t[2])];
                let r = (0..3).min_by_key(|&r| k[r].clone()).unwrap();
                [k[r].clone(), k[(r + 1) % 3].clone(), k[(r + 2) % 3].clone()]
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn selection_follows_the_spike_gate() {
        assert!(select(DracoBackend::Auto).is_some());
        assert!(select(DracoBackend::Core).is_some());
        assert_eq!(
            select(DracoBackend::Sys).is_some(),
            crate::draco_sys::codec().is_some()
        );
        assert_eq!(
            DracoBackend::from_identifier("CORE"),
            Some(DracoBackend::Core)
        );
        assert_eq!(DracoBackend::Sys.identifier(), "SYS");
        assert!(
            DracoBackend::Sys
                .unavailable()
                .to_string()
                .contains("draco-sys")
        );
    }

    #[test]
    fn round_trip_within_quantization_bounds() {
        let m = grid(6);
        let q = DracoQuantization::default();
        let e = codec().encode(&m, &q, 6).unwrap();
        assert_eq!(e.attribute_ids, [0, 1, 2, 3, 4]);
        assert_eq!(e.point_count as usize, m.vertex_count());
        assert_eq!(e.index_count as usize, m.indices.len());
        let d = codec().decode_counted(&e.bytes, &e.attribute_ids).unwrap();
        assert_eq!(d.point_count, e.point_count);
        assert_eq!(d.mesh.indices.len(), m.indices.len());
        // Positions: Draco quantizes over the largest extent of the bounding box, so each
        // component is within half a step of `range / (2^bits - 1)`.
        let step = 3.0 / ((1u32 << q.position) - 1) as f32;
        let got = f32s(&d.mesh.attributes[0].data);
        let want = f32s(&m.attributes[0].data);
        for p in got.chunks_exact(3) {
            let best = want
                .chunks_exact(3)
                .map(|w| (0..3).map(|c| (w[c] - p[c]).abs()).fold(0f32, f32::max))
                .fold(f32::MAX, f32::min);
            assert!(best <= step / 2.0 + 1e-6, "{p:?}: {best}");
        }
        // Normals (octahedral, 10 bits) stay unit length and close.
        for n in f32s(&d.mesh.attributes[1].data).chunks_exact(3) {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
            assert!(
                (n[0] - 0.196).abs() < 0.01 && (n[2] - 0.98).abs() < 0.01,
                "{n:?}"
            );
        }
        // Integer attributes (normalized colours, joints) are lossless, with their types and
        // `normalized`.
        let a = &d.mesh.attributes[3];
        assert_eq!(
            (a.kind, a.components, a.normalized),
            (AttributeKind::Color, 4, true)
        );
        let mut got: Vec<Vec<u16>> = match &a.data {
            AttrData::U16(v) => v.chunks_exact(4).map(<[u16]>::to_vec).collect(),
            other => panic!("{other:?}"),
        };
        let mut want: Vec<Vec<u16>> = match &m.attributes[3].data {
            AttrData::U16(v) => v.chunks_exact(4).map(<[u16]>::to_vec).collect(),
            _ => unreachable!(),
        };
        got.sort();
        want.sort();
        assert_eq!(got, want);
        assert!(matches!(d.mesh.attributes[4].data, AttrData::U8(_)));
        // The triangles (corner positions) survive the reordering.
        let lossless = DracoQuantization { position: 0, ..q };
        let e = codec().encode(&m, &lossless, 6).unwrap();
        let d = codec().decode(&e.bytes, &e.attribute_ids).unwrap();
        assert_eq!(triangles(&d, 0), triangles(&m, 0));
        assert_eq!(
            f32s(&d.attributes[0].data).len(),
            f32s(&m.attributes[0].data).len()
        );
    }

    #[test]
    fn preserving_order_keeps_points_and_triangles() {
        let m = grid(4);
        let q = DracoQuantization {
            position: 0,
            normal: 0,
            texcoord: 0,
            ..DracoQuantization::default()
        };
        for level in [0, 6, 9, 10] {
            let e = codec().encode_preserving_order(&m, &q, level).unwrap();
            let d = codec().decode(&e.bytes, &e.attribute_ids).unwrap();
            assert_eq!(d.indices, m.indices, "level {level}");
            for a in 0..3 {
                assert_eq!(d.attributes[a].data, m.attributes[a].data, "level {level}");
            }
            assert_eq!(d.attributes[3].data, m.attributes[3].data);
        }
    }

    #[test]
    fn every_level_and_integer_type_decodes() {
        let mut m = grid(3);
        let n = m.vertex_count();
        let attr = |kind, components, data| Attribute {
            kind,
            components,
            normalized: false,
            data,
        };
        m.attributes.push(attr(
            AttributeKind::Generic,
            2,
            AttrData::I8((0..2 * n).map(|i| (i % 50) as i8 - 25).collect()),
        ));
        m.attributes.push(attr(
            AttributeKind::Generic,
            1,
            AttrData::I16((0..n).map(|i| i as i16 * -7).collect()),
        ));
        m.attributes.push(attr(
            AttributeKind::Generic,
            1,
            AttrData::U32((0..n).map(|i| i as u32 * 100_000).collect()),
        ));
        for level in 0..=10 {
            let e = codec()
                .encode(&m, &DracoQuantization::default(), level)
                .unwrap();
            let d = codec().decode(&e.bytes, &e.attribute_ids).unwrap();
            assert_eq!(d.attributes.len(), 8, "level {level}");
            assert!(matches!(d.attributes[5].data, AttrData::I8(_)));
            assert!(matches!(d.attributes[6].data, AttrData::I16(_)));
            assert!(matches!(d.attributes[7].data, AttrData::U32(_)));
            let mut got = match &d.attributes[7].data {
                AttrData::U32(v) => v.clone(),
                _ => unreachable!(),
            };
            got.sort_unstable();
            assert_eq!(got, (0..n as u32).map(|i| i * 100_000).collect::<Vec<_>>());
        }
    }

    #[test]
    fn bad_input_and_bad_streams_are_errors() {
        let mut m = grid(2);
        m.indices.push(0);
        assert!(matches!(
            codec().encode(&m, &DracoQuantization::default(), 6),
            Err(CodecError::Encode(_))
        ));
        let mut m = grid(2);
        m.indices[0] = 1000;
        assert!(matches!(
            codec().encode(&m, &DracoQuantization::default(), 6),
            Err(CodecError::Encode(_))
        ));
        let mut m = grid(2);
        m.attributes[1].data = AttrData::F32(vec![0.0; 3]);
        assert!(
            codec()
                .encode(&m, &DracoQuantization::default(), 6)
                .is_err()
        );
        let e = codec()
            .encode(&grid(2), &DracoQuantization::default(), 6)
            .unwrap();
        let err = codec().decode(&e.bytes, &[0, 9]).unwrap_err();
        assert_eq!(
            err,
            CodecError::Decode("Attribute with id=9 does not exist in Draco data".into())
        );
        assert!(matches!(
            codec().decode(b"not a draco stream", &[]),
            Err(CodecError::Decode(_))
        ));
        assert!(codec().decode(&e.bytes[..e.bytes.len() / 2], &[0]).is_err());
    }

    #[test]
    fn quantization_follows_the_semantic() {
        let q = DracoQuantization {
            position: 1,
            normal: 2,
            texcoord: 3,
            color: 4,
            generic: 5,
        };
        assert_eq!(quantization_bits(&q, AttributeKind::Position), 1);
        assert_eq!(quantization_bits(&q, AttributeKind::Normal), 2);
        assert_eq!(quantization_bits(&q, AttributeKind::TexCoord), 3);
        assert_eq!(quantization_bits(&q, AttributeKind::Color), 4);
        assert_eq!(quantization_bits(&q, AttributeKind::Generic), 5);
    }
}
