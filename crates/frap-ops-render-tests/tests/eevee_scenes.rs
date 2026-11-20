// SPDX-License-Identifier: GPL-3.0-or-later
//! M5 exit goldens (WP 5.6, ARCHITECTURE §22.7 "Exit: EEVEE goldens SSIM ≥ 0.90 vs Blender on
//! 12 scenes, strict self-goldens; colour goldens ΔE < 1").
//!
//! Every case of `tests/golden/cases/render_eevee/scenes/` carries a directive line
//!
//! ```text
//! # render-golden: image=<path>.png[,<path>.png…] [min_ssim=<x>] [probes=x,y;x,y…]
//! ```
//!
//! and renders with `#@render` lines (comments for the generic golden suite, which compares the
//! scene the case builds; statements here and in Blender). Each written image `out/<stem>.png`
//! is compared with
//!
//! - `tests/golden/fixtures/render_eevee/scenes/<stem>.blender.png`, Blender 5.2.4's EEVEE
//!   render of the same case:
//!   - **scene goldens** (no `probes=`): SSIM ≥ `min_ssim` (never below the M5 gate 0.90) and
//!     mean CIE76 ΔE ≤ 2 in the centre crop;
//!   - **colour goldens** (`probes=`): flat patches (an emission-free HDRI world of colour
//!     swatches seen through each view transform and look); at every probe (mean of the 3 x 3
//!     pixels around `x, y`, top row 0) the colour is within CIE76 ΔE < 1 of Blender's;
//! - `tests/golden/fixtures/render_eevee/scenes/<stem>.frap.png`, the frap-blessed
//!   self-golden: SSIM ≥ 0.995 and mean ΔE ≤ 2 (strict; gating on every CI adapter).
//!
//! Without an adapter the test fails, or prints a notice and passes under
//! `FRAP_GPU_OPTIONAL=1`.
//!
//! Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests --test eevee_scenes --
//! --nocapture` rewrites the `.frap.png` files from this machine's renders and, when Blender
//! 5.2 is found, the `.blender.png` files through the golden runner (`=frap` or `=blender`
//! blesses one side only). `FRAP_RENDER_FILTER=<substring>` restricts the run to the cases
//! whose name contains it; `FRAP_RENDER_KEEP=<dir>` copies every frap render there.

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender, stage_inputs};
use frap_golden::compare::image_ssim::{
    self, DEFAULT_MAX_DELTA_E, EEVEE_MIN_SSIM, SELF_MIN_SSIM, delta_e, to_rgba8,
};
use frap_imbuf::DecodedImage;
use frap_session::{Session, SessionOptions};

/// Case area.
const AREA: &str = "render_eevee/scenes";
const DIRECTIVE: &str = "# render-golden:";
const RENDER_LINE: &str = "#@render";
/// The M5 exit gate on colour goldens: CIE76 ΔE strictly below 1 at every probe.
const COLOUR_MAX_DELTA_E: f64 = 1.0;
/// Number of scene goldens the M5 exit asks for.
const EXIT_SCENES: usize = 12;
/// Every view transform of Blender 5.2.4's `config.ocio` for the sRGB display.
const VIEW_TRANSFORMS: [&str; 9] = [
    "Standard",
    "ACES 1.3",
    "ACES 2.0",
    "Khronos PBR Neutral",
    "AgX",
    "Filmic",
    "Filmic Log",
    "False Color",
    "Raw",
];

struct Case {
    name: String,
    path: PathBuf,
    text: String,
    images: Vec<String>,
    min_ssim: f64,
    probes: Vec<[u32; 2]>,
}

impl Case {
    fn is_colour(&self) -> bool {
        !self.probes.is_empty()
    }
}

fn directive(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix(DIRECTIVE)?
            .split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).map(str::to_owned))
    })
}

fn parse_probes(s: &str) -> Vec<[u32; 2]> {
    s.split(';')
        .map(|p| {
            let (x, y) = p.split_once(',').expect("probe is x,y");
            [x.parse().expect("probe x"), y.parse().expect("probe y")]
        })
        .collect()
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
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            let images = directive(&text, "image=")
                .unwrap_or_else(|| panic!("{name}: no `{DIRECTIVE} image=` line"))
                .split(',')
                .map(str::to_owned)
                .collect();
            Case {
                min_ssim: directive(&text, "min_ssim=").map_or(EEVEE_MIN_SSIM, |s| {
                    s.parse().expect("min_ssim= is a number")
                }),
                probes: directive(&text, "probes=").map_or_else(Vec::new, |s| parse_probes(&s)),
                images,
                name,
                path: p,
                text,
            }
        })
        .collect()
}

fn filtered(repo: &Repo) -> Vec<Case> {
    let filter = std::env::var("FRAP_RENDER_FILTER").unwrap_or_default();
    cases(repo)
        .into_iter()
        .filter(|c| c.name.contains(&filter))
        .collect()
}

fn strip_prefix_line<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// Render lines become statements, `#@frap` lines too, `#@blender` lines blank.
fn frap_source(text: &str) -> String {
    text.lines()
        .map(|l| {
            if let Some(b) = strip_prefix_line(l, RENDER_LINE) {
                b
            } else if let Some(b) = strip_prefix_line(l, "#@frap") {
                b
            } else if strip_prefix_line(l, "#@blender").is_some() {
                ""
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render lines become statements (the golden runner handles the `#@` prefixes).
fn blender_case_text(text: &str) -> String {
    text.lines()
        .map(|l| strip_prefix_line(l, RENDER_LINE).unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

fn stem(image: &str) -> String {
    Path::new(image)
        .file_stem()
        .expect("image path has a file name")
        .to_string_lossy()
        .into_owned()
}

fn fixture(repo: &Repo, image: &str, side: &str) -> PathBuf {
    repo.fixtures_dir()
        .join(AREA)
        .join(format!("{}.{side}.png", stem(image)))
}

/// Runs in `dir` and restores the previous process directory when dropped, also on a panic
/// (`frap-render-scene` build/images.rs resolves relative image paths against the process
/// directory, not the session's; WP 5.5 follow-up, then this guard goes). The process directory
/// is global, so the guard holds [`CWD_LOCK`] while it is changed: any test of this binary that
/// reads or changes the process directory (or resolves a relative path) must take the lock too.
/// Today no other test does.
struct CwdGuard {
    prev: PathBuf,
    _lock: std::sync::MutexGuard<'static, ()>,
}

/// Serialises the tests of this binary that depend on the process directory.
static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl CwdGuard {
    fn enter(dir: &Path) -> CwdGuard {
        // A panic while the lock was held already restored the directory (Drop): not poisoned
        // in any way that matters here.
        let lock = CWD_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        CwdGuard { prev, _lock: lock }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Renders a case through frap in `work`; `Ok(false)`: no GPU (skipped).
fn render_frap(case: &Case, work: &Path) -> Result<bool, String> {
    stage_inputs(&case.path, work).map_err(|e| e.to_string())?;
    let mut opts = SessionOptions::default();
    opts.cwd = work.to_owned();
    let mut s = Session::new(opts).map_err(|e| e.message)?;
    let responses = {
        let _cwd = CwdGuard::enter(work);
        s.exec_source(&frap_source(&case.text))
    };
    if common::any_gpu_unavailable(&responses) {
        return Ok(false);
    }
    if let Some(bad) = responses.iter().find(|r| !r.ok()) {
        return Err(format!("{}: frap failed: {}", case.name, bad.to_json()));
    }
    // Every render is EEVEE's, with nothing left out (no WARNING report).
    for r in &responses {
        let j = r.to_json();
        if j["op"] == "render.render" {
            if j["result"]["engine"] != "BLENDER_EEVEE" {
                return Err(format!("{}: not rendered by EEVEE: {j}", case.name));
            }
            if j.to_string().contains("WARNING") {
                return Err(format!("{}: unexpected warning: {j}", case.name));
            }
        }
    }
    for image in &case.images {
        if !work.join(image).is_file() {
            return Err(format!("{}: frap wrote no {image}", case.name));
        }
    }
    Ok(true)
}

fn bless_blender(repo: &Repo, case: &Case) -> Result<(), String> {
    let Some(blender) = frap_golden::locate_blender() else {
        eprintln!(
            "{}: Blender 5.2 not found; .blender.png not refreshed",
            case.name
        );
        return Ok(());
    };
    let work = ScratchDir::new(&format!("bless_{}", case.name)).map_err(|e| e.to_string())?;
    let case_file = work.path().join(format!("{}.py", case.name));
    std::fs::write(&case_file, blender_case_text(&case.text)).map_err(|e| e.to_string())?;
    let run = work.path().join("run");
    stage_inputs(&case.path, &run).map_err(|e| e.to_string())?;
    let state = run_blender(&blender, repo, &case_file, &run).map_err(|e| e.to_string())?;
    if let Some(bad) = state.lines.iter().find(|l| !l.ok) {
        return Err(format!(
            "{}: Blender failed on line {}: {:?} {:?}",
            case.name, bad.line, bad.exception, bad.message
        ));
    }
    for image in &case.images {
        let dst = fixture(repo, image, "blender");
        std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::copy(run.join(image), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        eprintln!("blessed {}", dst.display());
    }
    Ok(())
}

/// Mean 8-bit sRGB colour of the 3 x 3 pixels around `p` (composited over black).
fn probe(img: &DecodedImage, rgba: &[u8], p: [u32; 2]) -> [u8; 3] {
    let (w, h) = (i64::from(img.width), i64::from(img.height));
    let mut s = [0u32; 3];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let x = (i64::from(p[0]) + dx).clamp(0, w - 1);
            let y = (i64::from(p[1]) + dy).clamp(0, h - 1);
            let i = ((y * w + x) * 4) as usize;
            let a = u32::from(rgba[i + 3]);
            for c in 0..3 {
                s[c] += (u32::from(rgba[i + c]) * a + 127) / 255;
            }
        }
    }
    s.map(|v| ((v + 4) / 9) as u8)
}

/// Probe colour mismatches of `got` against `want` (ΔE ≥ `max`).
fn compare_probes(
    label: &str,
    probes: &[[u32; 2]],
    got: &DecodedImage,
    want: &DecodedImage,
    max: f64,
) -> (f64, Vec<String>) {
    if (got.width, got.height) != (want.width, want.height) {
        return (
            f64::INFINITY,
            vec![format!(
                "{label}: size {}x{} vs {}x{}",
                got.width, got.height, want.width, want.height
            )],
        );
    }
    let (g, w) = (to_rgba8(got), to_rgba8(want));
    let mut worst = 0.0f64;
    let mut out = vec![];
    for p in probes {
        let (a, b) = (probe(got, &g, *p), probe(want, &w, *p));
        let de = delta_e(a, b);
        worst = worst.max(de);
        if de >= max {
            out.push(format!(
                "{label}: probe {p:?}: frap {a:?} vs {b:?}: ΔE {de:.3} ≥ {max}"
            ));
        }
    }
    (worst, out)
}

#[test]
fn directives_parse() {
    let d = "# render-golden: image=out/a.png,out/b.png min_ssim=0.93 probes=1,2;3,4\n";
    assert_eq!(
        directive(d, "image=").as_deref(),
        Some("out/a.png,out/b.png")
    );
    assert_eq!(directive(d, "min_ssim=").as_deref(), Some("0.93"));
    assert_eq!(
        parse_probes(&directive(d, "probes=").unwrap()),
        vec![[1, 2], [3, 4]]
    );
    assert_eq!(stem("out/a.png"), "a");
    let t = "a\n#@render bpy.ops.render.render(write_still=True)\n#@blender x\n#@frap y = 1";
    assert_eq!(
        frap_source(t),
        "a\nbpy.ops.render.render(write_still=True)\n\ny = 1"
    );
    assert_eq!(
        blender_case_text(t),
        "a\nbpy.ops.render.render(write_still=True)\n#@blender x\n#@frap y = 1"
    );
}

#[test]
fn probe_delta_e() {
    let img = DecodedImage {
        width: 3,
        height: 3,
        channels: 3,
        pixels: frap_imbuf::PixelData::U8([10u8, 20, 30].repeat(9)),
    };
    let other = DecodedImage {
        pixels: frap_imbuf::PixelData::U8([10u8, 20, 31].repeat(9)),
        ..img.clone()
    };
    let (worst, bad) = compare_probes("t", &[[1, 1]], &img, &img, 1.0);
    assert!(bad.is_empty() && worst == 0.0);
    let (worst, bad) = compare_probes("t", &[[1, 1]], &img, &other, 1.0);
    assert!(worst > 0.0 && worst < 1.0, "{worst}");
    assert!(bad.is_empty());
    let (_, bad) = compare_probes("t", &[[1, 1]], &img, &other, 0.1);
    assert_eq!(bad.len(), 1);
}

/// The exit set: 12 EEVEE scene goldens at SSIM ≥ 0.90 and colour goldens over every view
/// transform, each image with a Blender reference and a frap self-golden.
#[test]
fn scenes_cases_and_fixtures() {
    let repo = Repo::locate();
    let cases = cases(&repo);
    let scenes: Vec<&Case> = cases.iter().filter(|c| !c.is_colour()).collect();
    assert!(
        scenes.len() >= EXIT_SCENES,
        "{} scene goldens, the M5 exit asks for {EXIT_SCENES}",
        scenes.len()
    );
    let mut stems = std::collections::BTreeSet::new();
    let mut transforms = std::collections::BTreeSet::new();
    for c in &cases {
        assert!(
            c.text.contains("render.engine = 'BLENDER_EEVEE'"),
            "{}: EEVEE",
            c.name
        );
        assert!(c.text.contains(RENDER_LINE), "{}: no #@render line", c.name);
        assert!(
            c.min_ssim >= EEVEE_MIN_SSIM,
            "{}: min_ssim {} below the M5 gate",
            c.name,
            c.min_ssim
        );
        assert!(
            !c.text.contains("'CYCLES'") && !c.text.contains("'//"),
            "{}: D-12 / D-08",
            c.name
        );
        let renders = c
            .text
            .lines()
            .filter(|l| l.trim_start().starts_with(RENDER_LINE))
            .count();
        assert_eq!(renders, c.images.len(), "{}: one render per image", c.name);
        for image in &c.images {
            assert!(image.ends_with(".png"), "{}: {image}", c.name);
            assert!(
                c.text
                    .contains(&format!("filepath = '{}'", image.trim_end_matches(".png"))),
                "{}: no render.filepath for {image}",
                c.name
            );
            assert!(stems.insert(stem(image)), "{}: duplicate {image}", c.name);
            for side in ["frap", "blender"] {
                let p = fixture(&repo, image, side);
                assert!(p.is_file(), "{}: missing {}", c.name, p.display());
            }
        }
        for l in c.text.lines() {
            if let Some(v) = l
                .split("view_settings.view_transform = '")
                .nth(1)
                .and_then(|r| r.split('\'').next())
            {
                transforms.insert(v.to_owned());
            }
        }
    }
    // Blender's default dither (1.0) stays gated: the factory scene and a dark, low-contrast
    // scene render with it (the others turn it off, `night_low_contrast_dither.py` says why).
    let default_dither = scenes
        .iter()
        .filter(|c| {
            !c.text
                .lines()
                .any(|l| !l.starts_with('#') && l.contains("render.dither_intensity"))
        })
        .count();
    assert!(
        default_dither >= 2,
        "{default_dither} scene goldens keep the default dither, 2 expected"
    );
    for vt in VIEW_TRANSFORMS {
        assert!(
            transforms.contains(vt),
            "no golden renders through the {vt} view transform"
        );
    }
}

#[test]
fn eevee_scene_goldens() {
    let repo = Repo::locate();
    let bless = std::env::var("FRAP_RENDER_BLESS").unwrap_or_default();
    let (bless_frap, bless_blender_side) = match bless.as_str() {
        "1" => (true, true),
        "frap" => (true, false),
        "blender" => (false, true),
        _ => (false, false),
    };
    let keep = std::env::var("FRAP_RENDER_KEEP").ok();
    let mut failures = Vec::new();
    let mut table = Vec::new();
    for case in filtered(&repo) {
        if bless_blender_side && let Err(e) = bless_blender(&repo, &case) {
            failures.push(e);
            continue;
        }
        let work = ScratchDir::new(&format!("eevee_scenes_{}", case.name)).unwrap();
        match render_frap(&case, work.path()) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter");
                return;
            }
            Err(e) => {
                failures.push(e);
                continue;
            }
        }
        for image in &case.images {
            let out = work.path().join(image);
            let id = format!("{AREA}/{}", stem(image));
            if let Some(dir) = &keep {
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::copy(&out, Path::new(dir).join(format!("{}.png", stem(image))));
            }
            if bless_frap {
                let dst = fixture(&repo, image, "frap");
                std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
                std::fs::copy(&out, &dst).unwrap();
            }
            let ours = match image_ssim::load_png(&out) {
                Ok(i) => i,
                Err(e) => {
                    failures.push(format!("{id}: {e}"));
                    continue;
                }
            };
            match image_ssim::load_png(&fixture(&repo, image, "blender")) {
                Ok(blender) => {
                    let sc = image_ssim::score(&ours, &blender);
                    if case.is_colour() {
                        let (worst, bad) = compare_probes(
                            &format!("{id} vs Blender 5.2.4"),
                            &case.probes,
                            &ours,
                            &blender,
                            COLOUR_MAX_DELTA_E,
                        );
                        table.push(format!("{id}: worst probe ΔE {worst:.3} (gate < 1)"));
                        failures.extend(bad);
                    } else {
                        if let Ok(sc) = sc {
                            table.push(format!(
                                "{id}: SSIM {:.4} (gate {}), mean ΔE {:.3}",
                                sc.ssim, case.min_ssim, sc.mean_delta_e
                            ));
                        }
                        failures.extend(
                            image_ssim::compare(
                                &format!("{id} vs Blender 5.2.4"),
                                &ours,
                                &blender,
                                case.min_ssim,
                                DEFAULT_MAX_DELTA_E,
                            )
                            .into_iter()
                            .map(|m| m.to_string()),
                        );
                    }
                }
                Err(e) => failures.push(format!("{id}: Blender reference: {e}")),
            }
            match image_ssim::load_png(&fixture(&repo, image, "frap")) {
                Ok(golden) => {
                    failures.extend(
                        image_ssim::compare(
                            &format!("{id} vs self-golden"),
                            &ours,
                            &golden,
                            SELF_MIN_SSIM,
                            DEFAULT_MAX_DELTA_E,
                        )
                        .into_iter()
                        .map(|m| m.to_string()),
                    );
                    if case.is_colour() {
                        failures.extend(
                            compare_probes(
                                &format!("{id} vs self-golden"),
                                &case.probes,
                                &ours,
                                &golden,
                                COLOUR_MAX_DELTA_E,
                            )
                            .1,
                        );
                    }
                }
                Err(e) => failures.push(format!("{id}: self-golden: {e}")),
            }
        }
    }
    eprintln!(
        "EEVEE scene goldens vs Blender 5.2.4:\n{}",
        table.join("\n")
    );
    assert!(
        failures.is_empty(),
        "EEVEE scene goldens:\n{}",
        failures.join("\n")
    );
}
