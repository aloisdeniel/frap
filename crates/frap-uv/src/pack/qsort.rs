// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_qsort_r`: the BSD `qsort` (Bentley–McIlroy three-way partitioning with an insertion
//! sort below 7 elements) Blender ships for portability. It is **not stable**, and the box
//! packer sorts vertices with many equal keys, so the exact element order matters: this is a
//! port of the algorithm on a slice instead of `char *` arithmetic.
//!
//! Ported from blender/source/blender/blenlib/intern/sort.cc (Blender 5.2.4).
//!
//! [`qsort_libc`] is the C library `qsort` of macOS (`libsystem_c`), which Blender calls
//! directly where it does not use `BLI_qsort_r` (`uv.smart_project`'s area sort): the same
//! partitioning, but an insertion sort up to 7 elements, and after a partition pass without
//! swaps the pivot is moved into place first and the insertion sort gives up after
//! `1 + n / 4` swaps (FreeBSD's "nevermind" shortcut), going on with the partitions. The
//! algorithm was reconstructed from comparison traces of the system `qsort` and checked
//! against it on 300000 random arrays of up to 198 elements with many equal keys (identical
//! permutations of equal keys every time, WP 8.1).

use core::cmp::Ordering;

/// `med3`.
fn med3<T: Copy>(
    a: &[T],
    i: usize,
    j: usize,
    k: usize,
    cmp: &mut impl FnMut(&T, &T) -> Ordering,
) -> usize {
    if cmp(&a[i], &a[j]) == Ordering::Less {
        if cmp(&a[j], &a[k]) == Ordering::Less {
            j
        } else if cmp(&a[i], &a[k]) == Ordering::Less {
            k
        } else {
            i
        }
    } else if cmp(&a[j], &a[k]) == Ordering::Greater {
        j
    } else if cmp(&a[i], &a[k]) == Ordering::Less {
        i
    } else {
        k
    }
}

/// `vecswap(a, b, n)`.
fn vecswap<T>(a: &mut [T], i: usize, j: usize, n: usize) {
    for k in 0..n {
        a.swap(i + k, j + k);
    }
}

fn insertion<T: Copy>(a: &mut [T], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    for pm in 1..a.len() {
        let mut pl = pm;
        while pl > 0 && cmp(&a[pl - 1], &a[pl]) == Ordering::Greater {
            a.swap(pl, pl - 1);
            pl -= 1;
        }
    }
}

/// `BLI_qsort_r(a, n, es, cmp, thunk)`.
pub fn qsort_r<T: Copy>(mut a: &mut [T], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    loop {
        let n = a.len();
        let mut swap_cnt = false;
        if n < 7 {
            insertion(a, cmp);
            return;
        }
        let mut pm = n / 2;
        if n > 7 {
            let mut pl = 0;
            let mut pn = n - 1;
            if n > 40 {
                let d = n / 8;
                pl = med3(a, pl, pl + d, pl + 2 * d, cmp);
                pm = med3(a, pm - d, pm, pm + d, cmp);
                pn = med3(a, pn - 2 * d, pn - d, pn, cmp);
            }
            pm = med3(a, pl, pm, pn, cmp);
        }
        a.swap(0, pm);
        let mut pa = 1usize;
        let mut pb = 1usize;
        // `pc`/`pd` may step below `pb` (to `pb - 1`), so they are signed.
        let mut pc = n as isize - 1;
        let mut pd = n as isize - 1;
        loop {
            while pb as isize <= pc {
                let r = cmp(&a[pb], &a[0]);
                if r == Ordering::Greater {
                    break;
                }
                if r == Ordering::Equal {
                    swap_cnt = true;
                    a.swap(pa, pb);
                    pa += 1;
                }
                pb += 1;
            }
            while pb as isize <= pc {
                let r = cmp(&a[pc as usize], &a[0]);
                if r == Ordering::Less {
                    break;
                }
                if r == Ordering::Equal {
                    swap_cnt = true;
                    a.swap(pc as usize, pd as usize);
                    pd -= 1;
                }
                pc -= 1;
            }
            if pb as isize > pc {
                break;
            }
            a.swap(pb, pc as usize);
            swap_cnt = true;
            pb += 1;
            pc -= 1;
        }
        if !swap_cnt {
            // Switch to insertion sort.
            insertion(a, cmp);
            return;
        }

        let pn = n;
        let r = pa.min(pb - pa);
        vecswap(a, 0, pb - r, r);
        let r = ((pd - pc) as usize).min(pn - pd as usize - 1);
        vecswap(a, pb, pn - r, r);
        let r = pb - pa;
        if r > 1 {
            qsort_r(&mut a[..r], cmp);
        }
        let r = (pd - pc) as usize;
        if r > 1 {
            let tmp = a;
            a = &mut tmp[pn - r..];
            continue;
        }
        return;
    }
}

/// Insertion sort that gives up after more than `limit` swaps (`false`).
fn insertion_limited<T: Copy>(
    a: &mut [T],
    limit: usize,
    cmp: &mut impl FnMut(&T, &T) -> Ordering,
) -> bool {
    let mut swaps = 0usize;
    for pm in 1..a.len() {
        let mut pl = pm;
        while pl > 0 && cmp(&a[pl - 1], &a[pl]) == Ordering::Greater {
            a.swap(pl, pl - 1);
            swaps += 1;
            if swaps > limit {
                return false;
            }
            pl -= 1;
        }
    }
    true
}

/// The macOS C library `qsort(a, n, es, cmp)` (see the module documentation). Not stable.
pub fn qsort_libc<T: Copy>(mut a: &mut [T], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    loop {
        let n = a.len();
        let mut swap_cnt = false;
        if n < 8 {
            insertion(a, cmp);
            return;
        }
        let mut pl = 0;
        let mut pm = n / 2;
        let mut pn = n - 1;
        if n > 40 {
            let d = n / 8;
            pl = med3(a, pl, pl + d, pl + 2 * d, cmp);
            pm = med3(a, pm - d, pm, pm + d, cmp);
            pn = med3(a, pn - 2 * d, pn - d, pn, cmp);
        }
        pm = med3(a, pl, pm, pn, cmp);
        a.swap(0, pm);
        let mut pa = 1usize;
        let mut pb = 1usize;
        let mut pc = n as isize - 1;
        let mut pd = n as isize - 1;
        loop {
            while pb as isize <= pc {
                let r = cmp(&a[pb], &a[0]);
                if r == Ordering::Greater {
                    break;
                }
                if r == Ordering::Equal {
                    swap_cnt = true;
                    a.swap(pa, pb);
                    pa += 1;
                }
                pb += 1;
            }
            while pb as isize <= pc {
                let r = cmp(&a[pc as usize], &a[0]);
                if r == Ordering::Less {
                    break;
                }
                if r == Ordering::Equal {
                    swap_cnt = true;
                    a.swap(pc as usize, pd as usize);
                    pd -= 1;
                }
                pc -= 1;
            }
            if pb as isize > pc {
                break;
            }
            a.swap(pb, pc as usize);
            swap_cnt = true;
            pb += 1;
            pc -= 1;
        }
        let pn = n;
        let r = pa.min(pb - pa);
        vecswap(a, 0, pb - r, r);
        let r = ((pd - pc) as usize).min(pn - pd as usize - 1);
        vecswap(a, pb, pn - r, r);
        if !swap_cnt && insertion_limited(a, 1 + n / 4, cmp) {
            return;
        }
        let r = pb - pa;
        if r > 1 {
            qsort_libc(&mut a[..r], cmp);
        }
        let r = (pd - pc) as usize;
        if r > 1 {
            let tmp = a;
            a = &mut tmp[pn - r..];
            continue;
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_like_any_sort() {
        let mut rng = 12345u32;
        for len in [0usize, 1, 5, 7, 8, 40, 41, 100, 257] {
            let mut v: Vec<u32> = (0..len)
                .map(|_| {
                    rng = rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    (rng >> 16) % 17
                })
                .collect();
            let mut w = v.clone();
            qsort_r(&mut v, &mut |a: &u32, b: &u32| a.cmp(b));
            w.sort();
            assert_eq!(v, w);
        }
    }

    #[test]
    fn unstable_order_of_equal_keys_is_bsd() {
        // Keys compare on the high part only; the low part shows the permutation. Below 7
        // elements the BSD sort is an insertion sort, hence stable.
        let mut v = [(1, 0), (0, 1), (1, 2), (0, 3), (1, 4), (0, 5)];
        qsort_r(&mut v, &mut |a: &(i32, i32), b: &(i32, i32)| a.0.cmp(&b.0));
        assert_eq!(v, [(0, 1), (0, 3), (0, 5), (1, 0), (1, 2), (1, 4)]);
    }

    #[test]
    fn libc_qsort_sorts() {
        let mut rng = 99u32;
        for len in [0usize, 1, 7, 8, 9, 40, 41, 100, 300] {
            let mut v: Vec<u32> = (0..len)
                .map(|_| {
                    rng = rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    (rng >> 16) % 13
                })
                .collect();
            let mut w = v.clone();
            qsort_libc(&mut v, &mut |a: &u32, b: &u32| a.cmp(b));
            w.sort();
            assert_eq!(v, w);
        }
    }

    /// Permutations of equal keys recorded from macOS's `qsort` (libsystem_c, macOS 26) with
    /// `(key, index)` elements compared on the key only.
    #[test]
    fn libc_qsort_matches_macos_permutations() {
        let cases: [(&[i32], &[usize]); 4] = [
            (
                &[9, 0, 7, 7, 4, 7, 1, 3, 8, 7, 6, 10],
                &[1, 6, 7, 4, 10, 5, 9, 2, 3, 8, 0, 11],
            ),
            (
                &[7, 4, 4, 4, 0, 3, 4, 1, 3, 1, 5],
                &[4, 7, 9, 8, 5, 1, 6, 2, 3, 10, 0],
            ),
            (
                &[6, 4, 2, 5, 0, 0, 1, 2, 3, 2, 7],
                &[4, 5, 6, 2, 9, 7, 8, 1, 3, 0, 10],
            ),
            (
                &[1, 6, 6, 2, 9, 7, 10, 9, 3, 7, 0, 3, 11],
                &[10, 0, 3, 8, 11, 1, 2, 9, 5, 7, 4, 6, 12],
            ),
        ];
        for (keys, expect) in cases {
            let mut v: Vec<(i32, usize)> = keys.iter().copied().zip(0..).collect();
            qsort_libc(&mut v, &mut |a: &(i32, usize), b: &(i32, usize)| {
                a.0.cmp(&b.0)
            });
            let ids: Vec<usize> = v.iter().map(|e| e.1).collect();
            assert_eq!(ids, expect, "{keys:?}");
            // FreeBSD's order differs on these inputs.
            let mut w: Vec<(i32, usize)> = keys.iter().copied().zip(0..).collect();
            qsort_r(&mut w, &mut |a: &(i32, usize), b: &(i32, usize)| {
                a.0.cmp(&b.0)
            });
            assert_ne!(w, v);
        }
    }
}
