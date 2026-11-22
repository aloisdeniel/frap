// SPDX-License-Identifier: GPL-3.0-or-later
//! The iteration order of a `GHash` with integer keys (`BLI_ghash_int_new`), so that
//! `BM_mesh_intersect` splits the faces of `face_edges` in Blender's order.
//!
// Ported from blender/source/blender/blenlib/intern/BLI_ghash.cc (hashsizes,
// GHASH_LIMIT_GROW, ghash_buckets_resize, ghash_buckets_expand, ghash_buckets_reset,
// ghash_insert_ex_keyonly_entry, BLI_ghashIterator_init/_step; GHASH_USE_MODULO_BUCKETS) and
// BLI_ghash_utils.cc (BLI_ghashutil_inthash_p on a 64-bit `uintptr_t`), Blender 5.2.4,
// GPL-2.0-or-later. Only the keys are kept: entries are never removed here.

/// `hashsizes`: the next prime after `2^n`.
const HASHSIZES: [u32; 27] = [
    5, 11, 17, 37, 67, 131, 257, 521, 1031, 2053, 4099, 8209, 16411, 32771, 65537, 131101, 262147,
    524309, 1048583, 2097169, 4194319, 8388617, 16777259, 33554467, 67108879, 134217757, 268435459,
];

/// `GHASH_LIMIT_GROW`.
fn limit_grow(nbuckets: u32) -> u32 {
    (nbuckets * 3) / 4
}

/// `BLI_ghashutil_inthash_p(POINTER_FROM_INT(key))` (64-bit pointers).
pub fn inthash_p(key: i32) -> u32 {
    let mut k = key as i64 as u64;
    k = k.wrapping_add(!(k << 16));
    k ^= k >> 5;
    k = k.wrapping_add(k << 3);
    k ^= k >> 13;
    k = k.wrapping_add(!(k << 9));
    k ^= k >> 17;
    (k & 0xffff_ffff) as u32
}

/// The buckets of a `GHash` of integer keys.
#[derive(Clone, Debug)]
pub struct IntKeyOrder {
    /// Each bucket's entries, head first.
    buckets: Vec<Vec<i32>>,
    nentries: u32,
    cursize: usize,
    limit_grow: u32,
}

impl Default for IntKeyOrder {
    fn default() -> Self {
        Self::new()
    }
}

impl IntKeyOrder {
    /// `BLI_ghash_int_new` (`ghash_buckets_reset(gh, 0)`).
    pub fn new() -> Self {
        let nbuckets = HASHSIZES[0];
        IntKeyOrder {
            buckets: vec![Vec::new(); nbuckets as usize],
            nentries: 0,
            cursize: 0,
            limit_grow: limit_grow(nbuckets),
        }
    }

    fn bucket(&self, key: i32) -> usize {
        (inthash_p(key) % self.buckets.len() as u32) as usize
    }

    /// `ghash_insert_ex_keyonly_entry` of a key not in the hash yet.
    pub fn insert_new(&mut self, key: i32) {
        let b = self.bucket(key);
        self.buckets[b].insert(0, key);
        self.nentries += 1;
        self.expand();
    }

    /// `ghash_buckets_expand(gh, nentries, false)`.
    fn expand(&mut self) {
        if self.nentries < self.limit_grow {
            return;
        }
        let mut new_nbuckets = self.buckets.len() as u32;
        while self.nentries > self.limit_grow && self.cursize < HASHSIZES.len() - 1 {
            self.cursize += 1;
            new_nbuckets = HASHSIZES[self.cursize];
            self.limit_grow = limit_grow(new_nbuckets);
        }
        if new_nbuckets as usize == self.buckets.len() {
            return;
        }
        self.limit_grow = limit_grow(new_nbuckets);
        // `ghash_buckets_resize` (growing): every old bucket in order, each entry prepended
        // to its new bucket.
        let old = std::mem::replace(&mut self.buckets, vec![Vec::new(); new_nbuckets as usize]);
        for list in old {
            for key in list {
                let b = self.bucket(key);
                self.buckets[b].insert(0, key);
            }
        }
    }

    /// `GHASH_ITER`: the keys bucket by bucket, each bucket head first.
    pub fn iter_order(&self) -> Vec<i32> {
        self.buckets.iter().flatten().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_like_blender() {
        let mut g = IntKeyOrder::new();
        for k in 0..4 {
            g.insert_new(k);
        }
        // Four entries exceed 5 * 3 / 4: 11 buckets.
        assert_eq!(g.buckets.len(), 11);
        let order = g.iter_order();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3]);
        // Bucket order: by hash modulo the bucket count.
        let b: Vec<u32> = order.iter().map(|&k| inthash_p(k) % 11).collect();
        assert!(b.windows(2).all(|w| w[0] <= w[1]));
    }
}
