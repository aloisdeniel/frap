// SPDX-License-Identifier: GPL-3.0-or-later
//! Determinism, layout, round trip, version refusal, migration on load and corruption checks.

use std::fmt::Write as _;

use proptest::prelude::*;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;
use crate::manifest::SCHEMA_VERSION;

/// A small scene with one buffer-sized and one inline array, plus a packed image.
fn sample() -> (Vec<u8>, BufferStore, ImageStore) {
    let mut buffers = BufferStore::new();
    let mut images = ImageStore::new();
    let positions: Vec<[f32; 3]> = (0..100u8)
        .map(|i| [f32::from(i) * 0.5, -1.0, 0.25])
        .collect();
    let offsets: Vec<i32> = (0..=6).map(|i| i * 4).collect();
    let png = images
        .insert(b"\x89PNG\r\n\x1a\nnot really a png".to_vec(), "png")
        .unwrap();
    let scene = json!({
        "window": {"scene": "Scene", "view_layer": "ViewLayer"},
        "images": {"Tex": {"packed_file": png}},
        "meshes": {"Cube": {
            "face_offsets": buffers.encode(&offsets),
            "attributes": [{"name": "position", "domain": "POINT", "data_type": "FLOAT_VECTOR",
                            "data": buffers.encode(&positions)}],
        }},
    });
    (to_json_pretty(&scene).unwrap(), buffers, images)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Writes an arbitrary zip (for malformed and old-version fixtures).
fn raw_zip(entries: &[(&str, CompressionMethod, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, method, bytes) in entries {
        zip.start_file(*name, options(*method)).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// The entries of a `.frap` as `(name, method, bytes)`, for rebuilding a modified copy.
fn entries_of(bytes: &[u8]) -> Vec<(String, CompressionMethod, Vec<u8>)> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            (f.name().unwrap().into_owned(), f.compression(), data)
        })
        .collect()
}

fn rebuild(entries: &[(String, CompressionMethod, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(&str, CompressionMethod, &[u8])> = entries
        .iter()
        .map(|(n, m, b)| (n.as_str(), *m, b.as_slice()))
        .collect();
    raw_zip(&refs)
}

fn modify(bytes: &[u8], f: impl FnOnce(&mut Vec<(String, CompressionMethod, Vec<u8>)>)) -> Vec<u8> {
    let mut entries = entries_of(bytes);
    f(&mut entries);
    rebuild(&entries)
}

fn strict() -> ReadOptions {
    ReadOptions::default()
}

fn lenient() -> ReadOptions {
    ReadOptions { lenient: true }
}

#[test]
fn saving_twice_is_byte_identical() {
    let (scene, buffers, images) = sample();
    let a = write(&scene, &buffers, &images).unwrap();
    let b = write(&scene, &buffers, &images).unwrap();
    assert_eq!(a, b);
    // Rebuilding the same state from scratch (different insertion order) gives the same bytes.
    let (scene2, buffers2, images2) = sample();
    let mut reordered = BufferStore::new();
    let mut hashes: Vec<_> = buffers2.iter().map(|(h, b)| (*h, b.to_vec())).collect();
    hashes.reverse();
    for (_, bytes) in hashes {
        reordered.insert(bytes);
    }
    assert_eq!(write(&scene2, &reordered, &images2).unwrap(), a);
    // And re-saving what was read gives the same bytes again.
    let loaded = read(&a, strict()).unwrap();
    assert_eq!(
        write(
            &to_json_pretty(&loaded.scene).unwrap(),
            &loaded.buffers,
            &loaded.images
        )
        .unwrap(),
        a
    );
}

#[test]
fn archive_layout() {
    let (scene, buffers, images) = sample();
    let bytes = write(&scene, &buffers, &images).unwrap();

    // `mimetype` is the first local header, Stored, no extra field: name at 30, content at 38.
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(
        u16::from_le_bytes([bytes[8], bytes[9]]),
        0,
        "mimetype must be Stored"
    );
    assert_eq!(u16::from_le_bytes([bytes[26], bytes[27]]), 8);
    assert_eq!(
        u16::from_le_bytes([bytes[28], bytes[29]]),
        0,
        "no extra field on mimetype"
    );
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..38 + MIMETYPE.len()], MIMETYPE.as_bytes());

    let mut zip = ZipArchive::new(Cursor::new(&bytes[..])).unwrap();
    let mut listing = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index(i).unwrap();
        let t = f.last_modified().unwrap();
        assert_eq!(
            (
                t.year(),
                t.month(),
                t.day(),
                t.hour(),
                t.minute(),
                t.second()
            ),
            (1980, 1, 1, 0, 0, 0)
        );
        assert_eq!(f.unix_mode(), Some(0o100_644));
        assert!(
            f.extra_data().is_none_or(|d| d.is_empty()),
            "extra field on {}",
            f.name().unwrap()
        );
        listing.push((f.name().unwrap().into_owned(), f.compression()));
    }
    let buf_names: Vec<_> = buffers
        .iter()
        .map(|(h, _)| (buffer_path(h), CompressionMethod::Deflated))
        .collect();
    let mut expected = vec![
        ("mimetype".to_owned(), CompressionMethod::Stored),
        ("manifest.json".to_owned(), CompressionMethod::Deflated),
        ("scene.json".to_owned(), CompressionMethod::Deflated),
    ];
    expected.extend(buf_names);
    expected.extend(
        images
            .iter()
            .map(|(r, _)| (r.path(), CompressionMethod::Stored)),
    );
    assert_eq!(listing, expected);
    // Buffers are in hash order; the inline face offsets made no buffer.
    assert_eq!(buffers.len(), 1);
}

#[test]
fn archive_snapshot() {
    // Pins the exact bytes (CI runs this on macOS, Linux and Windows: a platform-dependent byte
    // anywhere in the container fails one of them).
    let (scene, buffers, images) = sample();
    let bytes = write_with_generator(&scene, &buffers, &images, "frap <pinned>").unwrap();
    let mut zip = ZipArchive::new(Cursor::new(&bytes[..])).unwrap();
    let mut out = format!("size {}\nsha256 {}\n", bytes.len(), sha256_hex(&bytes));
    for i in 0..zip.len() {
        let f = zip.by_index(i).unwrap();
        let _ = writeln!(
            out,
            "{} {:?} size={} compressed={} crc32={:08x}",
            f.name().unwrap(),
            f.compression(),
            f.size(),
            f.compressed_size(),
            f.crc32()
        );
    }
    let manifest = entries_of(&bytes)
        .into_iter()
        .find(|(n, ..)| n == ENTRY_MANIFEST)
        .unwrap()
        .2;
    let manifest = String::from_utf8(manifest).unwrap();
    let scene = String::from_utf8(scene).unwrap();
    insta::assert_snapshot!(format!(
        "{out}\n--- manifest.json\n{manifest}--- scene.json\n{scene}"
    ));
}

#[test]
fn round_trip() {
    let (scene, buffers, images) = sample();
    let loaded = read(&write(&scene, &buffers, &images).unwrap(), strict()).unwrap();
    assert_eq!(
        loaded.scene,
        serde_json::from_slice::<Value>(&scene).unwrap()
    );
    assert_eq!(loaded.buffers, buffers);
    assert_eq!(loaded.images, images);
    assert_eq!(loaded.manifest, Manifest::new(&buffers, &images));
    assert_eq!(loaded.manifest.schema_version, SCHEMA_VERSION);
    assert!(loaded.migration.is_none());
    let positions: Vec<[f32; 3]> = loaded
        .buffers
        .decode(&loaded.scene["meshes"]["Cube"]["attributes"][0]["data"])
        .unwrap();
    assert_eq!(positions.len(), 100);
    assert_eq!(positions[3], [1.5, -1.0, 0.25]);
    let offsets: Vec<i32> = loaded
        .buffers
        .decode(&loaded.scene["meshes"]["Cube"]["face_offsets"])
        .unwrap();
    assert_eq!(offsets, [0, 4, 8, 12, 16, 20, 24]);
    let png: ImageRef =
        serde_json::from_value(loaded.scene["images"]["Tex"]["packed_file"].clone()).unwrap();
    assert!(loaded.images.get(&png).unwrap().starts_with(b"\x89PNG"));
}

#[test]
fn empty_project_round_trips() {
    let bytes = write(b"{}", &BufferStore::new(), &ImageStore::new()).unwrap();
    let loaded = read(&bytes, strict()).unwrap();
    assert_eq!(loaded.scene, json!({}));
    assert!(loaded.buffers.is_empty() && loaded.images.is_empty());
}

#[test]
fn write_rejects_invalid_scene_json() {
    let (b, i) = (BufferStore::new(), ImageStore::new());
    assert!(matches!(
        write(b"{", &b, &i),
        Err(ProjectError::Json {
            entry: "scene.json",
            ..
        })
    ));
    assert!(matches!(
        write(b"[]", &b, &i),
        Err(ProjectError::Json {
            entry: "scene.json",
            ..
        })
    ));
}

fn manifest_json(version: Value, extra: &[(&str, Value)]) -> Vec<u8> {
    let mut m =
        serde_json::to_value(Manifest::new(&BufferStore::new(), &ImageStore::new())).unwrap();
    m["schema_version"] = version;
    for (k, v) in extra {
        m[*k] = v.clone();
    }
    serde_json::to_vec(&m).unwrap()
}

#[test]
fn newer_schema_is_refused() {
    let bytes = raw_zip(&[
        (
            ENTRY_MIMETYPE,
            CompressionMethod::Stored,
            MIMETYPE.as_bytes(),
        ),
        (
            ENTRY_MANIFEST,
            CompressionMethod::Deflated,
            &manifest_json(json!(SCHEMA_VERSION + 1), &[]),
        ),
        (ENTRY_SCENE, CompressionMethod::Deflated, b"{}"),
    ]);
    for opts in [strict(), lenient()] {
        let err = read(&bytes, opts).unwrap_err();
        assert!(err.is_newer_schema(), "{err}");
        assert_eq!(
            err.to_string(),
            format!(
                "file written by a newer frap (schema_version {}); this build reads schema_version {SCHEMA_VERSION} and older",
                SCHEMA_VERSION + 1
            )
        );
    }
}

#[test]
fn v0_file_is_migrated_on_load() {
    let mut buffers = BufferStore::new();
    let positions: Vec<[f32; 3]> = vec![[1.0, 2.0, 3.0]; 70];
    let pos = buffers.put(&positions);
    let manifest = json!({
        "format": "frap", "schema_version": 0, "blender_rna_version": "5.2", "generator": "frap 0.0.1",
        "buffers": [{"hash": pos.hash, "size": 840}], "images": [],
    });
    let scene = json!({"meshes": {"Cube": {"attributes": [
        {"name": "position", "domain": "POINT", "type": "FLOAT_VECTOR", "data": pos},
    ]}}});
    let bytes = raw_zip(&[
        (
            ENTRY_MIMETYPE,
            CompressionMethod::Stored,
            MIMETYPE.as_bytes(),
        ),
        (
            ENTRY_MANIFEST,
            CompressionMethod::Deflated,
            &serde_json::to_vec(&manifest).unwrap(),
        ),
        (
            ENTRY_SCENE,
            CompressionMethod::Deflated,
            &serde_json::to_vec(&scene).unwrap(),
        ),
        (
            &pos.path(),
            CompressionMethod::Deflated,
            buffers.get(&pos.hash).unwrap(),
        ),
    ]);
    let loaded = read(&bytes, strict()).unwrap();
    let report = loaded.migration.as_ref().unwrap();
    assert_eq!((report.from, report.to), (0, 1));
    assert_eq!(loaded.manifest.schema_version, 1);
    assert_eq!(loaded.manifest.blender_api, "5.2");
    assert_eq!(loaded.manifest.generator, "frap 0.0.1");
    let attr = &loaded.scene["meshes"]["Cube"]["attributes"][0];
    assert_eq!(attr["data_type"], "FLOAT_VECTOR");
    assert!(attr.get("type").is_none());
    assert_eq!(
        loaded.buffers.decode::<[f32; 3]>(&attr["data"]).unwrap(),
        positions
    );
    // Saving writes the current schema.
    let resaved = read(
        &write(
            &to_json_pretty(&loaded.scene).unwrap(),
            &loaded.buffers,
            &loaded.images,
        )
        .unwrap(),
        strict(),
    )
    .unwrap();
    assert!(resaved.migration.is_none());
    assert_eq!(resaved.scene, loaded.scene);
}

#[test]
fn not_frap_files() {
    assert!(matches!(read(b"", strict()), Err(ProjectError::NotFrap(_))));
    assert!(matches!(
        read(b"PK\x03\x04garbage", strict()),
        Err(ProjectError::NotFrap(_))
    ));
    assert!(matches!(
        read(&raw_zip(&[]), strict()),
        Err(ProjectError::NotFrap(_))
    ));
    let (scene, buffers, images) = sample();
    let good = write(&scene, &buffers, &images).unwrap();
    // mimetype not first.
    let swapped = modify(&good, |e| e.swap(0, 1));
    assert!(matches!(
        read(&swapped, lenient()),
        Err(ProjectError::NotFrap(_))
    ));
    // mimetype compressed.
    let deflated = modify(&good, |e| e[0].1 = CompressionMethod::Deflated);
    assert!(matches!(
        read(&deflated, lenient()),
        Err(ProjectError::NotFrap(_))
    ));
    // wrong media type.
    let other = modify(&good, |e| e[0].2 = b"application/vnd.frap+zap".to_vec());
    assert!(matches!(
        read(&other, lenient()),
        Err(ProjectError::NotFrap(_))
    ));
    let err = read(&other, lenient()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "not a .frap file: the first entry is not `mimetype` = \"application/vnd.frap+zip\""
    );
}

#[test]
fn corrupt_entries_are_detected() {
    let (scene, buffers, images) = sample();
    let good = write(&scene, &buffers, &images).unwrap();
    let buf = buffer_path(buffers.iter().next().unwrap().0);

    // A buffer whose bytes no longer match its hash.
    let tampered = modify(&good, |e| {
        let b = e.iter_mut().find(|(n, ..)| *n == buf).unwrap();
        b.2[0] ^= 1;
    });
    let err = read(&tampered, lenient()).unwrap_err();
    assert!(
        matches!(err, ProjectError::Buffer(BufferError::HashMismatch { .. })),
        "{err}"
    );

    // A listed buffer that is missing.
    let missing = modify(&good, |e| e.retain(|(n, ..)| *n != buf));
    assert!(matches!(read(&missing, lenient()), Err(ProjectError::MissingEntry(p)) if p == buf));

    // A buffer whose size differs from the manifest.
    let resized = modify(&good, |e| {
        let b = e.iter_mut().find(|(n, ..)| *n == buf).unwrap();
        b.2.push(0);
    });
    assert!(matches!(
        read(&resized, lenient()),
        Err(ProjectError::Corrupt(_))
    ));

    // Missing scene.json / manifest.json.
    for entry in [ENTRY_SCENE, ENTRY_MANIFEST] {
        let gone = modify(&good, |e| e.retain(|(n, ..)| n != entry));
        assert!(matches!(read(&gone, lenient()), Err(ProjectError::MissingEntry(p)) if p == entry));
    }

    // Broken JSON.
    let broken = modify(&good, |e| e[2].2 = b"{\"a\":".to_vec());
    assert!(matches!(
        read(&broken, lenient()),
        Err(ProjectError::Json {
            entry: "scene.json",
            ..
        })
    ));
    let array = modify(&good, |e| e[2].2 = b"[]".to_vec());
    assert!(matches!(
        read(&array, lenient()),
        Err(ProjectError::Json {
            entry: "scene.json",
            ..
        })
    ));
    let no_version = modify(&good, |e| e[1].2 = br#"{"format":"frap"}"#.to_vec());
    assert!(matches!(
        read(&no_version, lenient()),
        Err(ProjectError::Manifest(ManifestError::MissingSchemaVersion))
    ));
}

#[test]
fn unknown_content_needs_lenient() {
    let (scene, buffers, images) = sample();
    let good = write(&scene, &buffers, &images).unwrap();

    // An entry that is not part of the format.
    let extra = modify(&good, |e| {
        e.push((
            "notes.txt".into(),
            CompressionMethod::Stored,
            b"hi".to_vec(),
        ))
    });
    let err = read(&extra, strict()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "unexpected entry `notes.txt` in .frap file (open with --lenient to ignore it)"
    );
    assert_eq!(
        read(&extra, lenient()).unwrap().scene,
        read(&good, strict()).unwrap().scene
    );

    // A well-formed buffer entry the manifest does not list.
    let stray = ContentHash::of(b"stray");
    let unlisted = modify(&good, |e| {
        e.push((
            buffer_path(&stray),
            CompressionMethod::Deflated,
            b"stray".to_vec(),
        ))
    });
    assert!(
        matches!(read(&unlisted, strict()), Err(ProjectError::UnexpectedEntry(p)) if p == buffer_path(&stray))
    );
    assert!(!read(&unlisted, lenient()).unwrap().buffers.contains(&stray));

    // An unknown manifest field.
    let field = modify(&good, |e| {
        let mut m: Value = serde_json::from_slice(&e[1].2).unwrap();
        m["future"] = json!(true);
        e[1].2 = serde_json::to_vec(&m).unwrap();
    });
    assert!(matches!(
        read(&field, strict()),
        Err(ProjectError::Manifest(ManifestError::UnknownField { .. }))
    ));
    assert!(read(&field, lenient()).is_ok());
}

#[test]
fn save_file_is_atomic_and_readable() {
    let dir = std::env::temp_dir().join(format!("frap-project-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.frap");
    let (scene, buffers, images) = sample();
    let bytes = write(&scene, &buffers, &images).unwrap();
    save_file(&path, b"old").unwrap();
    save_file(&path, &bytes).unwrap();
    assert!(!temp_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(read_file(&path, strict()).unwrap().buffers, buffers);
    assert_eq!(temp_path(&path), dir.join("x.frap@"));
    // A failed save leaves no temporary file behind.
    assert!(save_file(&dir.join("missing-dir").join("y.frap"), &bytes).is_err());
    assert!(matches!(
        read_file(&dir.join("nope.frap"), strict()),
        Err(ProjectError::Io(_))
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_projects_are_deterministic_and_round_trip(
        arrays in proptest::collection::vec(proptest::collection::vec(any::<i32>(), 0..200), 0..6),
        floats in proptest::collection::vec(proptest::collection::vec(any::<[u32; 3]>(), 0..120), 0..4),
        imgs in proptest::collection::vec((proptest::collection::vec(any::<u8>(), 0..300), "[a-z]{1,4}"), 0..3),
    ) {
        let build = || {
            let mut buffers = BufferStore::new();
            let mut images = ImageStore::new();
            let ints: Vec<String> =
                arrays.iter().map(|a| serde_json::to_string(&buffers.encode(a)).unwrap()).collect();
            let floats: Vec<Vec<[f32; 3]>> =
                floats.iter().map(|v| v.iter().map(|a| a.map(f32::from_bits)).collect()).collect();
            let fl: Vec<String> =
                floats.iter().map(|a| serde_json::to_string(&buffers.encode(a)).unwrap()).collect();
            let im: Vec<ImageRef> = imgs.iter().map(|(b, e)| images.insert(b.clone(), e).unwrap()).collect();
            let scene = format!(
                "{{\"ints\":[{}],\"floats\":[{}],\"images\":{}}}\n",
                ints.join(","),
                fl.join(","),
                serde_json::to_string(&im).unwrap()
            );
            (scene.into_bytes(), buffers, images, floats)
        };
        let (scene, buffers, images, floats) = build();
        let a = write(&scene, &buffers, &images).unwrap();
        let (scene2, buffers2, images2, _) = build();
        prop_assert_eq!(&write(&scene2, &buffers2, &images2).unwrap(), &a);

        let loaded = read(&a, strict()).unwrap();
        prop_assert_eq!(&loaded.buffers, &buffers);
        prop_assert_eq!(&loaded.images, &images);
        for (i, arr) in arrays.iter().enumerate() {
            prop_assert_eq!(&loaded.buffers.decode::<i32>(&loaded.scene["ints"][i]).unwrap(), arr);
        }
        for (i, arr) in floats.iter().enumerate() {
            let back: Vec<[f32; 3]> = loaded.buffers.decode(&loaded.scene["floats"][i]).unwrap();
            prop_assert_eq!(
                back.iter().map(|a| a.map(f32::to_bits)).collect::<Vec<_>>(),
                arr.iter().map(|a| a.map(f32::to_bits)).collect::<Vec<_>>()
            );
        }
    }
}
