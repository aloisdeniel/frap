// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_pattern(pattern='*', case_sensitive=False, extend=True)`, tier T0
//! (`tests/golden/cases/object/select/pattern*.py`). Port of the Python operator
//! `SelectPattern` (`scripts/startup/bl_operators/object.py`, Blender 5.2.4,
//! GPL-2.0-or-later) and of CPython's `fnmatch.fnmatchcase` / `fnmatch.translate` (the
//! pattern language: `*`, `?`, `[seq]`, `[!seq]`, ranges; an unclosed `[` is literal).
//! Verified on 5.2.4:
//! - matches `context.visible_objects` by name; case-insensitive by default (both sides
//!   upper-cased, as the operator does);
//! - `extend=True` (the default) adds to the selection, `extend=False` first runs
//!   `object.select_all(action='DESELECT')`; a match is selected with `select_set(True)`, which
//!   leaves an unselectable object (`hide_select`) unselected;
//! - always `FINISHED`, also without a match; the active object never changes;
//! - poll: no active object, or an active object in Object Mode (Pose Mode and armature Edit
//!   Mode match bone names instead: M6, `OP_NOT_IMPLEMENTED`); Edit Mode on a mesh fails.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, ObjectId};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;

use super::select_by_type::common::{plan, set_selection, visible_bases};

/// Bone selection applies: Pose Mode, or an armature in Edit Mode.
fn bone_mode(c: &Context) -> bool {
    poll::active(c).is_some_and(|(_, o)| {
        o.mode == ObjectMode::Pose || (o.mode == ObjectMode::Edit && o.object_type() == "ARMATURE")
    })
}

/// `SelectPattern.poll`: objects (no active object or Object Mode) or bones (Pose Mode,
/// armature Edit Mode).
fn pattern_poll(c: &Context) -> Result<(), PollFail> {
    match poll::active(c) {
        Some((_, o)) if o.mode != ObjectMode::Object && !bone_mode(c) => Err(poll::fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        )),
        _ => Ok(()),
    }
}

/// `SelectPattern.poll`
const PATTERN_POLL: Poll = Poll {
    requires: &[PollReq::ModeObject],
    check: pattern_poll,
};

/// One element of a compiled `fnmatch` pattern.
#[derive(Clone, Debug, PartialEq)]
enum Tok {
    /// `*` (consecutive stars are one).
    Star,
    /// `?`
    Any,
    /// `[...]`: negated, inclusive ranges (a single character is `(c, c)`).
    Set(bool, Vec<(char, char)>),
    /// A literal character.
    Lit(char),
}

/// `fnmatch.translate` (CPython 3.11+), as tokens instead of a regular expression.
fn compile(pat: &[char]) -> Vec<Tok> {
    let n = pat.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = pat[i];
        i += 1;
        match c {
            '*' => {
                if out.last() != Some(&Tok::Star) {
                    out.push(Tok::Star);
                }
            }
            '?' => out.push(Tok::Any),
            '[' => {
                let mut j = i;
                if j < n && pat[j] == '!' {
                    j += 1;
                }
                if j < n && pat[j] == ']' {
                    j += 1;
                }
                while j < n && pat[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    out.push(Tok::Lit('['));
                    continue;
                }
                out.push(set(&pat[i..j]));
                i = j + 1;
            }
            c => out.push(Tok::Lit(c)),
        }
    }
    out
}

/// The character class `[stuff]` of `fnmatch.translate`: `-` between two characters is a
/// range (not at the start, a trailing one is literal), empty ranges (`z-a`) are dropped
/// together with their end points; an empty set never matches, a negated empty set matches
/// any character.
fn set(stuff: &[char]) -> Tok {
    let (neg, body) = match stuff.first() {
        Some('!') => (true, &stuff[1..]),
        _ => (false, stuff),
    };
    // Python splits `stuff` on the range hyphens (searching from the second character).
    let mut chunks: Vec<Vec<char>> = Vec::new();
    let mut start = 0;
    let mut k = 1;
    while let Some(off) = body.get(k..).and_then(|s| s.iter().position(|&c| c == '-')) {
        let at = k + off;
        chunks.push(body[start..at].to_vec());
        start = at + 1;
        k = at + 3;
    }
    let last = body.get(start..).unwrap_or_default().to_vec();
    if last.is_empty() {
        if let Some(c) = chunks.last_mut() {
            c.push('-');
        }
    } else {
        chunks.push(last);
    }
    for k in (1..chunks.len()).rev() {
        let (lo, hi) = (chunks[k - 1].last().copied(), chunks[k].first().copied());
        if let (Some(lo), Some(hi)) = (lo, hi)
            && lo > hi
        {
            let mut merged = chunks[k - 1][..chunks[k - 1].len() - 1].to_vec();
            merged.extend_from_slice(&chunks[k][1..]);
            chunks[k - 1] = merged;
            chunks.remove(k);
        }
    }
    let mut items: Vec<(char, char)> = Vec::new();
    for (ci, chunk) in chunks.iter().enumerate() {
        for (pos, &c) in chunk.iter().enumerate() {
            let is_lo = pos + 1 == chunk.len() && ci + 1 < chunks.len();
            let is_hi = pos == 0 && ci > 0;
            if is_hi {
                continue; // the upper end of the range opened by the previous chunk
            }
            if is_lo {
                let hi = chunks[ci + 1].first().copied().unwrap_or(c);
                items.push((c, hi));
            } else {
                items.push((c, c));
            }
        }
    }
    Tok::Set(neg, items)
}

fn tok_matches(t: &Tok, c: char) -> bool {
    match t {
        Tok::Any => true,
        Tok::Lit(l) => *l == c,
        Tok::Set(neg, items) => items.iter().any(|&(lo, hi)| lo <= c && c <= hi) != *neg,
        Tok::Star => false,
    }
}

/// Full match of `name` against compiled tokens (iterative glob with star backtracking).
fn matches(toks: &[Tok], name: &[char]) -> bool {
    let (mut t, mut s) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while s < name.len() {
        if t < toks.len() && toks[t] == Tok::Star {
            star = Some((t, s));
            t += 1;
        } else if t < toks.len() && tok_matches(&toks[t], name[s]) {
            t += 1;
            s += 1;
        } else if let Some((st, ss)) = star {
            t = st + 1;
            s = ss + 1;
            star = Some((st, ss + 1));
        } else {
            return false;
        }
    }
    toks[t..].iter().all(|x| *x == Tok::Star)
}

/// `fnmatch.fnmatchcase(name, pattern)`, or the operator's case-insensitive variant
/// (`fnmatchcase(name.upper(), pattern.upper())`).
pub(crate) fn fnmatch(name: &str, pattern: &str, case_sensitive: bool) -> bool {
    let (n, p) = if case_sensitive {
        (name.to_owned(), pattern.to_owned())
    } else {
        (name.to_uppercase(), pattern.to_uppercase())
    };
    let p: Vec<char> = p.chars().collect();
    let n: Vec<char> = n.chars().collect();
    matches(&compile(&p), &n)
}

#[operator(
    poll = PATTERN_POLL,
    tier = T0,
    support = Partial("bone names (Pose Mode, armature Edit Mode) arrive with armatures in M6")
)]
fn exec(cx: &mut OpCtx, p: object::SelectPattern) -> OpResult {
    if bone_mode(&cx.ctx()) {
        return Err(OpError::NotImplemented(
            "frap: select_pattern on bone names (Pose Mode, armature Edit Mode) arrives with \
             armatures in M6"
                .into(),
        ));
    }
    let bases = visible_bases(cx);
    let changes = {
        let main = cx.main();
        let hit = |ob: IdRef<ObjectId>| -> bool {
            main.objects.get(ob).is_some_and(|o| {
                fnmatch(&o.header.name, &p.pattern.value, p.case_sensitive.value)
            })
        };
        plan(&bases, p.extend.value, hit)
    };
    set_selection(cx, &changes);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::fnmatch;

    #[test]
    fn python_fnmatch_cases() {
        // (name, pattern, fnmatchcase result) checked against CPython 3.11.
        let cases = [
            ("Cube", "C*", true),
            ("Cube", "c*", false),
            ("Light", "L?ght", true),
            ("Light", "L?ht", false),
            ("Cube", "[CL]*", true),
            ("Camera", "[!C]*", false),
            ("Light", "[!C]*", true),
            ("Cube", "*", true),
            ("", "*", true),
            ("", "?", false),
            ("a[b", "a[b", true),
            ("ab", "a[b", false),
            ("Cube.001", "*.0[0-9][1-3]", true),
            ("x", "[z-a]", false),
            ("x", "[!z-a]", true),
            ("-", "[a-]", true),
            ("-", "[-a]", true),
            ("b", "[a-c]", true),
            ("]", "[]]", true),
            ("a", "[!]]", true),
            ("]", "[!]]", false),
            ("a\nb", "a*b", true),
            ("aXbXc", "a*b*c", true),
            ("abc", "a**c", true),
            ("abc", "ab", false),
            ("b", "[a-cx-z]", true),
            ("y", "[a-cx-z]", true),
            ("-", "[a-c-z]", true),
            ("d", "[a-c-z]", false),
            ("-", "[--0]", true),
            ("/", "[--0]", true),
        ];
        for (name, pat, want) in cases {
            assert_eq!(fnmatch(name, pat, true), want, "{name:?} ~ {pat:?}");
        }
    }

    #[test]
    fn case_insensitive_upper_cases_both() {
        assert!(fnmatch("Cube", "c*", false));
        assert!(fnmatch("cube", "CUBE", false));
        assert!(!fnmatch("Cube", "c*", true));
    }
}
