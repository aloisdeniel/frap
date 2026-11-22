// SPDX-License-Identifier: GPL-3.0-or-later
//! Compares two sets of dumps (`spec/blender-5.2/` and a re-dump) and classifies every
//! difference as a drift item: operator / struct / node type / modifier type added or removed,
//! property renamed, new enum item, changed default, poll cell, ... (WP 8.9).
//!
//! The Blender version and build hash (`_meta.json`, `startup.json` `blender`) are not drift:
//! they are compared separately, so a newer 5.2.x with an unchanged API passes `--check`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::diff::{self, Change, Seg};

/// Dump files by path relative to the spec directory (`ops/object.json`, `startup.json`).
pub type Files = BTreeMap<String, Vec<u8>>;

/// A dump family: one `tools/blender/dump_<script>.py` and its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// `rna/<Struct>.json`
    Rna,
    /// `ops/<namespace>.json`
    Ops,
    /// `nodes/<bl_idname>.json`
    Nodes,
    /// `modifiers/<TYPE>.json`
    Modifiers,
    /// `startup.json`
    Startup,
}

impl Family {
    /// In dump order.
    pub const ALL: [Family; 5] = [
        Family::Rna,
        Family::Ops,
        Family::Nodes,
        Family::Modifiers,
        Family::Startup,
    ];

    /// `dump_<script>.py`.
    pub fn script(self) -> &'static str {
        match self {
            Family::Rna => "rna",
            Family::Ops => "ops",
            Family::Nodes => "nodes",
            Family::Modifiers => "modifiers",
            Family::Startup => "startup",
        }
    }

    /// The output path relative to the spec directory (a directory, or `startup.json`).
    pub fn output(self) -> &'static str {
        match self {
            Family::Startup => "startup.json",
            f => f.script(),
        }
    }

    /// Report section title.
    pub fn title(self) -> &'static str {
        match self {
            Family::Rna => "RNA structs (`rna/`)",
            Family::Ops => "Operators (`ops/`)",
            Family::Nodes => "Shader nodes (`nodes/`)",
            Family::Modifiers => "Modifier types (`modifiers/`)",
            Family::Startup => "Factory startup (`startup.json`)",
        }
    }

    /// The family of a dump file.
    pub fn of(rel: &str) -> Option<Family> {
        if rel == "startup.json" {
            return Some(Family::Startup);
        }
        let (dir, _) = rel.split_once('/')?;
        Family::ALL
            .into_iter()
            .find(|f| *f != Family::Startup && f.script() == dir)
    }
}

/// What a drift item is about.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subject {
    /// `object.select_pattern`
    Operator(String),
    /// An operator namespace file (`ops/object.json`) as a whole.
    Namespace(String),
    /// `Object`
    Struct(String),
    /// `ShaderNodeMath`
    Node(String),
    /// `BEVEL`
    Modifier(String),
    /// `startup.json`
    Startup,
    /// A `_meta.json` (struct, namespace and type lists, enums of the dump).
    Meta(String),
}

impl Subject {
    /// How the subject is written in the report.
    pub fn label(&self) -> String {
        match self {
            Subject::Operator(s)
            | Subject::Namespace(s)
            | Subject::Struct(s)
            | Subject::Node(s)
            | Subject::Modifier(s)
            | Subject::Meta(s) => s.clone(),
            Subject::Startup => "startup.json".into(),
        }
    }
}

/// One classified difference.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub family: Family,
    /// Dump file, relative to the spec directory.
    pub file: String,
    /// `operator removed`, `property renamed`, `enum item added`, `default changed`,
    /// `poll cell changed`, ...
    pub category: String,
    pub subject: Subject,
    /// Property, function, parameter, socket or value of the subject.
    pub member: Option<String>,
    /// Human-readable change (`1` → `2`).
    pub detail: String,
    /// JSON path inside the file.
    pub path: String,
    /// Renames: (old key, new key).
    pub renamed: Option<(String, String)>,
}

impl Item {
    /// Markdown: `` `object.select_pattern` `level` `` / `` `Object.location` ``.
    pub fn locus_md(&self) -> String {
        match (&self.subject, &self.member) {
            (Subject::Struct(_) | Subject::Modifier(_), _) | (_, None) => {
                format!("`{}`", self.locus())
            }
            (s, Some(m)) => format!("`{}` `{m}`", s.label()),
        }
    }

    /// `object.select_pattern` / `Object.location` / `Object.select_get(view_layer)`.
    pub fn locus(&self) -> String {
        let s = self.subject.label();
        match (&self.subject, &self.member) {
            (_, None) => s,
            (Subject::Struct(_) | Subject::Modifier(_), Some(m)) => format!("{s}.{m}"),
            (_, Some(m)) => format!("{s} {m}"),
        }
    }
}

/// Blender version and build hash recorded by a set of dumps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    pub version: String,
    pub build_hash: String,
}

/// The result of comparing two dump sets.
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    /// Files in either set.
    pub files: usize,
    /// Byte-identical files.
    pub identical: usize,
    /// Files that differ only in the recorded Blender version / build hash.
    pub version_only: Vec<String>,
    /// Version recorded by the old (spec) and the new dumps.
    pub old_version: Option<Recorded>,
    pub new_version: Option<Recorded>,
    pub items: Vec<Item>,
}

impl Comparison {
    /// Any API drift (version-only differences are not drift).
    pub fn has_drift(&self) -> bool {
        !self.items.is_empty()
    }
    /// The byte-level differences (drift and version fields): what a refresh writes.
    pub fn differs(&self) -> bool {
        self.identical != self.files
    }
}

/// Paths of the version fields in a dump file.
fn version_paths(rel: &str) -> Option<[&'static [&'static str]; 2]> {
    if rel == "startup.json" {
        Some([&["blender", "blender_version"], &["blender", "build_hash"]])
    } else if rel.ends_with("/_meta.json") {
        Some([&["blender_version"], &["build_hash"]])
    } else {
        None
    }
}

fn take(v: &mut Value, path: &[&str]) -> Option<Value> {
    let (last, parents) = path.split_last()?;
    let mut cur = v;
    for p in parents {
        cur = cur.get_mut(*p)?;
    }
    cur.as_object_mut()?.shift_remove(*last)
}

/// Removes the version fields; returns them.
fn split_version(rel: &str, v: &mut Value) -> Option<Recorded> {
    let [vp, hp] = version_paths(rel)?;
    let version = take(v, vp)?.as_str()?.to_owned();
    let build_hash = take(v, hp)?.as_str()?.to_owned();
    Some(Recorded {
        version,
        build_hash,
    })
}

fn stem(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_suffix(".json").unwrap_or(name).to_owned()
}

fn file_subject(family: Family, rel: &str) -> Subject {
    let s = stem(rel);
    if s == "_meta" {
        return Subject::Meta(rel.to_owned());
    }
    match family {
        Family::Rna => Subject::Struct(s),
        Family::Ops => Subject::Namespace(s),
        Family::Nodes => Subject::Node(s),
        Family::Modifiers => Subject::Modifier(s),
        Family::Startup => Subject::Startup,
    }
}

fn file_kind(family: Family) -> &'static str {
    match family {
        Family::Rna => "struct",
        Family::Ops => "namespace",
        Family::Nodes => "node type",
        Family::Modifiers => "modifier type",
        Family::Startup => "file",
    }
}

/// Compact JSON for the report, shortened to 72 characters.
pub fn short(v: &Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    if s.chars().count() > 72 {
        let mut t: String = s.chars().take(71).collect();
        t.push('…');
        t
    } else {
        s
    }
}

/// Compares the old (`spec/blender-5.2`) and new dumps.
pub fn compare(old: &Files, new: &Files) -> Comparison {
    let mut c = Comparison::default();
    let names: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    c.files = names.len();
    for rel in names {
        let Some(family) = Family::of(rel) else {
            continue;
        };
        let item = |category: String, detail: String| Item {
            family,
            file: rel.clone(),
            category,
            subject: file_subject(family, rel),
            member: None,
            detail,
            path: String::new(),
            renamed: None,
        };
        let (a, b) = match (old.get(rel), new.get(rel)) {
            (Some(a), Some(b)) => (a, b),
            (Some(_), None) => {
                c.items.push(item(
                    format!("{} removed", file_kind(family)),
                    format!("`{rel}` is not dumped any more"),
                ));
                continue;
            }
            (None, Some(_)) => {
                c.items.push(item(
                    format!("{} added", file_kind(family)),
                    format!("new file `{rel}`"),
                ));
                continue;
            }
            (None, None) => unreachable!("name taken from one of the maps"),
        };
        if a == b {
            c.identical += 1;
            // Still record the versions.
            if let Ok(mut v) = serde_json::from_slice::<Value>(a) {
                let r = split_version(rel, &mut v);
                if c.old_version.is_none() {
                    c.old_version = r.clone();
                }
                if c.new_version.is_none() {
                    c.new_version = r;
                }
            }
            continue;
        }
        let (Ok(mut va), Ok(mut vb)) = (
            serde_json::from_slice::<Value>(a),
            serde_json::from_slice::<Value>(b),
        ) else {
            c.items.push(item(
                "unreadable".into(),
                format!("`{rel}` is not valid JSON"),
            ));
            continue;
        };
        let (ra, rb) = (split_version(rel, &mut va), split_version(rel, &mut vb));
        if c.old_version.is_none() {
            c.old_version = ra.clone();
        }
        if c.new_version.is_none() {
            c.new_version = rb.clone();
        }
        if va == vb {
            if ra != rb {
                c.version_only.push(rel.clone());
            } else {
                c.items.push(item(
                    "formatting changed".into(),
                    "same JSON content, different bytes (number format, key order or \
                     whitespace)"
                        .into(),
                ));
            }
            continue;
        }
        for ch in diff::diff(&va, &vb) {
            if let Some(it) = classify(family, rel, &ch) {
                c.items.push(it);
            }
        }
    }
    c
}

/// What kind of member a path segment pair names.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    None,
    Property,
    Function,
    Parameter,
    Socket,
    Value,
    ContextMember,
    Fresh,
}

impl Level {
    fn noun(self) -> &'static str {
        match self {
            Level::None => "",
            Level::Property => "property",
            Level::Function => "function",
            Level::Parameter => "parameter",
            Level::Socket => "socket",
            Level::Value => "property",
            Level::ContextMember => "context member",
            Level::Fresh => "fresh value",
        }
    }
}

fn key(s: &Seg) -> Option<&str> {
    match s {
        Seg::Key(k) => Some(k),
        Seg::Id(_) => None,
    }
}

fn id(s: &Seg) -> Option<&str> {
    match s {
        Seg::Id(k) => Some(k),
        Seg::Key(_) => None,
    }
}

/// Splits `rest` (the path below the subject) into (level, member, tail).
fn member(family: Family, rest: &[Seg]) -> (Level, Option<String>, usize) {
    let k0 = rest.first().and_then(key);
    match (family, k0, rest.get(1)) {
        (Family::Rna, Some("funcs"), Some(Seg::Id(f))) => {
            match (rest.get(2).and_then(key), rest.get(3).and_then(id)) {
                (Some("params" | "returns"), Some(p)) => {
                    (Level::Parameter, Some(format!("{f}({p})")), 4)
                }
                _ => (Level::Function, Some(format!("{f}()")), 2),
            }
        }
        (Family::Rna | Family::Ops, Some("props"), Some(Seg::Id(p))) => {
            (Level::Property, Some(p.clone()), 2)
        }
        (Family::Rna, Some("context_members"), Some(Seg::Key(m))) => {
            (Level::ContextMember, Some(m.clone()), 2)
        }
        (Family::Rna, Some("fresh"), Some(Seg::Key(d))) if d == "differs_from_default" => {
            match rest.get(2).and_then(key) {
                Some(m) => (Level::Fresh, Some(m.to_owned()), 3),
                None => (Level::None, None, 0),
            }
        }
        (Family::Nodes, Some(io @ ("inputs" | "outputs")), Some(Seg::Id(s))) => {
            (Level::Socket, Some(format!("{io}[{s}]")), 2)
        }
        (Family::Nodes, Some("props"), Some(Seg::Key(p))) => (Level::Value, Some(p.clone()), 2),
        (Family::Modifiers, Some("values"), Some(Seg::Key(p))) => {
            (Level::Value, Some(p.clone()), 2)
        }
        _ => (Level::None, None, 0),
    }
}

/// What a rename inside the array at `rest` renames, and the member that owns the array
/// (`props` → a property; `props[mode].items` → an enum item of `mode`).
fn renamed_what(family: Family, rest: &[Seg]) -> (Option<String>, &'static str) {
    let Some((Seg::Key(last), parent)) = rest.split_last() else {
        return (None, "element");
    };
    let (_, owner, _) = member(family, parent);
    match last.as_str() {
        "props" => (owner, "property"),
        "funcs" => (None, "function"),
        "params" | "returns" => (owner, "parameter"),
        "inputs" | "outputs" => (None, "socket"),
        "items" | "items_static" => (owner, "enum item"),
        "operators" => (None, "operator"),
        _ => (owner, "element"),
    }
}

fn classify(family: Family, rel: &str, ch: &Change) -> Option<Item> {
    let at = ch.at();
    let (subject, rest): (Subject, &[Seg]) = match family {
        _ if rel.ends_with("/_meta.json") => (Subject::Meta(rel.to_owned()), &at[..]),
        Family::Ops => match (at.first().and_then(key), at.get(1)) {
            (Some("count"), _) => return None, // derived from `operators`
            (Some("operators"), Some(Seg::Id(op))) => (Subject::Operator(op.clone()), &at[2..]),
            _ => (Subject::Namespace(stem(rel)), &at[..]),
        },
        _ => (file_subject(family, rel), &at[..]),
    };
    let mut it = Item {
        family,
        file: rel.to_owned(),
        category: String::new(),
        subject,
        member: None,
        detail: String::new(),
        path: diff::fmt_path(at),
        renamed: None,
    };
    let is_meta = matches!(it.subject, Subject::Meta(_));

    if let Change::Renamed {
        from, to, value, ..
    } = ch
    {
        let (owner, noun) = renamed_what(family, rest);
        if noun == "operator" {
            it.subject = Subject::Operator(to.clone());
        }
        it.member = owner;
        it.category = format!("{noun} renamed");
        it.detail = format!("`{from}` → `{to}`");
        if noun == "enum item"
            && let Some(n) = value.get("name").and_then(Value::as_str)
        {
            it.detail.push_str(&format!(" (\"{n}\")"));
        }
        it.renamed = Some((from.clone(), to.clone()));
        return Some(it);
    }
    if let Change::Reordered { old, new, .. } = ch {
        let (_, m, tail) = member(family, rest);
        it.member = m;
        let what = diff::fmt_path(&rest[tail..]);
        it.category = "order changed".into();
        it.detail = format!(
            "`{}` order: {} → {}",
            if what.is_empty() { "elements" } else { &what },
            old.join(", "),
            new.join(", ")
        );
        return Some(it);
    }

    let (level, m, tail_at) = if is_meta {
        (Level::None, None, 0)
    } else {
        member(family, rest)
    };
    it.member = m;
    let tail = &rest[tail_at..];
    let tail_s = diff::fmt_path(tail);
    let verb = match ch {
        Change::Added { .. } => "added",
        Change::Removed { .. } => "removed",
        _ => "changed",
    };
    let shown = match ch {
        Change::Added { value, .. } | Change::Removed { value, .. } => Some(value),
        _ => None,
    };

    // The subject or member itself appeared or disappeared.
    if let (true, Some(v)) = (tail.is_empty(), shown) {
        let noun = match (&it.subject, level) {
            (Subject::Operator(_), Level::None) => "operator",
            (_, l) if l != Level::None => l.noun(),
            _ => "member",
        };
        it.category = format!("{noun} {verb}");
        it.detail = match (noun, level) {
            ("operator", _) => v
                .get("label")
                .and_then(Value::as_str)
                .map(|l| format!("\"{l}\""))
                .unwrap_or_default(),
            (_, Level::Property | Level::Parameter) => {
                let t = v.get("type").and_then(Value::as_str).unwrap_or("?");
                match v.get("default") {
                    Some(d) => format!("{t}, default `{}`", short(d)),
                    None => t.to_owned(),
                }
            }
            _ => format!("`{}`", short(v)),
        };
        return Some(it);
    }

    let first = tail.first().and_then(key);
    let elem = tail.get(1).and_then(id);
    it.category = match (first, elem, ch) {
        _ if is_meta => match (tail.len(), ch) {
            (2, Change::Added { .. } | Change::Removed { .. }) if elem.is_some() => {
                format!("meta list {verb}")
            }
            _ => "meta changed".into(),
        },
        (
            Some("items" | "items_static"),
            Some(_),
            Change::Added { .. } | Change::Removed { .. },
        ) if tail.len() == 2 => {
            format!("enum item {verb}")
        }
        (Some("items" | "items_static"), _, _) => "enum item changed".into(),
        (Some("items_by_context" | "dynamic_hint"), _, _) => "dynamic enum changed".into(),
        (Some("default" | "default_value"), _, _) => "default changed".into(),
        (Some("poll"), _, _) => format!("poll cell {verb}"),
        (Some("options"), _, _) => "bl_options changed".into(),
        (Some("flags"), _, _) => format!("flag {verb}"),
        (Some("hard_min" | "hard_max" | "soft_min" | "soft_max" | "step" | "precision"), _, _) => {
            "range changed".into()
        }
        (Some("type" | "subtype" | "unit" | "array" | "fixed_type"), _, _) => "type changed".into(),
        (Some("enabled"), _, _) if level == Level::Socket => "socket availability changed".into(),
        (Some("availability"), _, _) => "socket availability changed".into(),
        _ => match level {
            Level::Value if tail.is_empty() => "default changed".into(),
            Level::None => match &it.subject {
                Subject::Operator(_) => "operator changed".into(),
                Subject::Struct(_) => "struct changed".into(),
                Subject::Node(_) => "node changed".into(),
                Subject::Modifier(_) => "modifier changed".into(),
                Subject::Startup => "factory state changed".into(),
                Subject::Namespace(_) | Subject::Meta(_) => "meta changed".into(),
            },
            l => format!("{} changed", l.noun()),
        },
    };
    it.detail = match ch {
        Change::Changed { old, new, .. } => {
            let what = if tail_s.is_empty() {
                String::new()
            } else {
                format!("`{tail_s}`: ")
            };
            format!("{what}`{}` → `{}`", short(old), short(new))
        }
        Change::Added { value, .. } | Change::Removed { value, .. } => {
            let sign = if verb == "added" { "+" } else { "−" };
            match (first, elem) {
                (Some("items" | "items_static"), Some(e)) if tail.len() == 2 => {
                    let n = value.get("name").and_then(Value::as_str).unwrap_or("");
                    let val = value.get("value").map(short).unwrap_or_default();
                    format!("{sign} `{e}` (\"{n}\", value {val})")
                }
                (_, Some(e)) if tail.len() == 2 => {
                    format!("`{}` {sign} `{e}`", diff::fmt_path(&tail[..1]))
                }
                _ => format!("{sign} `{tail_s}` = `{}`", short(value)),
            }
        }
        _ => unreachable!("renames and reorders returned above"),
    };
    Some(it)
}
