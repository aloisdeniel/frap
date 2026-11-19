// SPDX-License-Identifier: GPL-3.0-or-later
//! `probe` reads what `decode` gives (size, channels, sample type) from the header alone, for
//! every format and channel count frap writes, and for the Blender-written fixtures.

use frap_imbuf::{DecodedImage, EncodeFormat, ImageInfo, PixelData, decode, encode, probe};

fn image(channels: u8, bits: u8) -> DecodedImage {
    let (w, h) = (5u32, 3u32);
    let n = (w * h) as usize * usize::from(channels);
    let pixels = match bits {
        8 => PixelData::U8((0..n).map(|i| (i * 13 % 256) as u8).collect()),
        16 => PixelData::U16((0..n).map(|i| (i * 4099 % 65536) as u16).collect()),
        _ => PixelData::F32((0..n).map(|i| i as f32 * 0.1).collect()),
    };
    DecodedImage::new(w, h, channels, pixels).unwrap()
}

fn same_as_decode(bytes: &[u8], what: &str) {
    let info = probe(bytes).unwrap_or_else(|e| panic!("{what}: {e}"));
    let d = decode(bytes).unwrap();
    assert_eq!(
        info,
        ImageInfo {
            format: info.format,
            width: d.width,
            height: d.height,
            channels: d.channels,
            bit_depth: d.bit_depth(),
        },
        "{what}"
    );
    assert_eq!(Some(info.format), frap_imbuf::FileFormat::detect(bytes));
}

#[test]
fn probe_matches_decode() {
    let formats = [
        (
            EncodeFormat::Png {
                bit_depth: 8,
                compression: 15,
            },
            8,
        ),
        (
            EncodeFormat::Png {
                bit_depth: 16,
                compression: 15,
            },
            16,
        ),
        (EncodeFormat::OpenExr { bit_depth: 32 }, 32),
        (EncodeFormat::OpenExr { bit_depth: 16 }, 32),
        (EncodeFormat::Hdr, 32),
        (EncodeFormat::Targa, 8),
        (EncodeFormat::Bmp, 8),
        (EncodeFormat::Jpeg { quality: 90 }, 8),
        (
            EncodeFormat::WebP {
                lossless: true,
                quality: 100,
            },
            8,
        ),
    ];
    for (f, bits) in formats {
        for ch in 1..=4u8 {
            let Ok(bytes) = encode(&image(ch, bits), &f) else {
                continue;
            };
            same_as_decode(&bytes, &format!("{f:?} {ch} channels"));
        }
    }
}

#[test]
fn probe_matches_decode_on_blender_files() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/blender");
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        if decode(&bytes).is_err() {
            continue;
        }
        same_as_decode(&bytes, &p.display().to_string());
        n += 1;
    }
    assert!(n > 0);
}

#[test]
fn probe_rejects_garbage() {
    assert!(probe(b"not an image").is_err());
    // A PNG signature without a header.
    assert!(probe(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).is_err());
}
