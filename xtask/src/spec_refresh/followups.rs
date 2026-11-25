// SPDX-License-Identifier: GPL-3.0-or-later
//! The follow-ups of a drift (WP 8.9): what `cargo xtask codegen` would change (a dry run on a
//! shadow workspace whose `spec/blender-5.2/` is the re-dump), the `docs/OPERATORS.md` rows to
//! revisit, and the `.frap` migrations a rename or removal needs (§15: `.frap` keys and enum
//! values are RNA identifiers).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::compare::{Comparison, Family, Item, Subject};

/// Result of the codegen dry run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Codegen {
    /// No drift: nothing to regenerate.
    Skipped,
    /// The re-dump generates exactly the committed code.
    Clean,
    /// Generated files that would change, and committed generated files that would go stale.
    Changes {
        changed: Vec<String>,
        stale: Vec<String>,
    },
    /// Codegen refuses the re-dump (e.g. an operator OPERATORS.md lists is gone, a new dynamic
    /// enum without an overlay entry).
    Fails(String),
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let rd = std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?;
    for e in rd.flatten() {
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t)?;
        } else {
            std::fs::copy(&p, &t).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", from.display()))
}

/// Builds `shadow/` = the inputs of codegen (`spec/overlay`, `spec/frap`,
/// `spec/current_milestone`, `docs/OPERATORS.md` without the §A rows of `drop_ops`, the
/// sources `gen_packed` packs) with `dumps/` as `spec/blender-5.2/`. Returns the operators
/// whose rows were dropped.
fn build_shadow(
    root: &Path,
    dumps: &Path,
    shadow: &Path,
    drop_ops: &[String],
) -> Result<Vec<String>, String> {
    let _ = std::fs::remove_dir_all(shadow);
    copy_dir(&root.join("spec/overlay"), &shadow.join("spec/overlay"))?;
    copy_dir(&root.join("spec/frap"), &shadow.join("spec/frap"))?;
    copy_file(
        &root.join("spec/current_milestone"),
        &shadow.join("spec/current_milestone"),
    )?;
    let md_path = root.join("docs/OPERATORS.md");
    let md =
        std::fs::read_to_string(&md_path).map_err(|e| format!("{}: {e}", md_path.display()))?;
    let rows = parse_rows(&md);
    let mut dropped = Vec::new();
    let mut skip = std::collections::BTreeSet::new();
    for op in drop_ops {
        for r in rows_for(&rows, &Subject::Operator(op.clone())) {
            skip.insert(r.line);
            dropped.push(op.clone());
        }
    }
    let mut out: String = md
        .lines()
        .enumerate()
        .filter(|(i, _)| !skip.contains(&(i + 1)))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    let d = shadow.join("docs");
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    std::fs::write(d.join("OPERATORS.md"), out).map_err(|e| format!("{}: {e}", d.display()))?;
    let spec = shadow.join("spec/blender-5.2");
    for f in Family::ALL {
        let src = dumps.join(f.output());
        if src.is_dir() {
            copy_dir(&src, &spec.join(f.output()))?;
        } else if src.is_file() {
            copy_file(&src, &spec.join(f.output()))?;
        }
    }
    // The packed data files (LUTs, EEVEE textures; the startup dump when the re-dump has none).
    for (src, _) in crate::codegen::gen_packed::PACKED {
        if !shadow.join(src).exists() {
            copy_file(&root.join(src), &shadow.join(src))?;
        }
    }
    Ok(dropped)
}

/// `cargo xtask codegen` on the re-dump, compared with the committed generated code. The
/// OPERATORS.md rows of `removed_ops` are deleted first (their follow-up), otherwise codegen
/// stops at the first one; returns the outcome and the operators whose rows were deleted.
pub fn codegen(
    root: &Path,
    dumps: &Path,
    shadow: &Path,
    removed_ops: &[String],
) -> (Codegen, Vec<String>) {
    let dropped = match build_shadow(root, dumps, shadow, removed_ops) {
        Ok(d) => d,
        Err(e) => {
            return (
                Codegen::Fails(format!("cannot prepare the dry run: {e}")),
                vec![],
            );
        }
    };
    let r = crate::codegen::generate(shadow);
    let out = match r {
        Err(e) => Codegen::Fails(e),
        Ok(files) => {
            let rerooted: crate::codegen::Files = files
                .into_iter()
                .map(|(p, c)| {
                    let rel = p.strip_prefix(shadow).map(Path::to_path_buf).unwrap_or(p);
                    (root.join(rel), c)
                })
                .collect();
            let (changed, stale) = crate::codegen::drift(root, &rerooted);
            let rel = |v: Vec<PathBuf>| -> Vec<String> {
                v.iter()
                    .map(|p| {
                        p.strip_prefix(root)
                            .unwrap_or(p)
                            .to_string_lossy()
                            .replace('\\', "/")
                    })
                    .collect()
            };
            if changed.is_empty() && stale.is_empty() {
                Codegen::Clean
            } else {
                Codegen::Changes {
                    changed: rel(changed),
                    stale: rel(stale),
                }
            }
        }
    };
    let _ = std::fs::remove_dir_all(shadow);
    (out, dropped)
}

// ------------------------------------------------------------------------------ OPERATORS.md

/// A table row of `docs/OPERATORS.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// 1-based line number.
    pub line: usize,
    /// Section letter (`A` operators, `B` modifier types, `C` shader nodes, `D` RNA structs,
    /// `E` RNA functions).
    pub section: char,
    /// Trimmed cells without the empty first and last ones.
    pub cells: Vec<String>,
}

impl Row {
    fn status(&self) -> &'static str {
        match self.cells.first().map(String::as_str) {
            Some("[x]") => "implemented",
            Some("[ ]") => "planned",
            _ => "not planned",
        }
    }
    /// The milestone cell (`M1`, `OUT`, `M1 data / M4 eval`).
    fn milestone(&self) -> &str {
        let i = match self.section {
            'B' | 'C' => 3,
            _ => 2,
        };
        self.cells.get(i).map_or("", String::as_str)
    }
    fn notes(&self) -> &str {
        self.cells.last().map_or("", String::as_str)
    }
}

/// Every table row of OPERATORS.md under a `## <letter>.` heading.
pub fn parse_rows(md: &str) -> Vec<Row> {
    let mut section = ' ';
    let mut out = Vec::new();
    for (i, line) in md.lines().enumerate() {
        if let Some(h) = line.strip_prefix("## ") {
            section = h.chars().next().unwrap_or(' ');
            continue;
        }
        let t = line.trim();
        if !t.starts_with('|') || t.starts_with("|---") || t.starts_with("| ✓") {
            continue;
        }
        let cells: Vec<String> = t
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_owned())
            .collect();
        if cells.len() >= 3 {
            out.push(Row {
                line: i + 1,
                section,
                cells,
            });
        }
    }
    out
}

fn tokens(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
}

/// The rows a subject is listed in.
pub fn rows_for<'a>(rows: &'a [Row], subject: &Subject) -> Vec<&'a Row> {
    let code = |r: &Row, name: &str| r.cells.get(1).is_some_and(|c| *c == format!("`{name}`"));
    rows.iter()
        .filter(|r| match subject {
            Subject::Operator(op) => r.section == 'A' && code(r, op),
            Subject::Modifier(t) => r.section == 'B' && code(r, t),
            Subject::Node(n) => r.section == 'C' && code(r, n),
            Subject::Struct(s) => {
                matches!(r.section, 'D' | 'E')
                    && r.cells.get(1).is_some_and(|c| tokens(c).any(|t| t == s))
            }
            _ => false,
        })
        .collect()
}

fn categories(items: &[&Item]) -> String {
    let mut v: Vec<String> = Vec::new();
    for it in items {
        let s = match (&it.member, &it.renamed) {
            (_, Some((a, b))) => format!("{} `{a}` → `{b}`", it.category),
            (Some(m), None) => format!("{} `{m}`", it.category),
            (None, None) => it.category.clone(),
        };
        if !v.contains(&s) {
            v.push(s);
        }
    }
    v.join("; ")
}

/// One follow-up line per drifted subject that OPERATORS.md tracks (or should track).
pub fn operators_md(md: &str, cmp: &Comparison, current_milestone: u32) -> Vec<String> {
    let rows = parse_rows(md);
    let mut by_subject: BTreeMap<&Subject, Vec<&Item>> = BTreeMap::new();
    for it in &cmp.items {
        if matches!(
            it.subject,
            Subject::Operator(_) | Subject::Struct(_) | Subject::Node(_) | Subject::Modifier(_)
        ) {
            by_subject.entry(&it.subject).or_default().push(it);
        }
    }
    let mut out = Vec::new();
    for (subject, items) in by_subject {
        let name = subject.label();
        let cats = categories(&items);
        let found = rows_for(&rows, subject);
        let whole = |verb: &str| {
            items.iter().any(|i| {
                ["operator", "struct", "node type", "modifier type"]
                    .iter()
                    .any(|n| i.category == format!("{n} {verb}"))
            })
        };
        let (removed, added) = (whole("removed"), whole("added"));
        if found.is_empty() {
            out.push(match subject {
                Subject::Operator(op) if added => format!(
                    "`{op}` ({cats}): not listed, so OUT by rule (3); add a row to the `{}.*` \
                     table if it is in scope",
                    op.split('.').next().unwrap_or("")
                ),
                Subject::Operator(_) => {
                    format!("`{name}` ({cats}): not listed (OUT by rule (3)): no follow-up")
                }
                Subject::Struct(_) => {
                    format!("`{name}` ({cats}): no §D/§E row (a closure-only struct): codegen only")
                }
                Subject::Modifier(_) if added => {
                    format!("`{name}` ({cats}): new modifier type: add a §B row")
                }
                Subject::Node(_) if added => {
                    format!("`{name}` ({cats}): new shader node: add a §C row")
                }
                _ => format!("`{name}` ({cats}): not listed: no follow-up"),
            });
            continue;
        }
        for r in found {
            let m = r.milestone();
            let typed = m
                .strip_prefix('M')
                .and_then(|n| n.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|n| n.parse::<u32>().ok())
                .is_some_and(|n| n <= current_milestone);
            let wp = r
                .notes()
                .split(';')
                .map(str::trim)
                .find(|s| s.starts_with("WP "))
                .map(|s| format!(", {s}"))
                .unwrap_or_default();
            let head = format!(
                "OPERATORS.md:{} §{} `{name}` ({}, {m}{wp}): {cats}",
                r.line,
                r.section,
                r.status()
            );
            let what = match (r.status(), removed) {
                (_, true) if r.section == 'A' => format!(
                    "Blender removed it: delete the row{}{}",
                    if r.status() == "implemented" {
                        ", unregister frap's operator and drop its `--describe` snapshot and \
                         golden cases (deviations.md if agents may still call it)"
                    } else {
                        ""
                    },
                    if typed {
                        "; until then codegen keeps asking for its typed props struct"
                    } else {
                        ""
                    }
                ),
                (_, true) => "Blender removed it: update the row".to_owned(),
                ("implemented", false) => "Update the implementation (typed props and tables are \
                     regenerated by codegen), its `--describe` snapshot and golden cases; untick \
                     the row until they pass again (rule 2)"
                    .to_owned(),
                ("planned", false) => {
                    "Not implemented yet: the WP implementing it works from the refreshed spec"
                        .to_owned()
                }
                _ => "OUT: no follow-up".to_owned(),
            };
            out.push(format!("{head}. {what}."));
        }
    }
    out
}

// ---------------------------------------------------------------------------- .frap migrations

/// `pub const SCHEMA_VERSION: u32 = N;` of frap-project's manifest.
pub fn schema_version(root: &Path) -> Option<u32> {
    let s = std::fs::read_to_string(root.join("crates/frap-project/src/manifest/mod.rs")).ok()?;
    let rest = s.split("pub const SCHEMA_VERSION: u32 = ").nth(1)?;
    rest.split(';').next()?.trim().parse().ok()
}

/// `.frap` follow-ups: renamed / removed RNA properties, enum items and structs.
pub fn migrations(cmp: &Comparison, schema_version: Option<u32>) -> Vec<String> {
    let step = match schema_version {
        Some(n) => format!("v{n}_to_v{}.rs", n + 1),
        None => "v<N>_to_v<N+1>.rs".to_owned(),
    };
    let mut out = Vec::new();
    let mut defaults = 0usize;
    for it in &cmp.items {
        if it.family != Family::Rna || it.member.as_deref().is_some_and(|m| m.contains('(')) {
            continue; // function parameters are not persisted
        }
        let Subject::Struct(s) = &it.subject else {
            continue;
        };
        let what = match (it.category.as_str(), &it.renamed, &it.member) {
            ("property renamed", Some((a, b)), _) => format!("`{s}.{a}` → `{s}.{b}`"),
            ("property removed", _, Some(m)) => format!("`{s}.{m}` removed"),
            ("enum item renamed", Some((a, b)), Some(m)) => {
                format!("`{s}.{m}` value `'{a}'` → `'{b}'`")
            }
            ("enum item removed", _, Some(m)) => {
                format!("`{s}.{m}` value removed ({})", it.detail)
            }
            ("struct removed", _, _) => format!("struct `{s}` removed"),
            ("default changed", _, _) => {
                defaults += 1;
                continue;
            }
            _ => continue,
        };
        out.push(format!(
            "{what}: if frap persists it, add `crates/frap-project/src/migrate/steps/{step}` \
             (`pub const MIGRATION`, appended to `MIGRATIONS`; `rename_key`/`for_each_id` on the \
             `scene.json` tree) and bump `SCHEMA_VERSION`"
        ));
    }
    if defaults > 0 {
        out.push(format!(
            "{defaults} RNA default(s) changed: no migration (`.frap` writes every value of an \
             implemented property, §15; files keep the values they were saved with)"
        ));
    }
    out
}
