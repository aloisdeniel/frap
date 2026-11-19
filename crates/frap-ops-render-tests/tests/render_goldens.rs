// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench render goldens (WP 1.24, WP 2.3; ARCHITECTURE §20.1 "Render", §20.4
//! `image_ssim`).
//!
//! Every case of `tests/golden/cases/render/m1/`, `…/render/m2/` and `…/modifiers/integration/`
//! (WP 4.7: renders show modifiers) with a
//! `# render-golden: image=<path> [min_ssim=<x>]` line is a render golden. Its render statements
//! are `#@render` lines: comments for the generic golden suite (`cargo test -p frap-golden`,
//! which compares the scene the case builds with Blender's and must run without a GPU),
//! statements here. This test runs the case through frap with the render lines and compares the
//! written image with
//!
//! - `tests/golden/fixtures/<area>/<case>.frap.png`, the frap-blessed self-golden: SSIM ≥ 0.995
//!   and mean ΔE ≤ 2 in the centre crop, gating on every CI adapter (gpu.yml: Metal, lavapipe,
//!   WARP);
//! - `tests/golden/fixtures/<area>/<case>.blender.png`, Blender 5.2.4's Workbench render of the
//!   same case: SSIM ≥ 0.97 for the M1 scenes and ≥ `min_ssim` (0.95 for the M2 X-ray,
//!   wireframe, matcap, outline and shadow scenes) and mean ΔE ≤ 2, **gating** from M2 (WP 2.3).
//!
//! - with `bg_max_delta_e=<x>` on the directive line, every background pixel of Blender's
//!   render (its top-left colour, with the 3 x 3 neighbourhood also background) must be within
//!   CIE76 ΔE `x` in frap's render, pixel for pixel over the whole image: the mean SSIM is blind
//!   to a few stray pixels an agent would take for geometry.
//!
//! Without an adapter the test fails, or prints a notice and passes under
//! `FRAP_GPU_OPTIONAL=1` (ci.yml's GPU-less test job).
//!
//! Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests --test render_goldens --
//! --nocapture` rewrites the `.frap.png` files from this machine's render and, when Blender
//! 5.2 is found (`$BLENDER_BIN`, `PATH`, `~/Applications/Blender-5.2.app`), the `.blender.png`
//! files through the golden runner (`tools/blender/golden_runner.py`, VIEW_3D override and all).
//! `FRAP_RENDER_KEEP=<dir>` copies every frap render there (debugging).

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_golden::compare::image_ssim::{
    self, DEFAULT_MAX_DELTA_E, SELF_MIN_SSIM, WORKBENCH_MIN_SSIM,
};
use frap_session::{Session, SessionOptions};

/// Case areas of the render goldens.
const AREAS: [&str; 3] = ["render/m1", "render/m2", "modifiers/integration"];
/// Bless switch.
const BLESS_ENV: &str = "FRAP_RENDER_BLESS";
/// Directive naming the rendered image (case-relative output path).
const DIRECTIVE: &str = "# render-golden:";
/// Prefix of render-only statements.
const RENDER_LINE: &str = "#@render";

/// One render golden.
struct RenderCase {
    area: &'static str,
    name: String,
    text: String,
    image: String,
    /// Minimum SSIM against Blender's render.
    min_ssim: f64,
    /// Per-pixel ΔE limit on Blender's background pixels (`bg_max_delta_e=`).
    bg_max_delta_e: Option<f64>,
}

impl RenderCase {
    fn id(&self) -> String {
        format!("{}/{}", self.area, self.name)
    }
}

fn directive(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix(DIRECTIVE)?
            .split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).map(str::to_owned))
    })
}

fn render_cases(repo: &Repo) -> Vec<RenderCase> {
    let mut out = Vec::new();
    for area in AREAS {
        let dir = repo.cases_dir().join(area);
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "py"))
            .collect();
        paths.sort();
        for p in paths {
            let text = std::fs::read_to_string(&p).unwrap();
            let Some(image) = directive(&text, "image=") else {
                continue;
            };
            let min_ssim = directive(&text, "min_ssim=")
                .map(|s| s.parse().expect("min_ssim= is a number"))
                .unwrap_or(WORKBENCH_MIN_SSIM);
            let bg_max_delta_e = directive(&text, "bg_max_delta_e=")
                .map(|s| s.parse().expect("bg_max_delta_e= is a number"));
            out.push(RenderCase {
                area,
                name: p.file_stem().unwrap().to_string_lossy().into_owned(),
                text,
                image,
                min_ssim,
                bg_max_delta_e,
            });
        }
    }
    out
}

fn strip_prefix_line<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// The source frap runs: render lines and `#@frap` lines become statements, `#@blender` lines
/// blank (line numbers kept).
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

/// The case Blender runs when blessing: render lines become shared statements.
fn blender_case_text(text: &str) -> String {
    text.lines()
        .map(|l| strip_prefix_line(l, RENDER_LINE).unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fixture(repo: &Repo, case: &RenderCase, side: &str) -> PathBuf {
    repo.fixtures_dir()
        .join(case.area)
        .join(format!("{}.{side}.png", case.name))
}

/// Renders a case through frap; `Ok(None)`: no GPU (skipped).
fn render_frap(case: &RenderCase, work: &Path) -> Result<Option<PathBuf>, String> {
    let mut opts = SessionOptions::default();
    opts.cwd = work.to_owned();
    let mut s = Session::new(opts).map_err(|e| e.message)?;
    let responses = s.exec_source(&frap_source(&case.text));
    if common::any_gpu_unavailable(&responses) {
        return Ok(None);
    }
    if let Some(bad) = responses.iter().find(|r| !r.ok()) {
        return Err(format!("{}: frap failed: {}", case.id(), bad.to_json()));
    }
    let out = work.join(&case.image);
    if !out.is_file() {
        return Err(format!("{}: frap wrote no {}", case.id(), case.image));
    }
    Ok(Some(out))
}

/// Pixel-for-pixel background check: every pixel of `reference` that has its top-left colour
/// (±1 per channel), together with its 8 neighbours (anti-aliased silhouettes excluded), must
/// be within ΔE `max` in `frap`. Returns the failure message, if any.
fn background_check(
    id: &str,
    frap: &frap_imbuf::DecodedImage,
    reference: &frap_imbuf::DecodedImage,
    max: f64,
) -> Option<String> {
    let (w, h) = (reference.width as usize, reference.height as usize);
    if (frap.width as usize, frap.height as usize) != (w, h) {
        return Some(format!("{id}: background check: size mismatch"));
    }
    let (a, b) = (image_ssim::to_rgba8(frap), image_ssim::to_rgba8(reference));
    let rgb = |v: &[u8], x: usize, y: usize| {
        let i = (y * w + x) * 4;
        [v[i], v[i + 1], v[i + 2]]
    };
    let bg = rgb(&b, 0, 0);
    let is_bg = |x: usize, y: usize| {
        rgb(&b, x, y)
            .iter()
            .zip(bg)
            .all(|(c, k)| c.abs_diff(k) <= 1)
    };
    let (mut checked, mut bad, mut worst) = (0usize, Vec::new(), 0.0f64);
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            if !(y - 1..=y + 1).all(|yy| (x - 1..=x + 1).all(|xx| is_bg(xx, yy))) {
                continue;
            }
            checked += 1;
            let d = image_ssim::delta_e(rgb(&a, x, y), rgb(&b, x, y));
            worst = worst.max(d);
            if d > max {
                bad.push((x, y));
            }
        }
    }
    eprintln!("{id}: background check: {checked} pixels, max ΔE {worst:.2} (gate {max})");
    (checked == 0 || !bad.is_empty()).then(|| {
        format!(
            "{id}: {} of {checked} Blender background pixels differ by ΔE > {max} (max {worst:.2}; \
             first {:?})",
            bad.len(),
            &bad[..bad.len().min(8)]
        )
    })
}

fn bless_blender(repo: &Repo, case: &RenderCase) -> Result<(), String> {
    let Some(blender) = frap_golden::locate_blender() else {
        eprintln!(
            "{}: Blender 5.2 not found; .blender.png not refreshed",
            case.id()
        );
        return Ok(());
    };
    let work = ScratchDir::new(&format!("bless_{}", case.name)).map_err(|e| e.to_string())?;
    let case_file = work.path().join(format!("{}.py", case.name));
    std::fs::write(&case_file, blender_case_text(&case.text)).map_err(|e| e.to_string())?;
    let run = work.path().join("run");
    let state = run_blender(&blender, repo, &case_file, &run).map_err(|e| e.to_string())?;
    if let Some(bad) = state.lines.iter().find(|l| !l.ok) {
        return Err(format!(
            "{}: Blender failed on line {}: {:?} {:?}",
            case.id(),
            bad.line,
            bad.exception,
            bad.message
        ));
    }
    let dst = fixture(repo, case, "blender");
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::copy(run.join(&case.image), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    eprintln!("blessed {}", dst.display());
    Ok(())
}

#[test]
fn source_transforms() {
    let text = "# golden: compare=objects\nimport bpy\n#@render bpy.ops.render.render(write_still=True)\n#@frap  x = 1\n#@blender y = 2\n";
    assert_eq!(
        frap_source(text),
        "# golden: compare=objects\nimport bpy\nbpy.ops.render.render(write_still=True)\n x = 1\n"
    );
    assert_eq!(
        blender_case_text(text),
        "# golden: compare=objects\nimport bpy\nbpy.ops.render.render(write_still=True)\n#@frap  x = 1\n#@blender y = 2"
    );
    let d = "# render-golden: image=out/a.png min_ssim=0.95\n";
    assert_eq!(directive(d, "image=").as_deref(), Some("out/a.png"));
    assert_eq!(directive(d, "min_ssim=").as_deref(), Some("0.95"));
    assert_eq!(directive(d, "bg_max_delta_e="), None);
}

#[test]
fn workbench_scenes_and_fixtures() {
    let repo = Repo::locate();
    let cases = render_cases(&repo);
    let in_area = |a: &str| cases.iter().filter(|c| c.area == a).count();
    assert!(
        in_area("render/m1") >= 3,
        "{} M1 render goldens",
        in_area("render/m1")
    );
    // WP 2.3 acceptance: X-ray, wireframe, matcap, outline and shadows against Blender.
    let m2: Vec<&str> = cases
        .iter()
        .filter(|c| c.area == "render/m2")
        .map(|c| c.name.as_str())
        .collect();
    for feature in ["xray", "wireframe", "matcap", "outline", "shadows"] {
        assert!(
            m2.iter().any(|n| n.contains(feature)),
            "no render/m2 golden for {feature}: {m2:?}"
        );
    }
    for c in &cases {
        assert!(
            c.text.contains("render.engine = 'BLENDER_WORKBENCH'"),
            "{}: render goldens use Workbench",
            c.id()
        );
        assert!(c.text.contains(RENDER_LINE), "{}: no #@render line", c.id());
        // M1 scenes gate at 0.97 (§22.4); M2 feature scenes at 0.95 or more.
        let floor = if c.area == "render/m1" {
            WORKBENCH_MIN_SSIM
        } else {
            0.95
        };
        assert!(c.min_ssim >= floor, "{}: min_ssim {}", c.id(), c.min_ssim);
        if c.name.starts_with("shadows_background") {
            assert!(
                c.bg_max_delta_e.is_some_and(|x| x <= 2.0),
                "{}: background goldens gate pixel for pixel",
                c.id()
            );
        }
        // Committed self-golden and Blender reference.
        for side in ["frap", "blender"] {
            let p = fixture(&repo, c, side);
            assert!(p.is_file(), "{}: missing {}", c.id(), p.display());
        }
    }
}

#[test]
fn self_and_blender_goldens() {
    let repo = Repo::locate();
    let bless = std::env::var(BLESS_ENV).is_ok_and(|v| v == "1");
    let keep = std::env::var("FRAP_RENDER_KEEP").ok();
    let mut failures = Vec::new();
    for case in render_cases(&repo) {
        let work = ScratchDir::new(&case.name).unwrap();
        let out = match render_frap(&case, work.path()) {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        if let Some(dir) = &keep {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::copy(&out, Path::new(dir).join(format!("{}.png", case.name)));
        }
        let frap = image_ssim::load_png(&out).unwrap();
        if bless {
            let dst = fixture(&repo, &case, "frap");
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(&out, &dst).unwrap();
            eprintln!("blessed {}", dst.display());
            if let Err(e) = bless_blender(&repo, &case) {
                failures.push(e);
            }
        }
        match image_ssim::load_png(&fixture(&repo, &case, "frap")) {
            Ok(golden) => {
                for m in image_ssim::compare(
                    &format!("{} vs {}.frap.png", case.id(), case.name),
                    &frap,
                    &golden,
                    SELF_MIN_SSIM,
                    DEFAULT_MAX_DELTA_E,
                ) {
                    failures.push(m.to_string());
                }
            }
            Err(e) => failures.push(format!("{}: self-golden: {e}", case.id())),
        }
        // Gating against Blender 5.2.4's Workbench (§22.4, WP 2.3).
        match image_ssim::load_png(&fixture(&repo, &case, "blender")) {
            Ok(b) => {
                if let Ok(s) = image_ssim::score(&frap, &b) {
                    eprintln!(
                        "{}: vs Blender 5.2.4 Workbench: SSIM {:.4} (gate {}), mean ΔE {:.3} \
                         (centre crop)",
                        case.id(),
                        s.ssim,
                        case.min_ssim,
                        s.mean_delta_e
                    );
                }
                for m in image_ssim::compare(
                    &format!("{} vs {}.blender.png", case.id(), case.name),
                    &frap,
                    &b,
                    case.min_ssim,
                    DEFAULT_MAX_DELTA_E,
                ) {
                    failures.push(m.to_string());
                }
                if let Some(max) = case.bg_max_delta_e {
                    failures.extend(background_check(&case.id(), &frap, &b, max));
                }
            }
            Err(e) => failures.push(format!("{}: Blender reference: {e}", case.id())),
        }
    }
    assert!(
        failures.is_empty(),
        "render goldens:\n{}",
        failures.join("\n")
    );
}
