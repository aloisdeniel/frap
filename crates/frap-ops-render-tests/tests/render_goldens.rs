// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench render goldens (WP 1.24, ARCHITECTURE §20.1 "Render", §20.4 `image_ssim`).
//!
//! Every case of `tests/golden/cases/render/m1/` with a `# render-golden: image=<path>` line is
//! a render golden. Its render statements are `#@render` lines: comments for the generic golden
//! suite (`cargo test -p frap-golden`, which compares the scene the case builds with Blender's
//! and must run without a GPU), statements here. This test runs the case through frap with the
//! render lines and compares the written image with
//!
//! - `tests/golden/fixtures/render/m1/<case>.frap.png`, the frap-blessed self-golden:
//!   SSIM ≥ 0.995 and mean ΔE ≤ 2 in the centre crop, **gating** on every CI adapter (gpu.yml:
//!   Metal, lavapipe, WARP);
//! - `tests/golden/fixtures/render/m1/<case>.blender.png`, Blender 5.2.4's Workbench render of
//!   the same case: the SSIM is **reported** (it gates from M2, WP 2.3, at 0.97).
//!
//! Without an adapter the test fails, or prints a notice and passes under
//! `FRAP_GPU_OPTIONAL=1` (ci.yml's GPU-less test job).
//!
//! Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests --test render_goldens --
//! --nocapture` rewrites the `.frap.png` files from this machine's render and, when Blender
//! 5.2 is found (`$BLENDER_BIN`, `PATH`, `~/Applications/Blender-5.2.app`), the `.blender.png`
//! files through the golden runner (`tools/blender/golden_runner.py`, VIEW_3D override and all).

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_golden::compare::image_ssim::{
    self, DEFAULT_MAX_DELTA_E, SELF_MIN_SSIM, WORKBENCH_MIN_SSIM,
};
use frap_session::{Session, SessionOptions};

/// Case area of the M1 render goldens.
const AREA: &str = "render/m1";
/// Bless switch.
const BLESS_ENV: &str = "FRAP_RENDER_BLESS";
/// Directive naming the rendered image (case-relative output path).
const DIRECTIVE: &str = "# render-golden:";
/// Prefix of render-only statements.
const RENDER_LINE: &str = "#@render";

/// One render golden.
struct RenderCase {
    name: String,
    text: String,
    image: String,
}

impl RenderCase {
    fn id(&self) -> String {
        format!("{AREA}/{}", self.name)
    }
}

fn render_cases(repo: &Repo) -> Vec<RenderCase> {
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
            let image = text.lines().find_map(|l| {
                l.strip_prefix(DIRECTIVE)?
                    .split_whitespace()
                    .find_map(|kv| kv.strip_prefix("image=").map(str::to_owned))
            })?;
            Some(RenderCase {
                name: p.file_stem().unwrap().to_string_lossy().into_owned(),
                text,
                image,
            })
        })
        .collect()
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
        .join(AREA)
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
}

#[test]
fn at_least_three_workbench_scenes() {
    let repo = Repo::locate();
    let cases = render_cases(&repo);
    assert!(cases.len() >= 3, "{} render goldens", cases.len());
    for c in &cases {
        assert!(
            c.text.contains("render.engine = 'BLENDER_WORKBENCH'"),
            "{}: render goldens use Workbench",
            c.id()
        );
        assert!(c.text.contains(RENDER_LINE), "{}: no #@render line", c.id());
        // Committed self-golden and Blender reference.
        for side in ["frap", "blender"] {
            let p = fixture(&repo, c, side);
            assert!(p.is_file(), "{}: missing {}", c.id(), p.display());
        }
    }
}

#[test]
fn self_goldens() {
    let repo = Repo::locate();
    let bless = std::env::var(BLESS_ENV).is_ok_and(|v| v == "1");
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
        if let Ok(keep) = std::env::var("FRAP_RENDER_KEEP") {
            let _ = std::fs::create_dir_all(&keep);
            let _ = std::fs::copy(
                &out,
                std::path::Path::new(&keep).join(format!("{}.png", case.name)),
            );
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
        // Reported, not gating (§22.3 WP 1.24; gating from M2).
        match image_ssim::load_png(&fixture(&repo, &case, "blender"))
            .and_then(|b| image_ssim::score(&frap, &b))
        {
            Ok(s) => eprintln!(
                "{}: vs Blender 5.2.4 Workbench: SSIM {:.4} (M2 gate {WORKBENCH_MIN_SSIM}), \
                 mean ΔE {:.3} (centre crop)",
                case.id(),
                s.ssim,
                s.mean_delta_e
            ),
            Err(e) => eprintln!("{}: vs Blender: not compared: {e}", case.id()),
        }
    }
    assert!(
        failures.is_empty(),
        "render goldens:\n{}",
        failures.join("\n")
    );
}
