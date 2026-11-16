// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask codegen [--check]` (ARCHITECTURE §4.2): Blender dumps in `spec/blender-5.2/`,
//! frap's operator specs in `spec/frap/` and the overlays in `spec/overlay/` → every
//! `crates/*/src/generated/**`. Owned by WP 1.1. Hermetic (reads only committed files) and
//! deterministic; `--check` regenerates in memory and fails on any difference, including stale
//! files left in a generated directory.
//!
//! | Generator | Output |
//! |---|---|
//! | `gen_rna` | `crates/frap-spec-rna/src/generated/` (`StructDef` statics, property consts) |
//! | `gen_ops` | `crates/frap-spec-ops-<group>/src/generated/` (typed ops), `crates/frap-spec-catalog/src/generated/` (blob) |
//! | `gen_plain` | `crates/frap-scene/src/generated/` (plain structs, `ModifierSettings`) |
//! | `gen_nodes` | `crates/frap-nodes/src/generated/` (`NodeTypeDef` per `bl_idname`) |
//!
//! The WP 0.2 prototype this grew from is archived in `docs/design/rfc/0001-probe/codegen/`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub mod audit;
mod gen_nodes;
mod gen_ops;
mod gen_plain;
mod gen_rna;
pub mod model;
mod rust;
#[cfg(test)]
mod tests;

/// Generated files: path -> content.
pub type Files = BTreeMap<PathBuf, Vec<u8>>;

/// Directories whose whole content is generated (stale files are removed / reported).
fn owned_dirs(root: &Path) -> Vec<PathBuf> {
    let mut v = vec![
        root.join("crates/frap-spec-rna/src/generated"),
        root.join(gen_ops::CATALOG_DIR),
        root.join(gen_plain::DIR),
        root.join(gen_nodes::DIR),
    ];
    for g in model::GROUPS {
        v.push(root.join(gen_ops::group_dir(g)));
    }
    v
}

/// Generates every file in memory.
pub fn generate(root: &Path) -> Result<Files, String> {
    let spec = model::load(root)?;
    let mut files = Files::new();
    gen_rna::generate(&spec, &mut files)?;
    gen_ops::generate_typed(&spec, &mut files)?;
    gen_ops::generate_catalog(&spec, &mut files)?;
    gen_plain::generate(&spec, &mut files)?;
    gen_nodes::generate(&spec, &mut files)?;
    let mut errors = audit::frap_options(&spec);
    errors.extend(audit::compat_4x(&spec));
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let unused: Vec<&String> = spec
        .dynamic_overlay_keys
        .difference(&spec.used_dynamic_overlay_keys)
        .collect();
    if !unused.is_empty() {
        return Err(format!(
            "spec/overlay/dynamic_enums.toml: entries match no dumped enum: {unused:?}"
        ));
    }
    Ok(files)
}

fn walk(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.insert(p);
        }
    }
}

/// Differences between `files` and the disk: (changed or missing, stale).
pub fn drift(root: &Path, files: &Files) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut changed = Vec::new();
    for (p, want) in files {
        let same = match std::fs::read(p) {
            Ok(have) if have == *want => true,
            // Deflate output may differ between flate2 backends: compare the content.
            Ok(have) if p.extension().is_some_and(|e| e == "deflate") => {
                matches!((gen_ops::inflate(&have), gen_ops::inflate(want)), (Ok(a), Ok(b)) if a == b)
            }
            _ => false,
        };
        if !same {
            changed.push(p.clone());
        }
    }
    let mut on_disk = BTreeSet::new();
    for d in owned_dirs(root) {
        walk(&d, &mut on_disk);
    }
    let stale = on_disk
        .into_iter()
        .filter(|p| !files.contains_key(p))
        .collect();
    (changed, stale)
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

/// Entry point.
pub fn run(args: &[String]) -> Result<(), String> {
    let check = args.iter().any(|a| a == "--check");
    if let Some(a) = args.iter().find(|a| *a != "--check") {
        return Err(format!("unknown argument `{a}`"));
    }
    let root = crate::workspace_root();
    let files = generate(&root)?;
    let (changed, stale) = drift(&root, &files);
    if check {
        if changed.is_empty() && stale.is_empty() {
            println!("codegen --check: clean ({} files)", files.len());
            return Ok(());
        }
        let mut msg = String::from("generated code is out of date (run `cargo xtask codegen`):");
        for p in &changed {
            msg.push_str(&format!("\n  changed: {}", rel(&root, p)));
        }
        for p in &stale {
            msg.push_str(&format!("\n  stale:   {}", rel(&root, p)));
        }
        return Err(msg);
    }
    for p in &stale {
        std::fs::remove_file(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    for p in &changed {
        let content = &files[p];
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(p, content).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    println!(
        "codegen: {} files ({} written, {} stale removed)",
        files.len(),
        changed.len(),
        stale.len()
    );
    Ok(())
}
