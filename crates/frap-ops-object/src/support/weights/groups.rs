// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex-group lists by name: the flip maps of mirror editing
//! (`BKE_object_defgroup_flip_map[_unlocked|_single]`, `BKE_object_defgroup_flip_index`,
//! blenkernel/intern/deform.cc), the remap of a re-ordered list (`vgroup_init_remap` /
//! `vgroup_do_remap`, editors/object/object_vgroup.cc) and the natural name order of
//! `vertex_group_sort` (`BLI_strcasecmp_natural`, blenlib/intern/string.cc).
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later).

use std::cmp::Ordering;

use frap_scene::edit_armature::names::flip_side_name;
use smol_str::SmolStr;

/// `object_defgroup_unlocked_flip_map_ex(ob, use_default, use_only_unlocked)`: for every group
/// the index of the group named after its flipped side (`.L` ↔ `.R`, …), `-1` (or its own
/// index with `use_default`) when there is none. With `only_unlocked`, a locked group does not
/// look for its partner (it keeps its default unless its partner is unlocked and pairs with
/// it). Empty without groups.
pub fn flip_map(
    names: &[SmolStr],
    locks: &[bool],
    use_default: bool,
    only_unlocked: bool,
) -> Vec<i32> {
    let n = names.len();
    let mut map = vec![-1i32; n];
    for i in 0..n {
        if map[i] != -1 {
            continue;
        }
        if use_default {
            map[i] = i as i32;
        }
        if only_unlocked && locks.get(i).copied().unwrap_or(false) {
            continue;
        }
        let flipped = flip_side_name(&names[i], false);
        if flipped != names[i].as_str()
            && let Some(f) = names.iter().position(|x| x == flipped.as_str())
        {
            map[i] = f as i32;
            map[f] = i as i32;
        }
    }
    map
}

/// `BKE_object_defgroup_flip_map_single(ob, use_default, defgroup)`: the identity (or `-1`s)
/// except for `group` and its flipped partner, which swap.
pub fn flip_map_single(names: &[SmolStr], use_default: bool, group: usize) -> Vec<i32> {
    let n = names.len();
    let mut map: Vec<i32> = (0..n)
        .map(|i| if use_default { i as i32 } else { -1 })
        .collect();
    if let Some(name) = names.get(group) {
        let flipped = flip_side_name(name, false);
        if flipped != name.as_str()
            && let Some(f) = names.iter().position(|x| x == flipped.as_str())
        {
            map[group] = f as i32;
            map[f] = group as i32;
        }
    }
    map
}

/// `BKE_object_defgroup_flip_index(ob, index, use_default)`: the index of the group named
/// after group `index`'s flipped side; with `use_default` the group itself when there is
/// none, else `None`.
pub fn flip_index(names: &[SmolStr], index: usize, use_default: bool) -> Option<usize> {
    let flipped = names.get(index).and_then(|name| {
        let f = flip_side_name(name, false);
        if f == name.as_str() {
            None
        } else {
            names.iter().position(|x| x == f.as_str())
        }
    });
    match flipped {
        Some(f) => Some(f),
        None if use_default => Some(index),
        None => None,
    }
}

/// `vgroup_do_remap`'s `sort_map`: for every group of the old order, its index in the new
/// order (names are unique).
pub fn remap_from_names(old: &[SmolStr], new: &[SmolStr]) -> Vec<usize> {
    old.iter()
        .enumerate()
        .map(|(i, n)| new.iter().position(|x| x == n).unwrap_or(i))
        .collect()
}

/// `left_number_strcmp`: compares the digit runs at the start of `a` and `b` (leading zeros
/// skipped); equal values with different zero counts set `tiebreaker` once.
fn left_number_strcmp(a: &[u8], b: &[u8], tiebreaker: &mut i32) -> i32 {
    let z1 = a.iter().take_while(|&&c| c == b'0').count();
    let z2 = b.iter().take_while(|&&c| c == b'0').count();
    let (p1, p2) = (&a[z1..], &b[z2..]);
    let digit = |s: &[u8], i: usize| s.get(i).is_some_and(|c| c.is_ascii_digit());
    let mut numdigit = 0;
    loop {
        match (digit(p1, numdigit), digit(p2, numdigit)) {
            (true, true) => numdigit += 1,
            (true, false) => return 1,
            (false, true) => return -1,
            (false, false) => break,
        }
    }
    if numdigit > 0 {
        match p1[..numdigit].cmp(&p2[..numdigit]) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {}
        }
    }
    if *tiebreaker == 0 {
        *tiebreaker = match z1.cmp(&z2) {
            Ordering::Greater => 1,
            Ordering::Less => -1,
            Ordering::Equal => 0,
        };
    }
    0
}

/// `tolower` on a C `char` (signed on Blender's macOS and Windows builds): ASCII letters only.
fn lower(c: u8) -> i8 {
    c.to_ascii_lowercase() as i8
}

/// `BLI_strcasecmp_natural`: case-insensitive, digit runs compared as numbers, `.` sorting
/// before any other character, shorter strings first; leading zeros, then `strcmp` break ties.
pub fn strcasecmp_natural(s1: &str, s2: &str) -> Ordering {
    let (a, b) = (s1.as_bytes(), s2.as_bytes());
    let at = |s: &[u8], i: usize| s.get(i).copied().unwrap_or(0);
    let (mut d1, mut d2) = (0usize, 0usize);
    let mut tiebreaker = 0;
    loop {
        if at(a, d1).is_ascii_digit() && at(b, d2).is_ascii_digit() {
            let c = left_number_strcmp(&a[d1..], &b[d2..], &mut tiebreaker);
            if c != 0 {
                return c.cmp(&0);
            }
            d1 += 1;
            while at(a, d1).is_ascii_digit() {
                d1 += 1;
            }
            d2 += 1;
            while at(b, d2).is_ascii_digit() {
                d2 += 1;
            }
        }
        if at(a, d1) == 0 || at(b, d2) == 0 {
            break;
        }
        let (c1, c2) = (lower(at(a, d1)), lower(at(b, d2)));
        if c1 != c2 {
            if c1 == b'.' as i8 {
                return Ordering::Less;
            }
            if c2 == b'.' as i8 {
                return Ordering::Greater;
            }
            return c1.cmp(&c2);
        }
        d1 += 1;
        d2 += 1;
    }
    if tiebreaker != 0 {
        return tiebreaker.cmp(&0);
    }
    a.cmp(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<SmolStr> {
        v.iter().map(SmolStr::new).collect()
    }

    #[test]
    fn flip_maps() {
        let n = names(&["Arm.L", "Body", "Arm.R", "Leg.L"]);
        assert_eq!(flip_map(&n, &[], true, false), vec![2, 1, 0, 3]);
        assert_eq!(flip_map(&n, &[], false, false), vec![2, -1, 0, -1]);
        // A locked group does not look for its partner, but an unlocked partner finds it.
        assert_eq!(
            flip_map(&n, &[true, false, false, false], true, true),
            vec![2, 1, 0, 3]
        );
        assert_eq!(
            flip_map(&n, &[true, false, true, false], true, true),
            vec![0, 1, 2, 3]
        );
        assert_eq!(flip_map_single(&n, false, 2), vec![2, -1, 0, -1]);
        assert_eq!(flip_index(&n, 0, false), Some(2));
        assert_eq!(flip_index(&n, 3, false), None);
        assert_eq!(flip_index(&n, 3, true), Some(3));
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["b10", "B2", "a", "foo 1.bar", "foo.bar", "b02", "b2"];
        v.sort_by(|a, b| strcasecmp_natural(a, b));
        assert_eq!(
            v,
            vec!["a", "B2", "b2", "b02", "b10", "foo.bar", "foo 1.bar"]
        );
        assert_eq!(strcasecmp_natural("Group", "group"), Ordering::Less);
        assert_eq!(strcasecmp_natural("g", "g.001"), Ordering::Less);
    }

    #[test]
    fn remap_follows_names() {
        let old = names(&["A", "B", "C"]);
        let new = names(&["C", "A", "B"]);
        assert_eq!(remap_from_names(&old, &new), vec![1, 2, 0]);
    }
}
