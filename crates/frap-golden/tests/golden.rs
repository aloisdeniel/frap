// SPDX-License-Identifier: GPL-3.0-or-later
//! The golden suite (ARCHITECTURE §20.5) and the harness's own consistency tests (WP 1.8).
//!
//! - `fixtures`: frap runs every case of `tests/golden/cases/**` and is compared with the
//!   committed Blender fixtures. Runs everywhere (no Blender needed). `FRAP_GOLDEN_FILTER`
//!   narrows the cases (glob over `<area>/<name>`). A native fallback of the frap dumper that
//!   is not in `crates/frap-golden/rna_fallbacks.txt` fails its case; `FRAP_GOLDEN_STRICT_RNA=1`
//!   turns every fallback into a failure.
//! - `live` (`#[ignore]`d, so `cargo test` lists it as ignored with the reason): `cargo xtask
//!   golden live` runs it with `FRAP_GOLDEN_LIVE=1 … --include-ignored`; Blender re-runs every
//!   case and must reproduce its fixture (drift alarm). `FRAP_GOLDEN_LIVE=1` without Blender
//!   fails instead of passing silently.

use std::collections::BTreeSet;

use frap_golden::blender::{self, Live, Repo, ScratchDir};
use frap_golden::case::Case;
use frap_golden::runner::{self, FallbackAllowList, Props, RunOptions};
use frap_golden::{GObject, LineOutcome};

fn filter() -> Option<String> {
    std::env::var("FRAP_GOLDEN_FILTER")
        .ok()
        .filter(|f| !f.is_empty())
}

#[test]
fn fixtures() {
    let repo = Repo::locate();
    let props = Props::load(&repo.props_path()).expect("props.toml");
    let cases = repo.cases(filter().as_deref()).expect("cases parse");
    let strict = std::env::var_os(runner::STRICT_RNA_ENV).is_some_and(|v| v == "1");
    let reports: Vec<_> = cases
        .iter()
        .map(|c| runner::check_against_fixture(&repo, &props, c, strict))
        .collect();
    let fallbacks: usize = reports.iter().map(|r| r.rna_fallbacks.len()).sum();
    eprintln!(
        "golden fixtures: {} cases, {} passed, {fallbacks} datablocks read natively (RNA not bound yet)",
        reports.len(),
        reports.iter().filter(|r| r.passed()).count()
    );
    if filter().is_none() {
        let stale = FallbackAllowList::load(&repo)
            .expect("rna_fallbacks.txt")
            .stale(&reports);
        if !stale.is_empty() {
            eprintln!(
                "NOTICE: stale entries in {} (their binding landed; delete them):\n  {}",
                runner::ALLOW_LIST_NAME,
                stale.join("\n  ")
            );
        }
    }
    let failed = runner::format_reports(&reports, 30);
    assert!(failed.is_empty(), "golden cases failed:{failed}");
}

#[test]
#[ignore = "live: needs Blender 5.2; run `cargo xtask golden live` (FRAP_GOLDEN_LIVE=1, --include-ignored)"]
fn live() {
    let b = match blender::live() {
        Live::Off => {
            eprintln!(
                "NOTICE: live golden tests off (set {}=1 or run `cargo xtask golden live`); \
                 fixture comparisons still run",
                blender::LIVE_ENV
            );
            return;
        }
        // Asked for explicitly: a missing Blender is a failure, never a silent pass.
        Live::Skip(why) => panic!("live golden tests cannot run: {why}"),
        Live::Run(b) => b,
    };
    let repo = Repo::locate();
    let mut failures = Vec::new();
    for case in repo.cases(filter().as_deref()).expect("cases parse") {
        if case.skip_blender {
            continue;
        }
        let fixture = match blender::load_fixture(&case, &repo.fixtures_dir()) {
            Ok(Some(f)) => f,
            Ok(None) => {
                failures.push(format!("{}: not blessed", case.id()));
                continue;
            }
            Err(e) => {
                failures.push(format!("{}: {e}", case.id()));
                continue;
            }
        };
        let work = ScratchDir::new(&case.name).unwrap();
        match blender::run_live(&b, &repo, &case, work.path()) {
            Ok(now) => {
                if now.blender.as_ref().map(|s| &s.build_hash)
                    != fixture.blender.as_ref().map(|s| &s.build_hash)
                {
                    failures.push(format!(
                        "{}: blessed with Blender {:?}, running {:?}",
                        case.id(),
                        fixture.blender,
                        now.blender
                    ));
                }
                for m in runner::compare(&case, &now, &fixture) {
                    failures.push(format!("{}: drift {m}", case.id()));
                }
            }
            Err(e) => failures.push(format!("{}: {e}", case.id())),
        }
    }
    assert!(
        failures.is_empty(),
        "live goldens:\n{}",
        failures.join("\n")
    );
}

/// The three seed cases are blessed by the Blender-side runner (Blender 5.2.4) and committed.
#[test]
fn seed_cases_are_blessed() {
    let repo = Repo::locate();
    let cases = repo.cases(Some("harness/*")).unwrap();
    let ids: Vec<String> = cases.iter().map(Case::id).collect();
    assert_eq!(
        ids,
        ["harness/cube_add", "harness/errors", "harness/factory"]
    );
    for c in &cases {
        let f = blender::load_fixture(c, &repo.fixtures_dir())
            .unwrap()
            .unwrap_or_else(|| panic!("{} is not blessed", c.id()));
        let stamp = f.blender.expect("fixtures are stamped");
        assert!(stamp.version.starts_with("5.2"), "{stamp:?}");
        assert!(!stamp.build_hash.is_empty());
        assert!(!f.lines.is_empty());
    }
}

/// A one-element change in a fixture makes the comparison fail, for every compared section.
#[test]
fn fixture_comparison_detects_one_change() {
    let repo = Repo::locate();
    let props = Props::load(&repo.props_path()).unwrap();
    let case = repo.cases(Some("harness/cube_add")).unwrap().remove(0);
    let fixture = blender::load_fixture(&case, &repo.fixtures_dir())
        .unwrap()
        .unwrap();
    let work = ScratchDir::new("detect").unwrap();
    let frap = runner::run_frap(
        &case,
        &props,
        &RunOptions {
            workdir: work.path().to_owned(),
            strict_rna: false,
        },
    )
    .unwrap()
    .state;
    assert!(runner::compare(&case, &frap, &fixture).is_empty());

    type Change = Box<dyn Fn(&mut frap_golden::GoldenState)>;
    let changes: Vec<(&str, Change)> = vec![
        (
            "lines",
            Box::new(|s| {
                s.lines[1] = LineOutcome {
                    line: s.lines[1].line,
                    ok: false,
                    exception: Some("RuntimeError".into()),
                    message: Some("x".into()),
                }
            }),
        ),
        (
            "rna_values",
            Box::new(|s| {
                let o: &mut GObject = s.objects.get_mut("Cube.001").unwrap();
                o.matrix_world[2][3] = 1.5;
            }),
        ),
        (
            "mesh_exact",
            Box::new(|s| s.meshes.get_mut("Cube.001").unwrap().positions[7][0] = 0.9),
        ),
        (
            "rna_values",
            Box::new(|s| s.context.selected = vec!["Cube".into()]),
        ),
    ];
    for (comparator, change) in changes {
        let mut b = fixture.clone();
        change(&mut b);
        let m = runner::compare(&case, &frap, &b);
        assert_eq!(m.len(), 1, "{comparator}: {m:?}");
        assert_eq!(m[0].comparator, comparator);
    }
}

/// `tools/blender/golden_runner.py::NORMALISE` lists exactly the deviations of
/// `docs/deviations.md` §1, and an entry is `None` exactly when the deviation's Golden column
/// says "none" (§20.3: the list mirrors `deviations.md`).
#[test]
fn normalisation_list_matches_deviations() {
    let repo = Repo::locate();
    let dev = std::fs::read_to_string(repo.root.join("docs/deviations.md"))
        .unwrap()
        .replace("\r\n", "\n");
    let section = dev
        .split("## 1.")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .expect("deviations.md §1");
    let mut header: Option<Vec<String>> = None;
    let mut expected = Vec::new();
    for line in section.lines().filter(|l| l.starts_with('|')) {
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_owned())
            .collect();
        if header.is_none() {
            header = Some(cells);
            continue;
        }
        if cells[0].starts_with("---") {
            continue;
        }
        let h = header.as_ref().unwrap();
        let golden = h
            .iter()
            .position(|c| c.starts_with("Golden"))
            .expect("Golden normalisation column");
        expected.push((
            cells[0].clone(),
            cells[golden].to_lowercase().starts_with("none"),
        ));
    }
    assert!(expected.len() >= 14, "{expected:?}");

    let py = std::fs::read_to_string(repo.root.join("tools/blender/golden_runner.py"))
        .unwrap()
        .replace("\r\n", "\n");
    let block = py
        .split("NORMALISE = {")
        .nth(1)
        .and_then(|s| s.split("\n}\n").next())
        .expect("NORMALISE dict");
    let mut actual = Vec::new();
    for line in block.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("\"D-") {
            let id = format!("D-{}", &rest[..rest.find('"').unwrap()]);
            let value = t.split_once(':').unwrap().1.trim();
            actual.push((id, value.starts_with("None")));
        }
    }
    assert_eq!(
        actual, expected,
        "NORMALISE (id, is None) vs deviations.md (id, golden = none)"
    );
}

/// §20.3: the runner never applies a pending frame change (no time normalisation): it calls
/// `view_layer.update()` only when time is not dirty, and never `frame_set`.
#[test]
fn runner_has_no_time_normalisation() {
    let repo = Repo::locate();
    let py = std::fs::read_to_string(repo.root.join("tools/blender/golden_runner.py"))
        .unwrap()
        .replace("\r\n", "\n");
    let code: Vec<&str> = py
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect();
    for l in &code {
        assert!(!l.contains("frame_set("), "frame_set call: {l}");
        assert!(!l.contains(".frame_current ="), "frame write: {l}");
    }
    let updates: Vec<usize> = code
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("bpy.context.view_layer.update()"))
        .map(|(i, _)| i)
        .collect();
    assert!(!updates.is_empty());
    for i in updates {
        assert!(
            code[i.saturating_sub(1)..=i]
                .iter()
                .any(|l| l.contains("not self.time_dirty")),
            "view_layer.update() not guarded by the time-dirty flag: {}",
            code[i]
        );
    }
}

/// The Blender-free unit tests of `golden_runner.py`'s time tracking
/// (`tools/blender/test_golden_runner.py`: `+=`, `frame_float`, `frame_subframe`, `setattr`,
/// value-based detection). Needs `python3` (3.8+) on PATH; without it the test fails, unless
/// `FRAP_GOLDEN_NO_PYTHON=1` says the machine has none.
#[test]
fn golden_runner_python_unit_tests() {
    let repo = Repo::locate();
    let script = repo.root.join("tools/blender/test_golden_runner.py");
    let out = match std::process::Command::new("python3")
        .arg("-I")
        .arg("-B")
        .arg(&script)
        .output()
    {
        Ok(o) => o,
        Err(e) if std::env::var_os("FRAP_GOLDEN_NO_PYTHON").is_some_and(|v| v == "1") => {
            eprintln!("NOTICE: python3 unavailable ({e}); golden_runner.py unit tests skipped");
            return;
        }
        Err(e) => panic!("cannot run python3 {}: {e}", script.display()),
    };
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Live probe of the time-dirty tracking (§20.3, verified on Blender 5.2.4): after a pending
/// frame change (`+=`, `frame_float`, `setattr`, …) a derived read must not make the runner
/// apply the animation, so the manual `location` survives (the old runner recorded the
/// animated `[0, 0, 5]` after `+= 5`).
#[test]
#[ignore = "live: needs Blender 5.2; run `cargo xtask golden live` (FRAP_GOLDEN_LIVE=1, --include-ignored)"]
fn live_runner_keeps_pending_frame_changes() {
    let b = match blender::live() {
        Live::Off => {
            eprintln!(
                "NOTICE: live golden tests off (set {}=1)",
                blender::LIVE_ENV
            );
            return;
        }
        Live::Skip(why) => panic!("live golden tests cannot run: {why}"),
        Live::Run(b) => b,
    };
    let repo = Repo::locate();
    let work = ScratchDir::new("time-probe").unwrap();
    let cases = [
        ("bpy.context.scene.frame_current += 5", [7.0, 7.0, 7.0]),
        ("bpy.context.scene.frame_current = 6", [7.0, 7.0, 7.0]),
        ("bpy.context.scene.frame_float = 6.0", [7.0, 7.0, 7.0]),
        (
            "setattr(bpy.context.scene, 'frame_current', 6)",
            [7.0, 7.0, 7.0],
        ),
        // A frame change through an operator is applied by the operator itself.
        ("bpy.ops.screen.frame_offset(delta=5)", [7.0, 7.0, 7.0]),
    ];
    for (i, (step, expected)) in cases.iter().enumerate() {
        let src = format!(
            "# golden: compare=objects\n\
             ob = bpy.data.objects['Cube']\n\
             ob.location = (0, 0, 0)\n\
             ob.keyframe_insert('location', frame=1)\n\
             ob.location = (0, 0, 10)\n\
             ob.keyframe_insert('location', frame=11)\n\
             bpy.context.scene.frame_current = 1\n\
             bpy.context.view_layer.update()\n\
             {step}\n\
             ob.location = (7, 7, 7)\n\
             m = ob.matrix_world\n"
        );
        let file = work.path().join(format!("probe{i}.py"));
        std::fs::write(&file, src).unwrap();
        let state =
            blender::run_blender(&b, &repo, &file, &work.path().join(format!("w{i}"))).unwrap();
        assert!(
            state.lines.iter().all(|l| l.ok),
            "{step}: {:?}",
            state.lines
        );
        let ob = &state.objects["Cube"];
        assert_eq!(ob.location, *expected, "{step}: location");
    }
}

/// Every property path of `props.toml` is read by `dump_state.py` (both dumpers use one list).
#[test]
fn both_dumpers_read_props_toml() {
    let repo = Repo::locate();
    let py = std::fs::read_to_string(repo.root.join("tools/blender/dump_state.py"))
        .unwrap()
        .replace("\r\n", "\n");
    let props: toml::Table = toml::from_str(
        &std::fs::read_to_string(repo.props_path())
            .unwrap()
            .replace("\r\n", "\n"),
    )
    .unwrap();
    for (section, alias) in [
        ("object", "po"),
        ("mesh", "pm"),
        ("context", "pc"),
        ("material", "pm"),
    ] {
        let t = props[section].as_table().unwrap();
        for key in t.keys() {
            assert!(
                py.contains(&format!("{alias}[\"{key}\"]")),
                "dump_state.py does not read props.toml [{section}] {key}"
            );
        }
    }
    let runner_py = std::fs::read_to_string(repo.root.join("tools/blender/golden_runner.py"))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(runner_py.contains("props[\"derived\"][\"props\"]"));
}

/// Line numbers of side-only lines are reported on one side only and skipped by `lines`.
#[test]
fn side_only_lines_of_the_factory_seed() {
    let repo = Repo::locate();
    let case = repo.cases(Some("harness/factory")).unwrap().remove(0);
    let side: BTreeSet<u32> = case.side_only_lines();
    assert_eq!(side.len(), 2);
    let f = blender::load_fixture(&case, &repo.fixtures_dir())
        .unwrap()
        .unwrap();
    let blender_lines: BTreeSet<u32> = f.lines.iter().map(|l| l.line).collect();
    // The `#@blender` line ran in Blender, the `#@frap` line did not.
    let frap_only = case
        .lines
        .iter()
        .find(|l| l.side == frap_golden::case::Side::Frap)
        .unwrap()
        .line;
    let blender_only = case
        .lines
        .iter()
        .find(|l| l.side == frap_golden::case::Side::Blender)
        .unwrap()
        .line;
    assert!(blender_lines.contains(&blender_only));
    assert!(!blender_lines.contains(&frap_only));
}

/// M3 exit criterion (§22.5, decision log M34S-6): at least 60 static-glTF first exports
/// under `tests/golden/cases/gltf/**` are compared (`compare=glb:…`) with the round trip of
/// their own output (re-imported, exported again as `…_rt.<ext>`; WP 3.9's rule). The round
/// trips are named after the export (`car.glb` → `car_rt.glb`), its folder
/// (`out/web/crate.gltf` → `web_rt.glb`) or a variant (`survey_all.glb` → `survey_rt.glb`), and
/// `modifier_stack_reimport.py` round-trips `modifier_stack.py`'s applied export, so the suite
/// is checked by count: the M3 workflow suite (`gltf/m3_workflows/`) has at least one `_rt`
/// export per first export, and at least 60 first exports.
#[test]
fn m3_static_gltf_goldens_with_round_trips() {
    use frap_golden::case::CompareSpec;
    let repo = Repo::locate();
    let cases = repo.cases(Some("gltf/**")).unwrap();
    let (mut first, mut rt) = (Vec::new(), Vec::new());
    for c in cases.iter().filter(|c| c.area == "gltf/m3_workflows") {
        for s in &c.compare {
            if let CompareSpec::Glb(p) = s {
                let stem = p.rsplit_once('.').map_or(p.as_str(), |(s, _)| s);
                let id = format!("{}: {p}", c.id());
                if stem.ends_with("_rt") {
                    rt.push(id);
                } else {
                    first.push(id);
                }
            }
        }
    }
    assert!(
        first.len() >= 60,
        "only {} static-glTF first exports in gltf/m3_workflows (M3 exit: ≥ 60): {first:?}",
        first.len()
    );
    assert!(
        rt.len() >= first.len(),
        "{} first exports but {} round trips: {first:?} / {rt:?}",
        first.len(),
        rt.len()
    );
}
