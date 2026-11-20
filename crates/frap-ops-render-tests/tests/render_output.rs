// SPDX-License-Identifier: GPL-3.0-or-later
//! Output-format render goldens (WP 5.4, ARCHITECTURE §19.4): every case of
//! `tests/golden/cases/render/output/` lists the files its `#@render` lines write
//! (`# render-output: out/a.png out/b.exr …`). This test renders the case through frap and
//! compares each file with Blender 5.2.4's render of the same case
//! (`tests/golden/fixtures/render/output/<case>/<file name>`):
//!
//! - the same format (sniffed from the bytes; TIFF, IRIS, DPX, Cineon and multilayer OpenEXR by
//!   their own readers), size, channel count and sample type;
//! - for alpha files, the same transparent background (`film_transparent`): every pixel whose
//!   alpha is 0 in Blender's file is 0 in frap's, and the share of transparent pixels agrees;
//! - the pixels: SSIM ≥ 0.95 and mean ΔE ≤ 2 in the centre crop (Workbench renders, floats
//!   compared as stored).
//!
//! The exact behaviour of the output path (bytes and samples for a given film) is tested without
//! a GPU against Blender's outputs of one film (`frap-ops-render` `output::format` tests,
//! `frap-imbuf` `encode` tests); this test checks the whole operator.
//!
//! Without an adapter the test fails, or prints a notice and passes under
//! `FRAP_GPU_OPTIONAL=1`. Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests
//! --test render_output -- --nocapture` rewrites the Blender fixtures through the golden runner
//! when Blender 5.2 is found.

mod common;

use std::path::{Path, PathBuf};

use common::Env;
use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_golden::compare::image_ssim::{self, DEFAULT_MAX_DELTA_E};
use frap_imbuf::{DecodedImage, FileFormat, PixelData};
use frap_session::{Session, SessionOptions};

const AREA: &str = "render/output";
const DIRECTIVE: &str = "# render-output:";
const RENDER_LINE: &str = "#@render";
const MIN_SSIM: f64 = 0.95;

struct Case {
    name: String,
    text: String,
    files: Vec<String>,
}

fn cases(repo: &Repo) -> Vec<Case> {
    let dir = repo.cases_dir().join(AREA);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            let files = text
                .lines()
                .find_map(|l| l.strip_prefix(DIRECTIVE))?
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            Some(Case {
                name: p.file_stem().unwrap().to_string_lossy().into_owned(),
                text,
                files,
            })
        })
        .collect()
}

fn strip<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// frap's source: render lines and `#@frap` lines become statements, `#@blender` lines blank.
fn frap_source(text: &str) -> String {
    text.lines()
        .map(|l| {
            strip(l, RENDER_LINE)
                .or_else(|| strip(l, "#@frap"))
                .unwrap_or(if strip(l, "#@blender").is_some() {
                    ""
                } else {
                    l
                })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fixture(repo: &Repo, case: &Case, file: &str) -> PathBuf {
    let name = Path::new(file).file_name().unwrap();
    repo.fixtures_dir().join(AREA).join(&case.name).join(name)
}

fn bless_blender(repo: &Repo, case: &Case) -> Result<(), String> {
    let Some(blender) = frap_golden::locate_blender() else {
        eprintln!(
            "{}: Blender 5.2 not found; fixtures not refreshed",
            case.name
        );
        return Ok(());
    };
    let work = ScratchDir::new(&format!("bless_out_{}", case.name)).map_err(|e| e.to_string())?;
    let case_file = work.path().join(format!("{}.py", case.name));
    let text: Vec<&str> = case
        .text
        .lines()
        .map(|l| strip(l, RENDER_LINE).unwrap_or(l))
        .collect();
    std::fs::write(&case_file, text.join("\n")).map_err(|e| e.to_string())?;
    let run = work.path().join("run");
    let state = run_blender(&blender, repo, &case_file, &run).map_err(|e| e.to_string())?;
    if let Some(bad) = state.lines.iter().find(|l| !l.ok) {
        return Err(format!(
            "{}: Blender failed on line {}: {:?} {:?}",
            case.name, bad.line, bad.exception, bad.message
        ));
    }
    for f in &case.files {
        let dst = fixture(repo, case, f);
        std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::copy(run.join(f), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        eprintln!("blessed {}", dst.display());
    }
    Ok(())
}

fn sample_type(img: &DecodedImage) -> &'static str {
    match img.pixels {
        PixelData::U8(_) => "8-bit",
        PixelData::U16(_) => "16-bit",
        PixelData::F32(_) => "float",
        _ => "other",
    }
}

fn alpha(img: &DecodedImage) -> Option<Vec<f32>> {
    let c = usize::from(img.channels);
    (c == 2 || c == 4).then(|| {
        let s = if sample_type(img) == "8-bit" {
            255.0
        } else if sample_type(img) == "16-bit" {
            65535.0
        } else {
            1.0
        };
        img.to_f32().chunks_exact(c).map(|p| p[c - 1] * s).collect()
    })
}

/// The format of an output file: `FileFormat`'s sniffing, plus the render-only formats.
fn format_of(bytes: &[u8]) -> Option<String> {
    use frap_imbuf::encode::{is_cineon, is_dpx, is_sgi};
    if let Some(f) = FileFormat::detect(bytes) {
        let multilayer = f == FileFormat::OpenExr
            && frap_imbuf::decode_exr_channels(bytes)
                .is_ok_and(|(_, _, c)| c.iter().any(|(n, _)| n.contains('.')));
        return Some(if multilayer {
            "OpenExrMultilayer".into()
        } else {
            format!("{f:?}")
        });
    }
    Some(
        if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            "Tiff"
        } else if is_sgi(bytes) {
            "Iris"
        } else if is_dpx(bytes) {
            "Dpx"
        } else if is_cineon(bytes) {
            "Cineon"
        } else {
            return None;
        }
        .into(),
    )
}

/// Decodes an output file (DPX and Cineon as their stored codes; a multilayer OpenEXR as the
/// RGBA of its Combined pass).
fn decode_output(bytes: &[u8]) -> Result<DecodedImage, frap_imbuf::ImageError> {
    use frap_imbuf::encode::{decode_cineon, decode_dpx, decode_sgi, decode_tiff};
    match format_of(bytes).as_deref() {
        Some("Tiff") => decode_tiff(bytes),
        Some("Iris") => decode_sgi(bytes),
        Some("Dpx") => decode_dpx(bytes),
        Some("Cineon") => decode_cineon(bytes),
        Some("OpenExrMultilayer") => {
            let (w, h, channels) = frap_imbuf::decode_exr_channels(bytes)?;
            let plane = |c: &str| {
                channels
                    .iter()
                    .find(|(n, _)| n.ends_with(&format!(".Combined.{c}")))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            let [r, g, b, a] = ["R", "G", "B", "A"].map(plane);
            let data = (0..r.len())
                .flat_map(|i| [r[i], g[i], b[i], a[i]])
                .collect();
            DecodedImage::new(w, h, 4, PixelData::F32(data))
        }
        _ => frap_imbuf::decode(bytes),
    }
}

/// Compares one output file; returns the failures.
fn compare_file(what: &str, ours: &[u8], theirs: &[u8]) -> Vec<String> {
    let mut fails = Vec::new();
    let (fo, ft) = (format_of(ours), format_of(theirs));
    if fo != ft {
        return vec![format!("{what}: format {fo:?}, Blender's {ft:?}")];
    }
    let (a, b) = match (decode_output(ours), decode_output(theirs)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => return vec![format!("{what}: decode {:?} / {:?}", a.err(), b.err())],
    };
    let shape = |i: &DecodedImage| (i.width, i.height, i.channels, sample_type(i));
    if shape(&a) != shape(&b) {
        return vec![format!(
            "{what}: {:?}, Blender's {:?}",
            shape(&a),
            shape(&b)
        )];
    }
    if let (Some(aa), Some(ab)) = (alpha(&a), alpha(&b)) {
        let n = ab.len() as f64;
        let transparent = |v: &[f32]| v.iter().filter(|&&x| x == 0.0).count() as f64 / n;
        let (ta, tb) = (transparent(&aa), transparent(&ab));
        if (ta - tb).abs() > 0.02 {
            fails.push(format!(
                "{what}: {:.1} % transparent pixels, Blender's {:.1} %",
                ta * 100.0,
                tb * 100.0
            ));
        }
        // Away from silhouettes (a 3 x 3 neighbourhood transparent in Blender's), frap's
        // background is transparent too.
        let (w, h) = (b.width as usize, b.height as usize);
        let zero = |x: usize, y: usize| ab[y * w + x] == 0.0;
        let bad = (1..h.saturating_sub(1))
            .flat_map(|y| (1..w.saturating_sub(1)).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                (y - 1..=y + 1).all(|yy| (x - 1..=x + 1).all(|xx| zero(xx, yy)))
                    && aa[y * w + x] != 0.0
            })
            .count();
        if bad > 0 {
            fails.push(format!(
                "{what}: {bad} background pixels are not transparent"
            ));
        }
    }
    match image_ssim::score(&a, &b) {
        Ok(s) => {
            eprintln!(
                "{what}: SSIM {:.4}, mean ΔE {:.3} (centre crop)",
                s.ssim, s.mean_delta_e
            );
            if s.ssim < MIN_SSIM || s.mean_delta_e > DEFAULT_MAX_DELTA_E {
                fails.push(format!(
                    "{what}: SSIM {:.4} (gate {MIN_SSIM}), mean ΔE {:.3} (gate \
                     {DEFAULT_MAX_DELTA_E})",
                    s.ssim, s.mean_delta_e
                ));
            }
        }
        Err(e) => fails.push(format!("{what}: {e}")),
    }
    fails
}

#[test]
fn cases_and_fixtures() {
    let repo = Repo::locate();
    let cs = cases(&repo);
    assert!(cs.len() >= 2, "{} render/output cases", cs.len());
    let mut formats = std::collections::BTreeSet::new();
    for c in &cs {
        assert!(
            c.text.contains("render.engine = 'BLENDER_WORKBENCH'"),
            "{}",
            c.name
        );
        let renders = c
            .text
            .lines()
            .filter(|l| strip(l, RENDER_LINE).is_some())
            .count();
        assert_eq!(
            renders,
            c.files.len(),
            "{}: one #@render line per output",
            c.name
        );
        for f in &c.files {
            let p = fixture(&repo, c, f);
            let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            formats.insert(format_of(&bytes).unwrap());
        }
    }
    // Every output format against Blender.
    for f in [
        "Png",
        "Jpeg",
        "WebP",
        "OpenExr",
        "Hdr",
        "Targa",
        "Bmp",
        "Tiff",
        "Iris",
        "Dpx",
        "Cineon",
        "OpenExrMultilayer",
    ] {
        assert!(
            formats.contains(f),
            "no render/output golden writes {f}: {formats:?}"
        );
    }
}

#[test]
fn outputs_match_blender() {
    let repo = Repo::locate();
    let bless = std::env::var("FRAP_RENDER_BLESS").is_ok_and(|v| v == "1");
    let mut fails = Vec::new();
    for case in cases(&repo) {
        if bless && let Err(e) = bless_blender(&repo, &case) {
            fails.push(e);
            continue;
        }
        let work = ScratchDir::new(&format!("out_{}", case.name)).unwrap();
        let mut opts = SessionOptions::default();
        opts.cwd = work.path().to_owned();
        let mut s = Session::new(opts).unwrap();
        let responses = s.exec_source(&frap_source(&case.text));
        if common::any_gpu_unavailable(&responses) {
            return;
        }
        if let Some(bad) = responses.iter().find(|r| !r.ok()) {
            fails.push(format!("{}: frap failed: {}", case.name, bad.to_json()));
            continue;
        }
        for f in &case.files {
            let what = format!("{}/{f}", case.name);
            let ours = match std::fs::read(work.path().join(f)) {
                Ok(b) => b,
                Err(e) => {
                    fails.push(format!("{what}: frap wrote nothing: {e}"));
                    continue;
                }
            };
            let theirs = match std::fs::read(fixture(&repo, &case, f)) {
                Ok(b) => b,
                Err(e) => {
                    fails.push(format!("{what}: no Blender fixture: {e}"));
                    continue;
                }
            };
            fails.extend(compare_file(&what, &ours, &theirs));
        }
    }
    assert!(fails.is_empty(), "render outputs:\n{}", fails.join("\n"));
}

/// Blender cannot write a one-channel WebP: `render.render(write_still=True)` fails after
/// rendering with `Render error (<strerror(0)>) cannot save: '<path>'` and writes nothing
/// (verified on 5.2.4: `Render error (Undefined error: 0) cannot save: '…/webp_bw.webp'`).
#[test]
fn bw_webp_cannot_be_saved() {
    let mut e = Env::new("render_bw_webp");
    e.ok("bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 32
bpy.context.scene.render.resolution_y = 18
bpy.context.scene.render.image_settings.file_format = 'WEBP'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/bw'");
    let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
        return;
    };
    assert_eq!(j["error"]["exception"], "RuntimeError", "{j}");
    let msg = j["error"]["message"].as_str().unwrap();
    let path = e.path("out/bw.webp");
    assert!(msg.starts_with("Error: Render error ("), "{msg}");
    // The path as Blender computes it: a relative `filepath` stays relative (WP 6.6, verified
    // on 5.2.4: `cannot save: 'out/w_0001.webp'`).
    assert!(msg.ends_with(") cannot save: 'out/bw.webp'"), "{msg}");
    #[cfg(target_os = "macos")]
    assert!(msg.contains("(Undefined error: 0)"), "{msg}");
    assert!(!path.exists());
}

/// `film_transparent` reaches every alpha format: the Workbench background is alpha 0 in PNG,
/// Targa, BMP, WebP and OpenEXR (premultiplied: colour 0 too), and opaque without it.
#[test]
fn film_transparent_alpha_in_every_format() {
    let mut e = Env::new("render_film_transparent");
    e.ok("bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 48
bpy.context.scene.render.resolution_y = 27");
    for transparent in [true, false] {
        e.ok(&format!(
            "bpy.context.scene.render.film_transparent = {}",
            if transparent { "True" } else { "False" }
        ));
        for (fmt, ext, extra) in [
            ("PNG", "png", ""),
            ("TARGA", "tga", ""),
            ("TARGA_RAW", "tga", ""),
            ("BMP", "bmp", ""),
            (
                "WEBP",
                "webp",
                "bpy.context.scene.render.image_settings.quality = 100",
            ),
            ("OPEN_EXR", "exr", ""),
        ] {
            e.ok(&format!(
                "bpy.context.scene.render.image_settings.file_format = '{fmt}'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.filepath = 'out/ft'"
            ));
            if !extra.is_empty() {
                e.ok(extra);
            }
            let Some(j) = e.render("bpy.ops.render.render(write_still=True)") else {
                return;
            };
            assert_eq!(j["ok"], true, "{fmt}: {j}");
            let img = frap_imbuf::load(&e.path(&format!("out/ft.{ext}"))).unwrap();
            assert_eq!(img.channels, 4, "{fmt}");
            let px = img.to_f32();
            // The top-left corner is background.
            let corner = &px[..4];
            if transparent {
                assert_eq!(
                    corner[3], 0.0,
                    "{fmt} transparent={transparent}: {corner:?}"
                );
                if fmt == "OPEN_EXR" {
                    assert_eq!(&corner[..3], &[0.0; 3], "{fmt}: premultiplied");
                }
            } else {
                // Opaque. Float files show frap's Workbench film alpha, 511/512 (its
                // `Rgba16Float` sample targets; Blender's is 1): a renderer matter, not the
                // output path's, which writes the film's alpha as it is.
                assert!(
                    corner[3] >= 0.998,
                    "{fmt} transparent={transparent}: {corner:?}"
                );
            }
            // The cube in the middle is (nearly: anti-aliased) opaque either way.
            let mid = (img.height as usize / 2 * img.width as usize + img.width as usize / 2) * 4;
            assert!(px[mid + 3] > 0.99, "{fmt}: centre pixel {}", px[mid + 3]);
            std::fs::remove_file(e.path(&format!("out/ft.{ext}"))).unwrap();
        }
    }
}
