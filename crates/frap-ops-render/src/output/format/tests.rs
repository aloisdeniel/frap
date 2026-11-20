// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of `output::format`, and the CPU goldens of WP 5.4: Blender 5.2.4's render
//! outputs of one film in every format (`tests/golden/fixtures/render/output/film/`, written by
//! its `generate.py`, listed in `manifest.json`). The film is Blender's own premultiplied render
//! buffer, so these compare frap's output path alone, without a renderer or a GPU: bytes for
//! Targa, BMP, Radiance HDR and JPEG (without Blender's stamp `COM` markers), samples for PNG,
//! WebP and OpenEXR.

use super::*;
use frap_scene::generated::plain::ColorManagedViewSettings;
use serde_json::Value;

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/golden/fixtures/render/output/film"
);

fn settings(fmt: &str, mode: &str, depth: &str) -> ImageFormatSettings {
    ImageFormatSettings {
        file_format: fmt.into(),
        color_mode: mode.into(),
        color_depth: depth.into(),
        ..ImageFormatSettings::default()
    }
}

#[test]
fn factory_png_rgba8() {
    let ws = write_settings(&ImageFormatSettings::default()).unwrap();
    assert_eq!(
        ws.encode,
        EncodeFormat::Png {
            bit_depth: 8,
            compression: 15
        }
    );
    assert_eq!(
        (ws.channels, ws.linear, ws.bw, ws.write_fails),
        (4, false, false, false)
    );
    assert!(ws.options.srgb);
    assert_eq!(ws.options.ppm, None);
    let ws = render_write_settings(&RenderSettings::default()).unwrap();
    let ppm = 72.0 / f64::from(0.0254f32);
    assert_eq!(
        ws.options.ppm,
        Some([f64::from(72.0f32) / f64::from(0.0254f32); 2])
    );
    assert!((ws.options.ppm.unwrap()[0] - ppm).abs() < 1e-9);
}

#[test]
fn formats() {
    let w = |f, m, d| write_settings(&settings(f, m, d)).unwrap();
    assert_eq!(
        w("PNG", "RGB", "16").encode,
        EncodeFormat::Png {
            bit_depth: 16,
            compression: 15
        }
    );
    assert_eq!(w("PNG", "BW", "8").channels, 1);
    assert_eq!(w("PNG", "RGB", "8").channels, 3);
    assert_eq!(
        w("JPEG", "RGB", "8").encode,
        EncodeFormat::Jpeg { quality: 90 }
    );
    assert_eq!(w("JPEG", "RGBA", "8").channels, 3);
    let mut low = settings("JPEG", "RGB", "8");
    low.quality = 9;
    assert_eq!(
        write_settings(&low).unwrap().encode,
        EncodeFormat::Jpeg { quality: 90 }
    );
    low.quality = 10;
    assert_eq!(
        write_settings(&low).unwrap().encode,
        EncodeFormat::Jpeg { quality: 10 }
    );
    let exr = w("OPEN_EXR", "RGBA", "16");
    assert_eq!(exr.encode, EncodeFormat::OpenExr { bit_depth: 16 });
    assert!(exr.linear && !exr.options.srgb);
    assert_eq!(exr.options.exr_grey_channel, "V");
    assert_eq!(
        w("OPEN_EXR", "RGBA", "32").encode,
        EncodeFormat::OpenExr { bit_depth: 32 }
    );
    let mut dwa = settings("OPEN_EXR", "RGB", "16");
    dwa.exr_codec = "DWAB".into();
    dwa.quality = 70;
    assert_eq!(
        write_settings(&dwa).unwrap().options.exr_codec,
        ExrCodec::Dwab(70)
    );
    dwa.exr_codec = "HTJ2K".into();
    assert_eq!(
        write_settings(&dwa).unwrap_err(),
        "frap: render output image_settings.exr_codec='HTJ2K' is not supported yet"
    );
    let mut aces = settings("OPEN_EXR", "RGB", "32");
    aces.color_management = "OVERRIDE".into();
    aces.linear_colorspace_settings.name = "ACEScg".into();
    assert!(write_settings(&aces).is_err());
    aces.linear_colorspace_settings.name = "Linear Rec.709".into();
    assert!(write_settings(&aces).is_ok());
    assert!(w("HDR", "RGB", "32").linear);
    assert_eq!(w("HDR", "BW", "32").channels, 3);
    assert!(w("HDR", "BW", "32").bw);
    assert_eq!(w("TARGA", "RGBA", "8").encode, EncodeFormat::Targa);
    assert!(w("TARGA", "RGBA", "8").options.targa_rle);
    assert!(!w("TARGA_RAW", "RGBA", "8").options.targa_rle);
    assert_eq!(w("BMP", "RGB", "8").encode, EncodeFormat::Bmp);
    assert_eq!(w("BMP", "RGBA", "8").channels, 4);
    let mut s = settings("WEBP", "RGBA", "8");
    s.quality = 100;
    assert_eq!(
        write_settings(&s).unwrap().encode,
        EncodeFormat::WebP {
            lossless: true,
            quality: 100
        }
    );
    s.color_mode = "BW".into();
    assert!(write_settings(&s).unwrap().write_fails);
    assert_eq!(
        write_settings(&settings("TIFF", "RGBA", "8")).unwrap_err(),
        "frap: render output image_settings.file_format='TIFF' is not supported yet"
    );
    let video = ImageFormatSettings {
        media_type: "VIDEO".into(),
        ..ImageFormatSettings::default()
    };
    assert!(write_settings(&video).is_err());
}

#[test]
fn ppm_follows_pixel_aspect() {
    let mut r = RenderSettings {
        pixel_aspect_x: 2.0,
        ..RenderSettings::default()
    };
    let [x, y] = render_ppm(&r).unwrap();
    assert!((x * 2.0 - y).abs() < 1e-9);
    r.ppm_base = 0.0;
    assert_eq!(render_ppm(&r), None);
}

fn standard() -> ColorManagement {
    let view = ColorManagedViewSettings {
        view_transform: "Standard".into(),
        ..ColorManagedViewSettings::default()
    };
    color_management(&view, &ColorManagedDisplaySettings::default())
}

#[test]
fn standard_png_pixels() {
    let cm = standard();
    assert!(cm.warnings.is_empty());
    let ws = write_settings(&ImageFormatSettings::default()).unwrap();
    // Linear 0.25 → sRGB 0.5371 → 137.
    let img = file_pixels(&[[0.25, 0.0, 1.0, 0.5]], 1, 1, &ws, &cm, 0.0).unwrap();
    assert_eq!(img.pixels, PixelData::U8(vec![137, 0, 255, 128]));
    let bw = write_settings(&settings("PNG", "BW", "8")).unwrap();
    let img = file_pixels(&[[1.0, 1.0, 1.0, 1.0]], 1, 1, &bw, &cm, 0.0).unwrap();
    assert_eq!((img.channels, img.pixels), (1, PixelData::U8(vec![255])));
    // BW is the luma of the *linear* colour: linear 0.2126 → sRGB 0.4985 → 127.
    let img = file_pixels(&[[1.0, 0.0, 0.0, 1.0]], 1, 1, &bw, &cm, 0.0).unwrap();
    assert_eq!(img.pixels, PixelData::U8(vec![127]));
    let exr = write_settings(&settings("OPEN_EXR", "RGB", "32")).unwrap();
    // Float RGB files hold the premultiplied render buffer.
    let img = file_pixels(&[[2.0, 0.5, 0.25, 0.5]], 1, 1, &exr, &cm, 0.0).unwrap();
    assert_eq!(
        (img.channels, img.pixels),
        (3, PixelData::F32(vec![1.0, 0.25, 0.125]))
    );
    let rgb8 = write_settings(&settings("PNG", "RGB", "8")).unwrap();
    let img = file_pixels(&[[0.25, 0.0, 1.0, 0.5]], 1, 1, &rgb8, &cm, 0.0).unwrap();
    // Over black: linear 0.125 → sRGB 0.3896 → 99; 0.5 → 0.7354 → 188.
    assert_eq!(img.pixels, PixelData::U8(vec![99, 0, 188]));
    let exr = write_settings(&settings("OPEN_EXR", "RGBA", "32")).unwrap();
    let img = file_pixels(&[[2.0, 0.5, 0.25, 0.5]], 1, 1, &exr, &cm, 0.0).unwrap();
    assert_eq!(img.pixels, PixelData::F32(vec![1.0, 0.25, 0.125, 0.5]));
    let exr_bw = write_settings(&settings("OPEN_EXR", "BW", "32")).unwrap();
    let img = file_pixels(&[[1.0, 1.0, 1.0, 0.5]], 1, 1, &exr_bw, &cm, 0.0).unwrap();
    assert_eq!((img.channels, img.pixels), (1, PixelData::F32(vec![0.5])));
}

#[test]
fn factory_agx_has_no_warning() {
    let cm = color_management(
        &ColorManagedViewSettings::default(),
        &ColorManagedDisplaySettings::default(),
    );
    assert!(cm.warnings.is_empty(), "{:?}", cm.warnings);
}

#[test]
fn fallbacks_warn() {
    let view = ColorManagedViewSettings {
        view_transform: "Nope".into(),
        use_curve_mapping: true,
        ..ColorManagedViewSettings::default()
    };
    let cm = color_management(&view, &ColorManagedDisplaySettings::default());
    assert_eq!(cm.warnings.len(), 2, "{:?}", cm.warnings);
    assert!(cm.warnings[0].contains("'Nope'"));
}

#[test]
fn encodes_png() {
    let ws = write_settings(&ImageFormatSettings::default()).unwrap();
    let bytes = encode(&[[0.5, 0.5, 0.5, 1.0]; 4], 2, 2, &ws, &standard(), 1.0).unwrap();
    let back = frap_imbuf::decode(&bytes).unwrap();
    assert_eq!((back.width, back.height, back.channels), (2, 2, 4));
}

#[test]
fn bw_webp_fails_like_blender() {
    let mut s = settings("WEBP", "BW", "8");
    s.quality = 100;
    let ws = write_settings(&s).unwrap();
    let dir = std::env::temp_dir().join(format!("frap-wp54-{}", std::process::id()));
    let path = dir.join("bw.webp");
    let err = write_render(&path, &[[0.5; 4]], 1, 1, &ws, &standard(), 1.0).unwrap_err();
    assert!(err.starts_with("Render error ("), "{err}");
    assert!(
        err.ends_with(&format!(") cannot save: '{}'", path.display())),
        "{err}"
    );
    assert!(!path.exists());
    #[cfg(target_os = "macos")]
    assert_eq!(
        err,
        format!(
            "Render error (Undefined error: 0) cannot save: '{}'",
            path.display()
        )
    );
}

// --- Blender goldens -------------------------------------------------------------------------

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn manifest() -> Value {
    serde_json::from_slice(&read("manifest.json")).unwrap()
}

/// Blender's premultiplied film.
fn film(name: &str) -> (Vec<[f32; 4]>, u32, u32) {
    let img = frap_imbuf::decode(&read(name)).unwrap();
    assert_eq!(img.channels, 4, "{name}");
    let PixelData::F32(v) = img.pixels else {
        panic!("{name}: float film")
    };
    let px = v
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect();
    (px, img.width, img.height)
}

fn entry_settings(e: &Value) -> ImageFormatSettings {
    let s = &e["image_settings"];
    let text = |k: &str| s[k].as_str().unwrap().to_owned();
    ImageFormatSettings {
        file_format: text("file_format").into(),
        color_mode: text("color_mode").into(),
        color_depth: text("color_depth").into(),
        exr_codec: text("exr_codec").into(),
        quality: s["quality"].as_i64().unwrap() as i32,
        compression: s["compression"].as_i64().unwrap() as i32,
        ..ImageFormatSettings::default()
    }
}

/// Blender's JPEG without its stamp `COM` markers.
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

/// How two images of one size and channel count differ: the largest difference in units of the
/// stored type (relative to `max(1, |Blender's sample|)` for floats), and how many samples differ
/// at all. Pixels whose alpha is 0 in `want` are skipped when `skip_transparent`.
fn sample_diff(
    name: &str,
    got: &DecodedImage,
    want: &DecodedImage,
    skip_transparent: bool,
) -> (f64, usize) {
    assert_eq!(
        (got.width, got.height, got.channels),
        (want.width, want.height, want.channels),
        "{name}: size or channels"
    );
    let (a, b) = (got.to_f32(), want.to_f32());
    let scale = match (&got.pixels, &want.pixels) {
        (PixelData::U8(_), PixelData::U8(_)) => Some(255.0),
        (PixelData::U16(_), PixelData::U16(_)) => Some(65535.0),
        (PixelData::F32(_), PixelData::F32(_)) => None,
        _ => panic!("{name}: sample types differ"),
    };
    let c = usize::from(want.channels);
    let (mut worst, mut count) = (0.0f64, 0);
    for (pa, pb) in a.chunks_exact(c).zip(b.chunks_exact(c)) {
        if skip_transparent && c == 4 && pb[3] == 0.0 {
            continue;
        }
        for (x, y) in pa.iter().zip(pb) {
            let (x, y) = (f64::from(*x), f64::from(*y));
            let d = match scale {
                Some(s) => ((x - y) * s).abs().round(),
                None => (x - y).abs() / y.abs().max(1.0),
            };
            if d > 0.0 {
                count += 1;
            }
            worst = worst.max(d);
        }
    }
    (worst, count)
}

#[test]
fn render_outputs_match_blender() {
    let m = manifest();
    let mut checked = 0;
    let mut fails = Vec::new();
    for e in m["outputs"].as_array().unwrap() {
        let name = e["name"].as_str().unwrap();
        let s = entry_settings(e);
        let view = ColorManagedViewSettings {
            view_transform: e["view_transform"].as_str().unwrap().into(),
            ..ColorManagedViewSettings::default()
        };
        let cm = color_management(&view, &ColorManagedDisplaySettings::default());
        assert!(cm.warnings.is_empty(), "{name}: {:?}", cm.warnings);
        let dither = e["dither_intensity"].as_f64().unwrap() as f32;
        let mut ws = write_settings(&s).unwrap_or_else(|err| panic!("{name}: {err}"));
        ws.options.ppm = render_ppm(&RenderSettings::default());
        if let Some(err) = e["error"].as_str() {
            assert!(ws.write_fails, "{name}: Blender failed to write: {err}");
            let ours = format!("Error: {}", cannot_save(Path::new(&format!("{name}.webp"))));
            if cfg!(target_os = "macos") {
                assert_eq!(ours, err, "{name}");
            }
            checked += 1;
            continue;
        }
        let file = e["file"].as_str().unwrap();
        let (premul, w, h) = film(e["film"].as_str().unwrap());
        let img = file_pixels_premultiplied(&premul, w, h, &ws, &cm, dither)
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        let bytes = match frap_imbuf::encode::encode_with(&img, &ws.encode, &ws.options) {
            Ok(b) => b,
            Err(err) if err.to_string().contains("webp-lossy") => {
                eprintln!("{name}: skipped ({err})");
                continue;
            }
            Err(err) => panic!("{name}: {err}"),
        };
        let blender = read(file);
        let agx = e["view_transform"] == "AgX";
        match s.file_format.as_str() {
            "HDR" => {
                if bytes != blender {
                    fails.push(format!("{name}: bytes differ from Blender's {file}"));
                }
            }
            "JPEG" => {
                if bytes != strip_com(&blender) {
                    fails.push(format!("{name}: bytes differ from Blender's {file}"));
                }
            }
            fmt => {
                // Targa and BMP: same layout as Blender's (their writers are byte-exact on
                // Blender's samples, `frap-imbuf` `encode/tests.rs`).
                if matches!(fmt, "TARGA" | "TARGA_RAW" | "BMP") {
                    let header = if fmt == "BMP" { 54 } else { 18 };
                    if bytes[..header] != blender[..header]
                        || bytes.len() != blender.len() && fmt != "TARGA"
                    {
                        fails.push(format!(
                            "{name}: header or size differs from Blender's {file}"
                        ));
                    }
                }
                let got = frap_imbuf::decode(&bytes).unwrap();
                let want = frap_imbuf::decode(&blender).unwrap();
                // libwebp rewrites the colour of fully transparent pixels.
                let skip_transparent = fmt == "WEBP";
                let lossy_exr = fmt == "OPEN_EXR"
                    && matches!(s.exr_codec.as_str(), "DWAA" | "DWAB" | "B44" | "B44A");
                if lossy_exr {
                    // Lossy codecs: the `exr` crate's encoders, not OpenEXR's. Both files must be
                    // about as close to the film: frap's error at most twice Blender's.
                    let (ours, _) = sample_diff(name, &got, &img, false);
                    let (theirs, _) = sample_diff(name, &want, &img, false);
                    if ours > (2.0 * theirs).max(1e-3) {
                        fails.push(format!(
                            "{name}: error {ours} against the film, Blender's {theirs}"
                        ));
                    }
                    eprintln!("{name}: lossy error {ours} (Blender's {theirs})");
                    checked += 1;
                    continue;
                }
                let (diff, count) = sample_diff(name, &got, &want, skip_transparent);
                let samples = want.pixels.len();
                // (largest difference, share of samples that may differ)
                let (limit, share) = if fmt == "WEBP" && s.quality < 100 {
                    // Lossy WebP: another libwebp version than Blender's.
                    (24.0, 1.0)
                } else if fmt == "OPEN_EXR" || e["view_transform"] == "Raw" {
                    // Lossless floats, and the Raw view (no transform): exact.
                    (0.0, 0.0)
                } else if agx {
                    // frap's AgX is a LUT within ΔE 1 of Blender's (§19.5, WP 5.3).
                    (if got.bit_depth() == 16 { 1024.0 } else { 4.0 }, 1.0)
                } else {
                    // Standard: frap's analytic sRGB curve (`frap-color`) and OCIO's CPU
                    // processor differ in the last bits, which moves a few dithered or 16-bit
                    // samples across a rounding boundary (0.01 % of 8-bit, 1.6 % of 16-bit
                    // samples here); the Raw view, without that curve, is exact.
                    (1.0, 0.03)
                };
                if diff > limit || count as f64 > share * samples as f64 {
                    fails.push(format!(
                        "{name}: {count} of {samples} samples differ from Blender's {file}, by up \
                         to {diff} (limits {limit}, {share})"
                    ));
                } else if count > 0 {
                    eprintln!("{name}: {count} of {samples} samples differ by up to {diff}");
                }
            }
        }
        checked += 1;
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
    assert!(checked >= 35, "{checked} outputs checked");
}
