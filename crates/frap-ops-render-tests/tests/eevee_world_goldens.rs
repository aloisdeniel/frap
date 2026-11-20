// SPDX-License-Identifier: GPL-3.0-or-later
//! EEVEE HDRI-world render goldens (WP 5.2; ARCHITECTURE §20.4 `image_ssim`, "EEVEE vs Blender
//! ≥ 0.90").
//!
//! Every case of `tests/golden/cases/render_eevee/world/` (an HDRI world: environment texture,
//! Mapping rotation, Background strength, the world sun) runs through a real `Session` with its
//! `#@render` lines, its `<case>.inputs/` files staged into the working directory, and the
//! written image is compared with Blender 5.2.4's EEVEE render
//! `tests/golden/fixtures/render_eevee/world/<case>.blender.png`: SSIM ≥ the case's `min_ssim`
//! and mean ΔE ≤ 2 in the centre crop.
//!
//! The world lighting itself (`frap_render::eevee::ibl`) is gated against the same fixtures by
//! its own tests (`cargo test -p frap-render --lib ibl`), which light the scenes with an IBL
//! preview. This test gates the whole engine: while `render.render` still reports that
//! `BLENDER_EEVEE` falls back to Workbench (the forward engine, WP 5.1, not merged), it prints a
//! notice and passes. Without an adapter it fails, or passes with a notice under
//! `FRAP_GPU_OPTIONAL=1`. `FRAP_RENDER_KEEP=<dir>` copies every render there.

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, stage_inputs};
use frap_golden::compare::image_ssim::{self, DEFAULT_MAX_DELTA_E, EEVEE_MIN_SSIM};
use frap_session::{Session, SessionOptions};

const AREA: &str = "render_eevee/world";
const DIRECTIVE: &str = "# render-golden:";

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

/// Runs in `dir` and restores the previous process directory when dropped, also on a panic
/// (workaround until `frap-render-scene` build/images.rs resolves relative paths against the
/// session's directory).
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &Path) -> CwdGuard {
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        CwdGuard(prev)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn cases(repo: &Repo) -> Vec<PathBuf> {
    let dir = repo.cases_dir().join(AREA);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    v.sort();
    v
}

#[test]
fn world_cases_are_render_goldens_with_blender_references() {
    let repo = Repo::locate();
    let cases = cases(&repo);
    assert!(cases.len() >= 4, "{} HDRI scenes", cases.len());
    for case in cases {
        let text = std::fs::read_to_string(&case).unwrap();
        let name = case.file_stem().unwrap().to_string_lossy().into_owned();
        assert!(
            text.contains("render.engine = 'BLENDER_EEVEE'"),
            "{name}: EEVEE"
        );
        assert!(text.contains("#@render "), "{name}: no #@render line");
        assert!(
            text.contains("ShaderNodeTexEnvironment"),
            "{name}: an HDRI world"
        );
        let min = directive(&text, "min_ssim=").map_or(EEVEE_MIN_SSIM, |s| s.parse().unwrap());
        assert!(min >= EEVEE_MIN_SSIM, "{name}: min_ssim {min}");
        assert!(case.with_extension("inputs").is_dir(), "{name}: no inputs");
        let png = repo
            .fixtures_dir()
            .join(AREA)
            .join(format!("{name}.blender.png"));
        assert!(png.is_file(), "missing {}", png.display());
    }
}

#[test]
fn hdri_worlds_match_blender_eevee() {
    let repo = Repo::locate();
    let keep = std::env::var("FRAP_RENDER_KEEP").ok();
    let mut failures = vec![];
    for case in cases(&repo) {
        let text = std::fs::read_to_string(&case).unwrap();
        let name = case.file_stem().unwrap().to_string_lossy().into_owned();
        let image = directive(&text, "image=").expect("image=");
        let min = directive(&text, "min_ssim=").map_or(EEVEE_MIN_SSIM, |s| s.parse().unwrap());
        let work = ScratchDir::new(&format!("eevee_world_{name}")).unwrap();
        stage_inputs(&case, work.path()).unwrap();
        let mut opts = SessionOptions::default();
        opts.cwd = work.path().to_owned();
        let mut s = Session::new(opts).unwrap();
        // The render build resolves relative image paths against the process directory, not
        // the session's (`frap-render-scene` build/images.rs): run from the work directory. No
        // other test of this binary depends on the process directory.
        let responses = {
            let _cwd = CwdGuard::enter(work.path());
            s.exec_source(&frap_source(&text))
        };
        if common::any_gpu_unavailable(&responses) {
            return;
        }
        if let Some(bad) = responses.iter().find(|r| !r.ok()) {
            failures.push(format!("{name}: frap failed: {}", bad.to_json()));
            continue;
        }
        let fallback = responses.iter().any(|r| {
            r.to_json()
                .to_string()
                .contains("is not available in frap yet")
        });
        if fallback {
            eprintln!(
                "NOTICE {name}: BLENDER_EEVEE still falls back to Workbench (WP 5.1 not merged); \
                 the world lighting is gated by frap-render's ibl tests"
            );
            continue;
        }
        let out = work.path().join(&image);
        if let Some(dir) = &keep {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::copy(&out, Path::new(dir).join(format!("{name}.png")));
        }
        let ours = match image_ssim::load_png(&out) {
            Ok(i) => i,
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        let png = repo
            .fixtures_dir()
            .join(AREA)
            .join(format!("{name}.blender.png"));
        let blender = image_ssim::load_png(&png).unwrap();
        if let Ok(sc) = image_ssim::score(&ours, &blender) {
            eprintln!(
                "{name}: vs Blender 5.2.4 EEVEE: SSIM {:.4} (gate {min}), mean ΔE {:.3}",
                sc.ssim, sc.mean_delta_e
            );
        }
        for m in image_ssim::compare(
            &format!("{AREA}/{name} vs {name}.blender.png"),
            &ours,
            &blender,
            min,
            DEFAULT_MAX_DELTA_E,
        ) {
            failures.push(m.to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "HDRI goldens:\n{}",
        failures.join("\n")
    );
}

#[test]
fn source_transform() {
    let t = "a\n#@render bpy.ops.render.render(write_still=True)\n#@blender x\n#@frap y = 1";
    assert_eq!(
        frap_source(t),
        "a\nbpy.ops.render.render(write_still=True)\n\ny = 1"
    );
}
