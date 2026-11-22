// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: `frap_imbuf` format detection, `probe` and decoding (PNG, JPEG, WebP, HDR, EXR,
//! BMP, TGA, KTX2 with Basis transcoding) of arbitrary bytes: never panics, bounded memory
//! (ARCHITECTURE §20.1, nightly from M8; WP 8.7).
//!
//! The first byte picks a format whose magic is put in front of the rest (or none: the bytes
//! as they are), so every decoder sees mutated headers rather than only the sniffer. When both
//! `probe` and `decode` succeed they agree on the size, the channels and the sample type, and
//! the pixel buffer holds exactly `width × height × channels` samples.
#![no_main]

use frap_imbuf::{FileFormat, PixelData};
use libfuzzer_sys::fuzz_target;

/// Magic prefixes, by selector byte (index 0: none).
const MAGIC: [&[u8]; 9] = [
    b"",
    b"\x89PNG\r\n\x1a\n",
    &[0xFF, 0xD8, 0xFF],
    b"RIFF\0\0\0\0WEBP",
    &[0x76, 0x2F, 0x31, 0x01],
    b"#?RADIANCE\n",
    b"BM",
    &[
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ],
    b"",
];

fn exercise(bytes: &[u8]) {
    let _ = FileFormat::detect(bytes);
    let info = frap_imbuf::probe(bytes);
    let decoded = frap_imbuf::decode(bytes);
    if let (Ok(info), Ok(img)) = (&info, &decoded) {
        assert_eq!(
            (info.width, info.height, info.channels),
            (img.width, img.height, img.channels),
            "probe and decode disagree on {:?}",
            info.format
        );
        let (bits, samples) = match &img.pixels {
            PixelData::U8(p) => (8, p.len()),
            PixelData::U16(p) => (16, p.len()),
            PixelData::F32(p) => (32, p.len()),
            _ => return,
        };
        assert_eq!(
            info.bit_depth, bits,
            "probe and decode disagree on the sample type"
        );
        assert_eq!(
            samples as u64,
            u64::from(img.width) * u64::from(img.height) * u64::from(img.channels),
            "pixel buffer size"
        );
    }
    if FileFormat::detect(bytes) == Some(FileFormat::OpenExr)
        && let Ok((w, h, channels)) = frap_imbuf::decode_exr_channels(bytes)
    {
        let n = u64::from(w) * u64::from(h);
        assert!(channels.iter().all(|(_, s)| s.len() as u64 == n));
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((sel, rest)) = data.split_first() else {
        return;
    };
    let magic = MAGIC[usize::from(*sel) % MAGIC.len()];
    if magic.is_empty() {
        exercise(rest);
    } else {
        let mut v = Vec::with_capacity(magic.len() + rest.len());
        v.extend_from_slice(magic);
        v.extend_from_slice(rest);
        exercise(&v);
    }
});
