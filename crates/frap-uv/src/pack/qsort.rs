// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_qsort_r`: the BSD `qsort` (Bentley–McIlroy three-way partitioning with an insertion
//! sort below 7 elements) Blender ships for portability. It is **not stable**, and the box
//! packer sorts vertices with many equal keys, so the exact element order matters: this is a
//! port of the algorithm on a slice instead of `char *` arithmetic.
//!
//! Ported from blender/source/blender/blenlib/intern/sort.cc (Blender 5.2.4).
//!
//! [`qsort_libc`] is the C library `qsort` of macOS (`libsystem_c`), which Blender calls
//! directly where it does not use `BLI_qsort_r` (`uv.smart_project`'s area sort): Apple's
//! newer FreeBSD `qsort.c`, the same partitioning, but an insertion sort up to 7 elements, a
//! recursion depth limit of `2 * (flsl(n) - 1)` (each partition step counts, the iterated tail
//! included) past which it falls back to the C library `heapsort`, and after a partition pass
//! without swaps it gives up the insertion sort after `1 + n / 4` swaps ("nevermind", after
//! the `=` blocks were swapped into the middle) and goes on with the partitions. The tests
//! compare it with the system `qsort` (a C program the tests compile on macOS) on structured
//! tie patterns (descending and ascending runs, saw-tooth, organ pipe, all equal) and random
//! keys for `n` from 0 to 2048.
//!
//! Ported from Apple Libc stdlib/FreeBSD/qsort.c and heapsort.c (macOS 26 `libsystem_c`).

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

/// `_isort(a, n, es, thunk, cmp, swap_limit, …)`: insertion sort that gives up after more than
/// `limit` swaps (`false`); `limit == 0` means no limit.
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
            if limit != 0 && swaps > limit {
                return false;
            }
            pl -= 1;
        }
    }
    true
}

/// `heapsort(vbase, nmemb, size, compar)` of the C library (Knuth vol. 3, p. 145, with the
/// bottom-up `SELECT` of problem 18), the fallback of [`qsort_libc`] past its depth limit.
/// Items are numbered from 1, as in the C code (`base = vbase - size`).
///
/// Ported from Apple Libc stdlib/FreeBSD/heapsort.c (FreeBSD 1.6, macOS 26 `libsystem_c`).
fn heapsort_libc<T: Copy>(a: &mut [T], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    let mut nmemb = a.len();
    if nmemb <= 1 {
        return;
    }
    // `CREATE(l, nmemb, …)` for `l = nmemb / 2 .. 1`.
    for l in (1..=nmemb / 2).rev() {
        let mut par_i = l;
        loop {
            let mut child_i = par_i * 2;
            if child_i > nmemb {
                break;
            }
            if child_i < nmemb && cmp(&a[child_i - 1], &a[child_i]) == Ordering::Less {
                child_i += 1;
            }
            if cmp(&a[child_i - 1], &a[par_i - 1]) != Ordering::Greater {
                break;
            }
            a.swap(par_i - 1, child_i - 1);
            par_i = child_i;
        }
    }
    while nmemb > 1 {
        let k = a[nmemb - 1];
        a[nmemb - 1] = a[0];
        nmemb -= 1;
        // `SELECT(…)`: sift the hole down along the larger children, then `k` back up.
        let mut par_i = 1;
        loop {
            let mut child_i = par_i * 2;
            if child_i > nmemb {
                break;
            }
            if child_i < nmemb && cmp(&a[child_i - 1], &a[child_i]) == Ordering::Less {
                child_i += 1;
            }
            a[par_i - 1] = a[child_i - 1];
            par_i = child_i;
        }
        loop {
            let child_i = par_i;
            par_i = child_i / 2;
            if child_i == 1 || cmp(&k, &a[par_i - 1]) == Ordering::Less {
                a[child_i - 1] = k;
                break;
            }
            a[child_i - 1] = a[par_i - 1];
        }
    }
}

/// The macOS C library `qsort(a, n, es, cmp)` (see the module documentation). Not stable.
pub fn qsort_libc<T: Copy>(a: &mut [T], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    // `DEPTH(n)` = `2 * (flsl(n) - 1)` (LP64).
    let fls = (usize::BITS - a.len().leading_zeros()) as isize;
    qsort_libc_depth(a, cmp, 2 * (fls - 1));
}

/// `_qsort(a, n, es, cmp, depth_limit)`.
fn qsort_libc_depth<T: Copy>(
    mut a: &mut [T],
    cmp: &mut impl FnMut(&T, &T) -> Ordering,
    mut depth_limit: isize,
) {
    loop {
        let n = a.len();
        let mut swap_cnt = false;
        // `if (depth_limit-- <= 0)`: past the recursion limit, heapsort (every `goto loop`
        // iteration counts as a level too).
        if depth_limit <= 0 {
            heapsort_libc(a, cmp);
            return;
        }
        depth_limit -= 1;
        if n <= 7 {
            insertion_limited(a, 0, cmp);
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
        // No swap in the partition pass: try an insertion sort, but give up after
        // `1 + n / 4` swaps ("nevermind") and go on with the partitions.
        if !swap_cnt && insertion_limited(a, 1 + n / 4, cmp) {
            return;
        }
        let d1 = pb - pa;
        let d2 = (pd - pc) as usize;
        if d1 <= d2 {
            // Recurse on the left partition, then iterate on the right one.
            if d1 > 1 {
                qsort_libc_depth(&mut a[..d1], cmp, depth_limit);
            }
            if d2 > 1 {
                let tmp = a;
                a = &mut tmp[pn - d2..];
                continue;
            }
        } else {
            // Recurse on the right partition, then iterate on the left one.
            if d2 > 1 {
                qsort_libc_depth(&mut a[pn - d2..], cmp, depth_limit);
            }
            if d1 > 1 {
                let tmp = a;
                a = &mut tmp[..d1];
                continue;
            }
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

    /// The reference program: the system `qsort` on the patterns of [`pattern_key`], one line
    /// `pattern n fnv1a64(permutation)` per case. macOS only, like the test that compiles it.
    #[cfg(target_os = "macos")]
    const REFERENCE_C: &str = r#"#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef struct { int key; int idx; } E;
static int cmp(const void *a, const void *b) {
  int x = ((const E *)a)->key, y = ((const E *)b)->key;
  return x < y ? -1 : x > y;
}
#define NPAT 17
static int is_pow2(long n) { return n > 0 && (n & (n - 1)) == 0; }
static int key(int p, long n, long i, uint32_t *rng) {
  long r = n - 1 - i;
  switch (p) {
  case 0: return (int)(r / 2);
  case 1: return (int)(r / 3);
  case 2: return (int)r;
  case 3: return (int)(r / 4);
  case 4: return (int)(i / 2);
  case 5: return (int)(i / 3);
  case 6: return (int)(i % 2);
  case 7: return (int)(i % 3);
  case 8: return (int)(i % 7);
  case 9: return 0;
  case 10: return (int)(i < r ? i : r);
  case 11: return (int)((i < r ? i : r) / 2);
  case 12: return (int)(r % 5);
  default: {
    static const uint32_t mods[] = {2, 3, 17};
    *rng = *rng * 1103515245u + 12345u;
    uint32_t m = p < 16 ? mods[p - 13] : (uint32_t)n + 1;
    return (int)((*rng >> 16) % m);
  }
  }
}
int main(void) {
  E *v = malloc(sizeof(E) * 2048);
  for (long n = 0; n <= 2048; n++) {
    if (!(n <= 300 || n % 17 == 0 || is_pow2(n) || is_pow2(n + 1) || is_pow2(n - 1))) continue;
    for (int p = 0; p < NPAT; p++) {
      uint32_t rng = (uint32_t)n * 2654435761u + (uint32_t)p;
      for (long i = 0; i < n; i++) { v[i].key = key(p, n, i, &rng); v[i].idx = (int)i; }
      qsort(v, (size_t)n, sizeof(E), cmp);
      uint64_t h = 1469598103934665603ull;
      for (long i = 0; i < n; i++) { h ^= (uint32_t)v[i].idx; h *= 1099511628211ull; }
      printf("%d %ld %016llx\n", p, n, (unsigned long long)h);
    }
  }
  return 0;
}
"#;

    const PATTERNS: u32 = 17;

    fn is_pow2(n: i64) -> bool {
        n > 0 && n & (n - 1) == 0
    }

    /// The `n` of the reference program: every size up to 300, then every 17th, the powers
    /// of two and their neighbours up to 2048.
    fn sizes() -> impl Iterator<Item = i64> {
        (0..=2048i64)
            .filter(|&n| n <= 300 || n % 17 == 0 || is_pow2(n) || is_pow2(n + 1) || is_pow2(n - 1))
    }

    /// `key(p, n, i, &rng)` of `REFERENCE_C`.
    fn pattern_key(p: u32, n: i64, i: i64, rng: &mut u32) -> i32 {
        let r = n - 1 - i;
        (match p {
            0 => r / 2,
            1 => r / 3,
            2 => r,
            3 => r / 4,
            4 => i / 2,
            5 => i / 3,
            6 => i % 2,
            7 => i % 3,
            8 => i % 7,
            9 => 0,
            10 => i.min(r),
            11 => i.min(r) / 2,
            12 => r % 5,
            _ => {
                *rng = rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let m = match p {
                    13 => 2,
                    14 => 3,
                    15 => 17,
                    _ => n as u32 + 1,
                };
                i64::from((*rng >> 16) % m)
            }
        }) as i32
    }

    /// Every case of the reference program, sorted by [`qsort_libc`]: `(pattern, n, hash)`.
    fn frap_cases() -> Vec<(u32, i64, u64)> {
        let mut out = Vec::new();
        for n in sizes() {
            for p in 0..PATTERNS {
                let mut rng = (n as u32).wrapping_mul(2_654_435_761).wrapping_add(p);
                let mut v: Vec<(i32, u32)> = (0..n)
                    .map(|i| (pattern_key(p, n, i, &mut rng), i as u32))
                    .collect();
                qsort_libc(&mut v, &mut |a: &(i32, u32), b: &(i32, u32)| a.0.cmp(&b.0));
                assert!(v.windows(2).all(|w| w[0].0 <= w[1].0), "pattern {p}, n {n}");
                let mut h = 1_469_598_103_934_665_603u64;
                for e in &v {
                    h ^= u64::from(e.1);
                    h = h.wrapping_mul(1_099_511_628_211);
                }
                out.push((p, n, h));
            }
        }
        out
    }

    /// The per-pattern fold (FNV-1a over the per-case hashes in `n` order) of the system
    /// `qsort`'s output on macOS 26, so platforms without it still check the port.
    const MACOS_PATTERN_HASHES: [u64; PATTERNS as usize] = [
        0x43d754a1c91baa84,
        0x95fa66bc43a3c626,
        0xb2a004e9bac877ac,
        0xe8f98bd9273d25e8,
        0x3c51b82706f85632,
        0xbefb4c41c8071582,
        0x186dae400d321636,
        0xa546bc285596c878,
        0xe1b50cf41dd252c4,
        0x6693fc63eeb7804e,
        0x906c7376e6b70bc4,
        0x081e3592f0146582,
        0x8e6c2b88f7e3ab12,
        0x9645515ea065d60e,
        0x015b638c1220b930,
        0x664535a8e55c01d2,
        0x8b4eb2b05cfee308,
    ];

    fn fold(cases: &[(u32, i64, u64)]) -> [u64; PATTERNS as usize] {
        let mut out = [1_469_598_103_934_665_603u64; PATTERNS as usize];
        for &(p, _, h) in cases {
            let f = &mut out[p as usize];
            *f ^= h;
            *f = f.wrapping_mul(1_099_511_628_211);
        }
        out
    }

    #[test]
    fn libc_qsort_matches_recorded_macos_patterns() {
        assert_eq!(fold(&frap_cases()), MACOS_PATTERN_HASHES);
    }

    /// The port against the live system `qsort`: compiles [`REFERENCE_C`] with `cc` (skipped
    /// when no compiler is found). Unsafe FFI is not allowed in this crate (§21.6), hence the
    /// separate program.
    #[cfg(target_os = "macos")]
    #[test]
    fn libc_qsort_matches_system_qsort() {
        use std::process::Command;
        let dir = std::env::temp_dir().join(format!("frap-uv-qsort-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("ref.c");
        let exe = dir.join("ref");
        std::fs::write(&src, REFERENCE_C).unwrap();
        let built = Command::new("cc")
            .arg("-O1")
            .arg("-o")
            .arg(&exe)
            .arg(&src)
            .status();
        if !built.is_ok_and(|s| s.success()) {
            eprintln!("skipped: no C compiler");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let run = Command::new(&exe).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(run.status.success());
        let want: Vec<(u32, i64, u64)> = String::from_utf8(run.stdout)
            .unwrap()
            .lines()
            .map(|l| {
                let mut f = l.split(' ');
                let p = f.next().unwrap().parse().unwrap();
                let n = f.next().unwrap().parse().unwrap();
                let h = u64::from_str_radix(f.next().unwrap(), 16).unwrap();
                (p, n, h)
            })
            .collect();
        let got = frap_cases();
        assert_eq!(got.len(), want.len());
        let bad: Vec<_> = got
            .iter()
            .zip(&want)
            .filter(|(g, w)| g != w)
            .map(|(g, _)| (g.0, g.1))
            .collect();
        assert!(
            bad.is_empty(),
            "(pattern, n) differing from the system qsort: {bad:?}"
        );
    }
}
