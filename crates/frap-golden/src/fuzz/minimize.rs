// SPDX-License-Identifier: GPL-3.0-or-later
//! Delta debugging (Zeller and Hildebrandt, "Simplifying and Isolating Failure-Inducing
//! Input", 2002): `ddmin` over the statements of a failing sequence, then a final pass that
//! tries removing each remaining statement alone, so the result is 1-minimal: removing any
//! single statement makes the mismatch disappear (WP 8.7).

/// The smallest subsequence of `items` (order kept) for which `fails` still holds, found with
/// `ddmin`. `fails(items)` must hold on entry; `budget` caps the number of `fails` calls (each
/// one runs both tools). Returns the subsequence and the number of calls made.
pub fn ddmin<T: Clone>(
    items: &[T],
    budget: usize,
    mut fails: impl FnMut(&[T]) -> bool,
) -> (Vec<T>, usize) {
    let mut cur: Vec<T> = items.to_vec();
    let mut calls = 0usize;
    let mut test = |v: &[T], calls: &mut usize| {
        *calls += 1;
        fails(v)
    };
    let mut n = 2usize;
    while cur.len() >= 2 && calls < budget {
        let chunk = cur.len().div_ceil(n);
        let chunks: Vec<std::ops::Range<usize>> = (0..cur.len())
            .step_by(chunk)
            .map(|s| s..(s + chunk).min(cur.len()))
            .collect();
        let mut reduced = false;
        // Reduce to a subset.
        for r in &chunks {
            if calls >= budget {
                break;
            }
            let sub = cur[r.clone()].to_vec();
            if test(&sub, &mut calls) {
                cur = sub;
                n = 2;
                reduced = true;
                break;
            }
        }
        // Reduce to a complement.
        if !reduced && chunks.len() > 2 {
            for r in &chunks {
                if calls >= budget {
                    break;
                }
                let comp: Vec<T> = cur
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !r.contains(i))
                    .map(|(_, x)| x.clone())
                    .collect();
                if test(&comp, &mut calls) {
                    cur = comp;
                    n = (n - 1).max(2);
                    reduced = true;
                    break;
                }
            }
        }
        if !reduced {
            if n >= cur.len() {
                break;
            }
            n = (2 * n).min(cur.len());
        }
    }
    // 1-minimality: drop single statements while the failure stays.
    let mut i = 0;
    while i < cur.len() && cur.len() > 1 && calls < budget {
        let mut v = cur.clone();
        v.remove(i);
        if test(&v, &mut calls) {
            cur = v;
        } else {
            i += 1;
        }
    }
    (cur, calls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_failure_inducing_pair() {
        let items: Vec<u32> = (0..40).collect();
        let fails = |v: &[u32]| v.contains(&7) && v.contains(&31);
        let (min, calls) = ddmin(&items, 10_000, fails);
        assert_eq!(min, [7, 31]);
        assert!(calls < 120, "{calls} calls");
    }

    #[test]
    fn keeps_order_and_dependencies() {
        // Fails when 3 comes before 9 (an "add" before the "use").
        let items: Vec<u32> = (0..16).collect();
        let fails = |v: &[u32]| {
            let a = v.iter().position(|&x| x == 3);
            let b = v.iter().position(|&x| x == 9);
            matches!((a, b), (Some(a), Some(b)) if a < b)
        };
        assert_eq!(ddmin(&items, 10_000, fails).0, [3, 9]);
    }

    #[test]
    fn single_and_budget() {
        let items: Vec<u32> = (0..64).collect();
        assert_eq!(ddmin(&items, 10_000, |v: &[u32]| v.contains(&5)).0, [5]);
        // A budget of 3 calls stops early but never returns a passing sequence.
        let (v, calls) = ddmin(&items, 3, |v: &[u32]| v.contains(&5));
        assert!(calls <= 3 && v.contains(&5));
        // Already minimal.
        assert_eq!(ddmin(&[1u32], 10, |v: &[u32]| !v.is_empty()).0, [1]);
    }
}
