// SPDX-License-Identifier: GPL-3.0-or-later
//! `render.opengl` (Viewport Render, WP 8.3.6, headless-extended D-160) at T4 against Blender's
//! `-b` Workbench output, and its error paths.
//!
//! Every case of `tests/golden/cases/render/opengl/` names the image it writes with
//! `# render-golden: image=<path> [min_ssim=<x>]`. Its render statements are comments for the
//! generic golden suite: `#@opengl` lines run in frap only (`render.opengl`), `#@opengl-blender`
//! lines in Blender only (Blender cannot run `render.opengl` in background mode, so its
//! reference is `render.render` with `BLENDER_WORKBENCH`). The image frap writes is compared
//! with
//!
//! - `tests/golden/fixtures/render/opengl/<case>.frap.png`, the frap-blessed self-golden (SSIM
//!   ≥ 0.995, mean ΔE ≤ 2 in the centre crop);
//! - `tests/golden/fixtures/render/opengl/<case>.blender.png`, Blender 5.2.4's Workbench render
//!   of the same scene (SSIM ≥ `min_ssim`, default 0.97, mean ΔE ≤ 2), gating.
//!
//! Without an adapter the test fails, or prints a notice and passes under
//! `FRAP_GPU_OPTIONAL=1`. Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests
//! --test render_opengl -- --nocapture` rewrites both PNGs (the Blender one through the golden
//! runner when Blender 5.2 is found).

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_golden::compare::image_ssim::{
    self, DEFAULT_MAX_DELTA_E, SELF_MIN_SSIM, WORKBENCH_MIN_SSIM,
};
use frap_session::{Session, SessionOptions};

/// Case area.
const AREA: &str = "render/opengl";
/// Bless switch.
const BLESS_ENV: &str = "FRAP_RENDER_BLESS";
/// Directive naming the rendered image.
const DIRECTIVE: &str = "# render-golden:";
/// frap-only render statements.
const FRAP_LINE: &str = "#@opengl";
/// Blender-only render statements.
const BLENDER_LINE: &str = "#@opengl-blender";

struct Case {
    name: String,
    text: String,
    image: String,
    min_ssim: f64,
}

fn directive(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix(DIRECTIVE)?
            .split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).map(str::to_owned))
    })
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
            let image = directive(&text, "image=")?;
            let min_ssim = directive(&text, "min_ssim=")
                .map(|s| s.parse().expect("min_ssim= is a number"))
                .unwrap_or(WORKBENCH_MIN_SSIM);
            Some(Case {
                name: p.file_stem().unwrap().to_string_lossy().into_owned(),
                text,
                image,
                min_ssim,
            })
        })
        .collect()
}

/// `line` without `prefix` (and one following space), when it starts with it.
fn body<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(prefix)?;
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// What frap runs: `#@opengl` and `#@frap` lines become statements, the Blender-only lines
/// blank (line numbers kept).
fn frap_source(text: &str) -> String {
    text.lines()
        .map(|l| {
            if let Some(b) = body(l, FRAP_LINE).or_else(|| body(l, "#@frap")) {
                b
            } else if body(l, BLENDER_LINE).is_some() || body(l, "#@blender").is_some() {
                ""
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The case Blender runs when blessing: `#@opengl-blender` lines become shared statements,
/// `#@opengl` lines blank.
fn blender_case_text(text: &str) -> String {
    text.lines()
        .map(|l| {
            if let Some(b) = body(l, BLENDER_LINE) {
                b
            } else if body(l, FRAP_LINE).is_some() {
                ""
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fixture(repo: &Repo, case: &Case, side: &str) -> PathBuf {
    repo.fixtures_dir()
        .join(AREA)
        .join(format!("{}.{side}.png", case.name))
}

/// Renders a case through frap; `Ok(None)`: no GPU (skipped).
fn render_frap(case: &Case, work: &Path) -> Result<Option<PathBuf>, String> {
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
    let work =
        ScratchDir::new(&format!("bless_opengl_{}", case.name)).map_err(|e| e.to_string())?;
    let case_file = work.path().join(format!("{}.py", case.name));
    std::fs::write(&case_file, blender_case_text(&case.text)).map_err(|e| e.to_string())?;
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
fn source_transforms() {
    let text = "import bpy\n#@opengl bpy.ops.render.opengl()\n#@opengl-blender x = 1\n#@frap y = 2\n#@blender z = 3";
    assert_eq!(
        frap_source(text),
        "import bpy\nbpy.ops.render.opengl()\n\ny = 2\n"
    );
    assert_eq!(
        blender_case_text(text),
        "import bpy\n\nx = 1\n#@frap y = 2\n#@blender z = 3"
    );
}

#[test]
fn cases_and_fixtures_exist() {
    let repo = Repo::locate();
    let all = cases(&repo);
    assert!(all.len() >= 2, "{} render.opengl goldens", all.len());
    for c in &all {
        assert!(
            c.text.contains("#@opengl bpy.ops.render.opengl("),
            "{}",
            c.name
        );
        assert!(
            c.text
                .contains("#@opengl-blender bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'"),
            "{}: Blender's reference is a Workbench render",
            c.name
        );
        assert!(c.min_ssim >= WORKBENCH_MIN_SSIM, "{}: min_ssim", c.name);
        for side in ["frap", "blender"] {
            let p = fixture(&repo, c, side);
            assert!(p.is_file(), "{}: missing {}", c.name, p.display());
        }
    }
}

#[test]
fn opengl_against_blender_workbench() {
    let repo = Repo::locate();
    let bless = std::env::var(BLESS_ENV).is_ok_and(|v| v == "1");
    let mut failures = Vec::new();
    for case in cases(&repo) {
        let work = ScratchDir::new(&format!("opengl_{}", case.name)).unwrap();
        let out = match render_frap(&case, work.path()) {
            Ok(Some(p)) => p,
            Ok(None) => {
                assert!(
                    common::gpu_optional(),
                    "no GPU adapter (set FRAP_GPU_OPTIONAL=1 to skip)"
                );
                eprintln!("render.opengl goldens skipped: no GPU adapter (FRAP_GPU_OPTIONAL=1)");
                return;
            }
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
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
            Ok(golden) => failures.extend(
                image_ssim::compare(
                    &format!("{} vs {}.frap.png", case.name, case.name),
                    &frap,
                    &golden,
                    SELF_MIN_SSIM,
                    DEFAULT_MAX_DELTA_E,
                )
                .into_iter()
                .map(|m| m.to_string()),
            ),
            Err(e) => failures.push(format!("{}: self-golden: {e}", case.name)),
        }
        match image_ssim::load_png(&fixture(&repo, &case, "blender")) {
            Ok(b) => {
                if let Ok(s) = image_ssim::score(&frap, &b) {
                    eprintln!(
                        "{}: render.opengl vs Blender 5.2.4 Workbench: SSIM {:.4} (gate {}), \
                         mean ΔE {:.3}",
                        case.name, s.ssim, case.min_ssim, s.mean_delta_e
                    );
                }
                failures.extend(
                    image_ssim::compare(
                        &format!("{} vs {}.blender.png", case.name, case.name),
                        &frap,
                        &b,
                        case.min_ssim,
                        DEFAULT_MAX_DELTA_E,
                    )
                    .into_iter()
                    .map(|m| m.to_string()),
                );
            }
            Err(e) => failures.push(format!("{}: Blender reference: {e}", case.name)),
        }
    }
    assert!(
        failures.is_empty(),
        "render.opengl goldens:\n{}",
        failures.join("\n")
    );
}

/// The checks run before anything renders, so they need no GPU.
#[test]
fn opengl_errors() {
    let mut env = common::Env::new("opengl_errors");
    let r = env.run("bpy.ops.render.opengl(sequencer=True)");
    assert_eq!(r["error"]["code"], "OP_NOT_IMPLEMENTED", "{r}");
    let r = env.run("bpy.ops.render.opengl(animation=True, render_keyed_only=True)");
    assert_eq!(r["error"]["code"], "OP_NOT_IMPLEMENTED", "{r}");
    env.ok("bpy.context.scene.render.image_settings.media_type = 'VIDEO'");
    let r = env.run("bpy.ops.render.opengl(write_still=True)");
    assert_eq!(
        r["error"]["message"],
        "Error: Cannot write a single file with an animation format selected",
        "{r}"
    );
    env.ok("bpy.context.scene.render.image_settings.media_type = 'IMAGE'");
    env.ok("bpy.context.scene.camera = None");
    let r = env.run("bpy.ops.render.opengl()");
    assert_eq!(r["error"]["message"], "Error: Scene has no camera", "{r}");
    assert_eq!(r["error"]["exception"], "RuntimeError", "{r}");
}
