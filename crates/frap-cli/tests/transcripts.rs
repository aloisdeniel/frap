// SPDX-License-Identifier: GPL-3.0-or-later
//! CLI transcripts (ARCHITECTURE §20.1, §22.1): stdin/script → expected JSON Lines, run on all
//! three OSes. Every WP ships its transcripts under `tests/cli/<milestone>/<topic>/`; this runner
//! picks up every `*.trycmd` and `*.toml` case below `tests/cli/`, at any depth, so no WP edits
//! it to add cases. Files inside a case's sandbox (`<case>.in/`) or expected output
//! (`<case>.out/`) directories are fixtures, not cases.
//!
//! Re-bless with `TRYCMD=overwrite cargo test -p frap-cli --test transcripts`.

use std::path::PathBuf;

#[test]
fn cli_transcripts() {
    // `crates/frap-cli` → repository root, without `..` components (glob on Windows).
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("invariant: crates/frap-cli is two levels below the repository root")
        .join("tests")
        .join("cli");
    let cases = trycmd::TestCases::new();
    cases
        .register_bin("frap", PathBuf::from(env!("CARGO_BIN_EXE_frap")))
        .case(root.join("**").join("*.trycmd"))
        .case(root.join("**").join("*.toml"));
    for fixture_dir in ["*.in", "*.out"] {
        for ext in ["*.trycmd", "*.toml"] {
            cases.skip(root.join("**").join(fixture_dir).join("**").join(ext));
        }
    }
}
