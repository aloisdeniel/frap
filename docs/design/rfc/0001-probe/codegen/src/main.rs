// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo run -p frap-probe-codegen [-- --check]`
fn main() {
    let check = std::env::args().any(|a| a == "--check");
    let root = frap_probe_codegen::probe_root();
    let out = frap_probe_codegen::generate(&root);
    let drift = frap_probe_codegen::drift(&out);
    if check {
        if drift.is_empty() {
            println!("codegen --check: clean ({} files)", out.files.len());
            return;
        }
        eprintln!("codegen --check: {} file(s) out of date:", drift.len());
        for p in &drift {
            eprintln!("  {}", p.display());
        }
        std::process::exit(1);
    }
    for (p, s) in &out.files {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }
    println!(
        "codegen: wrote {} files ({} changed)\n{:#?}",
        out.files.len(),
        drift.len(),
        out.stats
    );
}
