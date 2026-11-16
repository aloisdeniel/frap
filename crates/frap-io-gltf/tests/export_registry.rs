// SPDX-License-Identifier: GPL-3.0-or-later
//! One concept, one file (ARCHITECTURE §3.4, §17.2): every file of `export/mesh/extract/`,
//! `writers/` and `post/` (except `_`-prefixed placeholders) registers exactly one entry in its
//! slice, named after the file; entry names are unique; the driver file names none of them.

use std::collections::BTreeSet;
use std::path::Path;

use frap_io_gltf::{PRIMITIVE_ATTRIBUTE_WRITERS, PRIMITIVE_EXTRACTORS, PRIMITIVE_POST_PROCESS};

fn stems(dir: &str) -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    std::fs::read_dir(&path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .filter(|s| !s.starts_with('_'))
        .collect()
}

fn check(dir: &str, names: Vec<&'static str>) {
    let files = stems(dir);
    let unique: BTreeSet<String> = names.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "duplicate entry names in {dir}: {names:?}"
    );
    assert_eq!(files, unique, "{dir}: files vs registered entries");
}

#[test]
fn every_file_registers_one_entry() {
    check(
        "src/export/mesh/extract",
        PRIMITIVE_EXTRACTORS.iter().map(|e| e.name).collect(),
    );
    check(
        "src/export/mesh/writers",
        PRIMITIVE_ATTRIBUTE_WRITERS.iter().map(|e| e.name).collect(),
    );
    check(
        "src/export/mesh/post",
        PRIMITIVE_POST_PROCESS.iter().map(|e| e.name).collect(),
    );
}

#[test]
fn driver_names_no_entry() {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/mesh/primitives.rs"),
    )
    .unwrap();
    for dir in ["extract", "writers", "post"] {
        for stem in stems(&format!("src/export/mesh/{dir}")) {
            assert!(
                !src.contains(&format!("{dir}::{stem}")),
                "primitives.rs names {dir}::{stem}"
            );
        }
    }
}
