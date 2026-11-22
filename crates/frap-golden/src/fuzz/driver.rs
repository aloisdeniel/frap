// SPDX-License-Identifier: GPL-3.0-or-later
//! Running sequences in both tools, classifying and minimising mismatches, writing cases
//! (WP 8.7).
//!
//! One trial: [`super::sequence::generate`] draws the statements; a first frap run learns which
//! meshes exist at the end (their names go into `compare=`); then the case runs in frap
//! (in-process, [`crate::runner::run_frap`]) and in Blender (`golden_runner.py`, a fresh
//! process per case) and [`crate::runner::compare`] lists the mismatches. A mismatch that
//! [`super::known`] does not explain is minimised with [`super::minimize::ddmin`] (every
//! candidate re-runs both tools and must still show a mismatch with the same signature) and
//! written as a golden case. Workers run trials in parallel; each signature is minimised
//! once per run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use frap_rna::{IdRef, MeshId};
use frap_session::{Session, SessionOptions, Startup as SessionStartup};

use super::catalog::Catalog;
use super::known;
use super::minimize;
use super::rng::Rng;
use super::sequence;
use crate::blender::{self, Repo, ScratchDir};
use crate::case::{Case, CompareSpec, Mismatch};
use crate::runner::{self, Props, RunOptions};

/// The case header every fuzz case uses: T2 (canonical mesh order), `tol=1e-4` (long
/// sequences accumulate float differences of solvers that are T2 elsewhere).
pub const TIER: &str = "tier=T2 tol=0.0001";
/// Lines before the first statement (header, `# fuzz:` comment, `import bpy`).
pub const PREAMBLE_LINES: u32 = 3;
/// At most this many meshes and evaluated objects are compared per case.
const MAX_COMPARED: usize = 4;
/// Clean trial seeds kept in the report.
const MAX_CLEAN_SEEDS: usize = 64;
/// Meshes above this vertex count are not compared (kept small to keep runs fast).
const MAX_COMPARED_VERTS: usize = 20_000;

/// Fuzzing options.
#[derive(Clone, Debug)]
pub struct Options {
    /// Run seed: trial `i` draws from `Rng::sequence_seed(seed, i)`.
    pub seed: u64,
    /// Stop after this many trials.
    pub count: Option<u64>,
    /// Stop after this long.
    pub duration: Option<Duration>,
    /// Statements per sequence.
    pub len: usize,
    /// Parallel workers.
    pub jobs: usize,
    /// Minimise unexplained mismatches.
    pub minimize: bool,
    /// Tool runs allowed per minimisation.
    pub budget: usize,
    /// Where minimised cases and `report.json` go.
    pub out: PathBuf,
    /// The Blender binary.
    pub blender: PathBuf,
}

/// The compared items of a sequence: `objects` and `context` always, plus the meshes of the
/// objects frap has at the end (by data name) and the evaluated meshes of objects with
/// modifiers.
pub fn compare_specs(statements: &[String]) -> Vec<CompareSpec> {
    let mut specs = vec![CompareSpec::Objects, CompareSpec::Context];
    let work = match ScratchDir::new("fuzz-probe") {
        Ok(w) => w,
        Err(_) => return specs,
    };
    let mut so = SessionOptions::default();
    so.cwd = work.path().to_owned();
    so.startup = SessionStartup::Factory;
    let Ok(mut s) = Session::new(so) else {
        return specs;
    };
    let src = source(statements);
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.exec_source_at(&src, 1);
    }));
    if ran.is_err() {
        return specs;
    }
    let main = s.main();
    let (mut meshes, mut evaluated) = (Vec::new(), Vec::new());
    for (_, o) in main.objects.iter() {
        let Some(m) = o
            .data
            .and_then(IdRef::<MeshId>::from_any)
            .and_then(|r| main.meshes.get(r))
        else {
            continue;
        };
        if m.verts_num() > MAX_COMPARED_VERTS {
            continue;
        }
        let name = m.header.name.to_string();
        if !meshes.contains(&name) {
            meshes.push(name);
        }
        if !o.modifiers.is_empty() {
            evaluated.push(o.header.name.to_string());
        }
    }
    specs.extend(meshes.into_iter().take(MAX_COMPARED).map(CompareSpec::Mesh));
    specs.extend(
        evaluated
            .into_iter()
            .take(MAX_COMPARED)
            .map(CompareSpec::Evaluated),
    );
    specs
}

/// The statements as case source with the preamble's line numbering (statement `i` on line
/// `PREAMBLE_LINES + 1 + i`), for running outside a case file.
fn source(statements: &[String]) -> String {
    let mut s = "\n".repeat(PREAMBLE_LINES as usize - 1);
    s.push_str("import bpy\n");
    for st in statements {
        s.push_str(st);
        s.push('\n');
    }
    s
}

/// The golden case text of a sequence.
pub fn case_text(statements: &[String], compare: &[CompareSpec], note: &str) -> String {
    let items: Vec<String> = compare.iter().map(CompareSpec::to_header).collect();
    let mut t = format!(
        "# golden: startup=factory compare={} {TIER}\n# fuzz: {note}\nimport bpy\n",
        items.join(",")
    );
    for s in statements {
        t.push_str(s);
        t.push('\n');
    }
    t
}

/// What one run of a sequence in both tools gave.
#[derive(Clone, Debug, Default)]
pub struct RunResult {
    /// Comparison mismatches (a frap panic or dump defect is a `harness` mismatch too).
    pub mismatches: Vec<Mismatch>,
    /// Blender itself failed (crashed, wrote no dump): not compared.
    pub blender_failure: Option<String>,
}

/// Runs `statements` in both tools and compares.
pub fn run_both(repo: &Repo, props: &Props, blender: &Path, statements: &[String]) -> RunResult {
    let specs = compare_specs(statements);
    let text = case_text(statements, &specs, "candidate");
    let work = match ScratchDir::new("fuzz") {
        Ok(w) => w,
        Err(e) => {
            return RunResult {
                blender_failure: Some(e.to_string()),
                ..Default::default()
            };
        }
    };
    let path = work.path().join("case.py");
    if let Err(e) = std::fs::write(&path, &text) {
        return RunResult {
            blender_failure: Some(format!("cannot write {}: {e}", path.display())),
            ..Default::default()
        };
    }
    let case = match Case::parse(&path, &text) {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                mismatches: vec![Mismatch::new("harness", "case", e.to_string())],
                ..Default::default()
            };
        }
    };
    let (wf, wb) = (work.path().join("frap"), work.path().join("blender"));
    let _ = std::fs::create_dir_all(&wf);
    let frap = runner::run_frap(
        &case,
        props,
        &RunOptions {
            workdir: wf,
            strict_rna: false,
        },
    );
    let bl = blender::run_live(blender, repo, &case, &wb);
    match (frap, bl) {
        (_, Err(e)) => RunResult {
            blender_failure: Some(e.to_string()),
            ..Default::default()
        },
        (Err(e), Ok(_)) => RunResult {
            mismatches: vec![Mismatch::new("harness", "frap", e.to_string())],
            ..Default::default()
        },
        (Ok(f), Ok(b)) => RunResult {
            mismatches: runner::compare(&case, &f.state, &b),
            ..Default::default()
        },
    }
}

/// The statement a `lines` mismatch path (`line 7`) points at.
fn statement_at<'a>(statements: &'a [String], path: &str) -> Option<&'a str> {
    let n: u32 = path.strip_prefix("line ")?.parse().ok()?;
    let i = n.checked_sub(PREAMBLE_LINES + 1)? as usize;
    statements.get(i).map(String::as_str)
}

/// The callee of a statement: `mesh.subdivide` for an operator call, the assigned or called
/// path without indices and arguments otherwise.
pub fn callee(statement: &str) -> String {
    if let Some(rest) = statement.strip_prefix("bpy.ops.") {
        return rest.split('(').next().unwrap_or(rest).to_owned();
    }
    let head = statement
        .split(" = ")
        .next()
        .unwrap_or(statement)
        .split('(')
        .next()
        .unwrap_or(statement);
    strip_keys(head)
}

/// `[...]` contents and digits removed (names and indices change while minimising).
fn strip_keys(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '[' => {
                depth += 1;
                if depth == 1 {
                    out.push('[');
                }
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    out.push(']');
                }
            }
            _ if depth > 0 => {}
            c if c.is_ascii_digit() => out.push('#'),
            c => out.push(c),
        }
    }
    out
}

/// A mismatch's identity across minimisation steps: the comparator plus the statement's
/// callee (`lines`) or the path with names and indices removed.
pub fn signature(statements: &[String], m: &Mismatch) -> String {
    if m.comparator == "lines" {
        let at = statement_at(statements, &m.path).map_or_else(|| m.path.clone(), callee);
        // The kind of difference: which side failed and with what exception class.
        let kind = m
            .detail
            .split(" != ")
            .map(|s| {
                s.trim_start_matches("frap ")
                    .trim_start_matches("blender ")
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("/");
        return format!("lines:{at}:{kind}");
    }
    if m.comparator == "harness" {
        return format!("harness:{}", m.path);
    }
    // `meshes['Cube'].positions[3]` -> `meshes[].positions[]`; keep the field.
    let path = strip_keys(&m.path);
    let path = path.split('.').take(2).collect::<Vec<_>>().join(".");
    format!("{}:{path}", m.comparator)
}

/// How [`known`] classifies the mismatches of one run.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict<'a> {
    /// Every mismatch is explained (or a cascade entry explains the run): the deviation.
    Explained(&'static str),
    /// The first mismatch nothing explains.
    Unexplained(&'a Mismatch),
}

/// Classifies `mismatches` (non-empty) of `statements`.
pub fn classify<'a>(statements: &[String], mismatches: &'a [Mismatch]) -> Verdict<'a> {
    let explain = |m: &Mismatch| {
        let at = (m.comparator == "lines")
            .then(|| statement_at(statements, &m.path))
            .flatten();
        known::explain(statements, at, m)
    };
    if let Some(k) = mismatches.iter().filter_map(explain).find(|k| k.cascade) {
        return Verdict::Explained(k.deviation);
    }
    match mismatches.iter().find(|m| explain(m).is_none()) {
        Some(m) => Verdict::Unexplained(m),
        None => Verdict::Explained(
            mismatches
                .first()
                .and_then(explain)
                .map_or("?", |k| k.deviation),
        ),
    }
}

/// One minimised, unexplained mismatch.
#[derive(Clone, Debug)]
pub struct Finding {
    /// Its signature.
    pub signature: String,
    /// The trial seed it was found with.
    pub seed: u64,
    /// The minimised statements.
    pub statements: Vec<String>,
    /// The mismatches of the minimised sequence.
    pub mismatches: Vec<Mismatch>,
    /// How many trials hit this signature.
    pub hits: u64,
    /// Where the case was written.
    pub case: Option<PathBuf>,
}

/// Counters and findings of a run.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Trials run.
    pub trials: u64,
    /// Trials without any mismatch.
    pub clean: u64,
    /// Trials whose mismatches were all explained, by deviation.
    pub explained: BTreeMap<String, u64>,
    /// Trials where Blender failed (by first line of the error).
    pub blender_failures: BTreeMap<String, u64>,
    /// Unexplained mismatches by signature.
    pub findings: BTreeMap<String, Finding>,
    /// Statements run per callee (operator idname or RNA path).
    pub callees: BTreeMap<String, u64>,
    /// The first clean trials' seeds (candidates for `tests/golden/cases/fuzz/seed_<hex>.py`).
    pub clean_seeds: Vec<u64>,
    /// Wall time.
    pub elapsed: Duration,
}

impl Report {
    /// The summary printed at the end of a run.
    pub fn summary(&self) -> String {
        let ops = self
            .callees
            .keys()
            .filter(|k| k.contains('.') && !k.starts_with("bpy"))
            .count();
        let mut s = format!(
            "fuzz: {} trials in {:.0?} ({} clean, {} explained, {} Blender failures), {} \
             distinct operators and {} RNA statement kinds run, {} unexplained signature(s)\n",
            self.trials,
            self.elapsed,
            self.clean,
            self.explained.values().sum::<u64>(),
            self.blender_failures.values().sum::<u64>(),
            ops,
            self.callees.len() - ops,
            self.findings.len()
        );
        for (d, n) in &self.explained {
            s.push_str(&format!("  explained {d}: {n} trial(s)\n"));
        }
        for (e, n) in &self.blender_failures {
            s.push_str(&format!("  blender failure ({n}x): {e}\n"));
        }
        for f in self.findings.values() {
            s.push_str(&format!(
                "  UNEXPLAINED {} ({} hit(s), seed {:#x}, {} statement(s)){}\n",
                f.signature,
                f.hits,
                f.seed,
                f.statements.len(),
                f.case
                    .as_ref()
                    .map(|p| format!(" -> {}", p.display()))
                    .unwrap_or_default()
            ));
            for m in f.mismatches.iter().take(3) {
                s.push_str(&format!("      {m}\n"));
            }
        }
        s
    }

    /// `report.json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "trials": self.trials,
            "clean": self.clean,
            "explained": self.explained,
            "blender_failures": self.blender_failures,
            "elapsed_secs": self.elapsed.as_secs_f64(),
            "callees": self.callees,
            "clean_seeds": self.clean_seeds.iter().map(|s| format!("{s:#x}")).collect::<Vec<_>>(),
            "findings": self.findings.values().map(|f| serde_json::json!({
                "signature": f.signature,
                "seed": f.seed,
                "hits": f.hits,
                "statements": f.statements,
                "mismatches": f.mismatches.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "case": f.case.as_ref().map(|p| p.to_string_lossy().into_owned()),
            })).collect::<Vec<_>>(),
        })
    }
}

/// A finding's key: the mismatch signature and the callees of the minimised statements.
pub fn finding_key(sig: &str, statements: &[String]) -> String {
    let mut c: Vec<String> = statements.iter().map(|s| callee(s)).collect();
    c.sort();
    c.dedup();
    format!("{sig} @ {}", c.join(","))
}

/// The file name of a finding's case.
fn case_name(sig: &str) -> String {
    let mut s: String = sig
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_');
    s.chars().take(120).collect()
}

/// Minimises `statements` while a mismatch with signature `sig` remains.
pub fn minimise(
    repo: &Repo,
    props: &Props,
    blender: &Path,
    statements: &[String],
    sig: &str,
    budget: usize,
) -> (Vec<String>, Vec<Mismatch>) {
    let fails = |v: &[String]| {
        let r = run_both(repo, props, blender, v);
        r.blender_failure.is_none() && r.mismatches.iter().any(|m| signature(v, m) == sig)
    };
    let (min, _) = minimize::ddmin(statements, budget, fails);
    let r = run_both(repo, props, blender, &min);
    (min, r.mismatches)
}

/// Runs the fuzzer.
pub fn run(repo: &Repo, opts: &Options) -> Result<Report, String> {
    let props = Props::load(&repo.props_path()).map_err(|e| e.to_string())?;
    let _ = Catalog::get();
    std::fs::create_dir_all(&opts.out)
        .map_err(|e| format!("cannot create {}: {e}", opts.out.display()))?;
    let start = Instant::now();
    let next = AtomicU64::new(0);
    let report = Mutex::new(Report::default());
    let deadline = opts.duration.map(|d| start + d);
    let worker = || {
        loop {
            let i = next.fetch_add(1, Ordering::SeqCst);
            if opts.count.is_some_and(|c| i >= c) || deadline.is_some_and(|d| Instant::now() >= d) {
                break;
            }
            let seed = Rng::sequence_seed(opts.seed, i);
            let statements = sequence::generate(seed, opts.len);
            let r = run_both(repo, &props, &opts.blender, &statements);
            let mut rep = report
                .lock()
                .expect("invariant: no worker panics holding the lock");
            rep.trials += 1;
            for s in &statements {
                *rep.callees.entry(callee(s)).or_insert(0) += 1;
            }
            if let Some(e) = r.blender_failure {
                let first = e.lines().next().unwrap_or("").chars().take(160).collect();
                *rep.blender_failures.entry(first).or_insert(0) += 1;
                continue;
            }
            if r.mismatches.is_empty() {
                rep.clean += 1;
                if rep.clean_seeds.len() < MAX_CLEAN_SEEDS {
                    rep.clean_seeds.push(seed);
                }
                continue;
            }
            let first = match classify(&statements, &r.mismatches) {
                Verdict::Explained(d) => {
                    *rep.explained.entry(d.to_owned()).or_insert(0) += 1;
                    continue;
                }
                Verdict::Unexplained(m) => m,
            };
            let sig = signature(&statements, first);
            drop(rep);
            // Every unexplained hit is minimised: one coarse signature (`mesh_canonical:
            // meshes[].positions[]`) covers many bugs, which the minimised statements tell
            // apart (the finding key is the signature plus their callees).
            let (min, mism) = if opts.minimize {
                minimise(repo, &props, &opts.blender, &statements, &sig, opts.budget)
            } else {
                (statements.clone(), r.mismatches.clone())
            };
            // A minimised sequence that only shows explained mismatches is explained.
            if let Verdict::Explained(d) = classify(&min, &mism)
                && !mism.is_empty()
            {
                let mut rep = report
                    .lock()
                    .expect("invariant: no worker panics holding the lock");
                *rep.explained.entry(d.to_owned()).or_insert(0) += 1;
                continue;
            }
            let key = finding_key(&sig, &min);
            let mut rep = report
                .lock()
                .expect("invariant: no worker panics holding the lock");
            if let Some(f) = rep.findings.get_mut(&key) {
                f.hits += 1;
                continue;
            }
            let specs = compare_specs(&min);
            let text = case_text(
                &min,
                &specs,
                &format!(
                    "seed={seed:#x} (run seed {:#x}, trial {i}) signature={sig}",
                    opts.seed
                ),
            );
            let path = opts.out.join(format!("{}.py", case_name(&key)));
            let written = std::fs::write(&path, text).is_ok().then_some(path);
            rep.findings.insert(
                key.clone(),
                Finding {
                    signature: key,
                    seed,
                    statements: min,
                    mismatches: mism,
                    hits: 1,
                    case: written,
                },
            );
        }
    };
    std::thread::scope(|s| {
        for _ in 0..opts.jobs.max(1) {
            s.spawn(worker);
        }
    });
    let mut rep = report
        .into_inner()
        .map_err(|_| "a fuzz worker panicked".to_owned())?;
    rep.elapsed = start.elapsed();
    let json = serde_json::to_string_pretty(&rep.to_json()).unwrap_or_default();
    let p = opts.out.join("report.json");
    std::fs::write(&p, json + "\n").map_err(|e| format!("cannot write {}: {e}", p.display()))?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callees_and_signatures() {
        assert_eq!(
            callee("bpy.ops.mesh.subdivide(number_cuts=2)"),
            "mesh.subdivide"
        );
        assert_eq!(
            callee("bpy.context.object.data.vertices[3].co = (1.0, 0.0, 0.0)"),
            "bpy.context.object.data.vertices[].co"
        );
        assert_eq!(
            callee("bpy.data.objects['Cube.001'].select_set(True)"),
            "bpy.data.objects[].select_set"
        );
        let st = vec![
            "bpy.ops.mesh.primitive_cube_add()".to_owned(),
            "bpy.ops.mesh.delete(type='VERT')".to_owned(),
        ];
        let m = Mismatch::new(
            "lines",
            "line 5",
            "frap ok != blender RuntimeError: Operator bpy.ops.mesh.delete.poll() failed",
        );
        assert_eq!(signature(&st, &m), "lines:mesh.delete:ok/RuntimeError");
        let m = Mismatch::new("mesh_canonical", "meshes['Cube.001'].positions[3]", "x");
        assert_eq!(signature(&st, &m), "mesh_canonical:meshes[].positions[]");
        let m = Mismatch::new("rna_values", "objects['Cube'].location[2]", "x");
        assert_eq!(signature(&st, &m), "rna_values:objects[].location[]");
    }

    #[test]
    fn case_text_numbers_statements_after_the_preamble() {
        let st = vec!["bpy.ops.mesh.primitive_cube_add()".to_owned()];
        let t = case_text(
            &st,
            &[CompareSpec::Objects, CompareSpec::Mesh("Cube".into())],
            "seed=1",
        );
        let c = Case::parse(Path::new("fuzz/x.py"), &t).unwrap();
        assert_eq!(
            c.compare,
            [CompareSpec::Objects, CompareSpec::Mesh("Cube".into())]
        );
        assert_eq!(c.tier, crate::case::Tier::T2);
        let line = c.lines.iter().find(|l| l.text == st[0]).unwrap().line;
        assert_eq!(line, PREAMBLE_LINES + 1);
        assert_eq!(
            statement_at(&st, &format!("line {line}")),
            Some(st[0].as_str())
        );
        // `source` numbers the same way.
        assert_eq!(
            source(&st).lines().nth(line as usize - 1),
            Some(st[0].as_str())
        );
    }

    #[test]
    fn case_names_are_flat() {
        assert_eq!(
            case_name("lines:mesh.delete:ok/RuntimeError"),
            "lines_mesh_delete_ok_runtimeerror"
        );
    }

    #[test]
    fn compare_specs_name_the_final_meshes() {
        let st = vec![
            "bpy.ops.mesh.primitive_plane_add()".to_owned(),
            "bpy.ops.object.modifier_add(type='ARRAY')".to_owned(),
        ];
        let specs = compare_specs(&st);
        assert!(
            specs.contains(&CompareSpec::Mesh("Cube".into())),
            "{specs:?}"
        );
        assert!(
            specs.contains(&CompareSpec::Mesh("Plane".into())),
            "{specs:?}"
        );
        assert!(
            specs.contains(&CompareSpec::Evaluated("Plane".into())),
            "{specs:?}"
        );
        assert!(!specs.contains(&CompareSpec::Evaluated("Cube".into())));
    }
}
