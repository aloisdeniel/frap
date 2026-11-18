// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask fmt [--check]`: rustfmt on every git-tracked hand-written `.rs` file.
//!
//! `cargo fmt` formats a crate by following its `mod` declarations from the crate roots, so it
//! never reaches the modules `automod::dir!` includes (`src/ops/*.rs` of the operator crates,
//! the bevel port, …). This task hands every tracked file to rustfmt directly (edition 2024,
//! the workspace edition). Generated code is left alone: files under a `generated/` directory
//! or starting with an `@generated` marker are owned by their generators (`cargo xtask codegen
//! --check`, the Blender dump scripts). The fuzz workspace is formatted by its own
//! `cargo fmt --manifest-path fuzz/Cargo.toml`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workspace_root;

/// Whether `path` (repository-relative) is generated code.
fn is_generated(root: &Path, rel: &str) -> bool {
    if rel.split('/').any(|c| c == "generated") {
        return true;
    }
    std::fs::read_to_string(root.join(rel))
        .map(|s| s.lines().take(5).any(|l| l.contains("@generated")))
        .unwrap_or(false)
}

/// The hand-written `.rs` files git tracks under `crates/` and `xtask/`.
fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z", "--", "crates/*.rs", "xtask/*.rs"])
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let list = String::from_utf8(out.stdout).map_err(|e| format!("git ls-files: {e}"))?;
    Ok(list
        .split('\0')
        .filter(|s| !s.is_empty() && !is_generated(root, s))
        .map(|s| root.join(s))
        .collect())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let check = match args {
        [] => false,
        [a] if a == "--check" => true,
        _ => return Err("usage: cargo xtask fmt [--check]".into()),
    };
    let root = workspace_root();
    let files = files(&root)?;
    let mut failed = false;
    for chunk in files.chunks(256) {
        let mut cmd = Command::new("rustfmt");
        cmd.current_dir(&root).args(["--edition", "2024"]);
        if check {
            cmd.args(["--check", "-l"]);
        }
        let status = cmd
            .args(chunk)
            .status()
            .map_err(|e| format!("rustfmt: {e}"))?;
        failed |= !status.success();
    }
    if failed {
        return Err(if check {
            "files above need formatting: run `cargo xtask fmt`".into()
        } else {
            "rustfmt failed".into()
        });
    }
    eprintln!(
        "xtask fmt: {} files {}",
        files.len(),
        if check { "checked" } else { "formatted" }
    );
    Ok(())
}
