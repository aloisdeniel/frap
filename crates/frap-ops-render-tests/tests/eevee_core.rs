// SPDX-License-Identifier: GPL-3.0-or-later
//! EEVEE core render goldens (WP 5.1, ARCHITECTURE §22.7): every case of
//! `tests/golden/cases/render_eevee/core/` with a
//! `# render-golden: image=<path>.exr probes=x,y;x,y… tol=<rel> [shadow_iou=<x>] [channels=rgb]`
//! line. The case is run through frap with its `#@render` lines, and the linear EXR it writes
//! is compared with Blender 5.2.4's EEVEE render of the same case
//! (`tests/golden/fixtures/render_eevee/core/<case>.blender.exr`):
//!
//! - **light units**: the radiance at every probe (mean of the 3 x 3 pixels around `x, y`, top
//!   row 0; red channel, or R, G and B with `channels=rgb`) within `tol` (relative, ±5% for the
//!   diffuse-plane goldens) plus 0.002 absolute;
//! - **shadow placement** (`shadow_iou=`): the shadow masks (pixels darker than 0.1% of the
//!   image maximum) of frap and Blender overlap with an intersection over union of at least
//!   `x`;
//! - the frap-blessed `<case>.frap.exr` stays within 2% at the probes (adapter drift alarm).
//!
//! Without an adapter the test fails, or prints a notice and passes under
//! `FRAP_GPU_OPTIONAL=1`. Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests
//! --test eevee_core -- --nocapture` rewrites the `.frap.exr` files and, when Blender 5.2 is
//! found, the `.blender.exr` files through the golden runner.

mod common;

use std::path::{Path, PathBuf};

use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_imbuf::{DecodedImage, PixelData};
use frap_session::{Session, SessionOptions};

/// Case area.
const AREA: &str = "render_eevee/core";
const DIRECTIVE: &str = "# render-golden:";
const RENDER_LINE: &str = "#@render";
/// Relative probe tolerance against the frap self-golden.
const SELF_TOL: f32 = 0.02;
/// Absolute floor of every probe tolerance.
const ABS_TOL: f32 = 0.002;

struct Case {
    name: String,
    text: String,
    image: String,
    probes: Vec<[u32; 2]>,
    tol: f32,
    shadow_iou: Option<f64>,
    rgb: bool,
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
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            let image = directive(&text, "image=")?;
            Some(Case {
                name: p.file_stem().unwrap().to_string_lossy().into_owned(),
                probes: parse_probes(&directive(&text, "probes=").expect("probes=")),
                tol: directive(&text, "tol=")
                    .map(|s| s.parse().expect("tol= is a number"))
                    .unwrap_or(0.05),
                shadow_iou: directive(&text, "shadow_iou=").map(|s| s.parse().expect("number")),
                rgb: directive(&text, "channels=").is_some_and(|c| c == "rgb"),
                image,
                text,
            })
        })
        .collect()
}

fn strip_prefix_line<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

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

fn blender_case_text(text: &str) -> String {
    text.lines()
        .map(|l| strip_prefix_line(l, RENDER_LINE).unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fixture(repo: &Repo, case: &Case, side: &str) -> PathBuf {
    repo.fixtures_dir()
        .join(AREA)
        .join(format!("{}.{side}.exr", case.name))
}

/// A float image's RGBA rows (top row first).
struct Img {
    w: u32,
    h: u32,
    px: Vec<[f32; 4]>,
}

fn load(path: &Path) -> Result<Img, String> {
    let d: DecodedImage =
        frap_imbuf::load(path).map_err(|e| format!("{}: {e:?}", path.display()))?;
    let ch = d.channels as usize;
    let PixelData::F32(v) = &d.pixels else {
        return Err(format!("{}: not a float image", path.display()));
    };
    let px = v
        .chunks_exact(ch)
        .map(|c| match ch {
            1 => [c[0], c[0], c[0], 1.0],
            3 => [c[0], c[1], c[2], 1.0],
            _ => [c[0], c[1], c[2], c[3]],
        })
        .collect();
    Ok(Img {
        w: d.width,
        h: d.height,
        px,
    })
}

impl Img {
    fn at(&self, x: i64, y: i64) -> [f32; 4] {
        let x = x.clamp(0, i64::from(self.w) - 1) as u32;
        let y = y.clamp(0, i64::from(self.h) - 1) as u32;
        self.px[(y * self.w + x) as usize]
    }
    /// Mean of the 3 x 3 pixels around `p`.
    fn probe(&self, p: [u32; 2]) -> [f32; 3] {
        let mut s = [0.0f32; 3];
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = self.at(i64::from(p[0]) + dx, i64::from(p[1]) + dy);
                for i in 0..3 {
                    s[i] += c[i] / 9.0;
                }
            }
        }
        s
    }
    fn shadow_mask(&self) -> Vec<bool> {
        let max = self
            .px
            .iter()
            .map(|p| p[0].max(p[1]).max(p[2]))
            .fold(0.0f32, f32::max);
        self.px
            .iter()
            .map(|p| p[0].max(p[1]).max(p[2]) < 1e-3 * max)
            .collect()
    }
}

/// Probe mismatches of `got` against `want` (empty: within tolerance).
fn compare_probes(label: &str, case: &Case, got: &Img, want: &Img, tol: f32) -> Vec<String> {
    let mut out = vec![];
    if (got.w, got.h) != (want.w, want.h) {
        return vec![format!(
            "{label}: size {}x{} vs {}x{}",
            got.w, got.h, want.w, want.h
        )];
    }
    let channels = if case.rgb { 3 } else { 1 };
    for p in &case.probes {
        let (g, w) = (got.probe(*p), want.probe(*p));
        for c in 0..channels {
            if (g[c] - w[c]).abs() > tol * w[c].abs() + ABS_TOL {
                out.push(format!(
                    "{label}: probe {p:?} channel {c}: frap {:.5} vs {:.5} (tol {tol})",
                    g[c], w[c]
                ));
            }
        }
    }
    out
}

fn iou(a: &[bool], b: &[bool]) -> f64 {
    let inter = a.iter().zip(b).filter(|(x, y)| **x && **y).count();
    let union = a.iter().zip(b).filter(|(x, y)| **x || **y).count();
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
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
    // The engine that rendered: EEVEE, not the Workbench fallback.
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
            "{}: Blender 5.2 not found; .blender.exr not refreshed",
            case.name
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
fn directives_parse() {
    let d = "# render-golden: image=out/a.exr probes=1,2;3,4 tol=0.05 shadow_iou=0.8\n";
    assert_eq!(directive(d, "image=").as_deref(), Some("out/a.exr"));
    assert_eq!(
        parse_probes(&directive(d, "probes=").unwrap()),
        vec![[1, 2], [3, 4]]
    );
    assert_eq!(directive(d, "shadow_iou=").as_deref(), Some("0.8"));
    assert_eq!(iou(&[true, false, true], &[true, true, false]), 1.0 / 3.0);
}

#[test]
fn core_cases_and_fixtures() {
    let repo = Repo::locate();
    let cases = cases(&repo);
    // WP 5.1 acceptance: a light-unit golden per light type and shadow-placement goldens.
    for want in [
        "light_units_point",
        "light_units_sun",
        "light_units_spot",
        "light_units_area",
        "shadows_sun",
        "shadows_point",
        "shadows_spot",
    ] {
        assert!(cases.iter().any(|c| c.name == want), "missing case {want}");
    }
    for c in &cases {
        assert!(
            c.text.contains("render.engine = 'BLENDER_EEVEE'"),
            "{}",
            c.name
        );
        assert!(c.text.contains(RENDER_LINE), "{}: no #@render line", c.name);
        assert!(c.image.ends_with(".exr"), "{}: linear output", c.name);
        if c.name.starts_with("light_units") {
            assert!(c.tol <= 0.05, "{}: light units gate at ±5%", c.name);
        }
        for side in ["frap", "blender"] {
            let p = fixture(&repo, c, side);
            assert!(p.is_file(), "{}: missing {}", c.name, p.display());
        }
    }
}

#[test]
fn eevee_core_goldens() {
    let repo = Repo::locate();
    let bless = std::env::var("FRAP_RENDER_BLESS").is_ok_and(|v| v == "1");
    let keep = std::env::var("FRAP_RENDER_KEEP").ok();
    let mut failures = Vec::new();
    for case in cases(&repo) {
        let work = ScratchDir::new(&case.name).unwrap();
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
            let _ = std::fs::copy(&out, Path::new(dir).join(format!("{}.exr", case.name)));
        }
        if bless {
            let dst = fixture(&repo, &case, "frap");
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(&out, &dst).unwrap();
            if let Err(e) = bless_blender(&repo, &case) {
                failures.push(e);
            }
        }
        let frap = match load(&out) {
            Ok(i) => i,
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        match load(&fixture(&repo, &case, "blender")) {
            Ok(b) => {
                let probes: Vec<String> = case
                    .probes
                    .iter()
                    .map(|p| format!("{:.4}/{:.4}", frap.probe(*p)[0], b.probe(*p)[0]))
                    .collect();
                eprintln!(
                    "{}: frap/Blender at the probes: {}",
                    case.name,
                    probes.join(" ")
                );
                failures.extend(compare_probes(
                    &format!("{} vs Blender 5.2.4", case.name),
                    &case,
                    &frap,
                    &b,
                    case.tol,
                ));
                if let Some(min) = case.shadow_iou {
                    let s = iou(&frap.shadow_mask(), &b.shadow_mask());
                    eprintln!("{}: shadow IoU {s:.3} (gate {min})", case.name);
                    if s < min {
                        failures.push(format!("{}: shadow IoU {s:.3} < {min}", case.name));
                    }
                }
            }
            Err(e) => failures.push(format!("{}: Blender reference: {e}", case.name)),
        }
        match load(&fixture(&repo, &case, "frap")) {
            Ok(g) => failures.extend(compare_probes(
                &format!("{} vs {}.frap.exr", case.name, case.name),
                &case,
                &frap,
                &g,
                SELF_TOL,
            )),
            Err(e) => failures.push(format!("{}: self-golden: {e}", case.name)),
        }
    }
    assert!(
        failures.is_empty(),
        "EEVEE core goldens:\n{}",
        failures.join("\n")
    );
}
