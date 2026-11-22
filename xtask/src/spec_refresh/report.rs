// SPDX-License-Identifier: GPL-3.0-or-later
//! The drift report (`report.md` for review, `report.json` for tools), WP 8.9. Deterministic:
//! no timestamps or scratch paths, so a seeded run has an exact expected report.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::compare::{Comparison, Family, Item};
use super::followups::Codegen;

/// The Blender that produced the re-dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlenderInfo {
    pub path: String,
    /// `5.2.4 LTS`
    pub version: String,
    pub build_hash: String,
}

/// Everything the report shows.
#[derive(Clone, Debug)]
pub struct Report {
    pub blender: BlenderInfo,
    /// The `--seed` script, as given.
    pub seed: Option<String>,
    pub cmp: Comparison,
    pub codegen: Codegen,
    /// Removed operators whose OPERATORS.md rows the codegen dry run deleted first.
    pub codegen_dropped_rows: Vec<String>,
    pub operators_md: Vec<String>,
    pub migrations: Vec<String>,
}

/// The routine after a drift (docs/spec-refresh.md).
const ROUTINE: [&str; 6] = [
    "Review this report; refresh with `cargo xtask spec-refresh --blender <path>` (no `--check`) and review `git diff spec/blender-5.2/`.",
    "Regenerate: `cargo xtask codegen`, then `cargo xtask codegen --check`.",
    "Run the spec drift tests and snapshots: `cargo test -p xtask`, `cargo test -p frap-spec-rna --test drift`, the `frap-spec-ops-*` drift tests and the `--describe` snapshots (frap-schema); after a `startup.json` change also `cargo test -p frap-scene --test startup`.",
    "Update docs/OPERATORS.md (and docs/deviations.md) as listed above.",
    "Write the `.frap` migrations listed above (`crates/frap-project/src/migrate/steps/`).",
    "Run the full gate and the golden suite live against the new Blender (`cargo xtask golden live`).",
];

impl Report {
    fn verdict(&self) -> String {
        let c = &self.cmp;
        if c.has_drift() {
            let files: std::collections::BTreeSet<&str> =
                c.items.iter().map(|i| i.file.as_str()).collect();
            format!(
                "**drift**: {} item(s) in {} file(s)",
                c.items.len(),
                files.len()
            )
        } else if !c.version_only.is_empty() {
            format!(
                "**no API drift**; only the recorded Blender version differs ({} file(s))",
                c.version_only.len()
            )
        } else {
            "**no drift**: the dumps are byte-identical to `spec/blender-5.2/`".to_owned()
        }
    }

    /// Markdown.
    pub fn markdown(&self) -> String {
        let c = &self.cmp;
        let mut s = String::from("# Spec refresh report\n\n");
        let b = &self.blender;
        s.push_str(&format!(
            "- Blender: {} (build hash `{}`), `{}`\n",
            b.version, b.build_hash, b.path
        ));
        if let Some(r) = &c.old_version {
            s.push_str(&format!(
                "- `spec/blender-5.2/` records: {} (build hash `{}`)\n",
                r.version, r.build_hash
            ));
        }
        if let Some(seed) = &self.seed {
            s.push_str(&format!("- Seeded drift: `{seed}`\n"));
        }
        s.push_str(&format!(
            "- Files: {} compared, {} byte-identical\n- Result: {}\n",
            c.files,
            c.identical,
            self.verdict()
        ));
        if !c.version_only.is_empty() {
            s.push_str(&format!(
                "\nVersion fields only (a refresh records the new version): {}\n",
                c.version_only
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !c.has_drift() {
            return s;
        }

        s.push_str("\n## Summary\n\n| Category | Count |\n|---|---|\n");
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for it in &c.items {
            *counts.entry(it.category.as_str()).or_default() += 1;
        }
        for (k, n) in counts {
            s.push_str(&format!("| {k} | {n} |\n"));
        }

        for fam in Family::ALL {
            let items: Vec<&Item> = c.items.iter().filter(|i| i.family == fam).collect();
            if items.is_empty() {
                continue;
            }
            s.push_str(&format!("\n## {}\n", fam.title()));
            let mut file = "";
            for it in items {
                if it.file != file {
                    file = &it.file;
                    s.push_str(&format!("\n### `{file}`\n\n"));
                }
                s.push_str(&format!("- {}: {}", it.category, it.locus_md()));
                if !it.detail.is_empty() {
                    s.push_str(&format!(" — {}", it.detail));
                }
                s.push('\n');
            }
        }

        s.push_str("\n## Follow-ups\n\n### `cargo xtask codegen`\n\n");
        if !self.codegen_dropped_rows.is_empty() {
            s.push_str(&format!(
                "Dry run on the re-dump, with the OPERATORS.md rows of the removed operators \
                 deleted first (their follow-up below): {}.\n\n",
                self.codegen_dropped_rows
                    .iter()
                    .map(|o| format!("`{o}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        match &self.codegen {
            Codegen::Skipped => s.push_str("Not run.\n"),
            Codegen::Clean => s.push_str("No generated file changes.\n"),
            Codegen::Fails(e) => {
                s.push_str("Codegen fails on the re-dump; fix the overlays or OPERATORS.md first:\n\n```text\n");
                s.push_str(e.trim_end());
                s.push_str("\n```\n");
            }
            Codegen::Changes { changed, stale } => {
                s.push_str(&format!(
                    "{} generated file(s) change, {} go stale:\n\n",
                    changed.len(),
                    stale.len()
                ));
                for p in changed {
                    s.push_str(&format!("- changed: `{p}`\n"));
                }
                for p in stale {
                    s.push_str(&format!("- stale: `{p}`\n"));
                }
            }
        }
        s.push_str("\n### `docs/OPERATORS.md`\n\n");
        if self.operators_md.is_empty() {
            s.push_str("None.\n");
        }
        for l in &self.operators_md {
            s.push_str(&format!("- {l}\n"));
        }
        s.push_str("\n### `.frap` migrations (`blender_api`)\n\n");
        if self.migrations.is_empty() {
            s.push_str(
                "None: no RNA property, enum item or struct was renamed or removed (operator \
                 properties are not persisted in `.frap`).\n",
            );
        }
        for l in &self.migrations {
            s.push_str(&format!("- {l}\n"));
        }
        s.push_str(
            "\n`blender_api` stays `\"5.2\"` for every 5.2.x; a 5.3 spec is a new \
             `spec/blender-5.3/` and a `blender_api` bump that carries these migrations.\n",
        );
        s.push_str("\n### Routine\n\n");
        for (i, l) in ROUTINE.iter().enumerate() {
            s.push_str(&format!("{}. {l}\n", i + 1));
        }
        s
    }

    /// JSON.
    pub fn json(&self) -> Value {
        let c = &self.cmp;
        let ver = |r: &Option<super::compare::Recorded>| {
            r.as_ref()
                .map(|r| json!({"version": r.version, "build_hash": r.build_hash}))
        };
        let codegen = match &self.codegen {
            Codegen::Skipped => json!({"status": "skipped"}),
            Codegen::Clean => json!({"status": "clean"}),
            Codegen::Fails(e) => json!({"status": "fails", "error": e}),
            Codegen::Changes { changed, stale } => {
                json!({"status": "changes", "changed": changed, "stale": stale})
            }
        };
        let codegen =
            json!({"result": codegen, "dropped_operator_rows": self.codegen_dropped_rows});
        json!({
            "blender": {
                "path": self.blender.path,
                "version": self.blender.version,
                "build_hash": self.blender.build_hash,
            },
            "spec_records": ver(&c.old_version),
            "dumps_record": ver(&c.new_version),
            "seed": self.seed,
            "files": c.files,
            "identical": c.identical,
            "version_only": c.version_only,
            "drift": c.has_drift(),
            "items": c.items.iter().map(|i| json!({
                "family": i.family.script(),
                "file": i.file,
                "category": i.category,
                "subject": i.subject.label(),
                "member": i.member,
                "detail": i.detail,
                "path": i.path,
            })).collect::<Vec<_>>(),
            "codegen": codegen,
            "operators_md": self.operators_md,
            "migrations": self.migrations,
        })
    }
}
