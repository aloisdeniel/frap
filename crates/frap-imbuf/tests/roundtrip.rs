// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.6 acceptance: PNG 8/16, JPEG, WebP lossless, HDR and EXR (plus TGA and BMP)
//! decode/encode round-trips.

use frap_imbuf::{
    DecodedImage, EncodeFormat, FileFormat, ImageCodec, ImageError, Imbuf, PixelData, decode,
    encode,
};
use proptest::prelude::*;

const W: u32 = 13;
const H: u32 = 7;

fn ramp_u8(channels: u8) -> DecodedImage {
    let n = (W * H) as usize * channels as usize;
    let data = (0..n).map(|i| ((i * 37 + i / 5) % 256) as u8).collect();
    DecodedImage::new(W, H, channels, PixelData::U8(data)).unwrap()
}

fn ramp_u16(channels: u8) -> DecodedImage {
    let n = (W * H) as usize * channels as usize;
    let data = (0..n).map(|i| ((i * 4099 + 17) % 65536) as u16).collect();
    DecodedImage::new(W, H, channels, PixelData::U16(data)).unwrap()
}

fn ramp_f32(channels: u8) -> DecodedImage {
    let n = (W * H) as usize * channels as usize;
    // HDR values over several decades, including 0.
    let data = (0..n)
        .map(|i| {
            if i % 11 == 0 {
                0.0
            } else {
                (i as f32 * 0.37).sin().abs() * 10f32.powi((i % 5) as i32 - 2)
            }
        })
        .collect();
    DecodedImage::new(W, H, channels, PixelData::F32(data)).unwrap()
}

fn png(bit_depth: u8, compression: u8) -> EncodeFormat {
    EncodeFormat::Png {
        bit_depth,
        compression,
    }
}

fn round_trip(img: &DecodedImage, f: &EncodeFormat) -> DecodedImage {
    let bytes = encode(img, f).unwrap_or_else(|e| panic!("{f:?}: {e}"));
    assert_eq!(FileFormat::detect(&bytes), Some(f.file_format()), "{f:?}");
    // Deterministic bytes.
    assert_eq!(encode(img, f).unwrap(), bytes, "{f:?} not deterministic");
    let back = decode(&bytes).unwrap_or_else(|e| panic!("{f:?}: {e}"));
    assert_eq!((back.width, back.height), (img.width, img.height), "{f:?}");
    back
}

#[test]
fn png_8_bit_is_lossless_for_all_channel_counts_and_compressions() {
    for ch in 1..=4 {
        let img = ramp_u8(ch);
        for c in [0, 15, 50, 100] {
            assert_eq!(
                round_trip(&img, &png(8, c)),
                img,
                "channels {ch} compression {c}"
            );
        }
    }
}

#[test]
fn png_16_bit_is_lossless_for_all_channel_counts() {
    for ch in 1..=4 {
        let img = ramp_u16(ch);
        assert_eq!(round_trip(&img, &png(16, 15)), img, "channels {ch}");
    }
}

#[test]
fn png_converts_sample_types_with_blender_rounding() {
    let f = DecodedImage::new(4, 1, 1, PixelData::F32(vec![-0.5, 0.5, 0.25, 7.0])).unwrap();
    let b8 = round_trip(&f, &png(8, 15));
    assert_eq!(b8.pixels, PixelData::U8(vec![0, 128, 64, 255]));
    let b16 = round_trip(&f, &png(16, 15));
    assert_eq!(b16.pixels, PixelData::U16(vec![0, 32768, 16384, 65535]));
    // 8 → 16 widens exactly; 16 → 8 uses unit_ushort_to_uchar.
    let b = round_trip(&ramp_u8(3), &png(16, 15));
    assert_eq!(b.to_u8(), ramp_u8(3).to_u8());
    assert!(matches!(
        encode(&ramp_u8(3), &png(12, 15)),
        Err(ImageError::Unsupported(_))
    ));
}

#[test]
fn png_compression_maps_like_blender() {
    assert_eq!(frap_imbuf::formats::png_zlib_level(0), 0);
    assert_eq!(frap_imbuf::formats::png_zlib_level(15), 1);
    assert_eq!(frap_imbuf::formats::png_zlib_level(90), 8);
    assert_eq!(frap_imbuf::formats::png_zlib_level(100), 9);
    let img = ramp_u8(4);
    let stored = encode(&img, &png(8, 0)).unwrap();
    let best = encode(&img, &png(8, 100)).unwrap();
    assert!(stored.len() > best.len());
}

fn mean_abs_diff(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(x.abs_diff(*y)))
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn jpeg_round_trips_within_quantisation() {
    // A smooth image: JPEG is lossy, so compare with a tolerance.
    let n = (W * H) as usize;
    let smooth: Vec<u8> = (0..n * 3)
        .map(|i| {
            let p = i / 3;
            let (x, y) = (p as u32 % W, p as u32 / W);
            (40 + x * 8 + y * 10 + (i % 3) as u32 * 20) as u8
        })
        .collect();
    let rgb = DecodedImage::new(W, H, 3, PixelData::U8(smooth)).unwrap();
    let back = round_trip(&rgb, &EncodeFormat::Jpeg { quality: 95 });
    assert_eq!(back.channels, 3);
    let err = mean_abs_diff(&back.to_u8(), &rgb.to_u8());
    assert!(err < 3.0, "mean error {err}");
    // Lower quality: larger error, smaller file.
    let q30 = encode(&rgb, &EncodeFormat::Jpeg { quality: 30 }).unwrap();
    let q95 = encode(&rgb, &EncodeFormat::Jpeg { quality: 95 }).unwrap();
    assert!(q30.len() < q95.len());
    // Grey stays grey; alpha is dropped (JPEG offers BW/RGB only).
    let grey = rgb.with_channels(1).unwrap();
    let back = round_trip(&grey, &EncodeFormat::Jpeg { quality: 95 });
    assert_eq!(back.channels, 1);
    assert!(mean_abs_diff(&back.to_u8(), &grey.to_u8()) < 3.0);
    let rgba = rgb.with_channels(4).unwrap();
    assert_eq!(
        round_trip(&rgba, &EncodeFormat::Jpeg { quality: 95 }).channels,
        3
    );
}

#[test]
fn webp_lossless_is_exact() {
    let lossless = EncodeFormat::WebP {
        lossless: true,
        quality: 100,
    };
    for ch in [3, 4] {
        let img = ramp_u8(ch);
        assert_eq!(round_trip(&img, &lossless), img, "channels {ch}");
    }
    // Grey input: the decoder returns RGB(A) with identical samples.
    for ch in [1, 2] {
        let img = ramp_u8(ch);
        let back = round_trip(&img, &lossless);
        let want = img.with_channels(if ch == 1 { 3 } else { 4 }).unwrap();
        assert_eq!(
            back.with_channels(want.channels).unwrap(),
            want,
            "channels {ch}"
        );
    }
}

#[cfg(feature = "webp-lossy")]
#[test]
fn webp_lossy_round_trips_approximately() {
    let lossy = EncodeFormat::WebP {
        lossless: false,
        quality: 90,
    };
    for ch in [1, 3, 4] {
        let img = ramp_u8(ch);
        let bytes = encode(&img, &lossy).unwrap();
        assert_eq!(FileFormat::detect(&bytes), Some(FileFormat::WebP));
        let back = decode(&bytes).unwrap();
        assert_eq!((back.width, back.height), (W, H));
        assert!(back.channels >= 3);
    }
}

#[cfg(not(feature = "webp-lossy"))]
#[test]
fn webp_lossy_needs_the_feature() {
    let r = encode(
        &ramp_u8(3),
        &EncodeFormat::WebP {
            lossless: false,
            quality: 90,
        },
    );
    assert!(matches!(r, Err(ImageError::Unsupported(m)) if m.contains("webp-lossy")));
}

#[test]
fn exr_float_is_exact_and_half_rounds_to_f16() {
    for ch in 1..=4 {
        let img = ramp_f32(ch);
        let back = round_trip(&img, &EncodeFormat::OpenExr { bit_depth: 32 });
        assert_eq!(back, img, "float, channels {ch}");
        let half = round_trip(&img, &EncodeFormat::OpenExr { bit_depth: 16 });
        let want: Vec<f32> = img
            .to_f32()
            .iter()
            .map(|v| half::f16::from_f32(*v).to_f32())
            .collect();
        assert_eq!(half.channels, ch);
        assert_eq!(half.pixels, PixelData::F32(want), "half, channels {ch}");
    }
    // 8-bit input is normalised to [0, 1].
    let b = round_trip(&ramp_u8(3), &EncodeFormat::OpenExr { bit_depth: 32 });
    assert_eq!(b.to_u8(), ramp_u8(3).to_u8());
    assert!(encode(&ramp_f32(3), &EncodeFormat::OpenExr { bit_depth: 8 }).is_err());
}

#[test]
fn hdr_round_trips_within_rgbe_precision() {
    let img = ramp_f32(3);
    let back = round_trip(&img, &EncodeFormat::Hdr);
    assert_eq!(back.channels, 3);
    let (src, got) = (img.to_f32(), back.to_f32());
    for (a, b) in src.chunks_exact(3).zip(got.chunks_exact(3)) {
        // RGBE: an 8-bit mantissa per channel sharing the exponent of the largest component.
        let max = a.iter().fold(0.0f32, |m, v| m.max(*v));
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() <= max / 128.0 + 1e-9, "{a:?} {b:?}");
        }
    }
    // RGBA: alpha dropped.
    assert_eq!(round_trip(&ramp_f32(4), &EncodeFormat::Hdr).channels, 3);
}

#[test]
fn targa_and_bmp_are_lossless() {
    for ch in [1, 3, 4] {
        let img = ramp_u8(ch);
        assert_eq!(
            round_trip(&img, &EncodeFormat::Targa),
            img,
            "TGA channels {ch}"
        );
    }
    for ch in [3, 4] {
        let img = ramp_u8(ch);
        assert_eq!(
            round_trip(&img, &EncodeFormat::Bmp),
            img,
            "BMP channels {ch}"
        );
    }
}

#[test]
fn codec_trait_and_errors() {
    let codec = Imbuf;
    let img = ramp_u8(4);
    let bytes = codec.encode(&img, &png(8, 15)).unwrap();
    assert_eq!(codec.decode(&bytes).unwrap(), img);
    assert!(matches!(
        decode(b"not an image at all"),
        Err(ImageError::Unsupported(_))
    ));
    assert!(matches!(
        decode(&bytes[..bytes.len() / 2]),
        Err(ImageError::Decode(_))
    ));
    let bad = DecodedImage {
        width: 2,
        height: 2,
        channels: 3,
        pixels: PixelData::U8(vec![0; 5]),
    };
    assert!(matches!(
        encode(&bad, &png(8, 15)),
        Err(ImageError::Encode(_))
    ));
}

#[test]
fn file_format_identifiers_and_extensions() {
    for f in FileFormat::ALL {
        assert_eq!(FileFormat::from_identifier(f.identifier()), Some(f));
        assert!(f.extension().starts_with('.'));
    }
    assert_eq!(FileFormat::Jpeg.extension(), ".jpg");
    assert_eq!(FileFormat::OpenExr.identifier(), "OPEN_EXR");
    assert_eq!(FileFormat::from_identifier("FFMPEG"), None);
}

#[test]
fn load_and_save_files() {
    let dir = std::env::temp_dir().join(format!("frap-imbuf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.png");
    let img = ramp_u16(4);
    frap_imbuf::save(&path, &img, &png(16, 15)).unwrap();
    assert_eq!(frap_imbuf::load(&path).unwrap(), img);
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(matches!(
        frap_imbuf::load(&dir.join("missing.png")),
        Err(ImageError::Io(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn png_round_trip_any_image(
        w in 1u32..9, h in 1u32..9, ch in 1u8..=4, sixteen in any::<bool>(), seed in any::<u64>()
    ) {
        let n = (w * h) as usize * ch as usize;
        let mut s = seed;
        let mut next = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (s >> 33) as u32 };
        let img = if sixteen {
            DecodedImage::new(w, h, ch, PixelData::U16((0..n).map(|_| next() as u16).collect())).unwrap()
        } else {
            DecodedImage::new(w, h, ch, PixelData::U8((0..n).map(|_| next() as u8).collect())).unwrap()
        };
        let f = png(if sixteen { 16 } else { 8 }, 15);
        prop_assert_eq!(decode(&encode(&img, &f).unwrap()).unwrap(), img);
    }
}
