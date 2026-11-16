// SPDX-License-Identifier: GPL-3.0-or-later
//! Statement corpus (ARCHITECTURE §11.5, §22.3 WP 1.3): one file per case under
//! `tests/corpus/`, each with an `insta` snapshot of its IR (or error) including every span.
//! Adding a case is adding a file; `INSTA_UPDATE=always` (or `cargo insta review`) blesses it.

mod common;

use std::path::Path;

fn read(path: &Path) -> String {
    // Normalise line endings so spans do not depend on the checkout's newline style.
    std::fs::read_to_string(path)
        .expect("corpus file is UTF-8")
        .replace("\r\n", "\n")
}

#[test]
fn corpus_snapshots() {
    insta::glob!("corpus/*.txt", |path| {
        let src = read(path);
        insta::assert_snapshot!(common::render(&src));
    });
}

#[test]
fn corpus_spans_are_consistent_and_every_error_has_one() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).expect("corpus dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|e| e == "txt") {
            common::check_all_spans(&read(&path));
            n += 1;
        }
    }
    assert!(
        n >= 250,
        "the corpus must hold at least 250 statements (one file each), has {n}"
    );
}
