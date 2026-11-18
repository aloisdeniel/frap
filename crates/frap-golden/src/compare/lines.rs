// SPDX-License-Identifier: GPL-3.0-or-later
//! `lines` (T0, §20.4): per statement, the same `ok`, the same exception class, and the same
//! message after normalising quotes, whitespace and paths. Lines only one tool runs (`#@frap`,
//! `#@blender`) must succeed on that side and are not compared.
//!
//! `print` lines are compared by outcome only (D-13): the dumpers never record values.

use std::collections::{BTreeMap, BTreeSet};

use crate::LineOutcome;
use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "lines";

/// Message normalisation context: path prefixes replaced by placeholders (the work
/// directories differ between the two runs).
#[derive(Clone, Debug, Default)]
pub struct MessageNorm {
    /// `(prefix, placeholder)` pairs, applied longest first.
    pub paths: Vec<(String, String)>,
}

impl MessageNorm {
    /// Replaces `dir` (in `/` and native spelling) by `placeholder`.
    pub fn with_dir(mut self, dir: &std::path::Path, placeholder: &str) -> Self {
        let s = dir.to_string_lossy().into_owned();
        self.paths
            .push((s.replace('\\', "/"), placeholder.to_owned()));
        if s.contains('\\') {
            self.paths.push((s, placeholder.to_owned()));
        }
        self.paths.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
        self
    }

    /// Normalised message: trimmed, one level of enclosing quotes removed (Python's
    /// `str(KeyError)` adds them), `"` and `'` unified, `\` path separators turned into `/`,
    /// known directories replaced, runs of whitespace collapsed.
    pub fn normalise(&self, msg: &str) -> String {
        let mut s = msg.trim().to_owned();
        for q in ['\'', '"'] {
            if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
                s = s[1..s.len() - 1].to_owned();
                break;
            }
        }
        for (p, ph) in &self.paths {
            if !p.is_empty() {
                s = s.replace(p.as_str(), ph);
            }
        }
        let s = s.replace("\\\\", "/").replace('\\', "/").replace('"', "'");
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// Compares two line-outcome lists.
pub fn compare(
    frap: &[LineOutcome],
    blender: &[LineOutcome],
    side_only: &BTreeSet<u32>,
    norm: &MessageNorm,
) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let by_line = |v: &[LineOutcome]| -> BTreeMap<u32, LineOutcome> {
        v.iter().map(|l| (l.line, l.clone())).collect()
    };
    let (f, b) = (by_line(frap), by_line(blender));
    let describe = |l: &LineOutcome| match (&l.exception, l.ok) {
        (Some(e), _) => format!("{e}: {}", l.message.as_deref().unwrap_or("")),
        (None, true) => "ok".to_owned(),
        (None, false) => "CANCELLED".to_owned(),
    };
    for line in f.keys().chain(b.keys()).copied().collect::<BTreeSet<_>>() {
        let path = format!("line {line}");
        if side_only.contains(&line) {
            for (who, l) in [("frap", f.get(&line)), ("blender", b.get(&line))] {
                if let Some(l) = l.filter(|l| !l.ok) {
                    out.push(Mismatch::new(
                        NAME,
                        &path,
                        format!("{who}-only line failed: {}", describe(l)),
                    ));
                }
            }
            continue;
        }
        let (fl, bl) = match (f.get(&line), b.get(&line)) {
            (Some(x), Some(y)) => (x, y),
            (Some(x), None) => {
                out.push(Mismatch::new(
                    NAME,
                    path,
                    format!("statement only in frap ({})", describe(x)),
                ));
                continue;
            }
            (None, Some(y)) => {
                out.push(Mismatch::new(
                    NAME,
                    path,
                    format!("statement only in blender ({})", describe(y)),
                ));
                continue;
            }
            (None, None) => continue,
        };
        let same = fl.ok == bl.ok
            && fl.exception == bl.exception
            && norm.normalise(fl.message.as_deref().unwrap_or(""))
                == norm.normalise(bl.message.as_deref().unwrap_or(""));
        if !same {
            out.push(Mismatch::new(
                NAME,
                path,
                format!("frap {} != blender {}", describe(fl), describe(bl)),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(line: u32) -> LineOutcome {
        LineOutcome {
            line,
            ok: true,
            exception: None,
            message: None,
        }
    }
    fn err(line: u32, exc: &str, msg: &str) -> LineOutcome {
        LineOutcome {
            line,
            ok: false,
            exception: Some(exc.into()),
            message: Some(msg.into()),
        }
    }

    #[test]
    fn equal_under_permutation_and_quote_normalisation() {
        let f = vec![
            ok(3),
            err(
                4,
                "KeyError",
                "bpy_prop_collection[key]: key \"Nope\" not found",
            ),
        ];
        // Python's str(KeyError) quotes the message; order of records is irrelevant.
        let b = vec![
            err(
                4,
                "KeyError",
                "'bpy_prop_collection[key]: key \"Nope\" not found'",
            ),
            ok(3),
        ];
        assert_eq!(
            compare(&f, &b, &BTreeSet::new(), &MessageNorm::default()),
            vec![]
        );
    }

    #[test]
    fn differs_under_one_change() {
        let f = vec![ok(3), err(4, "TypeError", "x")];
        for b in [
            vec![ok(3), err(4, "ValueError", "x")],
            vec![ok(3), err(4, "TypeError", "y")],
            vec![ok(3), ok(4)],
            vec![ok(3)],
            vec![ok(3), err(4, "TypeError", "x"), ok(5)],
        ] {
            assert_eq!(
                compare(&f, &b, &BTreeSet::new(), &MessageNorm::default()).len(),
                1,
                "{b:?}"
            );
        }
    }

    #[test]
    fn cancelled_is_not_ok() {
        let mut c = ok(2);
        c.ok = false;
        let m = compare(&[ok(2)], &[c], &BTreeSet::new(), &MessageNorm::default());
        assert_eq!(m[0].detail, "frap ok != blender CANCELLED");
    }

    #[test]
    fn side_only_lines_must_succeed_on_their_side() {
        let side = BTreeSet::from([5]);
        assert!(compare(&[ok(5)], &[], &side, &MessageNorm::default()).is_empty());
        let m = compare(
            &[],
            &[err(5, "NameError", "bmesh")],
            &side,
            &MessageNorm::default(),
        );
        assert_eq!(m.len(), 1);
        assert!(m[0].detail.starts_with("blender-only line failed"));
    }

    #[test]
    fn paths_and_whitespace_are_normalised() {
        let n = MessageNorm::default()
            .with_dir(std::path::Path::new("/tmp/frap-run"), "<work>")
            .with_dir(std::path::Path::new("/tmp/blender-run"), "<work>");
        assert_eq!(
            n.normalise("Error: Cannot write '/tmp/frap-run/out/x.png'\n"),
            n.normalise("Error: Cannot write \"/tmp/blender-run/out/x.png\"")
        );
        assert_eq!(n.normalise("C:\\a\\b  c"), "C:/a/b c");
    }
}
