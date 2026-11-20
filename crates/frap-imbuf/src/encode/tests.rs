// SPDX-License-Identifier: GPL-3.0-or-later
//! Byte-exact goldens of the ported writers against Blender 5.2.4's render outputs
//! (`tests/golden/fixtures/render/output/film/`, written by its `generate.py`).
//!
//! Every byte format Blender writes from the same display buffer: the 8-bit PNGs hold exactly
//! the samples Blender passed to its Targa, BMP and JPEG writers (same film, view transform and
//! dither), so encoding the PNG's samples must give Blender's file. The Radiance HDR files come
//! from the film itself (`film_transparent.exr`, Blender's premultiplied render buffer).

use super::*;
use crate::codec::PixelData;

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/golden/fixtures/render/output/film"
);

/// Blender's render `ppm` (72 dpi: `ppm_factor / ppm_base`).
const PPM: [f64; 2] = [72.0 / 0.0254, 72.0 / 0.0254];

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn load(name: &str) -> DecodedImage {
    crate::decode(&read(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn assert_same_bytes(name: &str, got: &[u8]) {
    let want = read(name);
    if got != want.as_slice() {
        let first = got.iter().zip(&want).position(|(a, b)| a != b);
        panic!(
            "{name}: {} bytes written, Blender's file has {}; first difference at {first:?}",
            got.len(),
            want.len()
        );
    }
}

#[test]
fn targa_matches_blender_byte_for_byte() {
    let raw = WriteOptions {
        targa_rle: false,
        ..WriteOptions::default()
    };
    for (tga, png, opts) in [
        ("tga_rgba.tga", "png8_rgba.png", WriteOptions::default()),
        ("tga_rgb.tga", "png8_rgb.png", WriteOptions::default()),
        ("tga_bw.tga", "png8_bw.png", WriteOptions::default()),
        ("tgaraw_rgba.tga", "png8_rgba.png", raw.clone()),
        ("tgaraw_bw.tga", "png8_bw.png", raw.clone()),
    ] {
        let got = encode_with(&load(png), &EncodeFormat::Targa, &opts).unwrap();
        assert_same_bytes(tga, &got);
        // And it reads back.
        assert_eq!(crate::decode(&got).unwrap(), load(png), "{tga}");
    }
}

#[test]
fn bmp_matches_blender_byte_for_byte() {
    let opts = WriteOptions {
        ppm: Some(PPM),
        ..WriteOptions::default()
    };
    for (bmp, png) in [
        ("bmp_rgb.bmp", "png8_rgb.png"),
        ("bmp_bw.bmp", "png8_bw.png"),
        ("bmp_rgba.bmp", "png8_rgba.png"),
    ] {
        let img = load(png);
        let got = encode_with(&img, &EncodeFormat::Bmp, &opts).unwrap();
        assert_same_bytes(bmp, &got);
        let back = crate::decode(&got).unwrap();
        // Grey BMPs read back through their palette as RGB.
        assert_eq!(back.with_channels(img.channels).unwrap(), img, "{bmp}");
    }
}

#[test]
fn bmp_32_bit_alpha_reads_back() {
    let img = load("png8_rgba.png");
    let info = crate::probe(&read("bmp_rgba.bmp")).unwrap();
    assert_eq!((info.width, info.height, info.channels), (64, 48, 4));
    assert_eq!(crate::decode(&read("bmp_rgba.bmp")).unwrap(), img);
}

/// The film's RGB (premultiplied), and its Rec.709 luma replicated (`IMB_color_to_bw`).
fn film_rgb(bw: bool) -> DecodedImage {
    let film = load("film_transparent.exr");
    assert_eq!(film.channels, 4);
    let PixelData::F32(v) = &film.pixels else {
        panic!("float film")
    };
    let mut rgb = Vec::with_capacity(v.len() / 4 * 3);
    for p in v.chunks_exact(4) {
        if bw {
            let l = 0.2126f32 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
            rgb.extend_from_slice(&[l, l, l]);
        } else {
            rgb.extend_from_slice(&p[..3]);
        }
    }
    DecodedImage::new(film.width, film.height, 3, PixelData::F32(rgb)).unwrap()
}

#[test]
fn hdr_matches_blender_byte_for_byte() {
    for (name, bw) in [("hdr_rgb.hdr", false), ("hdr_bw.hdr", true)] {
        let got = encode_with(&film_rgb(bw), &EncodeFormat::Hdr, &WriteOptions::default()).unwrap();
        assert_same_bytes(name, &got);
    }
}

/// Blender's JPEG without its stamp `COM` markers (frap writes no stamp metadata).
fn strip_com(jpeg: &[u8]) -> Vec<u8> {
    let mut out = jpeg[..2].to_vec();
    let mut i = 2;
    while i + 4 <= jpeg.len() && jpeg[i] == 0xFF {
        let code = jpeg[i + 1];
        let len = usize::from(u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]));
        if code != 0xFE {
            out.extend_from_slice(&jpeg[i..i + 2 + len]);
        }
        i += 2 + len;
        if code == 0xDA {
            break;
        }
    }
    out.extend_from_slice(&jpeg[i..]);
    out
}

#[test]
fn jpeg_matches_blender_byte_for_byte() {
    for (jpg, png, quality) in [
        ("jpeg_rgb.jpg", "png8_rgb.png", 90),
        ("jpeg_bw.jpg", "png8_bw.png", 90),
        // Blender maps qualities below 10 to 90 before the writer.
        ("jpeg_rgb_q5.jpg", "png8_rgb.png", 90),
        ("jpeg_rgb_q100.jpg", "png8_rgb.png", 100),
    ] {
        let got = encode_with(
            &load(png),
            &EncodeFormat::Jpeg { quality },
            &WriteOptions::default(),
        )
        .unwrap();
        let want = strip_com(&read(jpg));
        if got != want {
            let first = got.iter().zip(&want).position(|(a, b)| a != b);
            panic!(
                "{jpg}: {} bytes written, Blender's has {} (without COM); first difference at \
                 {first:?}",
                got.len(),
                want.len()
            );
        }
    }
}

#[test]
fn jpeg_odd_sizes_decode() {
    // Edge replication and dummy blocks: every size from 1x1 to 17x17 decodes to the right size,
    // close to the input.
    for w in [1u32, 7, 8, 9, 15, 16, 17, 33] {
        for h in [1u32, 7, 8, 9, 15, 16, 17] {
            for ch in [1u8, 3] {
                let n = (w * h) as usize * usize::from(ch);
                let data: Vec<u8> = (0..n).map(|i| (i * 7 % 200 + 20) as u8).collect();
                let img = DecodedImage::new(w, h, ch, PixelData::U8(data)).unwrap();
                let bytes = encode_with(
                    &img,
                    &EncodeFormat::Jpeg { quality: 100 },
                    &WriteOptions::default(),
                )
                .unwrap();
                let back = crate::decode(&bytes).unwrap();
                assert_eq!((back.width, back.height, back.channels), (w, h, ch));
            }
        }
    }
}

#[test]
fn exr_codecs_and_grey_name() {
    let film = film_rgb(true).with_channels(1).unwrap();
    for codec in [
        ExrCodec::None,
        ExrCodec::Zip,
        ExrCodec::Zips,
        ExrCodec::Piz,
        ExrCodec::Rle,
        ExrCodec::Pxr24,
        ExrCodec::B44,
        ExrCodec::B44a,
        ExrCodec::Dwaa(90),
        ExrCodec::Dwab(90),
    ] {
        let o = WriteOptions {
            exr_codec: codec,
            exr_grey_channel: "V",
            ..WriteOptions::default()
        };
        let bytes = encode_with(&film, &EncodeFormat::OpenExr { bit_depth: 16 }, &o).unwrap();
        let back = crate::decode(&bytes).unwrap();
        assert_eq!(back.channels, 1, "{codec:?}");
        let meta =
            ::exr::meta::MetaData::read_from_buffered(std::io::Cursor::new(&bytes), false).unwrap();
        let names: Vec<String> = meta.headers[0]
            .channels
            .list
            .iter()
            .map(|c| c.name.to_string())
            .collect();
        assert_eq!(names, ["V"], "{codec:?}");
    }
    assert_eq!(ExrCodec::from_identifier("HTJ2K", 90), None);
    assert_eq!(
        ExrCodec::from_identifier("DWAB", 80),
        Some(ExrCodec::Dwab(80))
    );
}

#[test]
fn png_chunks_for_render_output() {
    let o = WriteOptions {
        srgb: true,
        ppm: Some(PPM),
        ..WriteOptions::default()
    };
    let img = load("png8_rgba.png");
    let bytes = encode_with(
        &img,
        &EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        },
        &o,
    )
    .unwrap();
    let chunk = |name: &[u8]| bytes.windows(4).position(|w| w == name);
    let srgb = chunk(b"sRGB").expect("sRGB");
    assert_eq!(bytes[srgb + 4], 3, "absolute colorimetric");
    let gama = chunk(b"gAMA").expect("gAMA");
    assert_eq!(&bytes[gama + 4..gama + 8], &45455u32.to_be_bytes());
    assert!(chunk(b"cHRM").is_some());
    let phys = chunk(b"pHYs").expect("pHYs");
    assert_eq!(&bytes[phys + 4..phys + 8], &2834u32.to_be_bytes());
    // The cHRM bytes equal Blender's file's.
    let blender = read("png8_rgba.png");
    let bc = blender.windows(4).position(|w| w == b"cHRM").unwrap();
    let fc = chunk(b"cHRM").unwrap();
    assert_eq!(&bytes[fc..fc + 36], &blender[bc..bc + 36]);
    assert_eq!(crate::decode(&bytes).unwrap(), img);
}

#[test]
fn jpeg_of_an_odd_size_matches_blender_byte_for_byte() {
    // 75 x 43: partial MCUs at the right and bottom (edge replication, dummy blocks).
    let img = load("noise.png");
    assert_eq!((img.width, img.height, img.channels), (75, 43, 3));
    for (jpg, quality) in [("noise_q95.jpg", 95), ("noise_q30.jpg", 30)] {
        let got = encode_with(
            &img,
            &EncodeFormat::Jpeg { quality },
            &WriteOptions::default(),
        )
        .unwrap();
        assert_eq!(got, strip_com(&read(jpg)), "{jpg}");
    }
}
