// SPDX-License-Identifier: GPL-3.0-or-later
//! Animation render goldens (WP 6.6, ARCHITECTURE §19.4): every case of
//! `tests/golden/cases/render/animation/` with a `# render-animation:` line runs here with its
//! `#@render` lines as statements (the generic golden suite, `cargo test -p frap-golden`, runs
//! the same cases without them: it has no GPU), on both sides:
//!
//! - frap through the golden runner (`frap_golden::runner::run_frap`, the same dump), Blender
//!   5.2.4 through `golden_runner.py`; the two states are compared with the case's comparators
//!   (`frap_golden::runner::compare`): every line's outcome, render lines included (errors, and
//!   the strerror of `Render error (…) cannot save` and of `Error writing render result, …`
//!   normalised, it is the platform's), and what `compare=` dumps (`frame_current` and the
//!   scene range are moved into the cursor);
//! - `files`: every file the case writes under `out/` (relative path, empty or not) equals the
//!   list Blender wrote (`tests/golden/fixtures/render/animation/<case>/files.txt`);
//! - `images=a,b,…`: those files against Blender's (`…/<case>/<file name>`): SSIM ≥ 0.95 and
//!   mean ΔE ≤ 2 in the centre crop (Workbench renders);
//! - `needs=animation`: the case needs animation evaluation (WP 6.4); until a frap session
//!   applies an F-curve at a frame change its images are not compared (with a notice).
//!
//! Without an adapter the test fails, or prints a notice and passes under `FRAP_GPU_OPTIONAL=1`.
//! Blessing: `FRAP_RENDER_BLESS=1 cargo test -p frap-ops-render-tests --test render_animation
//! -- --nocapture` rewrites the fixtures through Blender 5.2 when it is found.

mod common;

use std::path::{Path, PathBuf};

use common::Env;
use frap_golden::blender::{Repo, ScratchDir, run_blender};
use frap_golden::case::Case;
use frap_golden::compare::image_ssim::{self, DEFAULT_MAX_DELTA_E};
use frap_golden::runner::{Props, RunOptions, compare, run_frap};
use frap_golden::{GoldenState, LineOutcome};

const AREA: &str = "render/animation";
const DIRECTIVE: &str = "# render-animation:";
const RENDER_LINE: &str = "#@render";
const MIN_SSIM: f64 = 0.95;
const STATE_FIXTURE: &str = "blender.json";
const FILES_FIXTURE: &str = "files.txt";

struct AnimCase {
    name: String,
    path: PathBuf,
    /// The case with its render lines as statements (both sides run this).
    text: String,
    files: bool,
    needs_animation: bool,
    images: Vec<String>,
}

fn strip<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

fn cases(repo: &Repo) -> Vec<AnimCase> {
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
            let raw = std::fs::read_to_string(&p).unwrap();
            let directive = raw.lines().find_map(|l| l.strip_prefix(DIRECTIVE))?;
            let mut c = AnimCase {
                name: p.file_stem().unwrap().to_string_lossy().into_owned(),
                path: p.clone(),
                text: raw
                    .lines()
                    .map(|l| strip(l, RENDER_LINE).unwrap_or(l))
                    .collect::<Vec<_>>()
                    .join("\n"),
                files: false,
                needs_animation: false,
                images: Vec::new(),
            };
            for word in directive.split_whitespace() {
                match word.split_once('=') {
                    None if word == "files" => c.files = true,
                    Some(("needs", "animation")) => c.needs_animation = true,
                    Some(("images", list)) => {
                        c.images = list.split(',').map(str::to_owned).collect();
                    }
                    _ => panic!("{}: unknown directive word {word:?}", c.name),
                }
            }
            Some(c)
        })
        .collect()
}

fn fixture_dir(repo: &Repo, case: &AnimCase) -> PathBuf {
    repo.fixtures_dir().join(AREA).join(&case.name)
}

/// Every file under `<work>/out`, as `<relative path> <empty|data>` lines, sorted.
fn written_files(work: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy();
                let rel = rel.replace('\\', "/");
                let len = e.metadata().map_or(0, |m| m.len());
                out.push(format!("{rel} {}", if len == 0 { "empty" } else { "data" }));
            }
        }
    }
    let mut out = Vec::new();
    walk(&work.join("out"), work, &mut out);
    out.sort();
    out
}

/// `Render error (<strerror>) cannot save` names the platform's strerror (`Undefined error:
/// 0` on macOS, `Success` on Linux), and so does the OpenEXR writer's `Error writing render
/// result, <strerror> (see console)` (opening a file below a regular file is `Not a directory`
/// on POSIX, `No such file or directory` on Windows): normalised on both sides.
fn normalise_lines(lines: &mut [LineOutcome]) {
    for l in lines {
        if let Some(m) = l.message.as_mut() {
            for (open, close) in [
                ("Render error (", ") cannot save"),
                ("Error writing render result, ", " (see console)"),
            ] {
                if let Some(i) = m.find(open)
                    && let Some(j) = m[i..].find(close)
                {
                    m.replace_range(i + open.len()..i + j, "…");
                }
            }
        }
    }
}

fn parse(case: &AnimCase) -> Case {
    Case::parse(&case.path, &case.text).unwrap_or_else(|e| panic!("{}: {e:?}", case.name))
}

fn bless_blender(repo: &Repo, case: &AnimCase) -> Result<(), String> {
    let Some(blender) = frap_golden::locate_blender() else {
        eprintln!(
            "{}: Blender 5.2 not found; fixtures not refreshed",
            case.name
        );
        return Ok(());
    };
    let work = ScratchDir::new(&format!("bless_anim_{}", case.name)).map_err(|e| e.to_string())?;
    let case_file = work.path().join(format!("{}.py", case.name));
    std::fs::write(&case_file, &case.text).map_err(|e| e.to_string())?;
    let run = work.path().join("run");
    let state = run_blender(&blender, repo, &case_file, &run).map_err(|e| e.to_string())?;
    let dir = fixture_dir(repo, case);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(STATE_FIXTURE), json + "\n").map_err(|e| e.to_string())?;
    if case.files {
        let files = written_files(&run).join("\n") + "\n";
        std::fs::write(dir.join(FILES_FIXTURE), files).map_err(|e| e.to_string())?;
    }
    for f in &case.images {
        let dst = dir.join(Path::new(f).file_name().unwrap());
        std::fs::copy(run.join(f), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    }
    eprintln!("blessed {}", dir.display());
    Ok(())
}

/// Whether frap evaluates animation (WP 6.4): an F-curve on the Cube's X location is applied
/// when the frame changes and the view layer updates.
fn animation_evaluates() -> bool {
    let mut e = Env::new("render_anim_probe");
    let src = "bpy.data.actions.new('P')
bpy.data.actions['P'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['P'].layers.new('Layer')
bpy.data.actions['P'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['P'].layers[0].strips[0].channelbag(bpy.data.actions['P'].slots[0], ensure=True)
bpy.data.actions['P'].layers[0].strips[0].channelbags[0].fcurves.new('location', index=0)
bpy.data.actions['P'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, 0.0)
bpy.data.actions['P'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(5, 4.0)
bpy.data.objects['Cube'].animation_data_create()
bpy.data.objects['Cube'].animation_data.action = bpy.data.actions['P']
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['P'].slots[0]
bpy.context.scene.frame_current = 5
bpy.context.view_layer.update()";
    if e.session.exec_source(src).iter().any(|r| !r.ok()) {
        return false;
    }
    let x = e.run("bpy.data.objects['Cube'].location[0]")["value"].as_f64();
    x.is_some_and(|x| (x - 4.0).abs() < 1e-4)
}

/// Whether a render works here (`None`: skipped without an adapter).
fn gpu_present() -> bool {
    let mut e = Env::new("render_anim_gpu");
    e.ok("bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 8
bpy.context.scene.render.resolution_y = 8");
    e.render("bpy.ops.render.render()").is_some()
}

fn compare_image(what: &str, ours: &Path, theirs: &Path) -> Option<String> {
    let load = |p: &Path| frap_imbuf::load(p).map_err(|e| format!("{}: {e:?}", p.display()));
    let (a, b) = match (load(ours), load(theirs)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => return Some(format!("{what}: {:?} / {:?}", a.err(), b.err())),
    };
    if (a.width, a.height) != (b.width, b.height) {
        return Some(format!(
            "{what}: {}x{}, Blender's {}x{}",
            a.width, a.height, b.width, b.height
        ));
    }
    match image_ssim::score(&a, &b) {
        Ok(s) => {
            eprintln!(
                "{what}: SSIM {:.4}, mean ΔE {:.3} (centre crop)",
                s.ssim, s.mean_delta_e
            );
            (s.ssim < MIN_SSIM || s.mean_delta_e > DEFAULT_MAX_DELTA_E).then(|| {
                format!(
                    "{what}: SSIM {:.4} (gate {MIN_SSIM}), mean ΔE {:.3} (gate \
                     {DEFAULT_MAX_DELTA_E})",
                    s.ssim, s.mean_delta_e
                )
            })
        }
        Err(e) => Some(format!("{what}: {e}")),
    }
}

/// Runs one case through frap and compares it with Blender's fixtures (the images only with
/// `images`); returns the failures.
fn check_case(repo: &Repo, props: &Props, case: &AnimCase, images: bool) -> Vec<String> {
    let mut fails = Vec::new();
    let dir = fixture_dir(repo, case);
    let golden = parse(case);
    let work = ScratchDir::new(&format!("anim_{}", case.name)).unwrap();
    let run = run_frap(
        &golden,
        props,
        &RunOptions {
            workdir: work.path().to_owned(),
            strict_rna: false,
        },
    );
    let mut ours = match run {
        Ok(r) => r.state,
        Err(e) => return vec![format!("{}: frap: {e}", case.name)],
    };
    let mut theirs: GoldenState = match std::fs::read_to_string(dir.join(STATE_FIXTURE)) {
        Ok(t) => serde_json::from_str(&t).unwrap(),
        Err(e) => return vec![format!("{}: no Blender fixture: {e}", case.name)],
    };
    normalise_lines(&mut ours.lines);
    normalise_lines(&mut theirs.lines);
    for m in compare(&golden, &ours, &theirs) {
        fails.push(format!("{}: {m}", case.name));
    }
    if case.files {
        let expected: Vec<String> = std::fs::read_to_string(dir.join(FILES_FIXTURE))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        let got = written_files(work.path());
        if got != expected {
            fails.push(format!(
                "{}: files written\n  frap:    {got:?}\n  Blender: {expected:?}",
                case.name
            ));
        }
    }
    for f in case.images.iter().filter(|_| images) {
        let theirs = dir.join(Path::new(f).file_name().unwrap());
        fails.extend(compare_image(
            &format!("{}/{f}", case.name),
            &work.path().join(f),
            &theirs,
        ));
    }
    fails
}

#[test]
fn cases_and_fixtures() {
    let repo = Repo::locate();
    let cs = cases(&repo);
    assert!(cs.len() >= 3, "{} render/animation cases", cs.len());
    for c in &cs {
        assert!(
            c.text.contains("render.engine = 'BLENDER_WORKBENCH'"),
            "{}",
            c.name
        );
        let dir = fixture_dir(&repo, c);
        assert!(dir.join(STATE_FIXTURE).is_file(), "{}: unblessed", c.name);
        if c.files {
            assert!(
                dir.join(FILES_FIXTURE).is_file(),
                "{}: no files.txt",
                c.name
            );
        }
        for f in &c.images {
            let p = dir.join(Path::new(f).file_name().unwrap());
            assert!(p.is_file(), "{}", p.display());
        }
        parse(c);
    }
    // Every image the animated case compares is a different picture in Blender's render: the
    // frames really differ, so a frame rendered with another frame's animation fails.
    let animated = cs
        .iter()
        .find(|c| c.name == "animated")
        .expect("animated case");
    let dir = fixture_dir(&repo, animated);
    let imgs: Vec<_> = animated
        .images
        .iter()
        .map(|f| frap_imbuf::load(&dir.join(Path::new(f).file_name().unwrap())).unwrap())
        .collect();
    for i in 0..imgs.len() {
        for j in i + 1..imgs.len() {
            let s = image_ssim::score(&imgs[i], &imgs[j]).unwrap();
            assert!(
                s.ssim < 0.9,
                "{} and {} look alike (SSIM {:.3})",
                animated.images[i],
                animated.images[j],
                s.ssim
            );
        }
    }
}

#[test]
fn animation_renders_match_blender() {
    let repo = Repo::locate();
    let props = Props::load(&repo.props_path()).unwrap();
    let bless = std::env::var("FRAP_RENDER_BLESS").is_ok_and(|v| v == "1");
    if !gpu_present() {
        return;
    }
    let animation = animation_evaluates();
    let mut fails = Vec::new();
    for case in cases(&repo) {
        if bless && let Err(e) = bless_blender(&repo, &case) {
            fails.push(e);
            continue;
        }
        let images = !case.needs_animation || animation;
        if !images {
            eprintln!(
                "SKIP {} images: frap does not evaluate animation yet (WP 6.4); lines, files \
                 and dumps are still compared",
                case.name
            );
        }
        fails.extend(check_case(&repo, &props, &case, images));
    }
    assert!(fails.is_empty(), "animation renders:\n{}", fails.join("\n"));
}

/// Frames render from copies: after `animation=True` the session is as before except
/// `Render Result` (D-07; `frame_current` as in Blender, which restores `cfra`).
#[test]
fn frames_render_from_copies() {
    let mut e = Env::new("render_anim_copies");
    e.ok("bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 32
bpy.context.scene.render.resolution_y = 18
bpy.context.scene.frame_start = 3
bpy.context.scene.frame_end = 6
bpy.context.scene.frame_step = 2
bpy.context.scene.frame_current = 9
bpy.context.scene.render.filepath = 'out/c_'");
    // A first render creates `Render Result`; the second must change nothing else.
    let Some(j) = e.render("bpy.ops.render.render()") else {
        return;
    };
    assert_eq!(j["ok"], true, "{j}");
    let before = common::state(&e.session);
    let j = e.render("bpy.ops.render.render(animation=True)").unwrap();
    assert_eq!(j["ok"], true, "{j}");
    assert_eq!(
        j["result"]["files"],
        serde_json::json!(["out/c_0003.png", "out/c_0005.png"])
    );
    assert_eq!(j["result"]["frames"], 2);
    let (main, rest) = common::state(&e.session);
    // `Render Result` has a new generation (its pixels changed); nothing else differs.
    let strip_gen = |s: String| {
        s.split("generation: ")
            .enumerate()
            .map(|(i, part)| {
                if i == 0 {
                    part.to_owned()
                } else {
                    part.trim_start_matches(|c: char| c.is_ascii_digit())
                        .to_owned()
                }
            })
            .collect::<String>()
    };
    common::assert_same_state(
        &strip_gen(format!("{main:?}")),
        &strip_gen(format!("{:?}", before.0)),
        "animation render",
    );
    common::assert_same_state(&rest, &before.1, "animation render");
    assert_eq!(e.run("bpy.context.scene.frame_current")["value"], 9);
}

/// `use_overwrite` off, every frame skipped: success with the INFO report, nothing rendered,
/// `skipped` lists the files; `use_placeholder` leaves no empty file after a failed frame.
#[test]
fn skipped_frames_and_placeholders() {
    let mut e = Env::new("render_anim_skip");
    e.ok("bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 16
bpy.context.scene.render.resolution_y = 8
bpy.context.scene.render.filepath = 'out/s_'");
    let Some(j) = e.render("bpy.ops.render.render(animation=True, frame_start=1, frame_end=2)")
    else {
        return;
    };
    assert_eq!(j["ok"], true, "{j}");
    e.ok("bpy.context.scene.render.use_overwrite = False");
    let j = e.run("bpy.ops.render.render(animation=True, frame_start=1, frame_end=3)");
    assert_eq!(j["result"]["frames"], 1, "{j}");
    assert_eq!(j["result"]["files"], serde_json::json!(["out/s_0003.png"]));
    assert_eq!(
        j["result"]["skipped"],
        serde_json::json!(["out/s_0001.png", "out/s_0002.png"])
    );
    assert!(common::reports(&j).is_empty(), "{j}");
    let j = e.run("bpy.ops.render.render(animation=True, frame_start=1, frame_end=3)");
    assert_eq!(j["ok"], true, "{j}");
    assert_eq!(j["result"]["frames"], 0, "{j}");
    assert_eq!(j["result"]["resolution"], serde_json::json!([16, 8]));
    assert!(j["result"].get("files").is_none(), "{j}");
    assert_eq!(
        common::reports(&j),
        [(
            "INFO".to_owned(),
            "No frames rendered, skipped to not overwrite".to_owned()
        )]
    );
    // A failing frame (one-channel WebP) leaves no placeholder.
    e.ok("bpy.context.scene.render.use_placeholder = True
bpy.context.scene.render.image_settings.file_format = 'WEBP'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/w/x_'");
    let j = e.run("bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)");
    assert_eq!(j["error"]["exception"], "RuntimeError", "{j}");
    assert!(
        j["error"]["message"]
            .as_str()
            .unwrap()
            .ends_with(") cannot save: 'out/w/x_0001.webp'"),
        "{j}"
    );
    assert!(!e.path("out/w/x_0001.webp").exists());
    assert!(
        e.path("out/w").is_dir(),
        "the placeholder's directory stays"
    );
}
