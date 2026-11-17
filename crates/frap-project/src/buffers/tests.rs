// SPDX-License-Identifier: GPL-3.0-or-later
//! Content addressing, the inline threshold and the array codecs.

use proptest::prelude::*;
use serde_json::json;

use super::*;

fn encode_to_json<T: Element>(store: &mut BufferStore, data: &[T]) -> String {
    serde_json::to_string(&store.encode(data)).unwrap()
}

#[test]
fn inline_threshold_is_64_elements() {
    let mut store = BufferStore::new();
    let short: Vec<f32> = (0..63u8).map(f32::from).collect();
    let long: Vec<f32> = (0..64u8).map(f32::from).collect();
    assert!(matches!(store.encode(&short), Encoded::Inline(_)));
    assert!(store.is_empty());
    assert!(matches!(store.encode(&long), Encoded::Buffer(_)));
    assert_eq!(store.len(), 1);

    // The threshold counts elements, not scalars: 63 vectors (189 floats) are still inline.
    let vecs = vec![[1.0f32, 2.0, 3.0]; 63];
    assert!(matches!(store.encode(&vecs), Encoded::Inline(_)));
    let vecs = vec![[1.0f32, 2.0, 3.0]; 64];
    assert!(matches!(store.encode(&vecs), Encoded::Buffer(_)));
    assert_eq!(INLINE_THRESHOLD, 64);
}

#[test]
fn non_finite_floats_force_a_buffer() {
    let mut store = BufferStore::new();
    let data = [0.0f32, f32::NAN, 1.0];
    let Encoded::Buffer(r) = store.encode(&data) else {
        panic!("NaN written inline")
    };
    let back: Vec<f32> = store.load(&r).unwrap();
    assert_eq!(
        back.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        data.map(f32::to_bits)
    );
    assert!(matches!(store.encode(&[f64::INFINITY]), Encoded::Buffer(_)));
}

#[test]
fn inline_json_shape_and_shortest_floats() {
    let mut store = BufferStore::new();
    assert_eq!(
        encode_to_json(&mut store, &[0.1f32, -0.0, 1.0, 3.4028235e38]),
        "[0.1,-0.0,1.0,3.4028235e+38]"
    );
    assert_eq!(
        encode_to_json(&mut store, &[[0.1f32, 0.2, 0.3]]),
        "[[0.1,0.2,0.3]]"
    );
    assert_eq!(encode_to_json(&mut store, &[true, false]), "[true,false]");
    assert_eq!(
        encode_to_json(&mut store, &[[0i32, 1], [1, 2]]),
        "[[0,1],[1,2]]"
    );
    assert_eq!(encode_to_json::<u8>(&mut store, &[]), "[]");
}

#[test]
fn buffer_reference_shape() {
    let mut store = BufferStore::new();
    let data: Vec<[f32; 3]> = (0..100u8).map(|i| [f32::from(i), 0.0, 1.0]).collect();
    let json = encode_to_json(&mut store, &data);
    let hash = ContentHash::of(&to_le_bytes(&data));
    assert_eq!(
        json,
        format!(r#"{{"$buf":"{hash}","dtype":"f32x3","len":100}}"#)
    );
    assert_eq!(store.get(&hash).unwrap().len(), 1200);
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(store.decode::<[f32; 3]>(&v).unwrap(), data);
    let r = BufRef::deserialize(&v).unwrap();
    assert_eq!(r.path(), format!("buffers/{hash}.bin"));
}

#[test]
fn little_endian_layout() {
    assert_eq!(
        to_le_bytes(&[1i32, -2]),
        [1, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        to_le_bytes(&[[1.0f32, 2.0]]),
        [0, 0, 0x80, 0x3f, 0, 0, 0, 0x40]
    );
    assert_eq!(to_le_bytes(&[true, false, true]), [1, 0, 1]);
    assert_eq!(to_le_bytes(&[0x0102u16]), [2, 1]);
}

#[test]
fn equal_content_is_stored_once() {
    let mut store = BufferStore::new();
    let a: Vec<i32> = (0..1000).collect();
    let r1 = store.put(&a);
    let r2 = store.put(&a.clone());
    assert_eq!(r1, r2);
    assert_eq!(store.len(), 1);
    // Same bytes under another dtype share the buffer; the dtype lives in the reference.
    let as_u32: Vec<u32> = (0..1000).collect();
    let r3 = store.put(&as_u32);
    assert_eq!(r3.hash, r1.hash);
    assert_ne!(r3.dtype, r1.dtype);
    assert_eq!(store.len(), 1);
    let other: Vec<i32> = (1..1001).collect();
    assert_ne!(store.put(&other).hash, r1.hash);
    assert_eq!(store.len(), 2);
    // Hash order iteration.
    let hashes: Vec<_> = store.iter().map(|(h, _)| *h).collect();
    let mut sorted = hashes.clone();
    sorted.sort();
    assert_eq!(hashes, sorted);
}

#[test]
fn load_rejects_mismatches() {
    let mut store = BufferStore::new();
    let r = store.put(&[1.0f32; 100]);
    assert_eq!(
        store.load::<i32>(&r),
        Err(BufferError::DtypeMismatch {
            expected: <i32>::dtype(),
            found: <f32>::dtype()
        })
    );
    let wrong_len = BufRef { len: 99, ..r };
    assert!(matches!(
        store.load::<f32>(&wrong_len),
        Err(BufferError::SizeMismatch {
            expected: 396,
            actual: 400,
            ..
        })
    ));
    let missing = BufRef {
        hash: ContentHash::of(b"nope"),
        ..r
    };
    assert_eq!(
        store.load::<f32>(&missing),
        Err(BufferError::Missing(missing.hash))
    );
    let huge = BufRef { len: u64::MAX, ..r };
    assert!(store.load::<f32>(&huge).is_err());

    let bools = store.put_raw(<bool>::dtype(), 3, vec![1, 2, 0]).unwrap();
    assert!(matches!(
        store.load::<bool>(&bools),
        Err(BufferError::InvalidData { .. })
    ));
    assert!(store.put_raw(<f32>::dtype(), 3, vec![0; 11]).is_err());
}

#[test]
fn decode_rejects_bad_inline() {
    let store = BufferStore::new();
    assert!(store.decode::<[f32; 3]>(&json!([[1, 2]])).is_err());
    assert!(store.decode::<u8>(&json!([300])).is_err());
    assert!(store.decode::<f32>(&json!("x")).is_err());
    assert!(
        store
            .decode::<f32>(&json!({"$buf": "00", "dtype": "f32", "len": 1}))
            .is_err()
    );
    let unknown_field =
        json!({"$buf": ContentHash::of(b"").to_hex(), "dtype": "f32", "len": 0, "x": 1});
    assert!(store.decode::<f32>(&unknown_field).is_err());
    assert_eq!(store.decode::<f32>(&json!([1, 2.5])).unwrap(), [1.0, 2.5]);
}

#[test]
fn images_are_content_addressed() {
    let mut images = ImageStore::new();
    let a = images.insert(b"\x89PNG fake".to_vec(), ".PNG").unwrap();
    assert_eq!(a.ext, "png");
    assert_eq!(
        a.path(),
        format!("images/{}.png", ContentHash::of(b"\x89PNG fake"))
    );
    assert_eq!(images.insert(b"\x89PNG fake".to_vec(), "png").unwrap(), a);
    assert_eq!(images.len(), 1);
    assert_eq!(images.get(&a), Some(&b"\x89PNG fake"[..]));
    assert!(images.insert(vec![], "").is_err());
    assert!(images.insert(vec![], "p/ng").is_err());
    assert!(images.insert(vec![], "toolongext").is_err());
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        format!(r#"{{"$img":"{}","ext":"png"}}"#, a.hash)
    );
    let bad = ImageRef {
        hash: ContentHash::of(b"x"),
        ext: "png".into(),
    };
    assert!(
        images
            .insert_verified(bad, b"y".to_vec(), "images/x.png")
            .is_err()
    );
}

#[test]
fn referenced_finds_every_ref() {
    let mut store = BufferStore::new();
    let mut images = ImageStore::new();
    let a = store.put(&[1u8; 100]);
    let b = store.put(&[2u8; 100]);
    let img = images.insert(b"img".to_vec(), "png").unwrap();
    let scene = json!({
        "meshes": {"M": {"attributes": [{"data": a}, {"data": [1, 2]}], "face_offsets": b}},
        "images": {"I": {"packed_file": img}},
        "other": {"$buf": "not a hash"},
    });
    let (bufs, imgs) = referenced(&scene);
    let mut expected = vec![a.hash, b.hash];
    expected.sort();
    assert_eq!(bufs.into_iter().collect::<Vec<_>>(), expected);
    assert_eq!(imgs.into_iter().collect::<Vec<_>>(), [img]);
}

fn roundtrip<T: Element + PartialEq>(data: &[T]) -> Vec<T> {
    let mut store = BufferStore::new();
    let json = serde_json::to_string(&store.encode(data)).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    store.decode(&v).unwrap()
}

proptest! {
    #[test]
    fn f32_round_trips_bit_exact(bits in proptest::collection::vec(any::<u32>(), 0..130)) {
        let data: Vec<f32> = bits.iter().map(|&b| f32::from_bits(b)).collect();
        let back = roundtrip(&data);
        prop_assert_eq!(back.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), bits);
    }

    #[test]
    fn f32x3_round_trips_bit_exact(v in proptest::collection::vec(any::<[u32; 3]>(), 0..130)) {
        let data: Vec<[f32; 3]> = v.iter().map(|a| a.map(f32::from_bits)).collect();
        let back = roundtrip(&data);
        prop_assert_eq!(back.iter().map(|a| a.map(f32::to_bits)).collect::<Vec<_>>(), v);
    }

    #[test]
    fn f64_round_trips_bit_exact(bits in proptest::collection::vec(any::<u64>(), 0..130)) {
        let data: Vec<f64> = bits.iter().map(|&b| f64::from_bits(b)).collect();
        let back = roundtrip(&data);
        prop_assert_eq!(back.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), bits);
    }

    #[test]
    fn ints_and_bools_round_trip(
        a in proptest::collection::vec(any::<i64>(), 0..130),
        b in proptest::collection::vec(any::<u64>(), 0..130),
        c in proptest::collection::vec(any::<[i8; 4]>(), 0..130),
        d in proptest::collection::vec(any::<bool>(), 0..130),
        e in proptest::collection::vec(any::<[u16; 2]>(), 0..130),
    ) {
        prop_assert_eq!(roundtrip(&a), a);
        prop_assert_eq!(roundtrip(&b), b);
        prop_assert_eq!(roundtrip(&c), c);
        prop_assert_eq!(roundtrip(&d), d);
        prop_assert_eq!(roundtrip(&e), e);
    }
}
