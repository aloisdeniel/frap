// SPDX-License-Identifier: GPL-3.0-or-later
//! Blender side: `$BLENDER_BIN -b --factory-startup --python tools/blender/golden_runner.py -- case.py out.json`. WP 1.8.
//!
//! Also fixtures (`tests/golden/fixtures/<area>/<case>.blender.json` plus copied output files),
//! bless, live mode and the repository layout (§20.5).

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value as Json, json};

use crate::case::{Case, CaseError};
use crate::compare::gltf_canonical;
use crate::{GFile, GoldenState};

/// Environment variable that turns live mode on (§20.5).
pub const LIVE_ENV: &str = "FRAP_GOLDEN_LIVE";
/// Environment variable naming the Blender binary.
pub const BLENDER_ENV: &str = "BLENDER_BIN";
/// The macOS default install location, relative to `$HOME`.
pub const MACOS_DEFAULT: &str = "Applications/Blender-5.2.app/Contents/MacOS/Blender";

/// A harness failure (not a comparison mismatch).
#[derive(Debug)]
#[non_exhaustive]
pub enum HarnessError {
    /// A case file does not parse.
    Case(CaseError),
    /// Filesystem.
    Io(String),
    /// Blender exited unsuccessfully or wrote no dump.
    Blender(String),
    /// A dump or fixture is not valid `GoldenState` JSON.
    Json(String),
    /// The frap side cannot run the case (e.g. a startup frap does not support yet).
    Frap(String),
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarnessError::Case(e) => write!(f, "case: {e}"),
            HarnessError::Io(e) => write!(f, "io: {e}"),
            HarnessError::Blender(e) => write!(f, "blender: {e}"),
            HarnessError::Json(e) => write!(f, "json: {e}"),
            HarnessError::Frap(e) => write!(f, "frap: {e}"),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<CaseError> for HarnessError {
    fn from(e: CaseError) -> Self {
        HarnessError::Case(e)
    }
}

fn io(what: &str, p: &Path, e: std::io::Error) -> HarnessError {
    HarnessError::Io(format!("{what} {}: {e}", p.display()))
}

/// Repository layout used by the harness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    /// Repository root.
    pub root: PathBuf,
}

impl Repo {
    /// The repository this crate lives in.
    pub fn locate() -> Repo {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        Repo {
            root: here
                .parent()
                .and_then(Path::parent)
                .unwrap_or(here)
                .to_owned(),
        }
    }
    /// `tests/golden/cases`.
    pub fn cases_dir(&self) -> PathBuf {
        self.root.join("tests/golden/cases")
    }
    /// `tests/golden/fixtures`.
    pub fn fixtures_dir(&self) -> PathBuf {
        self.root.join("tests/golden/fixtures")
    }
    /// `tests/golden/props.toml`.
    pub fn props_path(&self) -> PathBuf {
        self.root.join("tests/golden/props.toml")
    }
    /// `tools/blender/golden_runner.py`.
    pub fn runner_py(&self) -> PathBuf {
        self.root.join("tools/blender/golden_runner.py")
    }
    /// `tests/data/gltf-roundtrip/allow.toml`.
    pub fn roundtrip_allow_list(&self) -> PathBuf {
        self.root.join("tests/data/gltf-roundtrip/allow.toml")
    }
    /// Every case (sorted), optionally filtered by a glob over the case id.
    pub fn cases(&self, filter: Option<&str>) -> Result<Vec<Case>, HarnessError> {
        let root = self.cases_dir();
        crate::case::discover(&root, filter)
            .iter()
            .map(|p| Case::load(p, &root).map_err(HarnessError::from))
            .collect()
    }
}

/// A fresh, empty scratch directory (removed by [`ScratchDir`]'s `Drop`).
#[derive(Debug)]
pub struct ScratchDir(PathBuf);

impl ScratchDir {
    /// Creates `<tmp>/frap-golden-<pid>-<n>-<tag>`.
    pub fn new(tag: &str) -> Result<ScratchDir, HarnessError> {
        static N: AtomicU64 = AtomicU64::new(0);
        let tag: String = tag
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let p = std::env::temp_dir().join(format!(
            "frap-golden-{}-{}-{tag}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).map_err(|e| io("cannot create", &p, e))?;
        Ok(ScratchDir(p))
    }
    /// The path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// [`crate::locate_blender`] with its inputs explicit (tested without touching the process
/// environment): `$BLENDER_BIN` if it names a file, then `exe` in each `PATH` directory, then
/// `$HOME/Applications/Blender-5.2.app/Contents/MacOS/Blender`.
pub fn locate_from(
    blender_bin: Option<OsString>,
    path: Option<OsString>,
    home: Option<OsString>,
    exe: &str,
) -> Option<PathBuf> {
    if let Some(p) = blender_bin.map(PathBuf::from)
        && p.is_file()
    {
        return Some(p);
    }
    if let Some(path) = path {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let p = PathBuf::from(home?).join(MACOS_DEFAULT);
    p.is_file().then_some(p)
}

/// Live mode decision (§20.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Live {
    /// `FRAP_GOLDEN_LIVE` is not set: fixtures only.
    Off,
    /// Re-run Blender with this binary.
    Run(PathBuf),
    /// Live mode was asked for but cannot run (the reason is printed as a visible notice).
    Skip(String),
}

/// Decides live mode from `FRAP_GOLDEN_LIVE` and the located Blender.
pub fn live_decision(env_live: Option<&OsStr>, blender: Option<PathBuf>) -> Live {
    match env_live.and_then(|v| v.to_str()) {
        None | Some("" | "0" | "false" | "no") => Live::Off,
        Some(_) => match blender {
            Some(b) => Live::Run(b),
            None => Live::Skip(format!(
                "{LIVE_ENV} is set but Blender 5.2 was not found (set {BLENDER_ENV}, put \
                 `blender` on PATH, or install ~/{MACOS_DEFAULT}); live golden tests skipped, \
                 fixture comparisons still run"
            )),
        },
    }
}

/// [`live_decision`] for this process.
pub fn live() -> Live {
    live_decision(
        std::env::var_os(LIVE_ENV).as_deref(),
        crate::locate_blender(),
    )
}

/// Runs `golden_runner.py` on `case_file` with `workdir` as the current directory and returns
/// the parsed dump (output files referenced by absolute `$file` paths, unresolved).
pub fn run_blender(
    blender: &Path,
    repo: &Repo,
    case_file: &Path,
    workdir: &Path,
) -> Result<GoldenState, HarnessError> {
    std::fs::create_dir_all(workdir).map_err(|e| io("cannot create", workdir, e))?;
    let case_file =
        std::path::absolute(case_file).map_err(|e| io("cannot resolve", case_file, e))?;
    let case_file = case_file.as_path();
    let out = workdir.join(".golden-state.json");
    let _ = std::fs::remove_file(&out);
    let output = Command::new(blender)
        .arg("-b")
        .arg("--factory-startup")
        .arg("--python-exit-code")
        .arg("1")
        .arg("--python")
        .arg(repo.runner_py())
        .arg("--")
        .arg(case_file)
        .arg(&out)
        .arg("--props")
        .arg(repo.props_path())
        .arg("--workdir")
        .arg(workdir)
        .current_dir(workdir)
        .output()
        .map_err(|e| HarnessError::Blender(format!("cannot start {}: {e}", blender.display())))?;
    let tail = |b: &[u8]| {
        let s = String::from_utf8_lossy(b);
        let lines: Vec<&str> = s.lines().collect();
        lines[lines.len().saturating_sub(25)..].join("\n")
    };
    if !output.status.success() || !out.is_file() {
        return Err(HarnessError::Blender(format!(
            "{} on {} exited with {}\n--- stdout (tail)\n{}\n--- stderr (tail)\n{}",
            blender.display(),
            case_file.display(),
            output.status,
            tail(&output.stdout),
            tail(&output.stderr)
        )));
    }
    let text = std::fs::read_to_string(&out).map_err(|e| io("cannot read", &out, e))?;
    let _ = std::fs::remove_file(&out);
    let mut state: GoldenState = serde_json::from_str(&text)
        .map_err(|e| HarnessError::Json(format!("Blender dump: {e}")))?;
    crate::runner::scrub_workdir(&mut state.lines, workdir);
    Ok(state)
}

/// `$file` reference inside a `GFile`.
fn file_ref(v: &Json) -> Option<&str> {
    v.get("$file").and_then(|f| f.as_str())
}

/// Resolves output-file references: a glTF `{"$file": p}` becomes its canonical form, an
/// image's `{"$file": p}` becomes an absolute path. Relative paths resolve against `base`.
pub fn resolve_files(state: &mut GoldenState, base: &Path) -> Result<(), HarnessError> {
    for (key, f) in state.files.iter_mut() {
        match f {
            GFile::Gltf { canonical } => {
                if let Some(p) = file_ref(canonical) {
                    let p = base.join(p);
                    *canonical = gltf_canonical::canonicalize_file(&p)
                        .map_err(|e| HarnessError::Json(format!("files[{key:?}]: {e}")))?;
                }
            }
            GFile::Image { pixels, .. } => {
                if let Some(p) = file_ref(pixels) {
                    let p = base.join(p);
                    *pixels = json!({"$file": p.to_string_lossy()});
                }
            }
        }
    }
    Ok(())
}

/// The fixture file name of an output file (`out/part.glb` → `<case>.out_part.glb`).
pub fn fixture_file_name(case: &Case, key: &str) -> String {
    let flat: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.{flat}", case.name)
}

/// Writes a Blender dump as the case's fixture: output files are copied next to the JSON and
/// referenced by file name. Returns the fixture path.
pub fn write_fixture(
    case: &Case,
    fixtures_root: &Path,
    mut state: GoldenState,
) -> Result<PathBuf, HarnessError> {
    let path = case.fixture_path(fixtures_root);
    let dir = path
        .parent()
        .expect("invariant: fixture has a parent")
        .to_owned();
    std::fs::create_dir_all(&dir).map_err(|e| io("cannot create", &dir, e))?;
    for (key, f) in state.files.iter_mut() {
        let slot = match f {
            GFile::Gltf { canonical } => canonical,
            GFile::Image { pixels, .. } => pixels,
        };
        let Some(src) = file_ref(slot).map(PathBuf::from) else {
            continue;
        };
        let name = fixture_file_name(case, key);
        let dst = dir.join(&name);
        std::fs::copy(&src, &dst).map_err(|e| io("cannot copy", &src, e))?;
        *slot = json!({"$file": name});
    }
    let value = serde_json::to_value(&state)
        .map_err(|e| HarnessError::Json(format!("cannot serialise: {e}")))?;
    let text = fixture_json(&value);
    std::fs::write(&path, text).map_err(|e| io("cannot write", &path, e))?;
    Ok(path)
}

/// Fixture formatting: objects indented, arrays of scalars on one line (so a vertex, a face
/// or a matrix row is one line and fixtures diff well). Ends with a newline.
pub fn fixture_json(v: &Json) -> String {
    fn scalar(v: &Json) -> bool {
        !matches!(v, Json::Array(_) | Json::Object(_))
    }
    fn go(v: &Json, indent: usize, out: &mut String) {
        let pad = |n: usize| "  ".repeat(n);
        match v {
            Json::Object(o) if !o.is_empty() => {
                out.push_str("{\n");
                for (i, (k, x)) in o.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    out.push_str(&Json::from(k.as_str()).to_string());
                    out.push_str(": ");
                    go(x, indent + 1, out);
                    if i + 1 < o.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad(indent));
                out.push('}');
            }
            Json::Array(a) if !a.is_empty() && !a.iter().all(scalar) => {
                out.push_str("[\n");
                for (i, x) in a.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    go(x, indent + 1, out);
                    if i + 1 < a.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad(indent));
                out.push(']');
            }
            Json::Array(a) => {
                let items: Vec<String> = a
                    .iter()
                    .map(|x| {
                        let mut s = String::new();
                        go(x, indent, &mut s);
                        s
                    })
                    .collect();
                out.push('[');
                out.push_str(&items.join(", "));
                out.push(']');
            }
            Json::Number(n) if n.is_f64() => out.push_str(&number(n.as_f64().unwrap_or(0.0))),
            other => out.push_str(&other.to_string()),
        }
    }
    /// f32 values (every float of a dump) as their shortest f32 decimal.
    fn number(x: f64) -> String {
        let f = x as f32;
        if f as f64 == x && f.is_finite() {
            let s = format!("{f}");
            if s.contains(['.', 'e', 'E']) {
                s
            } else {
                format!("{s}.0")
            }
        } else {
            Json::from(x).to_string()
        }
    }
    let mut out = String::new();
    go(v, 0, &mut out);
    out.push('\n');
    out
}

/// Runs Blender on `case` and writes its fixture (§20.5 `cargo xtask golden bless`).
pub fn bless(blender: &Path, repo: &Repo, case: &Case) -> Result<PathBuf, HarnessError> {
    if case.skip_blender {
        return Err(HarnessError::Blender(format!(
            "{} is `skip_blender`: nothing to bless",
            case.id()
        )));
    }
    let work = ScratchDir::new(&case.name)?;
    let state = run_blender(blender, repo, &case.path, work.path())?;
    write_fixture(case, &repo.fixtures_dir(), state)
}

/// Loads the case's fixture with file references resolved; `None` when not blessed.
pub fn load_fixture(
    case: &Case,
    fixtures_root: &Path,
) -> Result<Option<GoldenState>, HarnessError> {
    let path = case.fixture_path(fixtures_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io("cannot read", &path, e)),
    };
    let mut state: GoldenState = serde_json::from_str(&text)
        .map_err(|e| HarnessError::Json(format!("{}: {e}", path.display())))?;
    resolve_files(
        &mut state,
        path.parent().expect("invariant: fixture has a parent"),
    )?;
    Ok(Some(state))
}

/// Runs Blender on `case` live (resolved, for drift checks and side-by-side runs).
pub fn run_live(
    blender: &Path,
    repo: &Repo,
    case: &Case,
    workdir: &Path,
) -> Result<GoldenState, HarnessError> {
    let mut s = run_blender(blender, repo, &case.path, workdir)?;
    resolve_files(&mut s, workdir)?;
    Ok(s)
}

/// Blender's import → export of `src` with default options (the reference side of the
/// round-trip law, §17.2). Returns the exported GLB bytes.
pub fn roundtrip(blender: &Path, repo: &Repo, src: &Path) -> Result<Vec<u8>, HarnessError> {
    let work = ScratchDir::new("roundtrip")?;
    let case = work.path().join("roundtrip.py");
    let text = crate::compare::gltf_roundtrip::blender_case(src, "roundtrip.glb");
    std::fs::write(&case, text).map_err(|e| io("cannot write", &case, e))?;
    let state = run_blender(blender, repo, &case, work.path())?;
    if let Some(bad) = state.lines.iter().find(|l| !l.ok) {
        return Err(HarnessError::Blender(format!(
            "round trip of {} failed on line {}: {} {}",
            src.display(),
            bad.line,
            bad.exception.as_deref().unwrap_or("CANCELLED"),
            bad.message.as_deref().unwrap_or("")
        )));
    }
    let out = work.path().join("roundtrip.glb");
    std::fs::read(&out).map_err(|e| io("cannot read", &out, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"#!/bin/sh\n").unwrap();
    }

    #[test]
    fn locate_order_env_then_path_then_home() {
        let d = ScratchDir::new("locate").unwrap();
        let env_bin = d.path().join("env/blender-custom");
        let path_a = d.path().join("path_a");
        let path_b = d.path().join("path_b");
        let home = d.path().join("home");
        std::fs::create_dir_all(&path_a).unwrap();
        touch(&path_b.join("blender"));
        touch(&home.join(MACOS_DEFAULT));
        touch(&env_bin);
        let path = std::env::join_paths([&path_a, &path_b]).unwrap();
        let s = |p: &Path| Some(p.as_os_str().to_owned());
        // 1. $BLENDER_BIN wins when it is a file.
        assert_eq!(
            locate_from(s(&env_bin), Some(path.clone()), s(&home), "blender"),
            Some(env_bin.clone())
        );
        // A $BLENDER_BIN that is not a file is ignored.
        assert_eq!(
            locate_from(
                s(&d.path().join("nope")),
                Some(path.clone()),
                s(&home),
                "blender"
            ),
            Some(path_b.join("blender"))
        );
        // 2. PATH, first directory that has the executable.
        touch(&path_a.join("blender"));
        assert_eq!(
            locate_from(None, Some(path.clone()), s(&home), "blender"),
            Some(path_a.join("blender"))
        );
        // 3. ~/Applications/Blender-5.2.app.
        assert_eq!(
            locate_from(None, None, s(&home), "blender"),
            Some(home.join(MACOS_DEFAULT))
        );
        assert_eq!(locate_from(None, Some(path), None, "blender.exe"), None);
        assert_eq!(
            locate_from(None, None, s(&d.path().join("nohome")), "blender"),
            None
        );
    }

    #[test]
    fn live_and_skip_decisions() {
        let b = Some(PathBuf::from("/opt/blender"));
        assert_eq!(live_decision(None, b.clone()), Live::Off);
        assert_eq!(live_decision(Some(OsStr::new("0")), b.clone()), Live::Off);
        assert_eq!(live_decision(Some(OsStr::new("")), b.clone()), Live::Off);
        assert_eq!(
            live_decision(Some(OsStr::new("1")), b.clone()),
            Live::Run(PathBuf::from("/opt/blender"))
        );
        match live_decision(Some(OsStr::new("1")), None) {
            Live::Skip(why) => assert!(why.contains("not found") && why.contains(LIVE_ENV)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn fixture_json_is_compact_and_round_trips() {
        let v = json!({"a": [[1.0, 2.5], [3, 4]], "b": {"c": [], "d": {}}, "e": [true, null, "x"]});
        let text = fixture_json(&v);
        assert_eq!(
            text,
            "{\n  \"a\": [\n    [1.0, 2.5],\n    [3, 4]\n  ],\n  \"b\": {\n    \"c\": [],\n    \"d\": {}\n  },\n  \"e\": [true, null, \"x\"]\n}\n"
        );
        assert_eq!(serde_json::from_str::<Json>(&text).unwrap(), v);
        // f32 values print short, f64-only values keep full precision.
        let w = json!([0.1f32 as f64, 0.1f64, 1e-3f32 as f64, -2.0]);
        assert_eq!(fixture_json(&w), "[0.1, 0.1, 0.001, -2.0]\n");
        let back: Vec<f32> = serde_json::from_str(&fixture_json(&w)).unwrap();
        assert_eq!(back, [0.1f32, 0.1, 1e-3, -2.0]);
    }

    #[test]
    fn fixture_file_names_are_flat() {
        let c = Case::parse(Path::new("cases/io/part.py"), "# golden:\n").unwrap();
        assert_eq!(fixture_file_name(&c, "out/part.glb"), "part.out_part.glb");
        assert_eq!(fixture_file_name(&c, "a b/../x.png"), "part.a_b_.._x.png");
    }

    /// The bless path end to end with a stand-in "Blender" (a shell script that writes a
    /// canned dump and an output GLB), so it runs without Blender.
    #[cfg(unix)]
    #[test]
    fn bless_with_a_fake_blender_then_load() {
        use std::os::unix::fs::PermissionsExt;
        let d = ScratchDir::new("bless").unwrap();
        let repo = Repo {
            root: d.path().join("repo"),
        };
        let case_path = repo.cases_dir().join("io/export.py");
        std::fs::create_dir_all(case_path.parent().unwrap()).unwrap();
        std::fs::write(
            &case_path,
            "# golden: compare=glb:out/a.glb\nimport bpy\nbpy.ops.export_scene.gltf(filepath='out/a.glb')\n",
        )
        .unwrap();
        let glb = crate::compare::gltf_canonical::tests::quad_scene(
            [0, 1, 2, 3],
            [[0, 1, 2], [0, 2, 3]],
            0.0,
            "fake",
        );
        std::fs::write(d.path().join("a.glb"), &glb).unwrap();
        // The fake writes the dump to the 9th argument (`out.json`) and the GLB into the cwd.
        let state = json!({
            "blender": {"version": "5.2.4 LTS", "build_hash": "cd5dc428cce1"},
            "lines": [{"line": 2, "ok": true}, {"line": 3, "ok": true}],
            "objects": {}, "meshes": {}, "evaluated": {}, "materials": {}, "actions": {},
            "context": {"active": null, "selected": [], "mode": "OBJECT", "select_mode": null,
                        "cursor": [0.0, 0.0, 0.0]},
            "files": {"out/a.glb": {"kind": "gltf", "canonical": {"$file": "WORKDIR/out/a.glb"}}}
        });
        std::fs::write(d.path().join("state.json"), state.to_string()).unwrap();
        let fake = d.path().join("fake-blender");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nset -e\nout=\"$9\"\nmkdir -p out\ncp '{glb}' out/a.glb\n\
                 sed \"s#WORKDIR#$(pwd)#\" '{state}' > \"$out\"\n",
                glb = d.path().join("a.glb").display(),
                state = d.path().join("state.json").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let cases = repo.cases(None).unwrap();
        assert_eq!(cases.len(), 1);
        let case = &cases[0];
        assert_eq!(case.id(), "io/export");
        assert!(load_fixture(case, &repo.fixtures_dir()).unwrap().is_none());
        let fixture = bless(&fake, &repo, case).unwrap();
        assert_eq!(fixture, repo.fixtures_dir().join("io/export.blender.json"));
        // The GLB was copied next to the fixture and is referenced by name.
        let text = std::fs::read_to_string(&fixture).unwrap();
        assert!(text.contains("\"$file\": \"export.out_a.glb\""), "{text}");
        assert!(repo.fixtures_dir().join("io/export.out_a.glb").is_file());
        // Loading resolves it to the canonical glTF.
        let loaded = load_fixture(case, &repo.fixtures_dir()).unwrap().unwrap();
        let GFile::Gltf { canonical } = &loaded.files["out/a.glb"] else {
            panic!("not a glTF")
        };
        let expected =
            gltf_canonical::canonicalize(&gltf_canonical::load(&glb, None).unwrap()).unwrap();
        assert_eq!(canonical, &expected);
        assert_eq!(loaded.blender.as_ref().unwrap().build_hash, "cd5dc428cce1");

        // A Blender that fails is reported with its output.
        let bad = d.path().join("bad-blender");
        std::fs::write(&bad, "#!/bin/sh\necho boom >&2\nexit 3\n").unwrap();
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
        match bless(&bad, &repo, case) {
            Err(HarnessError::Blender(m)) => assert!(m.contains("boom"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn skip_blender_cases_are_never_blessed() {
        let c = Case::parse(Path::new("x.py"), "# golden: skip_blender\n").unwrap();
        let repo = Repo {
            root: PathBuf::from("/nonexistent"),
        };
        assert!(matches!(
            bless(Path::new("/bin/false"), &repo, &c),
            Err(HarnessError::Blender(_))
        ));
    }
}
