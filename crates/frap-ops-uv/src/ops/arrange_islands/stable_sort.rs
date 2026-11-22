// SPDX-License-Identifier: GPL-3.0-or-later
//! `std::stable_sort` as the reference build's C++ standard library runs it (LLVM libc++,
//! Blender 5.2.4 on macOS): insertion sort up to 128 elements of a trivially copyable type,
//! merge sort through a buffer above. `uv.arrange_islands` sorts with a comparator that is not
//! a strict weak order (`area_a >= area_b`), so the order of equal-area islands is the
//! algorithm's, not "stable": porting the exact algorithm keeps Blender's order.
//!
//! Ported from libc++ `<__algorithm/stable_sort.h>` (`__stable_sort`, `__stable_sort_move`,
//! `__insertion_sort`, `__insertion_sort_move`, `__merge_move_construct`,
//! `__merge_move_assign`), Apache-2.0 WITH LLVM-exception, which is compatible with
//! GPL-3.0-or-later.

/// `__stable_sort_switch<T>::value` for a trivially copy-assignable `T`.
const SWITCH: usize = 128;

/// `std::stable_sort(v.begin(), v.end(), comp)` (`comp(a, b)`: `a` goes before `b`).
pub(crate) fn stable_sort<T: Copy>(v: &mut [T], comp: &impl Fn(&T, &T) -> bool) {
    let len = v.len();
    if len <= 1 {
        return;
    }
    // `get_temporary_buffer` succeeds: a buffer of `len` elements.
    let mut buf: Vec<T> = v.to_vec();
    sort(v, comp, &mut buf);
}

/// `__stable_sort(first, last, comp, len, buff, buff_size)` with `buff_size >= len`.
fn sort<T: Copy>(v: &mut [T], comp: &impl Fn(&T, &T) -> bool, buf: &mut [T]) {
    let len = v.len();
    match len {
        0 | 1 => return,
        2 => {
            if comp(&v[1], &v[0]) {
                v.swap(0, 1);
            }
            return;
        }
        _ => {}
    }
    if len <= SWITCH {
        insertion_sort(v, comp);
        return;
    }
    let l2 = len / 2;
    {
        let (b1, b2) = buf.split_at_mut(l2);
        let (v1, v2) = v.split_at_mut(l2);
        sort_move(v1, comp, &mut b1[..l2]);
        sort_move(v2, comp, &mut b2[..len - l2]);
    }
    // `__merge_move_assign(buff, buff + l2, buff + l2, buff + len, first, comp)`.
    let (a, b) = buf[..len].split_at(l2);
    merge(a, b, v, comp);
}

/// `__stable_sort_move(first, last, comp, len, out)`: sorts `v` into `out`.
fn sort_move<T: Copy>(v: &mut [T], comp: &impl Fn(&T, &T) -> bool, out: &mut [T]) {
    let len = v.len();
    match len {
        0 => return,
        1 => {
            out[0] = v[0];
            return;
        }
        2 => {
            if comp(&v[1], &v[0]) {
                out[0] = v[1];
                out[1] = v[0];
            } else {
                out[0] = v[0];
                out[1] = v[1];
            }
            return;
        }
        _ => {}
    }
    if len <= 8 {
        insertion_sort_move(v, out, comp);
        return;
    }
    let l2 = len / 2;
    {
        let (v1, v2) = v.split_at_mut(l2);
        let (o1, o2) = out.split_at_mut(l2);
        sort(v1, comp, o1);
        sort(v2, comp, o2);
    }
    // `__merge_move_construct(first, m, m, last, out, comp)`.
    let (a, b) = v.split_at(l2);
    merge(a, b, out, comp);
}

/// `__merge_move_construct` / `__merge_move_assign`: merges `a` and `b` into `out`, taking
/// from `b` only when `comp(b, a)`.
fn merge<T: Copy>(a: &[T], b: &[T], out: &mut [T], comp: &impl Fn(&T, &T) -> bool) {
    let (mut i, mut j, mut k) = (0, 0, 0);
    while i < a.len() {
        if j == b.len() {
            while i < a.len() {
                out[k] = a[i];
                i += 1;
                k += 1;
            }
            return;
        }
        if comp(&b[j], &a[i]) {
            out[k] = b[j];
            j += 1;
        } else {
            out[k] = a[i];
            i += 1;
        }
        k += 1;
    }
    while j < b.len() {
        out[k] = b[j];
        j += 1;
        k += 1;
    }
}

/// `__insertion_sort`.
fn insertion_sort<T: Copy>(v: &mut [T], comp: &impl Fn(&T, &T) -> bool) {
    for i in 1..v.len() {
        let mut j = i - 1;
        if comp(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == 0 {
                    break;
                }
                k -= 1;
                if !comp(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
        }
    }
}

/// `__insertion_sort_move`: insertion-sorts `v` into `out`.
fn insertion_sort_move<T: Copy>(v: &[T], out: &mut [T], comp: &impl Fn(&T, &T) -> bool) {
    if v.is_empty() {
        return;
    }
    out[0] = v[0];
    for (last2, &x) in (1usize..).zip(v[1..].iter()) {
        let mut j2 = last2;
        let mut i2 = j2 - 1;
        if comp(&x, &out[i2]) {
            out[j2] = out[i2];
            j2 -= 1;
            while i2 != 0 {
                i2 -= 1;
                if !comp(&x, &out[i2]) {
                    break;
                }
                out[j2] = out[i2];
                j2 -= 1;
            }
            out[j2] = x;
        } else {
            out[j2] = x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_comparator_is_stable() {
        for n in [0usize, 1, 2, 3, 8, 9, 100, 128, 129, 300, 1000] {
            let mut v: Vec<(u32, usize)> = (0..n).map(|i| ((i * 7919 % 13) as u32, i)).collect();
            let mut expect = v.clone();
            expect.sort_by_key(|x| x.0);
            stable_sort(&mut v, &|a, b| a.0 < b.0);
            assert_eq!(v, expect, "n = {n}");
        }
    }

    #[test]
    fn non_strict_comparator_reverses_equal_runs_in_insertion_sort() {
        // `area_a >= area_b` (LARGE_TO_SMALL) on equal keys: insertion sort moves every later
        // element in front of the equal earlier ones, so equal runs come out reversed.
        let mut v: Vec<(u32, usize)> = vec![(1, 0), (1, 1), (2, 2), (1, 3)];
        stable_sort(&mut v, &|a, b| a.0 >= b.0);
        assert_eq!(v, vec![(2, 2), (1, 3), (1, 1), (1, 0)]);
        // Two elements: `comp(*--last, *first)` swaps equal ones.
        let mut w: Vec<(u32, usize)> = vec![(1, 0), (1, 1)];
        stable_sort(&mut w, &|a, b| a.0 >= b.0);
        assert_eq!(w, vec![(1, 1), (1, 0)]);
    }
}
