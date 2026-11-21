// SPDX-License-Identifier: GPL-3.0-or-later
//! KTX2 container read (WP 7.3, decision log M7W0-7): `KHR_texture_basisu` images and `.ktx2`
//! files decode to the base mip level of their first layer and face, through the `ktx2`
//! parser and, for Basis Universal payloads, the transcoder of [`crate::basis`] (feature
//! `basisu`). [`crate::decode`] and [`crate::probe`] dispatch here on the KTX2 identifier.
//! Blender 5.2 cannot load KTX2 at all (its glTF importer refuses `KHR_texture_basisu` and
//! `bpy.data.images.load` has no KTX2 reader), so this is a frap extension (deviation D-130).
//!
//! What decodes:
//!
//! | `vkFormat` | Payload | [`decode`] gives |
//! |---|---|---|
//! | `VK_FORMAT_UNDEFINED` (0) | Basis Universal: ETC1S (BasisLZ), UASTC LDR (none or Zstandard) | RGB or RGBA 8-bit (RGBA when the texture has alpha) |
//! | `VK_FORMAT_UNDEFINED` (0) | Basis Universal UASTC HDR (4x4, 6x6) | RGB or RGBA float (from half floats) |
//! | `R8`, `R8G8`, `R8G8B8`, `R8G8B8A8`, `B8G8R8A8` (`_UNORM`, `_SRGB`) | raw, Zstandard or zlib | grey, RGB (`G8` with B = 0), RGB, RGBA 8-bit |
//! | `R16…` `_UNORM` (1 to 4 channels) | raw, Zstandard or zlib | 16-bit |
//! | `R16…` `_SFLOAT`, `R32…` `_SFLOAT` (1 to 4 channels) | raw, Zstandard or zlib | float |
//!
//! Samples are returned as stored (no colour-space conversion; the DFD transfer function is
//! the caller's business, as a PNG's gamma is). Rows are top row first: a `KTXorientation`
//! of `?u` (bottom-up) is flipped. A 2-channel `R8G8` texture is not grey + alpha, so it
//! decodes as RGB with a zero blue channel. 3D textures are refused; arrays and cube maps
//! decode their first layer and face.

use crate::codec::{DecodedImage, ImageError};
use crate::formats::{FileFormat, ImageInfo};

/// The 12-byte KTX2 file identifier (`«KTX 20»\r\n\x1A\n`).
pub const IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

/// Whether `bytes` start with the KTX2 identifier.
pub fn is_ktx2(bytes: &[u8]) -> bool {
    bytes.starts_with(&IDENTIFIER)
}

/// MIME type of a KTX2 image (`KHR_texture_basisu`).
pub const MIME: &str = "image/ktx2";

/// Whether this build decodes KTX2 (feature `basisu`).
pub const CAN_DECODE: bool = cfg!(feature = "basisu");

/// Decodes the base mip level of a KTX2 file (see the module documentation).
pub fn decode(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
    imp::decode(bytes)
}

/// Reads a KTX2 header: what [`decode`] would give, without decoding the pixels (a
/// Basis Universal file is parsed by the transcoder, its payload is not transcoded).
pub fn probe(bytes: &[u8]) -> Result<ImageInfo, ImageError> {
    let (width, height, channels, bit_depth) = imp::probe(bytes)?;
    Ok(ImageInfo {
        format: FileFormat::Ktx2,
        width,
        height,
        channels,
        bit_depth,
    })
}

#[cfg(feature = "basisu")]
fn bad(msg: impl std::fmt::Display) -> ImageError {
    ImageError::Decode(format!("KTX2: {msg}"))
}

#[cfg(not(feature = "basisu"))]
mod imp {
    use super::*;

    fn unsupported() -> ImageError {
        ImageError::Unsupported("KTX2 images need frap built with the `basisu` feature".into())
    }

    pub(super) fn decode(_bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        Err(unsupported())
    }

    pub(super) fn probe(_bytes: &[u8]) -> Result<(u32, u32, u8, u8), ImageError> {
        Err(unsupported())
    }
}

#[cfg(feature = "basisu")]
mod imp {
    use std::borrow::Cow;

    use super::*;
    use crate::codec::PixelData;

    /// KTX2 supercompression schemes (`supercompressionScheme`).
    const SCHEME_NONE: u32 = 0;
    const SCHEME_BASISLZ: u32 = 1;
    const SCHEME_ZSTD: u32 = 2;
    const SCHEME_ZLIB: u32 = 3;

    /// Sample type of a raw (non-Basis) format.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Sample {
        U8,
        U16,
        F16,
        F32,
    }

    impl Sample {
        fn bytes(self) -> usize {
            match self {
                Sample::U8 => 1,
                Sample::U16 | Sample::F16 => 2,
                Sample::F32 => 4,
            }
        }
        fn bit_depth(self) -> u8 {
            match self {
                Sample::U8 => 8,
                Sample::U16 => 16,
                Sample::F16 | Sample::F32 => 32,
            }
        }
    }

    /// A raw format: stored channels, sample type, BGRA order.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Raw {
        pub channels: u8,
        pub sample: Sample,
        pub bgra: bool,
    }

    impl Raw {
        /// The channels [`decode`] gives (a 2-channel RG texture becomes RGB).
        fn out_channels(self) -> u8 {
            if self.channels == 2 { 3 } else { self.channels }
        }
    }

    /// The raw `VkFormat`s frap decodes.
    pub(super) fn raw_format(vk: u32) -> Option<Raw> {
        let r = |channels, sample| Raw {
            channels,
            sample,
            bgra: false,
        };
        Some(match vk {
            9 | 15 => r(1, Sample::U8),  // R8_UNORM, R8_SRGB
            16 | 22 => r(2, Sample::U8), // R8G8_UNORM, R8G8_SRGB
            23 | 29 => r(3, Sample::U8), // R8G8B8_UNORM, R8G8B8_SRGB
            37 | 43 => r(4, Sample::U8), // R8G8B8A8_UNORM, R8G8B8A8_SRGB
            44 | 50 => Raw {
                bgra: true,
                ..r(4, Sample::U8)
            }, // B8G8R8A8_UNORM, B8G8R8A8_SRGB
            70 => r(1, Sample::U16),     // R16_UNORM
            77 => r(2, Sample::U16),     // R16G16_UNORM
            84 => r(3, Sample::U16),     // R16G16B16_UNORM
            91 => r(4, Sample::U16),     // R16G16B16A16_UNORM
            76 => r(1, Sample::F16),     // R16_SFLOAT
            83 => r(2, Sample::F16),     // R16G16_SFLOAT
            90 => r(3, Sample::F16),     // R16G16B16_SFLOAT
            97 => r(4, Sample::F16),     // R16G16B16A16_SFLOAT
            100 => r(1, Sample::F32),    // R32_SFLOAT
            103 => r(2, Sample::F32),    // R32G32_SFLOAT
            106 => r(3, Sample::F32),    // R32G32B32_SFLOAT
            109 => r(4, Sample::F32),    // R32G32B32A32_SFLOAT
            _ => return None,
        })
    }

    /// A parsed file and what it holds.
    pub(super) struct Parsed<'a> {
        pub reader: ktx2::Reader<&'a [u8]>,
        pub vk: u32,
        pub scheme: u32,
        pub width: u32,
        pub height: u32,
    }

    pub(super) fn parse(bytes: &[u8]) -> Result<Parsed<'_>, ImageError> {
        let reader = ktx2::Reader::new(bytes).map_err(|e| bad(format!("{e:?}")))?;
        let h = reader.header();
        if h.pixel_depth > 1 {
            return Err(ImageError::Unsupported("KTX2: 3D textures".into()));
        }
        Ok(Parsed {
            vk: h.format.map_or(0, |f| f.value()),
            scheme: h.supercompression_scheme.map_or(SCHEME_NONE, |s| s.value()),
            width: h.pixel_width,
            height: h.pixel_height.max(1),
            reader,
        })
    }

    impl Parsed<'_> {
        /// `KTXorientation`'s second letter is `u`: rows are stored bottom-up.
        fn bottom_up(&self) -> bool {
            self.reader.key_value_data().any(|(k, v)| {
                k == "KTXorientation" && v.get(1).is_some_and(|c| c.eq_ignore_ascii_case(&b'u'))
            })
        }

        /// The base level's bytes, decompressed (`expected`: the bytes of its first image;
        /// the whole level is decompressed when the index records its length).
        fn level0(&self, expected: usize) -> Result<Cow<'_, [u8]>, ImageError> {
            let level = self
                .reader
                .levels()
                .next()
                .ok_or_else(|| bad("no levels"))?;
            let full = usize::try_from(level.uncompressed_byte_length)
                .map_err(|_| bad("level too large"))?;
            let data = match self.scheme {
                SCHEME_NONE => Cow::Borrowed(level.data),
                SCHEME_ZSTD => Cow::Owned(unzstd(level.data, full.max(expected))?),
                SCHEME_ZLIB => Cow::Owned(unzlib(level.data, full.max(expected))?),
                s => {
                    return Err(ImageError::Unsupported(format!(
                        "KTX2: supercompression scheme {s} on a raw format"
                    )));
                }
            };
            if data.len() < expected {
                return Err(bad(format!(
                    "level 0 holds {} bytes, the image needs {expected}",
                    data.len()
                )));
            }
            Ok(data)
        }
    }

    fn reserve(bytes: Option<u64>) -> Result<(), ImageError> {
        frap_math::budget::reserve_buffer("KTX2 image", bytes)
            .map_err(|e| ImageError::Decode(format!("KTX2: {e}")))
    }

    /// Zstandard (`supercompressionScheme` 2): at most `max` bytes (a frame that inflates
    /// beyond the level's recorded length is an error, not an allocation).
    pub(super) fn unzstd(data: &[u8], max: usize) -> Result<Vec<u8>, ImageError> {
        reserve(Some(max as u64))?;
        let mut out = Vec::with_capacity(max);
        ruzstd::decoding::FrameDecoder::new()
            .decode_all_to_vec(data, &mut out)
            .map_err(|e| bad(format!("Zstandard: {e}")))?;
        Ok(out)
    }

    /// zlib (`supercompressionScheme` 3), at most `max` bytes.
    fn unzlib(data: &[u8], max: usize) -> Result<Vec<u8>, ImageError> {
        use std::io::Read;
        reserve(Some(max as u64))?;
        let mut out = Vec::with_capacity(max);
        flate2::read::ZlibDecoder::new(data)
            .take(max as u64)
            .read_to_end(&mut out)
            .map_err(|e| bad(format!("zlib: {e}")))?;
        Ok(out)
    }

    pub(super) fn probe(bytes: &[u8]) -> Result<(u32, u32, u8, u8), ImageError> {
        let p = parse(bytes)?;
        if p.vk == 0 {
            let data = basis_container(&p)?;
            let (w, h, channels, bits) = crate::basis::info(&data)?;
            return Ok((w, h, channels, bits));
        }
        let raw = raw_format(p.vk).ok_or_else(|| unsupported_format(p.vk))?;
        Ok((
            p.width,
            p.height,
            raw.out_channels(),
            raw.sample.bit_depth(),
        ))
    }

    fn unsupported_format(vk: u32) -> ImageError {
        ImageError::Unsupported(format!("KTX2: vkFormat {vk}"))
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        let p = parse(bytes)?;
        let mut img = if p.vk == 0 {
            let data = basis_container(&p)?;
            crate::basis::transcode(&data)?
        } else {
            decode_raw(&p)?
        };
        if p.bottom_up() {
            flip_rows(&mut img);
        }
        Ok(img)
    }

    fn decode_raw(p: &Parsed) -> Result<DecodedImage, ImageError> {
        let raw = raw_format(p.vk).ok_or_else(|| unsupported_format(p.vk))?;
        let px = u64::from(p.width) * u64::from(p.height);
        let stored = px
            .checked_mul(u64::from(raw.channels) * raw.sample.bytes() as u64)
            .ok_or_else(|| bad("image too large"))?;
        reserve(Some(stored.saturating_mul(2)))?;
        let data = p.level0(stored as usize)?;
        let data = &data[..stored as usize];
        let out_n = raw.out_channels() as usize;
        // RG → RGB with a zero blue channel.
        fn expand<T: Copy>(v: Vec<T>, zero: T, n: u8) -> Vec<T> {
            if n != 2 {
                return v;
            }
            let mut o = Vec::with_capacity(v.len() / 2 * 3);
            for c in v.chunks_exact(2) {
                o.extend_from_slice(c);
                o.push(zero);
            }
            o
        }
        let n = raw.channels;
        let pixels = match raw.sample {
            Sample::U8 => {
                let mut v = data.to_vec();
                if raw.bgra {
                    v.chunks_exact_mut(4).for_each(|c| c.swap(0, 2));
                }
                PixelData::U8(expand(v, 0, n))
            }
            Sample::U16 => {
                let v: Vec<u16> = data
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                PixelData::U16(expand(v, 0, n))
            }
            Sample::F16 => {
                let v: Vec<f32> = data
                    .chunks_exact(2)
                    .map(|c| half::f16::from_le_bytes([c[0], c[1]]).to_f32())
                    .collect();
                PixelData::F32(expand(v, 0.0, n))
            }
            Sample::F32 => {
                let v: Vec<f32> = data
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                PixelData::F32(expand(v, 0.0, n))
            }
        };
        DecodedImage::new(p.width, p.height, out_n as u8, pixels)
    }

    /// The file as the Basis transcoder reads it: as is, or for a Zstandard-supercompressed
    /// UASTC file a copy with its base level decompressed (`basisu` is used without its own
    /// `zstd` feature, decision log M7W0-5).
    fn basis_container<'a>(p: &'a Parsed<'a>) -> Result<Cow<'a, [u8]>, ImageError> {
        match p.scheme {
            SCHEME_NONE | SCHEME_BASISLZ => Ok(Cow::Borrowed(p.reader.data())),
            SCHEME_ZSTD => {
                let level = p.reader.levels().next().ok_or_else(|| bad("no levels"))?;
                let full = usize::try_from(level.uncompressed_byte_length)
                    .map_err(|_| bad("level too large"))?;
                let data = unzstd(level.data, full)?;
                Ok(Cow::Owned(rebuild_one_level(p, &data)))
            }
            s => Err(ImageError::Unsupported(format!(
                "KTX2: supercompression scheme {s} on a Basis Universal texture"
            ))),
        }
    }

    /// A KTX2 file with `p`'s header, DFD and key/value data and `level0` as its only,
    /// uncompressed level (`supercompressionScheme` 0, no global data).
    pub(super) fn rebuild_one_level(p: &Parsed, level0: &[u8]) -> Vec<u8> {
        let src = p.reader.data();
        let h = p.reader.header();
        let ix = h.index;
        let dfd =
            &src[ix.dfd_byte_offset as usize..(ix.dfd_byte_offset + ix.dfd_byte_length) as usize];
        let kvd =
            &src[ix.kvd_byte_offset as usize..(ix.kvd_byte_offset + ix.kvd_byte_length) as usize];
        let align = |n: usize, a: usize| n.div_ceil(a) * a;
        let dfd_off = ktx2::Header::LENGTH + ktx2::LevelIndex::LENGTH;
        let kvd_off = align(dfd_off + dfd.len(), 4);
        // Level data of an uncompressed block-compressed format: aligned to lcm(16, 4).
        let level_off = align(kvd_off + kvd.len(), 16);
        let header = ktx2::Header {
            level_count: 1,
            supercompression_scheme: None,
            index: ktx2::Index {
                dfd_byte_offset: dfd_off as u32,
                dfd_byte_length: dfd.len() as u32,
                kvd_byte_offset: if kvd.is_empty() { 0 } else { kvd_off as u32 },
                kvd_byte_length: kvd.len() as u32,
                sgd_byte_offset: 0,
                sgd_byte_length: 0,
            },
            ..h
        };
        let mut out = Vec::with_capacity(level_off + level0.len());
        out.extend_from_slice(&header.as_bytes());
        let li = ktx2::LevelIndex {
            byte_offset: level_off as u64,
            byte_length: level0.len() as u64,
            uncompressed_byte_length: level0.len() as u64,
        };
        out.extend_from_slice(&li.as_bytes());
        out.extend_from_slice(dfd);
        out.resize(kvd_off, 0);
        out.extend_from_slice(kvd);
        out.resize(level_off, 0);
        out.extend_from_slice(level0);
        out
    }

    fn flip_rows(img: &mut DecodedImage) {
        let row = img.width as usize * usize::from(img.channels);
        fn flip<T: Clone>(v: &mut [T], row: usize) {
            let rows = v.len() / row.max(1);
            for y in 0..rows / 2 {
                let (a, b) = v.split_at_mut((rows - 1 - y) * row);
                a[y * row..(y + 1) * row].swap_with_slice(&mut b[..row]);
            }
        }
        match &mut img.pixels {
            PixelData::U8(v) => flip(v, row),
            PixelData::U16(v) => flip(v, row),
            PixelData::F32(v) => flip(v, row),
        }
    }
}

/// A KTX2 file of a raw (non-Basis) format: one level, `scheme` 0 (none), 2 (Zstandard) or
/// 3 (zlib), an optional `KTXorientation`. The writer of the tests and of
/// `tests/golden/fixtures/gltf/textures/` (`ktx2_fixtures`).
#[cfg(all(test, feature = "basisu"))]
pub(crate) fn write_raw(
    vk: u32,
    width: u32,
    height: u32,
    data: &[u8],
    scheme: u32,
    orientation: Option<&str>,
) -> Vec<u8> {
    let format = ktx2::Format::new(vk).expect("a VkFormat");
    let dfd = ktx2::dfd::Basic::from_format(format).map_or_else(
        // Formats the generator does not know (block formats): an empty basic block.
        |_| ktx2::dfd::Block::Unknown {
            header: ktx2::dfd::BlockHeader::BASIC,
            data: vec![0; 16],
        },
        |(b, _)| ktx2::dfd::Block::Basic(b),
    );
    let block = dfd;
    let mut dfd_bytes = Vec::new();
    let block_bytes = block.to_vec();
    dfd_bytes.extend_from_slice(&((block_bytes.len() + 4) as u32).to_le_bytes());
    dfd_bytes.extend_from_slice(&block_bytes);
    let mut kvd = Vec::new();
    if let Some(o) = orientation {
        let kv = format!("KTXorientation\0{o}\0");
        kvd.extend_from_slice(&(kv.len() as u32).to_le_bytes());
        kvd.extend_from_slice(kv.as_bytes());
        while kvd.len() % 4 != 0 {
            kvd.push(0);
        }
    }
    let stored = match scheme {
        0 => data.to_vec(),
        2 => ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest),
        3 => {
            use std::io::Write;
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).expect("in memory");
            e.finish().expect("in memory")
        }
        s => panic!("scheme {s}"),
    };
    let type_size = match vk {
        70..=97 => 2,
        100..=109 => 4,
        _ => 1,
    };
    let dfd_off = ktx2::Header::LENGTH + ktx2::LevelIndex::LENGTH;
    let kvd_off = dfd_off + dfd_bytes.len();
    let level_off = (kvd_off + kvd.len()).div_ceil(16) * 16;
    let header = ktx2::Header {
        format: Some(format),
        type_size,
        pixel_width: width,
        pixel_height: height,
        pixel_depth: 0,
        layer_count: 0,
        face_count: 1,
        level_count: 1,
        supercompression_scheme: ktx2::SupercompressionScheme::new(scheme),
        index: ktx2::Index {
            dfd_byte_offset: dfd_off as u32,
            dfd_byte_length: dfd_bytes.len() as u32,
            kvd_byte_offset: if kvd.is_empty() { 0 } else { kvd_off as u32 },
            kvd_byte_length: kvd.len() as u32,
            sgd_byte_offset: 0,
            sgd_byte_length: 0,
        },
    };
    let mut out = header.as_bytes().to_vec();
    out.extend_from_slice(
        &ktx2::LevelIndex {
            byte_offset: level_off as u64,
            byte_length: stored.len() as u64,
            uncompressed_byte_length: data.len() as u64,
        }
        .as_bytes(),
    );
    out.extend_from_slice(&dfd_bytes);
    out.extend_from_slice(&kvd);
    out.resize(level_off, 0);
    out.extend_from_slice(&stored);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier() {
        assert!(is_ktx2(&IDENTIFIER));
        assert!(!is_ktx2(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(
            crate::FileFormat::detect(&IDENTIFIER),
            Some(crate::FileFormat::Ktx2)
        );
    }

    #[cfg(not(feature = "basisu"))]
    #[test]
    fn without_the_feature_ktx2_is_unsupported() {
        let e = decode(&IDENTIFIER).unwrap_err();
        assert!(e.to_string().contains("`basisu` feature"), "{e}");
        assert!(probe(&IDENTIFIER).is_err());
    }

    #[cfg(feature = "basisu")]
    mod with_basisu {
        use super::super::*;
        use crate::codec::PixelData;

        /// 3x2 RGBA8 pixels, top row first.
        const RGBA: [u8; 24] = [
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, //
            10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120,
        ];

        #[test]
        fn raw_rgba8_in_every_supercompression() {
            for scheme in [0, 2, 3] {
                // R8G8B8A8_SRGB
                let f = write_raw(43, 3, 2, &RGBA, scheme, None);
                let d = decode(&f).unwrap_or_else(|e| panic!("scheme {scheme}: {e}"));
                assert_eq!((d.width, d.height, d.channels), (3, 2, 4));
                assert_eq!(d.pixels, PixelData::U8(RGBA.to_vec()), "scheme {scheme}");
                let i = probe(&f).unwrap();
                assert_eq!(
                    (i.format, i.width, i.height, i.channels, i.bit_depth),
                    (FileFormat::Ktx2, 3, 2, 4, 8)
                );
                // Through the sniffing entry points.
                assert_eq!(crate::decode(&f).unwrap(), d);
                assert_eq!(crate::probe(&f).unwrap(), i);
            }
        }

        #[test]
        fn bottom_up_orientation_is_flipped_and_bgra_swizzled() {
            let f = write_raw(37, 3, 2, &RGBA, 0, Some("ru"));
            let d = decode(&f).unwrap();
            let mut want = RGBA[12..].to_vec();
            want.extend_from_slice(&RGBA[..12]);
            assert_eq!(d.pixels, PixelData::U8(want));
            // `rd` is the default: unchanged.
            let f = write_raw(37, 3, 2, &RGBA, 0, Some("rd"));
            assert_eq!(decode(&f).unwrap().pixels, PixelData::U8(RGBA.to_vec()));
            // B8G8R8A8: blue and red swapped back.
            let f = write_raw(44, 3, 2, &RGBA, 0, None);
            let PixelData::U8(v) = decode(&f).unwrap().pixels else {
                panic!("8-bit expected")
            };
            assert_eq!(&v[..4], &[0, 0, 255, 255]);
        }

        #[test]
        fn grey_rg_16_bit_and_float_formats() {
            let f = write_raw(9, 2, 1, &[7, 9], 0, None);
            let d = decode(&f).unwrap();
            assert_eq!((d.channels, d.pixels), (1, PixelData::U8(vec![7, 9])));
            // R8G8 is not grey + alpha: RGB with blue 0.
            let f = write_raw(16, 2, 1, &[1, 2, 3, 4], 0, None);
            let d = decode(&f).unwrap();
            assert_eq!(
                (d.channels, d.pixels),
                (3, PixelData::U8(vec![1, 2, 0, 3, 4, 0]))
            );
            assert_eq!(probe(&f).unwrap().channels, 3);
            // R16G16B16A16_UNORM.
            let px: Vec<u8> = [1u16, 2, 3, 65535]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let d = decode(&write_raw(91, 1, 1, &px, 2, None)).unwrap();
            assert_eq!(d.pixels, PixelData::U16(vec![1, 2, 3, 65535]));
            // R16G16B16A16_SFLOAT and R32G32B32A32_SFLOAT.
            let h: Vec<u8> = [0.5f32, 1.0, 2.0, 1.0]
                .iter()
                .flat_map(|v| half::f16::from_f32(*v).to_le_bytes())
                .collect();
            let d = decode(&write_raw(97, 1, 1, &h, 0, None)).unwrap();
            assert_eq!(d.pixels, PixelData::F32(vec![0.5, 1.0, 2.0, 1.0]));
            let f: Vec<u8> = [0.25f32, 4.0, -1.0, 1.0]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let d = decode(&write_raw(109, 1, 1, &f, 3, None)).unwrap();
            assert_eq!(d.pixels, PixelData::F32(vec![0.25, 4.0, -1.0, 1.0]));
            assert_eq!(
                probe(&write_raw(109, 1, 1, &f, 3, None)).unwrap().bit_depth,
                32
            );
        }

        /// Mean absolute difference per 8-bit sample.
        fn mean_diff(a: &DecodedImage, b: &DecodedImage) -> f64 {
            let (a, b) = (a.to_u8(), b.to_u8());
            assert_eq!(a.len(), b.len());
            a.iter()
                .zip(&b)
                .map(|(x, y)| f64::from(x.abs_diff(*y)))
                .sum::<f64>()
                / a.len() as f64
        }

        /// Basis Universal files written by the reference encoder (`basisu_c_sys`, through
        /// `export_scene.gltf(frap_texture_format='KTX2')`, crates/frap-ops-io-tests
        /// `write_ktx2_inputs`) decode to their source pixels within the codec's error: a
        /// UASTC + Zstandard RGB image (the Zstandard path rebuilds the container) and an
        /// ETC1S (BasisLZ) RGBA one.
        #[test]
        fn reference_encoder_files_decode() {
            const FX: &str = "../../tests/golden/fixtures/gltf/textures/ktx2";
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FX);
            let read = |n: &str| std::fs::read(dir.join(n)).unwrap();
            for (ktx, png, channels, tol) in [
                ("grid_uastc.ktx2", "grid.png", 3, 2.0),
                ("grid_alpha_etc1s.ktx2", "grid_alpha.png", 4, 10.0),
            ] {
                let k = read(ktx);
                let d = decode(&k).unwrap();
                let want = crate::decode(&read(png))
                    .unwrap()
                    .with_channels(channels)
                    .unwrap();
                assert_eq!((d.width, d.height, d.channels), (32, 32, channels), "{ktx}");
                let m = mean_diff(&d, &want);
                assert!(m <= tol, "{ktx}: mean difference {m}");
                let i = probe(&k).unwrap();
                assert_eq!(
                    (i.width, i.height, i.channels, i.bit_depth),
                    (32, 32, channels, 8)
                );
            }
        }

        #[test]
        fn zstd_container_is_rebuilt_with_one_uncompressed_level() {
            const FX: &str = "../../tests/golden/fixtures/gltf/textures/ktx2/grid_uastc.ktx2";
            let k =
                std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FX)).unwrap();
            let p = imp::parse(&k).unwrap();
            assert_eq!(p.scheme, 2, "UASTC with Zstandard");
            let level = p.reader.levels().next().unwrap();
            let raw = imp::unzstd(level.data, level.uncompressed_byte_length as usize).unwrap();
            let r = imp::rebuild_one_level(&p, &raw);
            let rp = ktx2::Reader::new(&r[..]).unwrap();
            assert_eq!(rp.header().level_count, 1);
            assert_eq!(rp.header().supercompression_scheme, None);
            assert_eq!(rp.levels().next().unwrap().data, &raw[..]);
            assert_eq!(rp.dfd_blocks(), p.reader.dfd_blocks());
        }

        #[test]
        fn corrupt_and_unsupported_files_are_errors() {
            assert!(matches!(decode(&IDENTIFIER), Err(ImageError::Decode(_))));
            let mut f = write_raw(43, 3, 2, &RGBA, 0, None);
            f.truncate(f.len() - 4);
            assert!(decode(&f).is_err());
            // A Zstandard level that inflates beyond its recorded length (and the image's
            // size) is refused, not allocated.
            let mut z = write_raw(43, 3, 2, &[0; 100_000], 2, None);
            let li = ktx2::Header::LENGTH + 16;
            z[li..li + 8].copy_from_slice(&24u64.to_le_bytes());
            assert!(matches!(decode(&z), Err(ImageError::Decode(m)) if m.contains("Zstandard")));
            // BC1 (131): a block format frap does not decode.
            let bc1 = write_raw(131, 4, 4, &[0; 8], 0, None);
            assert!(matches!(decode(&bc1), Err(ImageError::Unsupported(_))));
        }
    }
}
