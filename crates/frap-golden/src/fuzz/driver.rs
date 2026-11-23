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

use std::collections::BTreeSet;

use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::{Main, ObjectMode};
use frap_session::{Session, SessionOptions, Startup as SessionStartup};

use super::catalog::Catalog;
use super::known::{self, Outcome, Trace};
use super::minimize;
use super::rng::Rng;
use super::sequence;
use crate::blender::{self, Repo, ScratchDir};
use crate::case::{Case, CompareSpec, Mismatch};
use crate::runner::{self, Props, RunOptions};
use crate::state::LineOutcome;

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
    probe(statements).0
}

/// A frap run of `statements`, one at a time: the compared items ([`compare_specs`]) and the
/// meshes and objects each statement acted on ([`Trace`] without the outcomes, which come
/// from the compared run of both tools).
pub fn probe(statements: &[String]) -> (Vec<CompareSpec>, Trace) {
    let mut specs = vec![CompareSpec::Objects, CompareSpec::Context];
    let mut trace = Trace::default();
    let work = match ScratchDir::new("fuzz-probe") {
        Ok(w) => w,
        Err(_) => return (specs, trace),
    };
    let mut so = SessionOptions::default();
    so.cwd = work.path().to_owned();
    so.startup = SessionStartup::Factory;
    let Ok(mut s) = Session::new(so) else {
        return (specs, trace);
    };
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.exec_source_at("import bpy\n", PREAMBLE_LINES);
    }));
    if ran.is_err() {
        return (specs, trace);
    }
    // Per statement, by identity (names change: renames, `.001` suffixes).
    let mut acted: Vec<Acted> = Vec::new();
    for (i, st) in statements.iter().enumerate() {
        let line = PREAMBLE_LINES + 1 + i as u32;
        let before = (acted_on(s.main(), ""), all_ids(s.main()));
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.exec_source_at(&format!("{st}\n"), line);
        }));
        if ran.is_err() {
            return (specs, trace);
        }
        acted.push(acted_on(s.main(), st));
        // Data made from other data (`mesh.separate`, `object.duplicate`, `object.convert`)
        // carries the history of what it came from: every earlier statement that acted on
        // the source acted on the copy. Added primitives and objects (`…_add(`) start anew.
        if !st.contains("_add(") {
            let now = all_ids(s.main());
            let new_meshes: Vec<_> = now.0.difference(&before.1.0).copied().collect();
            let new_objects: Vec<_> = now.1.difference(&before.1.1).copied().collect();
            let (src_meshes, src_objects) = &before.0;
            for a in acted.iter_mut().take(i) {
                if !a.0.is_disjoint(src_meshes) {
                    a.0.extend(new_meshes.iter().copied());
                }
                if !a.1.is_disjoint(src_objects) {
                    a.1.extend(new_objects.iter().copied());
                }
            }
        }
    }
    let main = s.main();
    let mesh_name = |r: IdRef<MeshId>| main.meshes.get(r).map(|m| m.header.name.to_string());
    let object_name = |r: IdRef<ObjectId>| main.objects.get(r).map(|o| o.header.name.to_string());
    for (meshes, objects) in acted {
        trace
            .meshes
            .push(meshes.into_iter().filter_map(mesh_name).collect());
        trace
            .objects
            .push(objects.into_iter().filter_map(object_name).collect());
    }
    let (mut meshes, mut evaluated) = (Vec::new(), Vec::new());
    for (_, o) in main.objects.iter() {
        if o.mode == ObjectMode::Edit {
            trace.edit_end.insert(o.header.name.to_string());
        }
        let Some(m) = o
            .data
            .and_then(IdRef::<MeshId>::from_any)
            .and_then(|r| main.meshes.get(r))
        else {
            continue;
        };
        trace
            .object_mesh
            .insert(o.header.name.to_string(), m.header.name.to_string());
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
    (specs, trace)
}

/// Every mesh and object of `main`.
fn all_ids(main: &Main) -> Acted {
    (
        main.meshes.iter().map(|(r, _)| r).collect(),
        main.objects.iter().map(|(r, _)| r).collect(),
    )
}

/// The meshes and objects one statement acted on, by identity.
type Acted = (BTreeSet<IdRef<MeshId>>, BTreeSet<IdRef<ObjectId>>);

/// What a statement acted on, read after it ran: the objects active, selected or in Edit
/// Mode in the window's view layer (the factory scene's), those it names in quotes, and
/// their meshes, plus meshes it names.
fn acted_on(main: &Main, statement: &str) -> Acted {
    let mut objects = BTreeSet::new();
    if let Some(layer) = main
        .scenes
        .iter()
        .next()
        .and_then(|(_, sc)| sc.view_layers.first())
    {
        objects.extend(layer.active);
        objects.extend(
            layer
                .bases
                .iter()
                .filter(|b| b.selected())
                .map(|b| b.object),
        );
    }
    objects.extend(
        main.objects
            .iter()
            .filter(|(_, o)| o.mode != ObjectMode::Object)
            .map(|(r, _)| r),
    );
    let quoted: Vec<&str> = statement.split('\'').skip(1).step_by(2).collect();
    objects.extend(quoted.iter().filter_map(|n| main.objects.find(n)));
    let mut meshes: BTreeSet<IdRef<MeshId>> = objects
        .iter()
        .filter_map(|r| main.objects.get(*r))
        .filter_map(|o| o.data.and_then(IdRef::<MeshId>::from_any))
        .collect();
    meshes.extend(quoted.iter().filter_map(|n| main.meshes.find(n)));
    (meshes, objects)
}

/// The statements as case source with the preamble's line numbering (statement `i` on line
/// `PREAMBLE_LINES + 1 + i`), for running outside a case file.
#[cfg(test)]
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
    /// What each statement did (for [`classify`]).
    pub trace: Trace,
}

/// Runs `statements` in both tools and compares.
pub fn run_both(repo: &Repo, props: &Props, blender: &Path, statements: &[String]) -> RunResult {
    let (specs, mut trace) = probe(statements);
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
        (Ok(f), Ok(b)) => {
            let mismatches = runner::compare(&case, &f.state, &b);
            trace.outcomes = outcomes(statements.len(), &f.state.lines, &mismatches);
            RunResult {
                mismatches,
                blender_failure: None,
                trace,
            }
        }
    }
}

/// Each statement's [`Outcome`]: [`Outcome::Differs`] where the `lines` comparator found a
/// mismatch (it normalises messages), else frap's outcome.
fn outcomes(n: usize, frap: &[LineOutcome], mismatches: &[Mismatch]) -> Vec<Outcome> {
    (0..n)
        .map(|i| {
            let line = PREAMBLE_LINES + 1 + i as u32;
            let path = format!("line {line}");
            if mismatches
                .iter()
                .any(|m| m.comparator == "lines" && m.path == path)
            {
                return Outcome::Differs;
            }
            match frap.iter().find(|l| l.line == line) {
                Some(l) if l.ok => Outcome::Ok,
                Some(l) => Outcome::Failed(l.message.clone().unwrap_or_default()),
                None => Outcome::Differs,
            }
        })
        .collect()
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

/// Classifies `mismatches` (non-empty) of `statements` by cause ([`known::explain_all`]).
pub fn classify<'a>(
    statements: &[String],
    trace: &Trace,
    mismatches: &'a [Mismatch],
) -> Verdict<'a> {
    let explained = known::explain_all(statements, trace, mismatches);
    match mismatches.iter().zip(&explained).find(|(_, k)| k.is_none()) {
        Some((m, _)) => Verdict::Unexplained(m),
        None => Verdict::Explained(
            explained
                .first()
                .copied()
                .flatten()
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
    /// Trial seeds whose unexplained mismatch did not come back when re-run.
    pub unreproduced: Vec<u64>,
    /// Unexplained mismatches by signature.
    pub findings: BTreeMap<String, Finding>,
    /// Statements run per callee (operator idname or RNA path).
    pub callees: BTreeMap<String, u64>,
    /// The first clean trials' seeds (candidates for `tests/golden/cases/fuzz/seed_<hex>.py`).
    pub clean_seeds: Vec<u64>,
    /// Wall time.
    pub elapsed: Duration,
}

/// A run is unhealthy when more than this fraction of its trials could not run in Blender.
pub const MAX_BLENDER_FAILURE_RATE: f64 = 0.05;

impl Report {
    /// Why the run did not really compare the tools, if so: no clean trial, or more than
    /// [`MAX_BLENDER_FAILURE_RATE`] of the trials failed in Blender (crash, no dump,
    /// timeout). A run that compared nothing must not pass for a clean one.
    pub fn health_problem(&self) -> Option<String> {
        let failures: u64 = self.blender_failures.values().sum();
        if self.trials > 0 && self.clean == 0 {
            return Some(format!(
                "no clean trial out of {} ({failures} Blender failures)",
                self.trials
            ));
        }
        if failures as f64 > MAX_BLENDER_FAILURE_RATE * self.trials as f64 {
            return Some(format!(
                "Blender failed on {failures} of {} trials (more than {:.0}%)",
                self.trials,
                MAX_BLENDER_FAILURE_RATE * 100.0
            ));
        }
        None
    }

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
        if let Some(p) = self.health_problem() {
            s.push_str(&format!("  UNHEALTHY RUN: {p}\n"));
        }
        for (d, n) in &self.explained {
            s.push_str(&format!("  explained {d}: {n} trial(s)\n"));
        }
        if !self.unreproduced.is_empty() {
            s.push_str(&format!(
                "  {} unexplained mismatch(es) did not come back on a re-run (seeds {})\n",
                self.unreproduced.len(),
                self.unreproduced
                    .iter()
                    .map(|x| format!("{x:#x}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
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
            "healthy": self.health_problem().is_none(),
            "health_problem": self.health_problem(),
            "unreproduced": self.unreproduced.iter().map(|s| format!("{s:#x}")).collect::<Vec<_>>(),
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

/// Whether `r` shows a mismatch with signature `sig` that [`known`] does not explain.
fn shows_unexplained(statements: &[String], r: &RunResult, sig: &str) -> bool {
    r.blender_failure.is_none()
        && known::explain_all(statements, &r.trace, &r.mismatches)
            .iter()
            .zip(&r.mismatches)
            .any(|(k, m)| k.is_none() && signature(statements, m) == sig)
}

/// Minimises `statements` while an unexplained mismatch with signature `sig` remains (so
/// the minimised sequence cannot drift to an explained mismatch of the same signature);
/// the minimised statements and their run.
pub fn minimise(
    repo: &Repo,
    props: &Props,
    blender: &Path,
    statements: &[String],
    sig: &str,
    budget: usize,
) -> (Vec<String>, RunResult) {
    let fails = |v: &[String]| shows_unexplained(v, &run_both(repo, props, blender, v), sig);
    let (min, _) = minimize::ddmin(statements, budget, fails);
    let r = run_both(repo, props, blender, &min);
    (min, r)
}

/// The key a Blender failure is counted under: its exit status and the last line it wrote
/// (the scratch paths of the first line differ on every trial).
fn failure_key(e: &str) -> String {
    let first = e.lines().next().unwrap_or("");
    let key = match first.split_once(" exited with ") {
        Some((_, status)) => {
            let last = e
                .lines()
                .rev()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with("---"))
                .unwrap_or("");
            format!("Blender exited with {status}: {last}")
        }
        None => first.to_owned(),
    };
    key.chars().take(160).collect()
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
                let first = failure_key(&e);
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
            let first = match classify(&statements, &r.trace, &r.mismatches) {
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
            let (min, mr) = if opts.minimize {
                minimise(repo, &props, &opts.blender, &statements, &sig, opts.budget)
            } else {
                (statements.clone(), r.clone())
            };
            // The unexplained mismatch did not come back when the sequence ran again (Blender
            // is not deterministic everywhere: D-19): reported, not a finding.
            if opts.minimize && !shows_unexplained(&min, &mr, &sig) {
                let mut rep = report
                    .lock()
                    .expect("invariant: no worker panics holding the lock");
                rep.unreproduced.push(seed);
                continue;
            }
            let mism = mr.mismatches;
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
    fn blender_failures_are_keyed_without_paths() {
        let e = "/x/Blender on /tmp/frap-golden-1-2-fuzz/case.py exited with signal: 9 (SIGKILL)\n\
                 --- stdout (tail)\nRead prefs\n--- stderr (tail)\nError: oops\n";
        assert_eq!(
            failure_key(e),
            "Blender exited with signal: 9 (SIGKILL): Error: oops"
        );
        assert_eq!(failure_key("cannot start x"), "cannot start x");
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

    #[test]
    fn probe_traces_what_each_statement_acted_on() {
        let st: Vec<String> = [
            "bpy.ops.mesh.primitive_plane_add()",
            "bpy.context.object.data.name = 'MeshData'",
            "bpy.data.objects['Cube'].select_set(True)",
            "bpy.ops.object.mode_set(mode='EDIT')",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let (_, t) = probe(&st);
        let set = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
        // Names at the end of the sequence (the plane's mesh was renamed).
        assert_eq!(t.meshes[0], set(&["MeshData"]));
        assert_eq!(t.objects[0], set(&["Plane"]));
        assert_eq!(t.meshes[2], set(&["MeshData", "Cube"]));
        assert_eq!(t.objects[3], set(&["Plane", "Cube"]));
        assert_eq!(t.edit_end, set(&["Plane", "Cube"]));
        // Copies share the history of what they were made from (of every selected object:
        // which copy came from which is not tracked); an added object does not.
        let st: Vec<String> = [
            "bpy.ops.mesh.primitive_plane_add()",
            "bpy.data.objects['Cube'].select_set(True)",
            "bpy.ops.object.duplicate()",
            "bpy.ops.mesh.primitive_cone_add()",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let (_, t) = probe(&st);
        assert_eq!(t.objects[0], set(&["Plane", "Plane.001", "Cube.001"]));
        assert_eq!(
            t.objects[1],
            set(&["Plane", "Cube", "Plane.001", "Cube.001"])
        );
        assert_eq!(t.objects[3], set(&["Cone"]));
    }
}
