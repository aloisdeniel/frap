// SPDX-License-Identifier: GPL-3.0-or-later
//! Unique names (ARCHITECTURE §5.4).
//!
//! Blender 5.2 has two algorithms, both ported here:
//!
//! * **ID names** (`bpy.data.*`): `BKE_main_namemap_get_unique_name`
//!   (`blenkernel/intern/main_namemap.cc`). On a collision the name is split at its last
//!   `.NNN`; the *lowest* number ≥ 1 not used by any name with the same base is taken
//!   (`Foo.001` requested while `Foo`, `Foo.001`, `Foo.002` exist → `Foo.003`; a duplicate of
//!   `Bar.5` → `Bar.001`). A result longer than the limit drops one UTF-8 character from the
//!   base and starts again (so a duplicate 255-byte name becomes the 254-byte name). See
//!   [`unique_id_name`] (exact, from the existing names) and [`unique_name`] (from a predicate).
//! * **Sub-data names** (modifiers, nodes, vertex groups, bones, …): `BLI_uniquename_cb`
//!   (`blenlib/intern/string_utils.cc`): counts up from the requested number (`Bevel.005`
//!   duplicated → `Bevel.006`) and truncates the base so that base + suffix fits. See
//!   [`unique_subname`].
//!
//! Both split names with `BLI_string_split_name_number` (all-ASCII-digit suffix after the last
//! `.`, parsed with `std::stoi`, so it must fit an `int`). Every case is pinned against Blender
//! 5.2.4 in `crates/frap-rna/tests/naming.rs`.

use std::collections::{BTreeSet, HashMap, HashSet};

use smol_str::SmolStr;

/// Largest suffix number the ID name map hands out (`MAX_NUMBER`).
const MAX_NUMBER: u32 = 999_999_999;
/// Numbers tracked exactly per base (`UniqueName_Value::max_exact_tracking`).
const MAX_EXACT_TRACKING: u32 = 1023;

/// Truncates to at most `max` bytes on a UTF-8 character boundary (`BLI_strncpy_utf8`).
pub fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// `BLI_string_split_name_number(name, '.')`: `"Foo.001"` → `("Foo", Some(1))`. The suffix
/// counts only when it is non-empty, all ASCII digits and fits an `int`; otherwise the whole
/// name is the base (`"Foo."`, `"Foo.abc"`, `"Foo.99999999999"` → `(name, None)`).
pub fn split_suffix(name: &str) -> (&str, Option<u32>) {
    split_name_number(name, '.')
}

fn split_name_number(name: &str, delim: char) -> (&str, Option<u32>) {
    if let Some(dot) = name.rfind(delim) {
        let digits = &name[dot + delim.len_utf8()..];
        if !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && let Ok(n) = digits.parse::<i32>()
        {
            return (&name[..dot], Some(n as u32));
        }
    }
    (name, None)
}

/// What the ID algorithm needs to know about the names already in use.
trait NameIndex {
    /// The full name is in use.
    fn full_taken(&self, name: &str) -> bool;
    /// `UniqueName_Value::get_smallest_unused` for `base` (`None`: no number available).
    fn smallest_unused(&self, base: &str) -> Option<u32>;
    /// `base` is known and its largest number in use is `MAX_NUMBER`.
    fn base_saturated(&self, base: &str) -> bool;
}

/// Exact index built from every existing name (Blender's `UniqueName_TypeMap`).
struct ExactIndex {
    full: HashSet<String>,
    /// base → (numbers ≤ 1023 in use, largest number ≤ MAX_NUMBER in use)
    bases: HashMap<String, (BTreeSet<u32>, Option<u32>)>,
}

impl ExactIndex {
    fn new<'a>(existing: impl IntoIterator<Item = &'a str>) -> Self {
        let mut idx = ExactIndex {
            full: HashSet::new(),
            bases: HashMap::new(),
        };
        for name in existing {
            idx.full.insert(name.to_owned());
            let (base, n) = split_suffix(name);
            let n = n.unwrap_or(0);
            let e = idx.bases.entry(base.to_owned()).or_default();
            if n <= MAX_EXACT_TRACKING {
                e.0.insert(n);
            }
            if n <= MAX_NUMBER {
                e.1 = Some(e.1.map_or(n, |m| m.max(n)));
            }
        }
        idx
    }
}

impl NameIndex for ExactIndex {
    fn full_taken(&self, name: &str) -> bool {
        self.full.contains(name)
    }
    fn smallest_unused(&self, base: &str) -> Option<u32> {
        let Some((mask, max)) = self.bases.get(base) else {
            return Some(1);
        };
        // `mask.size()` is the highest tracked number + 1.
        let mask_size = mask.last().map_or(0, |m| m + 1);
        if mask_size < 2 {
            return Some(1);
        }
        if let Some(n) = (1..mask_size).find(|n| !mask.contains(n)) {
            return Some(n);
        }
        if mask_size <= MAX_EXACT_TRACKING {
            return Some(mask_size);
        }
        match max {
            Some(m) if *m < MAX_NUMBER => Some(m + 1),
            Some(_) => None,
            None => Some(1),
        }
    }
    fn base_saturated(&self, base: &str) -> bool {
        self.bases
            .get(base)
            .is_some_and(|(_, max)| max.unwrap_or(0) >= MAX_NUMBER)
    }
}

/// Index answering from a "is this full name taken" predicate.
struct PredicateIndex<'a> {
    taken: &'a dyn Fn(&str) -> bool,
}

impl PredicateIndex<'_> {
    /// Whether some name splits to `(base, n)`: tries every zero padding up to 10 digits.
    fn number_used(&self, base: &str, n: u32) -> bool {
        let digits = n.to_string().len();
        (digits..=10).any(|w| (self.taken)(&format!("{base}.{n:0w$}")))
    }
}

impl NameIndex for PredicateIndex<'_> {
    fn full_taken(&self, name: &str) -> bool {
        (self.taken)(name)
    }
    fn smallest_unused(&self, base: &str) -> Option<u32> {
        (1..=MAX_NUMBER).find(|n| !self.number_used(base, *n))
    }
    fn base_saturated(&self, _base: &str) -> bool {
        false
    }
}

/// Removes the last UTF-8 character.
fn pop_char(s: &str) -> &str {
    match s.char_indices().last() {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// `id_name_final_build`: `Ok(final name)` or `Err(new name to retry with)`.
fn id_name_final_build(
    idx: &dyn NameIndex,
    base: &str,
    number: Option<u32>,
    max_bytes: usize,
) -> Result<String, String> {
    if let Some(n) = number {
        let name = format!("{base}.{n:03}");
        if name.len() <= max_bytes {
            return Ok(name);
        }
    }
    // Shorten the base one character at a time while it is longer than 8 bytes.
    let mut r = base.to_owned();
    while r.len() > 8 {
        r = pop_char(&r).to_owned();
        if !idx.base_saturated(&r) {
            return Err(r);
        }
    }
    // Short base: extend it with `_NNN` instead.
    let new_base = r;
    let mut suffix: u64 = 1;
    let mut r = format!("{new_base}_{suffix:03}");
    while r.len() + 12 < max_bytes + 1 {
        if !idx.base_saturated(&r) {
            return Err(r);
        }
        suffix += 1;
        r = format!("{new_base}_{suffix:03}");
    }
    Err(r)
}

fn id_unique(
    idx: &dyn NameIndex,
    requested: &str,
    max_bytes: usize,
    empty_default: &str,
) -> SmolStr {
    let requested = if requested.is_empty() {
        empty_default
    } else {
        requested
    };
    let mut name = truncate_utf8(requested, max_bytes).to_owned();
    loop {
        if !idx.full_taken(&name) {
            return name.into();
        }
        let (base, _) = split_suffix(&name);
        let base = base.to_owned();
        let number = idx.smallest_unused(&base);
        match id_name_final_build(idx, &base, number, max_bytes) {
            Ok(n) if !idx.full_taken(&n) => return n.into(),
            // Only reachable with a predicate index that missed a spelling; keep searching.
            Ok(n) => name = n,
            Err(n) => name = n,
        }
    }
}

/// Unique **ID** name (`BKE_main_namemap_get_unique_name`) from a predicate telling whether a
/// full name is taken. Returns `requested` if free (empty → `empty_default`, truncated to
/// `max_bytes` on a UTF-8 boundary), else `base.NNN` with the lowest free number ≥ 1.
///
/// A number counts as used when any zero-padded spelling of it is taken (`Foo.1`, `Foo.001`,
/// …, up to 10 digits). Where Blender stops tracking exact numbers (above 1023 in use for one
/// base) it continues from the largest number in use, which a predicate cannot know; this
/// function then takes the lowest free number. [`unique_id_name`] is exact.
pub fn unique_name(
    taken: &dyn Fn(&str) -> bool,
    requested: &str,
    max_bytes: usize,
    empty_default: &str,
) -> SmolStr {
    id_unique(
        &PredicateIndex { taken },
        requested,
        max_bytes,
        empty_default,
    )
}

/// Unique **ID** name, exact port of `BKE_main_namemap_get_unique_name` given every name
/// already in use in the ID's namespace (same type, same library).
pub fn unique_id_name<'a>(
    existing: impl IntoIterator<Item = &'a str>,
    requested: &str,
    max_bytes: usize,
    empty_default: &str,
) -> SmolStr {
    id_unique(
        &ExactIndex::new(existing),
        requested,
        max_bytes,
        empty_default,
    )
}

/// Unique **sub-data** name (`BLI_uniquename_cb` with delimiter `.`): modifiers, nodes,
/// sockets, vertex groups, bones, UV maps, view layers… `max_bytes` is the name's byte limit
/// (63 for `MAX_NAME` sub-data). Counts up from the requested number and truncates the base so
/// base + suffix fit.
pub fn unique_subname(
    taken: &dyn Fn(&str) -> bool,
    requested: &str,
    max_bytes: usize,
    empty_default: &str,
) -> SmolStr {
    let requested = if requested.is_empty() {
        empty_default
    } else {
        requested
    };
    let name = truncate_utf8(requested, max_bytes);
    if !taken(name) {
        return name.into();
    }
    let maxncpy = max_bytes + 1;
    let (left, number) = split_name_number(name, '.');
    let mut number = number.unwrap_or(0);
    loop {
        number = number.wrapping_add(1);
        let numstr = format!(".{number:03}");
        let candidate = if left.is_empty() || numstr.len() + 1 >= maxncpy {
            truncate_utf8(&numstr, max_bytes).to_owned()
        } else {
            format!(
                "{}{numstr}",
                truncate_utf8(left, maxncpy - numstr.len() - 1)
            )
        };
        if !taken(&candidate) {
            return candidate.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(taken: &[&str], req: &str) -> String {
        unique_name(&|n| taken.contains(&n), req, 255, "Object").to_string()
    }

    #[test]
    fn blender_rules() {
        assert_eq!(run(&[], "Cube"), "Cube");
        assert_eq!(run(&["Cube"], "Cube"), "Cube.001");
        assert_eq!(run(&["Cube", "Cube.001"], "Cube"), "Cube.002");
        assert_eq!(run(&["Foo.001", "Foo.002"], "Foo.001"), "Foo.003");
        assert_eq!(run(&["Bar.5"], "Bar.5"), "Bar.001");
        assert_eq!(run(&[], ""), "Object");
    }

    #[test]
    fn split() {
        assert_eq!(split_suffix("Foo.001"), ("Foo", Some(1)));
        assert_eq!(split_suffix("Foo.0001"), ("Foo", Some(1)));
        assert_eq!(split_suffix("Foo."), ("Foo.", None));
        assert_eq!(split_suffix("Foo.abc"), ("Foo.abc", None));
        assert_eq!(split_suffix("Foo.99999999999"), ("Foo.99999999999", None));
        assert_eq!(split_suffix("Foo.2147483647"), ("Foo", Some(2_147_483_647)));
        assert_eq!(split_suffix("Foo.+1"), ("Foo.+1", None));
        assert_eq!(split_suffix("Foo"), ("Foo", None));
        assert_eq!(pop_char("aé"), "a");
        assert_eq!(truncate_utf8("éé", 3), "é");
    }
}
