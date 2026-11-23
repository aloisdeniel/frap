// SPDX-License-Identifier: GPL-3.0-or-later
//! `std::sort` as the libc++ of Blender's macOS builds instantiates it for a range of pointers
//! with a custom comparator (M8 stabilization round 2): the order it leaves equal elements in
//! is part of Blender's results where a sort key ties (`bmo_connect_concave.cc` sorts the new
//! edges of a triangulated n-gon by length, and a grid's diagonals have equal lengths).
//!
//! Ported from libc++ (LLVM 17+, `include/__algorithm/sort.h`, `partial_sort.h`,
//! `make_heap.h`, `sort_heap.h`, `pop_heap.h`, `sift_down.h`, `push_heap.h`), Apache-2.0 WITH
//! LLVM-exception: `__introsort` with the branchy `__sort3`/`__sort4`/`__sort5` (the branchless
//! variants are only used for arithmetic values under `std::less`), insertion sort below 24
//! elements, Tukey's ninther above 128, `__partition_with_equals_on_right`/`_left` (no bitset
//! partition: that is also arithmetic-only), `__insertion_sort_incomplete` after an already
//! partitioned range, and the heap sort fallback (`__partial_sort`) at depth `2·log2(n)`.

/// `std::sort(v.begin(), v.end(), less)`.
pub fn sort<T: Copy>(v: &mut [T], less: impl Fn(&T, &T) -> bool) {
    let n = v.len();
    if n < 2 {
        return;
    }
    let depth = 2 * (usize::BITS - 1 - n.leading_zeros()) as usize;
    introsort(v, 0, n, &less, depth, true);
}

/// `__sort3`: returns nothing (the swap count is unused here).
fn sort3<T: Copy>(v: &mut [T], x: usize, y: usize, z: usize, c: &impl Fn(&T, &T) -> bool) {
    if !c(&v[y], &v[x]) {
        if !c(&v[z], &v[y]) {
            return;
        }
        v.swap(y, z);
        if c(&v[y], &v[x]) {
            v.swap(x, y);
        }
        return;
    }
    if c(&v[z], &v[y]) {
        v.swap(x, z);
        return;
    }
    v.swap(x, y);
    if c(&v[z], &v[y]) {
        v.swap(y, z);
    }
}

/// `__sort4`.
fn sort4<T: Copy>(v: &mut [T], i: [usize; 4], c: &impl Fn(&T, &T) -> bool) {
    sort3(v, i[0], i[1], i[2], c);
    if c(&v[i[3]], &v[i[2]]) {
        v.swap(i[2], i[3]);
        if c(&v[i[2]], &v[i[1]]) {
            v.swap(i[1], i[2]);
            if c(&v[i[1]], &v[i[0]]) {
                v.swap(i[0], i[1]);
            }
        }
    }
}

/// `__sort5`.
fn sort5<T: Copy>(v: &mut [T], i: [usize; 5], c: &impl Fn(&T, &T) -> bool) {
    sort4(v, [i[0], i[1], i[2], i[3]], c);
    if c(&v[i[4]], &v[i[3]]) {
        v.swap(i[3], i[4]);
        if c(&v[i[3]], &v[i[2]]) {
            v.swap(i[2], i[3]);
            if c(&v[i[2]], &v[i[1]]) {
                v.swap(i[1], i[2]);
                if c(&v[i[1]], &v[i[0]]) {
                    v.swap(i[0], i[1]);
                }
            }
        }
    }
}

/// `__insertion_sort` (and `__insertion_sort_unguarded`, which moves the same elements when
/// its precondition holds: an element at `first - 1` no greater than any of the range).
fn insertion_sort<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    c: &impl Fn(&T, &T) -> bool,
    guarded: bool,
) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        let mut j = i - 1;
        if c(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if guarded && j == first {
                    break;
                }
                k -= 1;
                if !c(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
        }
    }
}

/// `__insertion_sort_incomplete`: whether the range ended up sorted (it gives up after
/// eight moves).
fn insertion_sort_incomplete<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    c: &impl Fn(&T, &T) -> bool,
) -> bool {
    match last - first {
        0 | 1 => return true,
        2 => {
            if c(&v[last - 1], &v[first]) {
                v.swap(first, last - 1);
            }
            return true;
        }
        3 => {
            sort3(v, first, first + 1, last - 1, c);
            return true;
        }
        4 => {
            sort4(v, [first, first + 1, first + 2, last - 1], c);
            return true;
        }
        5 => {
            sort5(v, [first, first + 1, first + 2, first + 3, last - 1], c);
            return true;
        }
        _ => {}
    }
    let mut j = first + 2;
    sort3(v, first, first + 1, j, c);
    const LIMIT: u32 = 8;
    let mut count = 0;
    let mut i = j + 1;
    while i != last {
        if c(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !c(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
            count += 1;
            if count == LIMIT {
                return i + 1 == last;
            }
        }
        j = i;
        i += 1;
    }
    true
}

/// `__partition_with_equals_on_right`: the pivot's final index and whether the range was
/// already partitioned.
fn partition_with_equals_on_right<T: Copy>(
    v: &mut [T],
    begin: usize,
    end: usize,
    c: &impl Fn(&T, &T) -> bool,
) -> (usize, bool) {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    loop {
        first += 1;
        if !c(&v[first], &pivot) {
            break;
        }
    }
    if begin == first - 1 {
        while first < last {
            last -= 1;
            if c(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if c(&v[last], &pivot) {
                break;
            }
        }
    }
    let already_partitioned = first >= last;
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if !c(&v[first], &pivot) {
                break;
            }
        }
        loop {
            last -= 1;
            if c(&v[last], &pivot) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    if begin != pivot_pos {
        v[begin] = v[pivot_pos];
    }
    v[pivot_pos] = pivot;
    (pivot_pos, already_partitioned)
}

/// `__partition_with_equals_on_left`: the index after the pivot.
fn partition_with_equals_on_left<T: Copy>(
    v: &mut [T],
    begin: usize,
    end: usize,
    c: &impl Fn(&T, &T) -> bool,
) -> usize {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    if c(&pivot, &v[last - 1]) {
        loop {
            first += 1;
            if c(&pivot, &v[first]) {
                break;
            }
        }
    } else {
        loop {
            first += 1;
            if !(first < last && !c(&pivot, &v[first])) {
                break;
            }
        }
    }
    if first < last {
        loop {
            last -= 1;
            if !c(&pivot, &v[last]) {
                break;
            }
        }
    }
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if c(&pivot, &v[first]) {
                break;
            }
        }
        loop {
            last -= 1;
            if !c(&pivot, &v[last]) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    if begin != pivot_pos {
        v[begin] = v[pivot_pos];
    }
    v[pivot_pos] = pivot;
    first
}

/// `__sift_down` (heap of `len` elements at `first`, sifting from `start`).
fn sift_down<T: Copy>(
    v: &mut [T],
    first: usize,
    c: &impl Fn(&T, &T) -> bool,
    len: usize,
    mut start: usize,
) {
    let mut child = start - first;
    if len < 2 || (len - 2) / 2 < child {
        return;
    }
    child = 2 * child + 1;
    let mut child_i = first + child;
    if child + 1 < len && c(&v[child_i], &v[child_i + 1]) {
        child_i += 1;
        child += 1;
    }
    if c(&v[child_i], &v[start]) {
        return;
    }
    let top = v[start];
    loop {
        v[start] = v[child_i];
        start = child_i;
        if (len - 2) / 2 < child {
            break;
        }
        child = 2 * child + 1;
        child_i = first + child;
        if child + 1 < len && c(&v[child_i], &v[child_i + 1]) {
            child_i += 1;
            child += 1;
        }
        if c(&v[child_i], &top) {
            break;
        }
    }
    v[start] = top;
}

/// `__floyd_sift_down`: the hole's index.
fn floyd_sift_down<T: Copy>(
    v: &mut [T],
    first: usize,
    c: &impl Fn(&T, &T) -> bool,
    len: usize,
) -> usize {
    let mut hole = first;
    let mut child_i = first;
    let mut child = 0usize;
    loop {
        child_i += child + 1;
        child = 2 * child + 1;
        if child + 1 < len && c(&v[child_i], &v[child_i + 1]) {
            child_i += 1;
            child += 1;
        }
        v[hole] = v[child_i];
        hole = child_i;
        if child > (len - 2) / 2 {
            return hole;
        }
    }
}

/// `__sift_up` of the element before `last`.
fn sift_up<T: Copy>(
    v: &mut [T],
    first: usize,
    mut last: usize,
    c: &impl Fn(&T, &T) -> bool,
    mut len: usize,
) {
    if len > 1 {
        len = (len - 2) / 2;
        let mut ptr = first + len;
        last -= 1;
        if c(&v[ptr], &v[last]) {
            let t = v[last];
            loop {
                v[last] = v[ptr];
                last = ptr;
                if len == 0 {
                    break;
                }
                len = (len - 1) / 2;
                ptr = first + len;
                if !c(&v[ptr], &t) {
                    break;
                }
            }
            v[last] = t;
        }
    }
}

/// `__partial_sort(first, last, last)`: `__make_heap`, then `__sort_heap`.
fn heap_sort<T: Copy>(v: &mut [T], first: usize, last: usize, c: &impl Fn(&T, &T) -> bool) {
    let n = last - first;
    if n > 1 {
        for start in (0..=(n - 2) / 2).rev() {
            sift_down(v, first, c, n, first + start);
        }
    }
    let mut end = last;
    let mut len = n;
    while len > 1 {
        // `__pop_heap(first, end, comp, len)`.
        let top = v[first];
        let hole = floyd_sift_down(v, first, c, len);
        end -= 1;
        if hole == end {
            v[hole] = top;
        } else {
            v[hole] = v[end];
            v[end] = top;
            sift_up(v, first, hole + 1, c, hole + 1 - first);
        }
        len -= 1;
    }
}

/// `__introsort`.
fn introsort<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    c: &impl Fn(&T, &T) -> bool,
    mut depth: usize,
    mut leftmost: bool,
) {
    const LIMIT: usize = 24;
    const NINTHER_THRESHOLD: usize = 128;
    loop {
        let len = last - first;
        match len {
            0 | 1 => return,
            2 => {
                if c(&v[last - 1], &v[first]) {
                    v.swap(first, last - 1);
                }
                return;
            }
            3 => {
                sort3(v, first, first + 1, last - 1, c);
                return;
            }
            4 => {
                sort4(v, [first, first + 1, first + 2, last - 1], c);
                return;
            }
            5 => {
                sort5(v, [first, first + 1, first + 2, first + 3, last - 1], c);
                return;
            }
            _ => {}
        }
        if len < LIMIT {
            insertion_sort(v, first, last, c, leftmost);
            return;
        }
        if depth == 0 {
            heap_sort(v, first, last, c);
            return;
        }
        depth -= 1;
        let half = len / 2;
        if len > NINTHER_THRESHOLD {
            sort3(v, first, first + half, last - 1, c);
            sort3(v, first + 1, first + (half - 1), last - 2, c);
            sort3(v, first + 2, first + (half + 1), last - 3, c);
            sort3(v, first + (half - 1), first + half, first + (half + 1), c);
            v.swap(first, first + half);
        } else {
            sort3(v, first + half, first, last - 1, c);
        }
        if !leftmost && !c(&v[first - 1], &v[first]) {
            first = partition_with_equals_on_left(v, first, last, c);
            continue;
        }
        let (i, already) = partition_with_equals_on_right(v, first, last, c);
        if already {
            let fs = insertion_sort_incomplete(v, first, i, c);
            if insertion_sort_incomplete(v, i + 1, last, c) {
                if fs {
                    return;
                }
                last = i;
                continue;
            } else if fs {
                first = i + 1;
                continue;
            }
        }
        introsort(v, first, i, c, depth, leftmost);
        leftmost = false;
        first = i + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator.
    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed >> 33
    }

    #[test]
    fn sorts_every_size_with_many_ties() {
        let mut seed = 7u64;
        for n in 0..400usize {
            let mut v: Vec<(u32, usize)> =
                (0..n).map(|i| ((lcg(&mut seed) % 7) as u32, i)).collect();
            sort(&mut v, |a, b| a.0 < b.0);
            assert!(v.windows(2).all(|w| w[0].0 <= w[1].0), "n = {n}");
            let mut ids: Vec<usize> = v.iter().map(|x| x.1).collect();
            ids.sort_unstable();
            assert_eq!(ids, (0..n).collect::<Vec<_>>());
        }
        // The heap sort fallback.
        let mut v: Vec<u32> = (0..300).map(|_| (lcg(&mut seed) % 50) as u32).collect();
        let mut w = v.clone();
        heap_sort(&mut v, 0, 300, &|a: &u32, b: &u32| a < b);
        w.sort_unstable();
        assert_eq!(v, w);
    }

    /// libc++'s order of equal elements: below 6 elements the sorting networks, below 24
    /// insertion sort (stable), above that the partitions (not stable).
    #[test]
    fn tie_order_of_small_and_partitioned_ranges() {
        // Insertion sort is stable.
        let mut v: Vec<(u32, usize)> = [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]
            .iter()
            .enumerate()
            .map(|(i, &k)| (k, i))
            .collect();
        sort(&mut v, |a, b| a.0 < b.0);
        let ids: Vec<usize> = v.iter().map(|x| x.1).collect();
        assert_eq!(ids, vec![1, 3, 5, 7, 9, 11, 0, 2, 4, 6, 8, 10]);
        // `__sort3` of three equal keys leaves them in place.
        let mut v = [(0u32, 0usize), (0, 1), (0, 2)];
        sort(&mut v, |a, b| a.0 < b.0);
        assert_eq!(v.map(|x| x.1), [0, 1, 2]);
    }
}
