// SPDX-License-Identifier: GPL-3.0-or-later
//! Structural JSON diff of two dumps (WP 8.9).
//!
//! Objects are compared by key. Arrays of objects that all carry a unique `idname`,
//! `identifier` or `bl_idname` (operators, properties, functions, parameters, enum items,
//! sockets) are compared by that key, so an insertion is one `Added`, not a shifted tail; a
//! removed and an added element whose contents are equal apart from the key (or apart from the
//! key, `name` and `description`) are reported as one `Renamed`. Arrays of unique strings
//! (`options`, `flags`, the `_meta.json` lists) are compared as ordered sets. Every other array
//! (default arrays, matrices, coordinates) is one value.

use serde_json::Value;

/// One step of a path into a dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seg {
    /// Object member.
    Key(String),
    /// Element of a keyed array (its `identifier`/`idname`) or of a string set.
    Id(String),
}

/// Path from the document root.
pub type Path = Vec<Seg>;

/// One structural difference (old = `spec/blender-5.2`, new = the re-dump).
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// `at` (ending in the new element) exists only in the new dump.
    Added { at: Path, value: Value },
    /// `at` exists only in the old dump.
    Removed { at: Path, value: Value },
    /// An element of the keyed array `at` changed its key only.
    Renamed {
        at: Path,
        from: String,
        to: String,
        value: Value,
    },
    /// The value at `at` changed.
    Changed { at: Path, old: Value, new: Value },
    /// The common elements of the array `at` changed order.
    Reordered {
        at: Path,
        old: Vec<String>,
        new: Vec<String>,
    },
}

impl Change {
    /// The path the change is reported at.
    pub fn at(&self) -> &Path {
        match self {
            Change::Added { at, .. }
            | Change::Removed { at, .. }
            | Change::Renamed { at, .. }
            | Change::Changed { at, .. }
            | Change::Reordered { at, .. } => at,
        }
    }
}

/// Every difference between `old` and `new`, in document order (old order first).
pub fn diff(old: &Value, new: &Value) -> Vec<Change> {
    let mut out = Vec::new();
    walk(&mut Vec::new(), old, new, &mut out);
    out
}

const KEY_FIELDS: [&str; 3] = ["idname", "identifier", "bl_idname"];

fn key_of<'a>(v: &'a Value, field: &str) -> Option<&'a str> {
    v.as_object()?.get(field)?.as_str()
}

/// The key field of an array of objects, if every element has a unique string value for it.
fn key_field(arr: &[Value]) -> Option<&'static str> {
    if arr.is_empty() {
        return None;
    }
    KEY_FIELDS.into_iter().find(|f| {
        let mut seen = std::collections::BTreeSet::new();
        arr.iter()
            .all(|e| key_of(e, f).is_some_and(|k| seen.insert(k)))
    })
}

fn string_set(arr: &[Value]) -> Option<Vec<&str>> {
    let mut seen = std::collections::BTreeSet::new();
    arr.iter()
        .map(|e| e.as_str().filter(|s| seen.insert(*s)))
        .collect()
}

fn push(path: &[Seg], seg: Seg) -> Path {
    let mut p = path.to_vec();
    p.push(seg);
    p
}

fn walk(path: &mut Path, old: &Value, new: &Value, out: &mut Vec<Change>) {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => {
                        path.push(Seg::Key(k.clone()));
                        walk(path, va, vb, out);
                        path.pop();
                    }
                    None => out.push(Change::Removed {
                        at: push(path, Seg::Key(k.clone())),
                        value: va.clone(),
                    }),
                }
            }
            for (k, vb) in b {
                if !a.contains_key(k) {
                    out.push(Change::Added {
                        at: push(path, Seg::Key(k.clone())),
                        value: vb.clone(),
                    });
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a == b {
                return;
            }
            let field = match (key_field(a), key_field(b)) {
                (Some(x), Some(y)) if x == y => Some(x),
                (Some(x), None) if b.is_empty() => Some(x),
                (None, Some(y)) if a.is_empty() => Some(y),
                _ => None,
            };
            if let Some(f) = field {
                keyed(path, a, b, f, out);
            } else if let (Some(sa), Some(sb)) = (string_set(a), string_set(b)) {
                strings(path, &sa, &sb, out);
            } else {
                out.push(Change::Changed {
                    at: path.clone(),
                    old: old.clone(),
                    new: new.clone(),
                });
            }
        }
        _ if old == new => {}
        _ => out.push(Change::Changed {
            at: path.clone(),
            old: old.clone(),
            new: new.clone(),
        }),
    }
}

fn strings(path: &Path, a: &[&str], b: &[&str], out: &mut Vec<Change>) {
    for s in a.iter().filter(|s| !b.contains(s)) {
        out.push(Change::Removed {
            at: push(path, Seg::Id((*s).to_owned())),
            value: Value::String((*s).to_owned()),
        });
    }
    for s in b.iter().filter(|s| !a.contains(s)) {
        out.push(Change::Added {
            at: push(path, Seg::Id((*s).to_owned())),
            value: Value::String((*s).to_owned()),
        });
    }
    reorder(path, a, b, &[], out);
}

/// The element without the fields a rename may change (`strict`: the key only).
fn stripped(v: &Value, field: &str, strict: bool) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.shift_remove(field);
        if !strict {
            o.shift_remove("name");
            o.shift_remove("description");
        }
    }
    v
}

fn keyed(path: &mut Path, a: &[Value], b: &[Value], field: &str, out: &mut Vec<Change>) {
    let ka: Vec<&str> = a.iter().filter_map(|e| key_of(e, field)).collect();
    let kb: Vec<&str> = b.iter().filter_map(|e| key_of(e, field)).collect();
    let get = |arr: &'_ [Value], keys: &[&str], k: &str| {
        keys.iter()
            .position(|x| *x == k)
            .map(|i| arr[i].clone())
            .expect("invariant: key taken from this array")
    };
    let removed: Vec<&str> = ka.iter().copied().filter(|k| !kb.contains(k)).collect();
    let mut added: Vec<&str> = kb.iter().copied().filter(|k| !ka.contains(k)).collect();
    // Renames: a unique strict match first, then a unique loose match.
    let mut renames: Vec<(&str, &str)> = Vec::new();
    for strict in [true, false] {
        for r in &removed {
            if renames.iter().any(|(f, _)| f == r) {
                continue;
            }
            let want = stripped(&get(a, &ka, r), field, strict);
            let hits: Vec<&str> = added
                .iter()
                .copied()
                .filter(|x| stripped(&get(b, &kb, x), field, strict) == want)
                .collect();
            if let [hit] = hits[..] {
                renames.push((r, hit));
                added.retain(|x| *x != hit);
            }
        }
    }
    for (i, k) in ka.iter().enumerate() {
        if let Some((_, to)) = renames.iter().find(|(f, _)| f == k) {
            let nb = get(b, &kb, to);
            out.push(Change::Renamed {
                at: path.clone(),
                from: (*k).to_owned(),
                to: (*to).to_owned(),
                value: nb.clone(),
            });
            // A loose rename may also change `name`/`description`.
            path.push(Seg::Id((*to).to_owned()));
            walk(
                path,
                &stripped(&a[i], field, true),
                &stripped(&nb, field, true),
                out,
            );
            path.pop();
        } else if kb.contains(k) {
            path.push(Seg::Id((*k).to_owned()));
            walk(path, &a[i], &get(b, &kb, k), out);
            path.pop();
        } else {
            out.push(Change::Removed {
                at: push(path, Seg::Id((*k).to_owned())),
                value: a[i].clone(),
            });
        }
    }
    for k in &added {
        out.push(Change::Added {
            at: push(path, Seg::Id((*k).to_owned())),
            value: get(b, &kb, k),
        });
    }
    reorder(path, &ka, &kb, &renames, out);
}

/// Emits `Reordered` when the elements present in both arrays (renamed ones under their new
/// key) are not in the same relative order.
fn reorder(path: &Path, a: &[&str], b: &[&str], renames: &[(&str, &str)], out: &mut Vec<Change>) {
    let common_a: Vec<String> = a
        .iter()
        .map(|k| renames.iter().find(|(f, _)| f == k).map_or(*k, |(_, t)| *t))
        .filter(|k| b.contains(k))
        .map(str::to_owned)
        .collect();
    let common_b: Vec<String> = b
        .iter()
        .filter(|k| common_a.iter().any(|c| c == *k))
        .map(|k| (*k).to_owned())
        .collect();
    if common_a != common_b {
        out.push(Change::Reordered {
            at: path.clone(),
            old: common_a,
            new: common_b,
        });
    }
}

/// `props[case_sensitive].default`, `poll.D`.
pub fn fmt_path(p: &[Seg]) -> String {
    let mut s = String::new();
    for seg in p {
        match seg {
            Seg::Key(k) => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(k);
            }
            Seg::Id(k) => {
                s.push('[');
                s.push_str(k);
                s.push(']');
            }
        }
    }
    s
}
