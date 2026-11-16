// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests: whatever `BufferBuilder` writes (plain, padded-stride, interleaved,
//! sparse), `read_accessor` reads back exactly, the result validates, and the GLB container
//! round-trips arbitrary payloads.

use frap_gltf::accessor::{AccessorData, Components};
use frap_gltf::json::{AccessorType, ComponentType, Document, Target};
use frap_gltf::{
    AccessorOpts, BufferBuilder, Glb, WriteOpts, read_accessor, read_glb, validate, write_glb,
};
use proptest::prelude::*;

const TYPES: [AccessorType; 7] = [
    AccessorType::Scalar,
    AccessorType::Vec2,
    AccessorType::Vec3,
    AccessorType::Vec4,
    AccessorType::Mat2,
    AccessorType::Mat3,
    AccessorType::Mat4,
];

const CTS: [ComponentType; 6] = [
    ComponentType::Byte,
    ComponentType::UnsignedByte,
    ComponentType::Short,
    ComponentType::UnsignedShort,
    ComponentType::UnsignedInt,
    ComponentType::Float,
];

fn components(ct: ComponentType, n: usize) -> BoxedStrategy<Components> {
    match ct {
        ComponentType::Byte => prop::collection::vec(any::<i8>(), n)
            .prop_map(Components::I8)
            .boxed(),
        ComponentType::UnsignedByte => prop::collection::vec(any::<u8>(), n)
            .prop_map(Components::U8)
            .boxed(),
        ComponentType::Short => prop::collection::vec(any::<i16>(), n)
            .prop_map(Components::I16)
            .boxed(),
        ComponentType::UnsignedShort => prop::collection::vec(any::<u16>(), n)
            .prop_map(Components::U16)
            .boxed(),
        ComponentType::UnsignedInt => prop::collection::vec(any::<u32>(), n)
            .prop_map(Components::U32)
            .boxed(),
        ComponentType::Float => prop::collection::vec(-1e6f32..1e6, n)
            .prop_map(Components::F32)
            .boxed(),
    }
}

fn accessor_data(max_count: usize) -> impl Strategy<Value = AccessorData> {
    (0..TYPES.len(), 0..CTS.len(), 1..=max_count, any::<bool>()).prop_flat_map(
        |(t, c, count, norm)| {
            let ty = TYPES[t];
            let ct = CTS[c];
            components(ct, count * ty.components()).prop_map(move |comps| AccessorData {
                type_: ty,
                normalized: norm && ct.can_normalize(),
                components: comps,
            })
        },
    )
}

/// Vector data (interleavable vertex attributes) with a fixed count.
fn vector_data(count: usize) -> impl Strategy<Value = AccessorData> {
    (0..4usize, 0..CTS.len(), any::<bool>()).prop_flat_map(move |(t, c, norm)| {
        let ty = TYPES[t];
        let ct = CTS[c];
        components(ct, count * ty.components()).prop_map(move |comps| AccessorData {
            type_: ty,
            normalized: norm && ct.can_normalize(),
            components: comps,
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn push_then_read(data in accessor_data(12), attr in any::<bool>(), min_max in any::<bool>()) {
        let mut b = BufferBuilder::new();
        // ARRAY_BUFFER is only meaningful for vector data; matrices go in plain views.
        let target = (attr && data.type_.columns_rows().0 == 1).then_some(Target::ArrayBuffer);
        let i = b.push_accessor(&data, &AccessorOpts { target, min_max, name: None });
        let mut doc = Document::default();
        let bin = b.finish(&mut doc, None);
        prop_assert_eq!(read_accessor(&doc, &[&bin], i).unwrap(), data.clone());
        let r = validate(&doc);
        prop_assert!(r.is_ok(), "{}", r);
        if let Some(s) = doc.buffer_views[0].byte_stride {
            prop_assert_eq!(s % 4, 0);
        }
    }

    #[test]
    fn interleaved_then_read(
        cols in (1..20usize).prop_flat_map(|n| prop::collection::vec(vector_data(n), 1..5))
    ) {
        let opts = AccessorOpts::default();
        let refs: Vec<(&AccessorData, &AccessorOpts)> = cols.iter().map(|c| (c, &opts)).collect();
        let mut b = BufferBuilder::new();
        let ids = b.push_interleaved(&refs).unwrap();
        let mut doc = Document::default();
        let bin = b.finish(&mut doc, None);
        for (id, c) in ids.iter().zip(&cols) {
            prop_assert_eq!(&read_accessor(&doc, &[&bin], *id).unwrap(), c);
        }
        let r = validate(&doc);
        prop_assert!(r.is_ok(), "{}", r);
    }

    #[test]
    fn sparse_then_read(
        (data, picks, with_base) in accessor_data(40).prop_flat_map(|d| {
            let n = d.count();
            (Just(d), prop::collection::btree_set(0..n as u32, 1..=n), any::<bool>())
        })
    ) {
        let indices: Vec<u32> = picks.into_iter().collect();
        let n = data.type_.components();
        // The dense data equals the base everywhere except at `indices`.
        let base = AccessorData {
            components: {
                let mut c = data.components.clone();
                for e in 0..data.count() {
                    if !indices.contains(&(e as u32)) {
                        continue;
                    }
                    for k in 0..n {
                        zero(&mut c, e * n + k);
                    }
                }
                c
            },
            ..data.clone()
        };
        let mut b = BufferBuilder::new();
        let (dense, base_arg) = if with_base {
            (data.clone(), Some(&base))
        } else {
            // Without a base, everything outside `indices` reads as zero.
            let mut d = data.clone();
            for e in 0..d.count() {
                if !indices.contains(&(e as u32)) {
                    for k in 0..n {
                        zero(&mut d.components, e * n + k);
                    }
                }
            }
            (d, None)
        };
        let i = b.push_sparse(&dense, &indices, base_arg, &AccessorOpts::default()).unwrap();
        let mut doc = Document::default();
        let bin = b.finish(&mut doc, None);
        prop_assert_eq!(read_accessor(&doc, &[&bin], i).unwrap(), dense);
        let r = validate(&doc);
        prop_assert!(r.is_ok(), "{}", r);
    }

    #[test]
    fn glb_container_round_trip(bin in prop::collection::vec(any::<u8>(), 0..64), name in "[a-z]{0,9}") {
        let mut document = Document::default();
        document.asset.generator = Some(name);
        let g = Glb { document, bin: Some(bin.clone()) };
        let bytes = write_glb(&g, &WriteOpts::default());
        prop_assert_eq!(bytes.len() % 4, 0);
        let back = read_glb(&bytes).unwrap();
        prop_assert_eq!(back.document, g.document);
        match back.bin {
            None => prop_assert!(bin.is_empty()),
            Some(b) => {
                prop_assert_eq!(&b[..bin.len()], &bin[..]);
                prop_assert!(b[bin.len()..].iter().all(|&x| x == 0));
                prop_assert!(b.len() - bin.len() < 4);
            }
        }
    }

    #[test]
    fn read_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..96)) {
        let _ = frap_gltf::read(&bytes);
        let mut glb = b"glTF\x02\0\0\0".to_vec();
        glb.extend_from_slice(&((bytes.len() + 12) as u32).to_le_bytes());
        glb.extend_from_slice(&bytes);
        let _ = read_glb(&glb);
    }
}

fn zero(c: &mut Components, i: usize) {
    match c {
        Components::I8(v) => v[i] = 0,
        Components::U8(v) => v[i] = 0,
        Components::I16(v) => v[i] = 0,
        Components::U16(v) => v[i] = 0,
        Components::U32(v) => v[i] = 0,
        Components::F32(v) => v[i] = 0.0,
    }
}
