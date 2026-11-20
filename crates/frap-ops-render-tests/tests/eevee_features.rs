// SPDX-License-Identifier: GPL-3.0-or-later
//! Shading-feature render goldens (WP 5.5, ARCHITECTURE §22.7: "per-feature self-goldens +
//! loose Blender goldens"). Every case of `tests/golden/cases/render_eevee/features/` (normal
//! maps, alpha blend and dither, transmission, clearcoat, sheen, emission, vertex colours, and
//! the Workbench VERTEX and TEXTURE colour types) runs through a real `Session` with its
//! `#@render` lines and its `<case>.inputs/` files staged into the session's working directory
//! (the process directory is never changed: relative image paths resolve against the
//! session's, `frap-render-scene` build/images.rs). The PNG it writes must match:
//!
//! - Blender 5.2.4's render of the same case (`<case>.blender.png`) loosely: SSIM ≥ the case's
//!   `min_ssim` and mean ΔE ≤ `max_delta_e` (default [`LOOSE_MAX_DELTA_E`]) in the centre crop;
//! - frap's own blessed render (`<case>.frap.png`) strictly: SSIM ≥ 0.99 and mean ΔE ≤ 1
//!   (an adapter-drift alarm: each feature's look is pinned).
//!
//! No WARNING may come back (in particular no "colour type … not rendered yet" and no
//! Workbench fallback). Without an adapter the test fails, or passes with a notice under
//! `FRAP_GPU_OPTIONAL=1`. `FRAP_RENDER_KEEP=<dir>` copies every render there;
//! `FRAP_RENDER_BLESS=1` rewrites the `.frap.png` files and, when Blender 5.2 is found, the
//! `.blender.png` files through the golden runner.

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender, stage_inputs};
use frap_golden::compare::image_ssim;
use frap_session::{Session, SessionOptions};

const AREA: &str = "render_eevee/features";
const DIRECTIVE: &str = "# render-golden:";
/// Mean ΔE gate against Blender unless the case sets `max_delta_e=`.
const LOOSE_MAX_DELTA_E: f64 = 4.0;
/// Gates against the frap self-golden.
const SELF_MIN_SSIM: f64 = 0.99;
const SELF_MAX_DELTA_E: f64 = 1.0;
/// The shading features of the WP 5.5 acceptance, each with at least one case.
const FEATURES: [&str; 10] = [
    "normal_map",
    "alpha_blended",
    "alpha_dithered",
    "transmission",
    "clearcoat",
    "sheen",
    "emission",
    "vertex_colors",
    "workbench_vertex",
    "workbench_texture",
];

fn directive(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix(DIRECTIVE)?
            .split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).map(str::to_owned))
    })
}

/// Render lines become statements, `#@frap` lines too, `#@blender` lines blank.
fn frap_source(text: &str) -> String {
    text.lines()
        .map(|l| {
            let t = l.trim_start();
            if let Some(b) = t
                .strip_prefix("#@render")
                .or_else(|| t.strip_prefix("#@frap"))
            {
                b.strip_prefix(' ').unwrap_or(b)
            } else if t.starts_with("#@blender") {
                ""
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The case as Blender runs it: render lines become statements.
fn blender_source(text: &str) -> String {
    text.lines()
        .map(|l| {
            let t = l.trim_start();
            match t.strip_prefix("#@render") {
                Some(b) => b.strip_prefix(' ').unwrap_or(b),
                None => l,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct Case {
    name: String,
    path: PathBuf,
    text: String,
    image: String,
    min_ssim: f64,
    max_delta_e: f64,
}

fn cases(repo: &Repo) -> Vec<Case> {
    let dir = repo.cases_dir().join(AREA);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    v.sort();
    v.into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            Case {
                image: directive(&text, "image=")
                    .unwrap_or_else(|| panic!("{name}: no render-golden image=")),
                min_ssim: directive(&text, "min_ssim=")
                    .map_or(image_ssim::EEVEE_MIN_SSIM, |s| s.parse().unwrap()),
                max_delta_e: directive(&text, "max_delta_e=")
                    .map_or(LOOSE_MAX_DELTA_E, |s| s.parse().unwrap()),
                name,
                path,
                text,
            }
        })
        .collect()
}

fn fixture(repo: &Repo, case: &Case, side: &str) -> PathBuf {
    repo.fixtures_dir()
        .join(AREA)
        .join(format!("{}.{side}.png", case.name))
}

/// Renders a case through frap; `Ok(None)`: no GPU (skipped).
fn render_frap(case: &Case, work: &Path) -> Result<Option<PathBuf>, String> {
    stage_inputs(&case.path, work).map_err(|e| e.to_string())?;
    let mut opts = SessionOptions::default();
    opts.cwd = work.to_owned();
    let mut s = Session::new(opts).map_err(|e| e.message)?;
    let responses = s.exec_source(&frap_source(&case.text));
    if common::any_gpu_unavailable(&responses) {
        return Ok(None);
    }
    if let Some(bad) = responses.iter().find(|r| !r.ok()) {
        return Err(format!("{}: frap failed: {}", case.name, bad.to_json()));
    }
    if let Some(r) = responses
        .iter()
        .find(|r| r.to_json().to_string().contains("WARNING"))
    {
        return Err(format!(
            "{}: unexpected warning: {}",
            case.name,
            r.to_json()
        ));
    }
    let out = work.join(&case.image);
    if !out.is_file() {
        return Err(format!("{}: frap wrote no {}", case.name, case.image));
    }
    Ok(Some(out))
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
    std::fs::write(&case_file, blender_source(&case.text)).map_err(|e| e.to_string())?;
    // `run_blender` stages the inputs next to the case file it is given.
    let inputs = case.path.with_extension("inputs");
    if inputs.is_dir() {
        stage_inputs(&case.path, &case_file.with_extension("inputs")).map_err(|e| e.to_string())?;
    }
    let run = work.path().join("run");
    let state = run_blender(&blender, repo, &case_file, &run).map_err(|e| e.to_string())?;
    if let Some(bad) = state.lines.iter().find(|l| !l.ok) {
        return Err(format!(
            "{}: Blender failed on line {}: {:?} {:?}",
            case.name, bad.line, bad.exception, bad.message
        ));
    }
    let dst = fixture(repo, case, "blender");
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::copy(run.join(&case.image), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    eprintln!("blessed {}", dst.display());
    Ok(())
}

#[test]
fn feature_cases_and_fixtures() {
    let repo = Repo::locate();
    let cases = cases(&repo);
    for want in FEATURES {
        assert!(cases.iter().any(|c| c.name == want), "missing case {want}");
    }
    for c in &cases {
        assert!(c.text.contains("#@render "), "{}: no #@render line", c.name);
        assert!(c.image.ends_with(".png"), "{}: PNG output", c.name);
        let engine = if c.name.starts_with("workbench_") {
            "render.engine = 'BLENDER_WORKBENCH'"
        } else {
            "render.engine = 'BLENDER_EEVEE'"
        };
        assert!(c.text.contains(engine), "{}: {engine}", c.name);
        assert!(c.min_ssim >= 0.8, "{}: min_ssim {}", c.name, c.min_ssim);
        for side in ["frap", "blender"] {
            let p = fixture(&repo, c, side);
            assert!(p.is_file(), "{}: missing {}", c.name, p.display());
        }
    }
}

#[test]
fn shading_features_match_blender() {
    let repo = Repo::locate();
    let bless = std::env::var("FRAP_RENDER_BLESS").is_ok_and(|v| v == "1");
    let keep = std::env::var("FRAP_RENDER_KEEP").ok();
    let only = std::env::var("FRAP_RENDER_CASE").ok();
    let mut failures = vec![];
    for case in cases(&repo) {
        if only.as_deref().is_some_and(|o| o != case.name) {
            continue;
        }
        let work = ScratchDir::new(&format!("eevee_features_{}", case.name)).unwrap();
        let out = match render_frap(&case, work.path()) {
            Ok(Some(p)) => p,
            Ok(None) => {
                eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter");
                return;
            }
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        if let Some(dir) = &keep {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::copy(&out, Path::new(dir).join(format!("{}.png", case.name)));
        }
        if bless {
            let dst = fixture(&repo, &case, "frap");
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(&out, &dst).unwrap();
            if let Err(e) = bless_blender(&repo, &case) {
                failures.push(e);
            }
        }
        let ours = match image_ssim::load_png(&out) {
            Ok(i) => i,
            Err(e) => {
                failures.push(format!("{}: {e}", case.name));
                continue;
            }
        };
        for (side, min, max_de) in [
            ("blender", case.min_ssim, case.max_delta_e),
            ("frap", SELF_MIN_SSIM, SELF_MAX_DELTA_E),
        ] {
            let path = fixture(&repo, &case, side);
            let reference = match image_ssim::load_png(&path) {
                Ok(i) => i,
                Err(e) => {
                    failures.push(format!("{}: {e}", case.name));
                    continue;
                }
            };
            if let Ok(sc) = image_ssim::score(&ours, &reference) {
                eprintln!(
                    "{}: vs {side}: SSIM {:.4} (gate {min}), mean ΔE {:.3} (gate {max_de})",
                    case.name, sc.ssim, sc.mean_delta_e
                );
            }
            for m in image_ssim::compare(
                &format!("{AREA}/{} vs {}", case.name, path.display()),
                &ours,
                &reference,
                min,
                max_de,
            ) {
                failures.push(m.to_string());
            }
        }
    }
    assert!(
        failures.is_empty(),
        "shading feature goldens:\n{}",
        failures.join("\n")
    );
}

#[test]
fn source_transforms() {
    let t = "a\n#@render bpy.ops.render.render(write_still=True)\n#@blender x\n#@frap y = 1";
    assert_eq!(
        frap_source(t),
        "a\nbpy.ops.render.render(write_still=True)\n\ny = 1"
    );
    assert_eq!(
        blender_source(t),
        "a\nbpy.ops.render.render(write_still=True)\n#@blender x\n#@frap y = 1"
    );
    let d = "# render-golden: image=out/a.png min_ssim=0.9\n";
    assert_eq!(directive(d, "image=").as_deref(), Some("out/a.png"));
    assert_eq!(directive(d, "min_ssim=").as_deref(), Some("0.9"));
    assert_eq!(directive(d, "max_delta_e="), None);
}
